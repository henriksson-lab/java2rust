#[path = "common/mod.rs"]
mod common;
#[test]
fn enum_reflection_preserves_initialization_arrays_and_original_constants() {
    common::verify_java_fixture_zero_contract(
        "csv_enum_reflection",
        "EnumReflectionContracts",
        "tests/corpus/csv/EnumReflectionContracts.java",
        include_str!("corpus/csv/EnumReflectionContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn enum_reflection() { assert_eq!(EnumReflectionContracts::run().value(), 0); }
"#,
    );
}
