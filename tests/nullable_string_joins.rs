#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_call_results_join_before_display_normalization() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_string_joins.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_string_joins",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Text { fn text(self) -> String; }
impl Text for String { fn text(self) -> String { self } }
impl Text for Option<String> { fn text(self) -> String { self.unwrap() } }
impl<T: Text, E: std::fmt::Debug> Text for Result<T,E> { fn text(self) -> String { self.unwrap().text() } }
#[test] fn nullable_results_are_not_double_wrapped_or_displayed_as_options() {
    assert_eq!(NullableStringJoins::neighbor_null_result().text(), "[null]");
    assert_eq!(NullableStringJoins::neighbor_absent().text(), "[missing]");
    assert_eq!(NullableStringJoins::parent_null_result().text(), "[null]");
}
"#,
        }],
    );
}
