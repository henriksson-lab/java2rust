#[path = "common/mod.rs"]
mod common;

#[test]
fn collection_arrays_reuse_existing_destinations_and_preserve_tail_and_effects() {
    common::verify_java_fixture_zero_contract(
        "csv_existing_array",
        "ExistingCollectionArrayContracts",
        "tests/corpus/csv/ExistingCollectionArrayContracts.java",
        include_str!("corpus/csv/ExistingCollectionArrayContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(ExistingCollectionArrayContracts::run().value(), 0); }
"#,
    );
}
