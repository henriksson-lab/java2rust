#[path = "common/mod.rs"]
mod common;

#[test]
fn imported_enum_method_references_keep_all_overloads_in_complete_batch_context() {
    let sources = [
        (
            "producer/OverloadedEnum.java",
            include_str!("corpus/numbers/overload-context/producer/OverloadedEnum.java"),
        ),
        (
            "consumer/ImportedEnumProbe.java",
            include_str!("corpus/numbers/overload-context/consumer/ImportedEnumProbe.java"),
        ),
    ];
    let paths = sources
        .iter()
        .map(|(path, source)| {
            (
                format!("tests/corpus/numbers/overload-context/{path}"),
                *source,
            )
        })
        .collect::<Vec<_>>();
    let oracle = paths
        .iter()
        .map(|(path, source)| (path.as_str(), *source))
        .collect::<Vec<_>>();
    common::verify_java_sources_zero_contract("consumer.ImportedEnumProbe", &oracle);
    let batch = sources
        .iter()
        .map(|(path, source)| new_java2rust::BatchSource::new(*path, *source))
        .collect::<Vec<_>>();
    common::cargo_run_translated_batch_sources(
        "imported_enum_overloads",
        &batch,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ImportedEnumProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn enum_callback_payloads_keep_names_types_null_checks_and_short_borrows() {
    common::verify_java_fixture_zero_contract(
        "enum_callback_contracts",
        "EnumCallbackProbe",
        "tests/corpus/numbers/EnumCallbackProbe.java",
        include_str!("corpus/numbers/EnumCallbackProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn callbacks() { assert_eq!(EnumCallbackProbe::run().value(), 0); }
"#,
    );
}
