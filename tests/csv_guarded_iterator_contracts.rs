#[path = "common/mod.rs"]
mod common;

#[test]
fn guarded_nullable_storage_returns_non_null_live_iterator_values() {
    common::verify_java_fixture_zero_contract(
        "csv_guarded_iterator",
        "GuardedIteratorContracts",
        "tests/corpus/csv/GuardedIteratorContracts.java",
        include_str!("corpus/csv/GuardedIteratorContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn iterators() { assert_eq!(GuardedIteratorContracts::run().value(), 0); }
"#,
    );
}
