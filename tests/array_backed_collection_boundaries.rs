#[path = "common/mod.rs"]
mod common;

#[test]
fn backing_and_view_share_the_intrinsic_nullable_element_carrier() {
    use new_java2rust::{
        facts::FactSet,
        java_ir::{JavaType, Member},
        parser, representation, resolve, semantic_analysis, signature,
    };
    let parsed = parser::parse_java_source(include_str!(
        "fixtures/array_backed_collection_boundaries.java"
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
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = plan.method_signature(owner, method).unwrap();
    let view = signature
        .locals()
        .iter()
        .find(|local| local.name() == "view")
        .unwrap()
        .shape();
    let array = signature
        .locals()
        .iter()
        .find(|local| local.name() == "values")
        .unwrap()
        .shape();
    assert!(array.native_allocation.shared());
    assert!(view.native_allocation.shared());
    let child = view.sequence_element_type().unwrap();
    assert_eq!(
        new_java2rust::types::optional_element_type(&child),
        Some(JavaType::String)
    );
}

#[test]
fn source_collection_boundaries_keep_array_backing_and_runtime_kind() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/array_backed_collection_boundaries.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "array_backed_collection_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_backing_and_collection_contracts() {
    assert_eq!(ArrayBackedCollectionBoundaries::run().value(), 127);
}
"#,
        }],
    );
}
