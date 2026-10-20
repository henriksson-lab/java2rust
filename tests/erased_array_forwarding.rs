#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_array_elements_preserve_nullable_reference_carriers_through_source_calls() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/erased_array_forwarding.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "erased_array_forwarding",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value<T> { fn value(self) -> T; }
impl Value<bool> for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Value<bool> for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
impl Value<i32> for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value<i32> for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn null_forwarding_alias_identity_and_operand_effects() {
    assert!(UnseededErasedForward::run().value());
    assert!(SeededErasedForward::run().value());
    assert!(ErasedArrayForwarding::nulls().value());
    assert_eq!(ErasedArrayForwarding::alias_mutation().value(), 7);
    assert_eq!(ErasedArrayForwarding::local_and_constructor().value(), 1);
    assert_eq!(ErasedArrayForwarding::primitive_negative().value(), 3);
    assert_eq!(ErasedArrayForwarding::comparison_effects().value(), 22);
    assert_eq!(ErasedArrayForwarding::checked_comparison_effects().value(), 1);
}
"#,
        }],
    );
}
