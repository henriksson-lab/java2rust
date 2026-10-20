#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_method_and_owner_parameters_borrow_the_java_handle() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/generic_carrier_borrows.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "generic_carrier_borrows",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(GenericCarrierBorrows::run().value(), 0); }
"#,
        }],
    );
}
