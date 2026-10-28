#[path = "common/mod.rs"]
mod common;

#[test]
fn nested_array_rows_keep_allocation_identity_without_cloning_erased_payloads() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_array_rows.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_array_rows",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value<T> { fn value(self) -> T; }
impl Value<bool> for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Value<bool> for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
impl Value<String> for JavaString { fn value(self) -> String { String::from_utf16(self.units()).unwrap() } }
impl<E: std::fmt::Debug> Value<String> for Result<JavaString, E> { fn value(self) -> String { String::from_utf16(self.unwrap().units()).unwrap() } }
impl Value<String> for Option<JavaString> { fn value(self) -> String { String::from_utf16(self.expect("expected non-null Java string").units()).unwrap() } }
impl<E: std::fmt::Debug> Value<String> for Result<Option<JavaString>, E> { fn value(self) -> String { String::from_utf16(self.unwrap().expect("expected non-null Java string").units()).unwrap() } }
impl Value<i32> for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value<i32> for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reference_rows_are_not_deep_copied() {
    assert!(ErasedRowRead::run().value());
    assert!(ErasedRowRead::clone_run().value());
    assert!(ErasedRowRead::leaf_clone_run().value());
    assert!(NullableArrayRows::absent_result().value());
    assert!(NullableArrayRows::caught_result().value());
    assert_eq!(RetainedArrayRows::alias_result().value(), "old:null:null");
    assert_eq!(RetainedArrayRows::clone_result().value(), "shared:null");
    assert_eq!(RetainedArrayRows::call_result().value(), "call:direct");
    assert_eq!(RetainedArrayRows::negative_dimension().value(), 9);
}
"#,
        }],
    );
}
