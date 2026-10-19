#[path = "common/mod.rs"]
mod common;

#[test]
fn imported_nested_interface_and_implementation_share_the_result_abi() {
    use new_java2rust::BatchSource;
    let sources = [
        BatchSource::new(
            "api/Scope.java",
            include_str!("fixtures/nested_override_fallibility/api/Scope.java"),
        ),
        BatchSource::new(
            "client/ValueReader.java",
            include_str!("fixtures/nested_override_fallibility/client/ValueReader.java"),
        ),
        BatchSource::new(
            "client/ReaderUse.java",
            include_str!("fixtures/nested_override_fallibility/client/ReaderUse.java"),
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "nested_override_fallibility",
        &sources,
        r#"
use translated::{ReaderUse, Scope_Reader, ValueReader};
fn main() {
    let concrete = ValueReader::<String>::new();
    let reader: &dyn Scope_Reader<String> = &concrete;
    assert_eq!(ReaderUse::run(reader).unwrap().as_deref(), Some("ok"));
    assert!(reader.read(None).is_err());
    assert_eq!(reader.read(Some("next".to_string())).unwrap().as_deref(), Some("next"));
}
"#,
    );
}
