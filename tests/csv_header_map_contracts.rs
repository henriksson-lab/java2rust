#[path = "common/mod.rs"]
mod common;
#[test]
fn case_insensitive_header_maps_preserve_owner_keys_and_unicode_order() {
    common::verify_java_fixture_zero_contract(
        "csv_header_maps",
        "HeaderMapContracts",
        "tests/corpus/csv/HeaderMapContracts.java",
        include_str!("corpus/csv/HeaderMapContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn header_maps() { assert_eq!(HeaderMapContracts::run().value(), 0); }
"#,
    );
}
