#[path = "common/mod.rs"]
mod common;

#[test]
fn widening_retained_maps_preserves_owner_and_nested_array_aliases() {
    common::verify_java_fixture_zero_contract(
        "csv_map_widening",
        "MapWideningContracts",
        "tests/corpus/csv/MapWideningContracts.java",
        include_str!("corpus/csv/MapWideningContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn widening() { assert_eq!(MapWideningContracts::run().value(), 0); }
"#,
    );
}
