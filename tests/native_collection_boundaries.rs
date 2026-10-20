#[path = "common/mod.rs"]
mod common;

#[test]
fn native_container_boundaries_keep_references_but_not_fresh_copy_identity() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/native_collection_boundaries.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "native_collection_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_alias_and_set_value_equality_contracts_agree() {
    assert_eq!(NativeCollectionBoundaries::run().value(), 2);
}
"#,
        }],
    );
}
