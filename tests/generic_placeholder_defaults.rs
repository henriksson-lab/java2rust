#[path = "common/mod.rs"]
mod common;
#[test]
fn generic_carrier_default_requires_its_payload_default_without_inventing_collection_values() {
    common::verify_java_fixture_zero_contract(
        "generic_placeholder_default",
        "GenericPlaceholderDefaultProbe",
        "tests/fixtures/GenericPlaceholderDefaultProbe.java",
        include_str!("fixtures/GenericPlaceholderDefaultProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn aliases() { assert_eq!(GenericPlaceholderDefaultProbe::run().value(), 0); }
"#,
    );
}
