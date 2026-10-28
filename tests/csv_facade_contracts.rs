#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_facade_conversion_owns_its_backing_through_the_call() {
    common::verify_java_fixture_zero_contract(
        "csv_facade_temporary",
        "FacadeTemporaryContracts",
        "tests/corpus/csv/FacadeTemporaryContracts.java",
        include_str!("corpus/csv/FacadeTemporaryContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn facade_temporary() { assert_eq!(FacadeTemporaryContracts::run().value(), 0); }
"#,
    );
}
