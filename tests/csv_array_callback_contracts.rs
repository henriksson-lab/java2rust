#[path = "common/mod.rs"]
mod common;

#[test]
fn array_generators_release_target_loans_and_preserve_partial_writes() {
    common::verify_java_fixture_zero_contract(
        "csv_array_callback",
        "ArraySetAllContracts",
        "tests/corpus/csv/ArraySetAllContracts.java",
        include_str!("corpus/csv/ArraySetAllContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn callbacks() { assert_eq!(ArraySetAllContracts::run().value(), 0); }
"#,
    );
}
