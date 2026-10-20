#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_deques_preserve_endpoint_policy_aliases_and_checked_results() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_deque_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_deque_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(RetainedDequeContracts::run().value(), 0); }
"#,
        }],
    );
}
