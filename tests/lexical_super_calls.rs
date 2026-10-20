#[path = "common/mod.rs"]
mod common;

#[test]
fn constructors_and_instance_initializers_inventory_lexical_super_helpers() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/constructor_lexical_super.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    for helper in [
        "__java_super_constructor_super_base_set",
        "__java_super_constructor_super_base_initial",
    ] {
        assert!(
            translation.rust.contains(&format!("fn {helper}(")),
            "missing {helper}"
        );
    }
    common::cargo_test_generated_modules(
        "constructor_lexical_super",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn constructor_super_oracle() { assert_eq!(ConstructorLexicalSuper::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn covariant_super_helpers_keep_the_declaring_implementation_return_contract() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/covariant_super_contract.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    for suffix in ["(", "_with_outer("] {
        let header = translation
            .rust
            .lines()
            .find(|line| line.contains(&format!("fn __java_super_covariant_parent_get{suffix}")))
            .expect("lexical super implementation entry must be emitted");
        assert!(
            header.contains("CovariantItemValue"),
            "parent implementation must return its root carrier, not the child override ABI: {header}"
        );
    }
    common::cargo_test_generated_modules(
        "covariant_super_contract",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn covariant_super_oracle() { assert_eq!(CovariantSuperContract::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn generic_super_calls_keep_lexical_implementations_across_files_and_binders() {
    let sources = [
        new_java2rust::BatchSource::new(
            "SuperBase.java",
            include_str!("fixtures/lexical_super_calls/SuperBase.java"),
        ),
        new_java2rust::BatchSource::new(
            "SuperChild.java",
            include_str!("fixtures/lexical_super_calls/SuperChild.java"),
        ),
        new_java2rust::BatchSource::new(
            "SuperProbe.java",
            include_str!("fixtures/lexical_super_calls/SuperProbe.java"),
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "generic_lexical_super",
        &sources,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::SuperProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn lexical_super_calls_preserve_the_allocation_without_a_reentrant_guard() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/aliased_lexical_super.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "aliased_lexical_super",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn lexical_super_aliases() { assert_eq!(AliasedLexicalSuper::run().value(), 0); }
"#,
        }],
    );
}
