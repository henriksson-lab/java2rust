#[path = "common/mod.rs"]
mod common;

#[test]
fn private_outer_only_helper_retains_its_instance_receiver() {
    common::verify_java_fixture_zero_contract(
        "csv_outer_receiver",
        "OuterOnlyReceiver",
        "tests/corpus/csv/OuterOnlyReceiver.java",
        include_str!("corpus/csv/OuterOnlyReceiver.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn outer_capture() { assert_eq!(OuterOnlyReceiver::run().value(), 0); }
"#,
    );
}
