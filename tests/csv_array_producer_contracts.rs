#[path = "common/mod.rs"]
mod common;

#[test]
fn fresh_native_arrays_adopt_selected_storage_and_keep_existing_aliases() {
    common::verify_java_fixture_zero_contract(
        "csv_array_producer",
        "FreshNativeArrayContracts",
        "tests/corpus/csv/FreshNativeArrayContracts.java",
        include_str!("corpus/csv/FreshNativeArrayContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn arrays() { assert_eq!(FreshNativeArrayContracts::run().value(), 0); }
"#,
    );
}
