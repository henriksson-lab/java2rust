#[path = "common/mod.rs"]
mod common;

#[test]
fn iterator_factory_bindings_preserve_source_and_value_precedence() {
    common::verify_java_fixture_zero_contract(
        "csv_iterator_factory_precedence",
        "IteratorFactoryPrecedence",
        "tests/corpus/csv/IteratorFactoryPrecedence.java",
        include_str!("corpus/csv/IteratorFactoryPrecedence.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn precedence() { assert_eq!(IteratorFactoryPrecedence::run().value(), 0); }
"#,
    );
}

#[test]
fn iterator_spliterator_stream_bridge_retains_lazy_escaping_checked_cursor() {
    common::verify_java_fixture_zero_contract(
        "csv_iterator_stream",
        "IteratorStreamProbe",
        "tests/corpus/csv/IteratorStreamProbe.java",
        include_str!("corpus/csv/IteratorStreamProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn bridge() { assert_eq!(IteratorStreamProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn iterator_stream_bridge_survives_production_assembly() {
    common::cargo_run_translated_batch_sources(
        "csv_iterator_stream_batch",
        &[new_java2rust::BatchSource::new(
            "IteratorStreamProbe.java",
            include_str!("corpus/csv/IteratorStreamProbe.java"),
        )],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::IteratorStreamProbe::run().value(), 0); }
"#,
    );
}
