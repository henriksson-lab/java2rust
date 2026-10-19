#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_producer_results_remain_values_in_maps() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_map_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_map_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait JavaIntOutcome { fn value(self) -> i32; }
impl JavaIntOutcome for i32 { fn value(self) -> i32 { self } }
impl JavaIntOutcome for Result<i32, JavaException> {
    fn value(self) -> i32 { self.expect("unexpected uncaught Java exception") }
}
#[test]
fn null_is_stored_rather_than_removed_or_unwrapped() {
    assert_eq!(NullableMapValues::concrete().value(), 204);
    assert_eq!(NullableMapValues::via_interface().value(), 204);
    assert_eq!(NullableMapValues::field().value(), 204);
    assert_eq!(NullableMapValues::interface_values().value(), 14);
    assert_eq!(NullableMapValues::constructor_argument().value(), 204);
}
"#,
        }],
    );
}
