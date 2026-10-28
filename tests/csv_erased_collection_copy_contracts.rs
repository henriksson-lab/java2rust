#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_array_iterables_copy_original_references_and_release_loans_before_callbacks() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_collection_copy",
        "ErasedCollectionCopyContracts",
        "tests/corpus/csv/ErasedCollectionCopyContracts.java",
        include_str!("corpus/csv/ErasedCollectionCopyContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copy_contract() { assert_eq!(ErasedCollectionCopyContracts::run().value(), 0); }
"#,
    );
}
