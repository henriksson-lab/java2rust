#[path = "common/mod.rs"]
mod common;

#[test]
fn comparator_factories_preserve_types_captures_laziness_and_errors() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/comparator_factories.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "comparator_factories",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn comparator_factory_values_match_java() {
    assert_eq!(ComparatorFactories::no_eager(), 0);
    assert_eq!(ComparatorFactories::compared().unwrap(), 12);
    assert_eq!(ComparatorFactories::reversed().unwrap(), -1);
    assert_eq!(ComparatorFactories::chained().unwrap(), -1);
    assert_eq!(ComparatorFactories::caught().unwrap(), 9);
    assert_eq!(ComparatorFactories::count(), 1);
    assert_eq!(ComparatorFactories::retained().unwrap(), 0);
    assert_eq!(ComparatorFactories::caught_capture().unwrap(), 8);
    assert_eq!(ComparatorFactories::rebound().unwrap(), 1);
    assert_eq!(ComparatorFactories::bound_chain().unwrap(), -1);
    assert_eq!(ComparatorFactories::string_compared().unwrap(), 1);
}
"#,
        }],
    );
}

#[test]
fn source_comparator_factory_and_instance_methods_take_precedence() {
    let translation = new_java2rust::translate_source(
        r#"
class Comparator {
    static Comparator comparingInt(int value) { return new Comparator(); }
    Comparator reversed() { return this; }
    int thenComparingInt(int value) { return value + 7; }
}
class SourceComparatorFactory {
    static int check() {
        return Comparator.comparingInt(1).reversed().thenComparingInt(2);
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_comparator_factory",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_comparator_is_not_a_native_factory() {
    assert_eq!(SourceComparatorFactory::check(), 9);
}
"#,
        }],
    );
}
