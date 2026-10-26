#[path = "common/mod.rs"]
mod common;
#[test]
fn nullable_owned_generic_keys_query_the_existing_carrier_without_a_clone_bound() {
    common::verify_java_fixture_zero_contract(
        "generic_nullable_map_query",
        "GenericNullableMapLookupProbe",
        "tests/fixtures/GenericNullableMapLookupProbe.java",
        include_str!("fixtures/GenericNullableMapLookupProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lookup_and_null_keys() { assert_eq!(GenericNullableMapLookupProbe::run().value(), 0); }
"#,
    );
}
