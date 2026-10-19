#[path = "common/mod.rs"]
mod common;

#[test]
fn chained_array_assignments_return_converted_reference_carriers() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/chained_reference_assignments.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "chained_reference_assignments",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Bool { fn value(self) -> bool; }
impl Bool for bool { fn value(self) -> bool { self } }
impl<E: std::fmt::Debug> Bool for Result<bool, E> { fn value(self) -> bool { self.unwrap() } }
trait Int { fn value(self) -> i32; }
impl Int for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Int for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn null_assignment_returns_the_nullable_value() { assert!(ChainedReferenceAssignments::clear().value()); }
#[test] fn shared_assignment_does_not_clone_the_java_object() { assert_eq!(ChainedReferenceAssignments::fill().value(), 14); }
#[test] fn shared_strings_keep_reference_identity() { assert!(ChainedReferenceAssignments::strings().value()); }
"#,
        }],
    );
}
