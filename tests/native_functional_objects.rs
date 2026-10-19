#[path = "common/mod.rs"]
mod common;

#[test]
fn locally_converted_functional_objects_share_state_without_factory_escape_evidence() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/native_functional_alias.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "native_functional_alias",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn both_adapters_and_the_source_observe_the_same_counter() {
    assert_eq!(NativeFunctionalAlias::shared_counter().unwrap(), 345);
}
"#,
        }],
    );
}

#[test]
fn source_objects_keep_the_native_functional_value_contract() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/native_functional_objects.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation.diagnostics.has_code("unsupported_inheritance"),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .diagnostics
            .has_code("unsupported_interface_impl"),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "native_functional_objects",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn native_functional_objects_match_java() {
    assert_eq!(NativeFunctionalObjects::sampled().unwrap(), 345);
    assert_eq!(NativeFunctionalObjects::sampled_inherited().unwrap(), 8);
    assert_eq!(NativeFunctionalObjects::compared().unwrap(), 1);
    assert_eq!(NativeFunctionalObjects::field_conversion().unwrap(), 1);
    assert_eq!(NativeFunctionalObjects::replaced_field().unwrap(), 9);
    assert_eq!(NativeFunctionalObjects::through_field().unwrap(), 9);
    assert_eq!(NativeFunctionalObjects::joined().unwrap(), "firstsecond");
    assert_eq!(NativeFunctionalObjects::nullable(false).unwrap(), -1);
    assert_eq!(NativeFunctionalObjects::nullable(true).unwrap(), 5);
    assert_eq!(NativeFunctionalObjects::null_source(), 1);
    assert_eq!(NativeFunctionalObjects::shared_object().unwrap(), 78);
    assert_eq!(NativeFunctionalObjects::caught().unwrap(), 9);
}
"#,
        }],
    );
}
