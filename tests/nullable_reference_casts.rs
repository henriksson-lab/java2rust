#[path = "common/mod.rs"]
mod common;

#[test]
fn reference_casts_preserve_null_across_generic_returns_locals_and_calls() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_reference_casts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_reference_casts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value<T> { fn value(self) -> T; }
impl Value<bool> for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Value<bool> for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
impl Value<i32> for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value<i32> for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reference_cast_value_domains() {
    assert!(NullableReferenceCasts::null_result().value());
    assert!(NullableReferenceCasts::local_result().value());
    assert!(NullableReferenceCasts::assigned_result().value());
    assert!(NullableReferenceCasts::chosen_result().value());
    assert!(NullableReferenceCasts::present_result().value());
    assert!(NullableReferenceCasts::bad_cast().value());
    assert!(NullableReferenceCasts::array_result().value());
    assert!(NullableReferenceCasts::nested_array_result().value());
    assert!(NullableReferenceCasts::inferred_null_result().value());
    assert!(NullableReferenceCasts::inferred_present_result().value());
    assert!(NullableReferenceCasts::null_receiver_caught().value());
    assert_eq!(NullableReferenceCasts::null_receiver_order().value(), 1);
    assert_eq!(NullableReferenceCasts::primitive_result().value(), 3);
}
"#,
        }],
    );
}
