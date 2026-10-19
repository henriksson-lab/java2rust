#[path = "common/mod.rs"]
mod common;

#[test]
fn nested_source_nullable_calls_keep_the_selected_result_abi() {
    let source = include_str!("fixtures/nested_nullable_strings.java");
    let translation = new_java2rust::translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let batch = new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
        &[new_java2rust::BatchSource::new(
            "NestedNullableStrings.java",
            source,
        )],
        &[],
        &[],
        true,
        &new_java2rust::facts::FactSet::default(),
    );
    assert!(
        !batch[0].diagnostics().has_errors(),
        "{:?}",
        batch[0].diagnostics()
    );
    const PROBE: &str = r#"
trait Text { fn text(self) -> String; }
impl Text for String { fn text(self) -> String { self } }
impl Text for Option<String> { fn text(self) -> String { self.unwrap() } }
impl<T: Text, E: std::fmt::Debug> Text for Result<T,E> { fn text(self) -> String { self.unwrap().text() } }
#[test] fn nested_source_results_preserve_null_until_the_display_boundary() {
    assert_eq!(NestedNullableStrings::null_neighbor().text(), "[null]");
    assert_eq!(NestedNullableStrings::present_neighbor().text(), "[present]");
    assert_eq!(NestedNullableStrings::null_description().text(), "[null]");
}
"#;
    common::cargo_test_generated_modules(
        "nested_nullable_strings",
        vec![
            common::GeneratedCrateModule {
                name: "standalone",
                rust: translation.rust,
                tests: PROBE,
            },
            common::GeneratedCrateModule {
                name: "batch",
                rust: batch[0].rust().to_string(),
                tests: PROBE,
            },
        ],
    );
}
