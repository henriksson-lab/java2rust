#[path = "common/mod.rs"]
mod common;

#[test]
fn wildcard_carrier_conversions_survive_production_assembly() {
    common::cargo_run_translated_batch_sources(
        "existential_reference_carrier_assembly",
        &[new_java2rust::BatchSource::new(
            "ExistentialReferenceCarrierProbe.java",
            include_str!("fixtures/ExistentialReferenceCarrierProbe.java"),
        )],
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::ExistentialReferenceCarrierProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn wildcard_carrier_conversions_retain_concrete_types_and_shared_identity() {
    common::verify_java_fixture_zero_contract(
        "existential_reference_carrier",
        "ExistentialReferenceCarrierProbe",
        "tests/fixtures/ExistentialReferenceCarrierProbe.java",
        include_str!("fixtures/ExistentialReferenceCarrierProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn capture_mutation_and_aliases_match_java() { assert_eq!(ExistentialReferenceCarrierProbe::run().value(), 0); }
"#,
    );
}
