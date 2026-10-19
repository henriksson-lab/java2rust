#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_source_iterable_supplier_uses_a_live_interface_carrier() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/live_iterable_supplier.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "live_iterable_supplier",
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
fn retained_supplier_is_reinvoked_after_state_changes() {
    assert_eq!(LiveIterableSupplier::direct().checked_value(), 16);
    assert_eq!(LiveIterableSupplier::through_interface().checked_value(), 16);
}
"#,
        }],
    );
}
