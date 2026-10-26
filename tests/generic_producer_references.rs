#[path = "common/mod.rs"]
mod common;
#[test]
fn bounded_generic_producer_values_enter_reference_interfaces_with_their_real_type_contract() {
    common::verify_java_fixture_zero_contract(
        "generic_producer_reference",
        "GenericProducerReferenceProbe",
        "tests/fixtures/GenericProducerReferenceProbe.java",
        include_str!("fixtures/GenericProducerReferenceProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn producer_identity() { assert_eq!(GenericProducerReferenceProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn constructor_owner_arguments_specialize_nested_covariant_factory_contracts() {
    let product = include_str!("fixtures/generic_factory_holder/TypedProduct.java");
    let probe = include_str!("fixtures/generic_factory_holder/GenericFactoryHolderProbe.java");
    common::verify_java_sources_zero_contract(
        "GenericFactoryHolderProbe",
        &[
            (
                "tests/fixtures/generic_factory_holder/TypedProduct.java",
                product,
            ),
            (
                "tests/fixtures/generic_factory_holder/GenericFactoryHolderProbe.java",
                probe,
            ),
        ],
    );
    let translated = new_java2rust::translate_source(&format!("{product}\n{probe}"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "generic_factory_holder",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn holder() { assert_eq!(GenericFactoryHolderProbe::run().value(), 0); }
"#,
        }],
    );
}
