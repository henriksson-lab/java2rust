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
trait Value { fn value(self) -> String; }
impl Value for String { fn value(self) -> String { self } }
impl<E: std::fmt::Debug> Value for Result<String, E> { fn value(self) -> String { self.unwrap() } }
#[test]
fn java_generic_display_contract() {
    assert_eq!(GenericCollectionDisplay::probe().value(), "([alpha, beta]);map={k=v};[alpha, beta];<[alpha, beta]>;([alpha, beta])");
    assert_eq!(GenericCollectionDisplay::null_probe().value(), "(null)");
    struct NoDisplay;
    assert_eq!(GenericCollectionDisplay::unused::<NoDisplay>(Default::default()), "unused");
    struct NoClone(i32);
    impl JavaArraysDeepToStringElement for NoClone {
        fn java_arrays_deep_to_string(&self) -> String { self.0.to_string() }
    }
    let values = vec![NoClone(3), NoClone(1)];
    assert_eq!(GenericCollectionDisplay::wrapped(Some(&values)).value(), "([3, 1])");
    assert_eq!(GenericCollectionDisplay::forwarded(&values).value(), "([3, 1])");
    // A checked generic leaf remains displayable under nullable wrappers;
    // neither the wrapper nor its present value needs to be cloned.
    fn nullable_display<T: JavaObjectDisplay>(value: &Option<T>) -> String {
        JavaObjectDisplay::java_object_display_result(value).unwrap()
    }
    assert_eq!(nullable_display(&Some(NoClone(7))), "7");
    assert_eq!(nullable_display::<NoClone>(&None), "null");
    assert_eq!(nullable_display(&Some(Some(NoClone(9)))), "9");
    assert_eq!(nullable_display(&Some(None::<NoClone>)), "null");
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
