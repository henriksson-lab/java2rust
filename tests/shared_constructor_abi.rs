#[path = "common/mod.rs"]
mod common;

#[test]
fn constructor_allocated_source_handle_is_not_wrapped_again_at_interface_conversion() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/shared_constructor_abi.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "shared_constructor_abi",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn interface_conversion_keeps_the_constructor_allocation() {
    assert_eq!(SharedConstructorAbi::run(), 78);
    assert_eq!(SharedConstructorAbi::assigned(), 3);
}

#[test]
fn source_closure_adapter_obeys_the_solved_mutable_receiver() {
    let mut value = 0;
    let mut counter = move || { value += 1; value };
    assert_eq!(SharedCounter::next(&mut counter), 1);
    assert_eq!(SharedCounter::next(&mut counter), 2);
}
"#,
        }],
    );
}
