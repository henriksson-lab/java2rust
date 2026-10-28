#[path = "common/mod.rs"]
mod common;

#[test]
fn escaping_array_stream_observes_alias_writes_and_reentrant_callbacks() {
    common::verify_java_fixture_zero_contract(
        "csv_array_stream",
        "ArrayStreamContracts",
        "tests/corpus/csv/ArrayStreamContracts.java",
        include_str!("corpus/csv/ArrayStreamContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn streams() { assert_eq!(ArrayStreamContracts::run().value(), 0); }
"#,
    );
}
