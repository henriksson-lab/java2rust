#[path = "common/mod.rs"]
mod common;
#[test]
fn array_view_owner_domains_preserve_live_aliases_and_static_retention() {
    common::verify_java_fixture_zero_contract(
        "csv_array_view_domains",
        "ArrayViewDomainContracts",
        "tests/corpus/csv/ArrayViewDomainContracts.java",
        include_str!("corpus/csv/ArrayViewDomainContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn domains() { assert_eq!(ArrayViewDomainContracts::run().value(), 0); }
"#,
    );
}
