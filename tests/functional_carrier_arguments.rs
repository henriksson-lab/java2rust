#[path = "common/mod.rs"]
mod common;

#[test]
fn stored_function_arguments_use_the_shared_move_and_copy_contract() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/functional_carrier_arity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "functional_carrier_arguments",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn one_use_inputs_and_results_need_not_be_cloneable() {
    struct Left(i32);
    struct Right(i32);
    struct Answer(i32);
    let callback = BiFunction::from_fn(|left: Left, right: Right| Answer(left.0 + right.0));
    let slot = FunctionSlot::new(callback);
    assert_eq!(slot.apply(Left(20), Right(22)).unwrap().0, 42);
    assert_eq!(FunctionalCarrierArity::sum(20, 22).unwrap(), 42);
}
#[test]
fn repeated_inputs_keep_their_shared_state() {
    let left = std::rc::Rc::new(std::cell::Cell::new(1));
    let right = std::rc::Rc::new(std::cell::Cell::new(2));
    let callback = BiFunction::from_fn(|left: std::rc::Rc<std::cell::Cell<i32>>, right: std::rc::Rc<std::cell::Cell<i32>>| {
        left.set(left.get() + 1);
        right.set(right.get() + 1);
        left.get() + right.get()
    });
    let slot = FunctionSlot::new(callback);
    assert_eq!(slot.apply_twice(&left, &right).unwrap(), 7);
    assert_eq!(left.get(), 3);
    assert_eq!(right.get(), 4);
}
"#,
        }],
    );
}
