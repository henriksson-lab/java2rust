#[path = "common/mod.rs"]
mod common;

#[test]
fn logical_chains_keep_lazy_order_and_exception_boundaries_on_the_default_stack() {
    let translation = new_java2rust::translate_source(include_str!("fixtures/logical_chains.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "logical_chains",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lazy_boolean_evaluation_matches_java() { assert_eq!(LogicalChains::run().value(), 1); }
"#,
        }],
    );
}
