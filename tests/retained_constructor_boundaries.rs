#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_field_initialization_preserves_superclass_order_and_exceptions() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_constructor_initialization.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_constructor_initialization",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(RetainedConstructorInitialization::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn delegated_array_and_nullable_list_constructor_slots_keep_storage() {
    use new_java2rust::{
        facts::FactSet,
        java_ir::{JavaType, Member},
        parser, representation, resolve, semantic_analysis, signature,
    };
    let parsed = parser::parse_java_source(include_str!(
        "fixtures/retained_constructor_boundaries.java"
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
    let owner = &resolved.program.types[0];
    let mut checked = 0;
    for member in &owner.members {
        let Member::Constructor(ctor) = member else {
            continue;
        };
        if matches!(ctor.params[0].ty, JavaType::Primitive(_)) {
            continue;
        }
        let shape = plan.constructor_signature(owner, ctor).unwrap().params()[0].shape();
        assert!(shape.native_allocation.shared(), "{shape:?}");
        checked += 1;
    }
    assert_eq!(checked, 2);
}

#[test]
fn nullable_factory_and_delegation_preserve_aliases_and_immutability() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_constructor_boundaries.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_constructor_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn constructor_alias_contract() { assert_eq!(RetainedConstructorBoundaries::run().value(), 7); }
"#,
        }],
    );
}
