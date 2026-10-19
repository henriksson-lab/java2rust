#[path = "common/mod.rs"]
mod common;

#[test]
fn shared_field_array_writes_keep_allocation_identity_and_java_order() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/shared_field_arrays.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "shared_field_arrays",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Int { fn value(self) -> i32; }
impl Int for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Int for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
trait Bool { fn value(self) -> bool; }
impl Bool for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Bool for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
#[test] fn aliases_and_rebinding_keep_the_original_array() {
    assert_eq!(SharedFieldArrays::alias_write().value(), 8);
    assert_eq!(SharedFieldArrays::rhs_read().value(), 4);
    assert_eq!(SharedFieldArrays::compound_read().value(), 2);
    assert_eq!(SharedFieldArrays::rebinding().value(), 799);
    assert_eq!(SharedFieldArrays::self_rebinding().value(), 99);
    assert!(SharedFieldArrays::chained_nulls().value());
    assert_eq!(SharedFieldArrays::static_alias().value(), 8);
    assert_eq!(SharedFieldArrays::local_only().value(), 2);
    assert_eq!(SharedFieldArrays::updates().value(), 133);
    assert_eq!(SharedFieldArrays::division(2).value(), 4);
    assert_eq!(SharedFieldArrays::division(0).value(), 108);
    assert_eq!(SharedFieldArrays::cloned_allocation().value(), 17);
    assert_eq!(SharedFieldArrays::argument_mutation().value(), 6);
    assert_eq!(SharedFieldArrays::shallow_clone().value(), 72);
}
#[test] fn receiver_index_rhs_and_exceptions_preserve_java_order() {
    assert_eq!(SharedFieldArrays::order(true, false, 0).value(), 11);
    assert_eq!(SharedFieldArrays::order(false, true, 0).value(), 1231);
    assert_eq!(SharedFieldArrays::order(false, false, 2).value(), 1232);
    assert_eq!(SharedFieldArrays::order(false, false, 0).value(), 1230);
    assert_eq!(SharedFieldArrays::compound_order(true, 0).value(), 121);
    assert_eq!(SharedFieldArrays::compound_order(false, 2).value(), 122);
    assert_eq!(SharedFieldArrays::compound_order(false, 0).value(), 1230);
}
"#,
        }],
    );
}
