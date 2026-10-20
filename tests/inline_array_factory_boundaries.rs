#[path = "common/mod.rs"]
mod common;

#[test]
fn inline_factory_arguments_retain_backing_storage_in_class_generic_frames() {
    use new_java2rust::{
        facts::FactSet, java_ir::Member, parser, representation, resolve, semantic_analysis,
        signature,
    };
    let parsed = parser::parse_java_source(include_str!(
        "fixtures/inline_array_factory_boundaries.java"
    ));
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
    let owner = resolved
        .program
        .types
        .iter()
        .find(|owner| owner.name == "InlineArrayFactory")
        .unwrap();
    for name in ["fromArray", "make", "read"] {
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        let signature = plan.method_signature(owner, method).unwrap();
        assert!(
            signature.params()[0].shape().native_allocation.shared(),
            "{name}: {:?}",
            signature.params()
        );
    }
}

#[test]
fn inline_factory_call_and_constructor_keep_the_original_array() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/inline_array_factory_boundaries.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "inline_array_factory_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn inline_factory_alias_contract() { assert_eq!(InlineArrayFactoryBoundaries::run().value(), 1); }
"#,
        }],
    );
}
