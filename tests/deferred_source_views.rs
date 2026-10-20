#[path = "common/mod.rs"]
mod common;

#[test]
fn bound_field_references_capture_values_but_nested_lambdas_capture_the_owner() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/bound_field_reference_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "bound_field_reference_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(BoundFieldReferenceValues::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn deferred_views_keep_construction_effects_and_receiver_identity_separate() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/deferred_source_views.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "deferred_source_views",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(DeferredSourceViews::run().value(), 0); }
"#,
        }],
    );
}
