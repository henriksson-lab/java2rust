#[path = "common/mod.rs"]
mod common;

#[test]
fn constructors_receive_native_sam_values_with_deferred_effects() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/constructor_callback_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "constructor_callback_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> {
    fn checked_value(self) -> i32 { self.unwrap() }
}
#[test]
fn java_constructor_callback_values_and_effects() {
    assert_eq!(ConstructorCallbackValues::lambdas().checked_value(), 23);
    assert_eq!(ConstructorCallbackValues::references().checked_value(), 7);
    assert_eq!(ConstructorCallbackValues::deferred_failure().checked_value(), -1);
    assert_eq!(ConstructorCallbackValues::delegated().checked_value(), 9);
    assert_eq!(ConstructorCallbackValues::nullable().checked_value(), 4);
    assert_eq!(ConstructorCallbackValues::sequence_result().checked_value(), 9);
    assert_eq!(ConstructorCallbackValues::borrowed_result().checked_value(), 6);
    assert_eq!(ConstructorCallbackValues::stored_sequence().checked_value(), 6);
    assert_eq!(ConstructorCallbackValues::nested_delegation().checked_value(), 69);
    assert_eq!(ConstructorCallbackValues::nested_failure().checked_value(), -3);
    assert_eq!(ConstructorCallbackValues::delegated_result().checked_value(), 9);
    struct NoClone(i32);
    let value = Supplier::from_fallible_fn(|| Ok(Iterable::from_fallible_fn(|| {
        Ok(JavaIterator::new(vec![NoClone(3), NoClone(4)]))
    })));
    let sequence = CallbackSequence::new(value);
    let mut cursor = sequence.iterator().unwrap();
    assert_eq!(std::iter::Iterator::next(&mut cursor).unwrap().0, 3);
    assert_eq!(std::iter::Iterator::next(&mut cursor).unwrap().0, 4);
    assert!(std::iter::Iterator::next(&mut cursor).is_none());
}
"#,
        }],
    );
}
