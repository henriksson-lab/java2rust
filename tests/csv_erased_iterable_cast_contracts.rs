#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_iterable_casts_retain_source_factories_nulls_and_synchronized_consumers() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_iterable_casts",
        "ErasedIterableCastContracts",
        "tests/corpus/csv/ErasedIterableCastContracts.java",
        include_str!("corpus/csv/ErasedIterableCastContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn erased_casts() { assert_eq!(ErasedIterableCastContracts::run().value(), 0); }
"#,
    );
}

#[test]
fn erased_iterable_source_factories_survive_batch_runtime_assembly() {
    common::cargo_run_translated_batch_sources(
        "csv_erased_iterable_cast_batch",
        &[
            new_java2rust::BatchSource::new(
                "ErasedIterableCastContracts.java",
                include_str!("corpus/csv/ErasedIterableCastContracts.java"),
            ),
            new_java2rust::BatchSource::new(
                "ErasedIterableCastPeer.java",
                include_str!("corpus/csv/ErasedIterableCastPeer.java"),
            ),
        ],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ErasedIterableCastContracts::run().value(), 0); }
"#,
    );
}
