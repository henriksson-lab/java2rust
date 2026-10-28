#[path = "common/mod.rs"]
mod common;
#[test]
fn string_equals_object_preserves_runtime_type_nulls_and_operand_order() {
    common::verify_java_fixture_zero_contract(
        "csv_string_equals_object",
        "StringEqualsObjectContracts",
        "tests/corpus/csv/StringEqualsObjectContracts.java",
        include_str!("corpus/csv/StringEqualsObjectContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn equals_object() { assert_eq!(StringEqualsObjectContracts::run().value(), 0); }
"#,
    );
}
