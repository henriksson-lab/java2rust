#[path = "common/mod.rs"]
mod common;

#[test]
fn captured_arrays_keep_allocation_identity_across_aliases_calls_and_closures() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/captured_array_identity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "captured_array_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn java_array_aliases_and_effects() {
    assert_eq!(CapturedArrayIdentity::parameter_alias().checked_value(), 8);
    assert_eq!(CapturedArrayIdentity::returned_alias().checked_value(), 18);
    assert_eq!(CapturedArrayIdentity::direct_alias().checked_value(), 7);
    assert_eq!(CapturedArrayIdentity::rebound_alias().checked_value(), 16);
    assert_eq!(CapturedArrayIdentity::reference_elements().checked_value(), 3);
    assert_eq!(CapturedArrayIdentity::null_element().checked_value(), -6);
    assert_eq!(CapturedArrayIdentity::writes().checked_value(), 5);
    assert_eq!(CapturedArrayIdentity::shadowed().checked_value(), 7);
    assert_eq!(CapturedArrayIdentity::nested().checked_value(), 8);
    assert_eq!(CapturedArrayIdentity::renamed_capture().checked_value(), 9);
    assert_eq!(CapturedArrayIdentity::null_caught().checked_value(), -4);
    assert_eq!(CapturedArrayIdentity::negative_size().checked_value(), -5);
}
"#,
        }],
    );
}
