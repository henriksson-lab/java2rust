#[path = "common/mod.rs"]
mod common;

#[test]
fn iterable_factory_only_support_closure_preserves_source_list_precedence() {
    let translation = new_java2rust::translate_source(
        r#"
class List { int marker; }
class FactoryOnlyIterable {
    static java.lang.Iterable<Integer> create() { return java.util.Arrays.asList(2, 3); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "iterable_factory_only",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn factory_without_explicit_java_iterator_use_has_complete_support() {
    let values = FactoryOnlyIterable::create();
    let mut cursor = values.iterator_result().unwrap();
    assert_eq!(cursor.next_result().unwrap(), 2);
    assert_eq!(cursor.next_result().unwrap(), 3);
    assert!(!cursor.has_next_result().unwrap());
}
"#,
        }],
    );
}

#[test]
fn enhanced_for_preserves_lazy_factories_and_catchable_protocol_errors() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/checked_iterable.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "checked_iterable",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> {
    fn checked_value(self) -> i32 { self.unwrap() }
}
#[test]
fn checked_protocol_errors_reach_java_catch() {
    assert_eq!(CheckedIterable::factory_failure().checked_value(), -7);
    assert_eq!(CheckedIterable::fresh_lists().checked_value(), 14);
    assert_eq!(CheckedIterable::source_view().checked_value(), 16);
    let has_failure = Iterable::from_fallible_fn(|| Ok(JavaIterator::new_live(
        || Err(JavaException::new("IllegalStateException", "hasNext".to_string())),
        || Ok(1),
        || Ok(()),
    )));
    assert_eq!(CheckedIterable::consume(&has_failure).checked_value(), -7);
    let next_failure = Iterable::from_fallible_fn(|| Ok(JavaIterator::new_live(
        || Ok(true),
        || Err(JavaException::new("IllegalStateException", "next".to_string())),
        || Ok(()),
    )));
    assert_eq!(CheckedIterable::consume(&next_failure).checked_value(), -7);
}
"#,
        }],
    );
}
