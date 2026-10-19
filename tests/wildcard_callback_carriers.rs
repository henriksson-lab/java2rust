#[path = "common/mod.rs"]
mod common;

#[test]
fn wildcard_callback_interface_inputs_retain_generic_carriers() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/wildcard_callback_carriers.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "wildcard_callback_carriers",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn callback_generic_input_contract() { assert_eq!(WildcardCallbackCarriers::probe().unwrap(), 4); }
"#,
        }],
    );
}
