#[path = "common/mod.rs"]
mod common;

#[test]
fn char_buffer_copy_preserves_units_aliases_and_checked_allocation() {
    common::verify_java_fixture_zero_contract(
        "csv_char_buffer_contracts",
        "CharBufferCopyContracts",
        "tests/corpus/csv/CharBufferCopyContracts.java",
        include_str!("corpus/csv/CharBufferCopyContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn char_buffer_contract() { assert_eq!(CharBufferCopyContracts::run().value(), 0); }
"#,
    );
}
