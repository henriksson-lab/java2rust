#[path = "common/mod.rs"]
mod common;

#[test]
fn qualified_functional_types_share_the_declared_native_carrier_names() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/qualified_functional_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "qualified_functional_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn qualified_native_values_match_java() {
    assert_eq!(QualifiedFunctionalValues::run().unwrap(), 457);
    assert_eq!(QualifiedFunctionalValues::overflow().unwrap(), -2147483647);
}
"#,
        }],
    );
}
