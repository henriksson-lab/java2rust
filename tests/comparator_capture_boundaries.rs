#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_interface_capture_and_borrowed_forwarding_share_one_value_contract() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/comparator_capture_boundaries.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "comparator_capture_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn captured_and_borrowed_interface_values_match_java() {
    assert_eq!(ComparatorCaptureBoundaries::borrowed().unwrap(), 3);
    assert_eq!(ComparatorCaptureBoundaries::borrowed_null().unwrap(), -1);
    assert_eq!(ComparatorCaptureBoundaries::constructor_borrowed().unwrap(), 2);
    assert_eq!(ComparatorCaptureBoundaries::local_nullable_sample().unwrap(), 11);
    assert_eq!(ComparatorCaptureBoundaries::local_nullable_null().unwrap(), -1);
    assert_eq!(ComparatorCaptureBoundaries::null_bound_reference(), 8);
    assert_eq!(ComparatorCaptureBoundaries::retained().unwrap(), 0);
}
"#,
        }],
    );
}
