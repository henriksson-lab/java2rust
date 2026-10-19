#[path = "common/mod.rs"]
mod common;

#[test]
fn native_values_adapt_to_inline_source_callbacks_without_eager_invocation() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/inline_native_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "inline_native_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn inline_consumers_match_java() {
    assert_eq!(InlineNativeValues::shared().unwrap(), 345);
    assert_eq!(InlineNativeValues::two_inputs().unwrap(), 42);
    assert_eq!(InlineNativeValues::caught().unwrap(), 7);
    assert_eq!(InlineNativeValues::ignored_null().unwrap(), 9);
    assert_eq!(InlineNativeValues::caught_null().unwrap(), 8);
    assert_eq!(InlineNativeValues::evaluated_once().unwrap(), 19);
}
"#,
        }],
    );
}
