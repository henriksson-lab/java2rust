#[path = "common/mod.rs"]
mod common;

#[test]
fn merged_enum_lookup_uses_original_names_after_rust_name_disambiguation() {
    let sources = [
        (
            "ABC.java",
            include_str!("corpus/numbers/enumlookupbatch/ABC.java"),
        ),
        (
            "a/b/C.java",
            include_str!("corpus/numbers/enumlookupbatch/a/b/C.java"),
        ),
        (
            "x/C.java",
            include_str!("corpus/numbers/enumlookupbatch/x/C.java"),
        ),
        (
            "entry/BatchEnumProbe.java",
            include_str!("corpus/numbers/enumlookupbatch/entry/BatchEnumProbe.java"),
        ),
    ];
    let paths = sources
        .iter()
        .map(|(path, source)| {
            (
                format!("tests/corpus/numbers/enumlookupbatch/{path}"),
                *source,
            )
        })
        .collect::<Vec<_>>();
    let oracle = paths
        .iter()
        .map(|(path, source)| (path.as_str(), *source))
        .collect::<Vec<_>>();
    common::verify_java_sources_zero_contract("entry.BatchEnumProbe", &oracle);
    let batch = sources
        .into_iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(path, source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "enum_lookup_identity",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::BatchEnumProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_lookup_preserves_source_labels_canonical_failures_and_null_names() {
    common::verify_java_fixture_zero_contract(
        "enum_lookup",
        "enumlookup.EnumLookupProbe",
        "tests/corpus/numbers/EnumLookupProbe.java",
        include_str!("corpus/numbers/EnumLookupProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lookup() { assert_eq!(EnumLookupProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn pure_enum_lookup_carries_its_exception_runtime_dependency() {
    let translated = new_java2rust::translate_source("package pure; enum Only { one, TWO }");
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "pure_enum_lookup",
        vec![common::GeneratedCrateModule {
            name: "pure",
            rust: translated.rust,
            tests: r#"
#[test] fn lookup() {
    assert_eq!(Only::value_of("one").name(), "one");
    let failure = std::panic::catch_unwind(|| Only::value_of("absent")).unwrap_err();
    let error = failure.downcast_ref::<JavaException>().expect("typed Java exception");
    assert_eq!(error.class_name(), "IllegalArgumentException");
    assert_eq!(error.message().scalar_string().unwrap(), "No enum constant pure.Only.absent");
}
"#,
        }],
    );
}
