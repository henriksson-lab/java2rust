#[path = "common/mod.rs"]
mod common;

fn check(name: &str, source: &str, tests: &'static str) {
    let translation = new_java2rust::translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        name,
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests,
        }],
    );
}

#[test]
fn this_interface_conversions_reuse_impl_capabilities() {
    check(
        "this_interface_capabilities",
        include_str!("fixtures/this_interface_capabilities.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn this_uses_the_same_capabilities_as_an_ordinary_implementor() {
    assert_eq!(CapabilitySource::<String>::run().value(), 2);
}
"#,
    );
}

#[test]
fn finally_preserves_the_result_used_after_cleanup() {
    check(
        "finally_retained_result",
        include_str!("fixtures/finally_retained_result.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn cleanup_does_not_consume_a_live_java_reference() {
    assert_eq!(FinallyRetainedResult::run().value(), 5);
}
"#,
    );
}

#[test]
fn matrix_reads_keep_the_allocation_and_java_index_order() {
    common::cargo_run_translated_batch_sources(
        "shared_matrix_read_scope",
        &[new_java2rust::BatchSource::new(
            "SharedMatrix.java",
            include_str!("fixtures/shared_matrix_read_scope.java"),
        )],
        r#"
use translated::SharedMatrix;
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() {
    assert_eq!(SharedMatrix::run().value(), 12);
    assert_eq!(SharedMatrix::row_order().value(), 0);
    assert_eq!(SharedMatrix::null_order().value(), 1);
}
"#,
    );
}
