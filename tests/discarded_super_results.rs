#[path = "common/mod.rs"]
mod common;

#[test]
fn discarded_inherited_reader_results_preserve_exceptions_and_order() {
    common::verify_java_fixture_zero_contract(
        "discarded_super_results",
        "DiscardedSuperResultContracts",
        "tests/corpus/csv/DiscardedSuperResultContracts.java",
        include_str!("corpus/csv/DiscardedSuperResultContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn shared_reader() { assert_eq!(DiscardedSuperResultContracts::run().value(), 0); }
#[test] fn ordinary_reader() {
    let mut reader = SuperResultReader::new(StringReader::new(JavaString::intern_literal("a")).into());
    reader.close_discarded().unwrap();
    let buffer = std::rc::Rc::new(std::cell::RefCell::new(vec![JavaCodeUnit::from_u16(0)]));
    assert!(reader.read_discarded(&buffer).unwrap_err().is_instance(&["IOException"]));
    assert_eq!(reader.completed, 1);
}
"#,
    );
}

#[test]
fn discarded_super_result_contracts_survive_production_assembly() {
    common::cargo_run_translated_batch_sources(
        "discarded_super_results_batch",
        &[new_java2rust::BatchSource::new(
            "DiscardedSuperResultContracts.java",
            include_str!("corpus/csv/DiscardedSuperResultContracts.java"),
        )],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::DiscardedSuperResultContracts::run().value(), 0); }
"#,
    );
}
