#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_collection_formatting_preserves_callbacks_nulls_and_exceptions() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_collection_formatting.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_collection_formatting",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(RetainedCollectionFormatting::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn nullable_generic_interface_results_are_checked_before_collection_copying() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/nullable_source_collection_results.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_source_collection_results",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(NullableSourceCollectionResults::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn physical_values_survive_borrowed_staging_mutable_iteration_and_copying() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/physical_call_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "physical_call_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(PhysicalCallValues::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn nested_source_calls_preserve_receiver_identity_argument_order_and_exceptions() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_call_staging.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_call_staging",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(SourceCallStaging::run().value(), 0); }
"#,
        }],
    );
}
