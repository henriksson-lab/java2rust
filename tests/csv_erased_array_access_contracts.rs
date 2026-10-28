#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_array_reads_and_typed_strings_compare_reference_identity() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_array_identity",
        "ErasedArrayIdentityContracts",
        "tests/corpus/csv/ErasedArrayIdentityContracts.java",
        include_str!("corpus/csv/ErasedArrayIdentityContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn identity() { assert_eq!(ErasedArrayIdentityContracts::run().value(), 0); }
"#,
    );
}

#[test]
fn immediate_erased_array_access_preserves_java_order_aliases_and_reentry() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_array_access",
        "ErasedArrayAccessContracts",
        "tests/corpus/csv/ErasedArrayAccessContracts.java",
        include_str!("corpus/csv/ErasedArrayAccessContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn access() { assert_eq!(ErasedArrayAccessContracts::run().value(), 0); }
"#,
    );
}
