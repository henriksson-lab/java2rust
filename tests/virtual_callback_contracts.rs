#[path = "common/mod.rs"]
mod common;

#[test]
fn source_override_groups_share_one_callback_value_abi() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/virtual_callback_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "virtual_callback_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn virtual_callback_values_match_java() {
    assert_eq!(VirtualCallbackContracts::forwarded().unwrap(), 10);
    assert_eq!(VirtualCallbackContracts::direct().unwrap(), 7);
    assert_eq!(VirtualCallbackContracts::dispatched().unwrap(), 8);
    assert_eq!(VirtualCallbackContracts::retained().unwrap(), 6);
    assert_eq!(VirtualCallbackContracts::caught().unwrap(), 9);
}
"#,
        }],
    );
}
