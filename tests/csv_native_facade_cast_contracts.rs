#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_native_facade_casts_preserve_null_and_catchable_failures_at_receivers() {
    common::verify_java_fixture_zero_contract(
        "csv_native_facade_cast",
        "NativeFacadeCastContracts",
        "tests/corpus/csv/NativeFacadeCastContracts.java",
        include_str!("corpus/csv/NativeFacadeCastContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn casts() { assert_eq!(NativeFacadeCastContracts::run().value(), 0); }
"#,
    );
}
