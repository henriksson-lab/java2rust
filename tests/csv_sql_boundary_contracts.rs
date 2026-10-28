#[path = "common/mod.rs"]
mod common;

#[test]
fn sql_facade_forwarding_keeps_typed_loans_nullable_results_and_java_text() {
    // The null paths are a Java/Rust oracle; non-null database operations are
    // compiled here, but the unavailable JDBC backend is not claimed to work.
    common::verify_java_fixture_zero_contract(
        "csv_sql_boundary",
        "SqlBoundaryContracts",
        "tests/corpus/csv/SqlBoundaryContracts.java",
        include_str!("corpus/csv/SqlBoundaryContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn boundaries() { assert_eq!(SqlBoundaryContracts::run().value(), 0); }
"#,
    );
}
