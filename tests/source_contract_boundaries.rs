#[path = "common/mod.rs"]
mod common;

#[test]
fn inherited_array_abi_lexical_statics_and_checked_numeric_casts_execute() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_contract_boundaries.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "source_contract_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_contracts() { assert_eq!(SourceContractBoundaries::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}
