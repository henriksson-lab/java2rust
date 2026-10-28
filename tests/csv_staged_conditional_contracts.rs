#[path = "common/mod.rs"]
mod common;

#[test]
fn fallible_self_reading_conditionals_finish_before_nullable_erasure_and_later_arguments() {
    common::verify_java_fixture_zero_contract(
        "csv_staged_conditional",
        "FallibleStagedConditional",
        "tests/corpus/csv/FallibleStagedConditional.java",
        include_str!("corpus/csv/FallibleStagedConditional.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn staging() { assert_eq!(FallibleStagedConditional::run().value(), 0); }
"#,
    );
}
