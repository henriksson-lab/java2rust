#[path = "common/mod.rs"]
mod common;

#[test]
fn nested_nullable_map_field_returns_preserve_absence_and_aliases() {
    common::verify_java_fixture_zero_contract(
        "nested_nullable_map_returns",
        "NestedNullableMapContracts",
        "tests/corpus/csv/NestedNullableMapContracts.java",
        include_str!("corpus/csv/NestedNullableMapContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn nullable_fields() { assert_eq!(NestedNullableMapContracts::run().value(), 0); }
"#,
    );
}
