#[path = "common/mod.rs"]
mod common;

#[test]
fn stored_callbacks_share_state_and_defer_exceptions_until_invocation() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_callback_state.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_callback_state",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn stored_callback_contract_matches_the_java_oracle() {
    fn array(values: Vec<i32>) -> std::rc::Rc<std::cell::RefCell<Vec<i32>>> {
        std::rc::Rc::new(std::cell::RefCell::new(values))
    }
    let first = RetainedCallbackState::counter(3);
    let alias = first.clone();
    assert_eq!(first.get_result().unwrap(), 3);
    assert_eq!(alias.get_result().unwrap(), 4);
    assert_eq!(first.get_result().unwrap(), 5);
    assert_eq!(RetainedCallbackState::shared_counter(3).unwrap(), 34);
    assert_eq!(RetainedCallbackState::local_counter().unwrap(), 34);
    assert_eq!(RetainedCallbackState::held(9).unwrap(), 9);
    assert_eq!(RetainedCallbackState::invoke_returned(array(vec![4]), 0).unwrap(), 4);
    let error = RetainedCallbackState::invoke_returned(array(vec![4]), -1).unwrap_err();
    assert!(error.is_instance(&["ArrayIndexOutOfBoundsException"]));
    assert_eq!(RetainedCallbackState::caught_returned(array(vec![4]), -1).unwrap(), -1);
    assert_eq!(RetainedCallbackState::reference().getAsInt_result().unwrap(), 7);
    assert_eq!(RetainedCallbackState::plain(7).getAsInt_result().unwrap(), 7);
    let deferred = RetainedCallbackState::indexed(array(vec![4]), -1);
    let error = deferred.getAsInt_result().unwrap_err();
    assert!(error.is_instance(&["ArrayIndexOutOfBoundsException"]));
    assert_eq!(RetainedCallbackState::caught_index(array(vec![4]), 0).unwrap(), 4);
    assert_eq!(RetainedCallbackState::caught_index(array(vec![4]), -1).unwrap(), -1);
    let values = array(vec![4]);
    let read = RetainedCallbackState::indexed(values.clone(), 0);
    values.borrow_mut()[0] = 9;
    assert_eq!(read.getAsInt_result().unwrap(), 9);
}
"#,
        }],
    );
}

#[test]
fn pure_stored_callbacks_have_the_same_carrier_without_eager_effects() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/pure_callback_carrier.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "pure_callback_carrier",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn pure_factories_and_local_aliases_are_callable() {
    assert_eq!(PureCallbackCarrier::value(5).getAsInt_result().unwrap(), 5);
    assert_eq!(PureCallbackCarrier::local().unwrap(), 14);
}
"#,
        }],
    );
}
