#[path = "common/mod.rs"]
mod common;

#[test]
fn reusable_callback_parameter_contracts_are_resolved() {
    use new_java2rust::{
        facts::FactSet, java_ir::Member, parser, representation, resolve, semantic_analysis,
        signature,
    };
    let parsed = parser::parse_java_source(include_str!("fixtures/reusable_callbacks.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let owner = &resolved.program.types[0];
    for member in &owner.members {
        if let Member::Method(method) = member {
            let signature = plan
                .method_signature(owner, method)
                .expect("source method signature");
            assert!(method.params.iter().all(|param| param.binding.is_some()));
            if method.name == "consume" {
                assert_eq!(
                    signature.params()[0].shape().ownership,
                    new_java2rust::slot_shape::OwnershipShape::Owned
                );
            }
        }
    }
}

#[test]
fn reusable_callbacks_preserve_captures_without_cloning_fresh_or_borrowed_values() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/reusable_callbacks.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "reusable_callbacks",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
struct NoClone;
#[test] fn captures_survive_multiple_invocations() {
    assert_eq!(ReusableCallbacks::run().value(), 22);
}
#[test] fn borrowed_probes_and_fresh_parameters_need_no_payload_clone() {
    let probe = ReusableCallbacks::read_only::<NoClone>(Some(NoClone));
    assert!(!probe.get());
    assert!(!probe.get());
    let fresh = ReusableCallbacks::fresh::<NoClone>();
    let _ = fresh.apply(NoClone);
    let _ = fresh.apply(NoClone);
}
"#,
        }],
    );
}
