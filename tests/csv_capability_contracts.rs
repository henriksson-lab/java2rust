#[path = "common/mod.rs"]
mod common;
#[test]
fn native_capability_casts_preserve_null_state_and_checked_failures() {
    common::verify_java_fixture_zero_contract(
        "csv_capability_cast",
        "CapabilityCastContracts",
        "tests/corpus/csv/CapabilityCastContracts.java",
        include_str!("corpus/csv/CapabilityCastContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn casts() { assert_eq!(CapabilityCastContracts::run().value(), 0); }
"#,
    );
}
