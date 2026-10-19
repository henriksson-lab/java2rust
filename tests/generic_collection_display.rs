#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_display_capabilities_follow_selected_source_calls_without_clone_requirements() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/generic_collection_display.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "generic_collection_display",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn java_generic_display_contract() {
    assert_eq!(GenericCollectionDisplay::probe(), "([alpha, beta]);map={k=v};[alpha, beta];<[alpha, beta]>;([alpha, beta])");
    assert_eq!(GenericCollectionDisplay::null_probe(), "(null)");
    struct NoDisplay;
    assert_eq!(GenericCollectionDisplay::unused::<NoDisplay>(Default::default()), "unused");
    struct NoClone(i32);
    impl JavaArraysDeepToStringElement for NoClone {
        fn java_arrays_deep_to_string(&self) -> String { self.0.to_string() }
    }
    let values = vec![NoClone(3), NoClone(1)];
    assert_eq!(GenericCollectionDisplay::wrapped(Some(&values)), "([3, 1])");
    assert_eq!(GenericCollectionDisplay::forwarded(&values), "([3, 1])");
    // Visiting/rendering existing sets and maps needs no Java key, Ord, Eq,
    // Hash or Clone capability. Those are separate mutation obligations.
    fn set_display<T: JavaArraysDeepToStringElement>(value: &JavaSemanticHashSet<T>) -> String {
        value.java_arrays_deep_to_string()
    }
    fn tree_display<T: JavaArraysDeepToStringElement>(value: &JavaSemanticTreeSet<T>) -> String {
        value.java_arrays_deep_to_string()
    }
    fn map_display<K: JavaArraysDeepToStringElement, V: JavaArraysDeepToStringElement>(value: &JavaSemanticHashMap<K, V>) -> String {
        value.java_arrays_deep_to_string()
    }
    assert_eq!(set_display(&JavaSemanticHashSet::<NoClone>::new()), "[]");
    assert_eq!(tree_display(&JavaSemanticTreeSet::<NoClone>::new()), "[]");
    assert_eq!(map_display(&JavaSemanticHashMap::<NoClone, NoClone>::new()), "{}");
}
"#,
        }],
    );
}
