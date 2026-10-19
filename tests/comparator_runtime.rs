#[path = "common/mod.rs"]
mod common;

#[test]
fn native_comparator_composition_preserves_key_order_and_checked_effects() {
    let translation = new_java2rust::translate_source(
        "import java.util.Comparator; class ComparatorRuntimeDependency { static Comparator<Integer> order() { return (a, b) -> a - b; } }",
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "comparator_runtime",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn native_comparator_runtime_contract() {
    // Borrowed collection iteration must not require cloning or Java key
    // capabilities merely to visit stored elements.
    fn borrowed_count<T>(hash: &JavaSemanticHashSet<T>, tree: &JavaSemanticTreeSet<T>) -> usize {
        hash.into_iter().count() + tree.into_iter().count()
    }
    struct NotClone(i32);
    assert_eq!(borrowed_count(&JavaSemanticHashSet::<NotClone>::new(), &JavaSemanticTreeSet::<NotClone>::new()), 0);
    let mut hash = JavaSemanticHashSet::new();
    hash.insert(3_i32);
    hash.insert(1_i32);
    let mut tree = JavaSemanticTreeSet::new();
    tree.insert(3_i32);
    tree.insert(1_i32);
    assert_eq!(borrowed_count(&hash, &tree), 4);
    assert_eq!(java_collection_extremum_by_result(&hash, |a, b| Ok(a - b), true).unwrap(), 3);
    assert_eq!(java_collection_extremum_by_result(&tree, |a, b| Ok(a - b), false).unwrap(), 1);
    assert_eq!(hash.len(), 2);
    assert_eq!(tree.len(), 2);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let captured = seen.clone();
    let comparator = Comparator::comparing_int(move |value: NotClone| {
        captured.borrow_mut().push(value.0);
        if value.0 < 0 { return Err(JavaException::new("IllegalArgumentException", "negative".to_string())); }
        Ok(value.0)
    });
    assert!(seen.borrow().is_empty());
    assert_eq!(comparator.compare_result(NotClone(2), NotClone(1)).unwrap(), 1);
    assert_eq!(&*seen.borrow(), &[2, 1]);
    seen.borrow_mut().clear();
    assert!(comparator.compare_result(NotClone(-1), NotClone(3)).is_err());
    assert_eq!(&*seen.borrow(), &[-1]);
    let reversed = comparator.reversed();
    seen.borrow_mut().clear();
    assert_eq!(reversed.compare_result(NotClone(2), NotClone(1)).unwrap(), -1);
    assert_eq!(&*seen.borrow(), &[1, 2]);

    let secondary_calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let captured_calls = secondary_calls.clone();
    let primary = Comparator::comparing_int(|value: i32| Ok(value / 10));
    let chained = primary.then_comparing_int(move |value: i32| {
        captured_calls.set(captured_calls.get() + 1);
        Ok(value % 10)
    });
    assert_eq!(chained.compare_result(21, 22).unwrap(), -1);
    assert_eq!(secondary_calls.get(), 2);
    assert_eq!(chained.compare_result(21, 32).unwrap(), -1);
    assert_eq!(secondary_calls.get(), 2);

    let fail = Comparator::from_fallible_fn(|_: i32, _: i32| {
        Err(JavaException::new("IllegalArgumentException", "primary".to_string()))
    });
    let captured_calls = secondary_calls.clone();
    let combined = fail.then_comparing_int(move |value: i32| {
        captured_calls.set(captured_calls.get() + 1);
        Ok(value)
    });
    assert!(combined.compare_result(1, 2).is_err());
    assert_eq!(secondary_calls.get(), 2);
    let extreme = Comparator::from_fn(|left: i32, right: i32| {
        if left == right { 0 } else if left < right { i32::MIN } else { i32::MAX }
    });
    assert_eq!(extreme.reversed().compare_result(1, 2).unwrap(), i32::MAX);
    assert_eq!(Comparator::<i32>::natural_order().compare_result(9, 2).unwrap(), 1);
    assert_eq!(Comparator::<i32>::reverse_order().compare_result(9, 2).unwrap(), -1);
    assert_eq!(Comparator::<String>::natural_order().compare_result("a".into(), "d".into()).unwrap(), -3);
    assert_eq!(Comparator::<String>::reverse_order().compare_result("a".into(), "d".into()).unwrap(), 3);
    assert_eq!(Comparator::<String>::natural_order().compare_result("😀".into(), "\u{e000}".into()).unwrap(), -1987);
    let nullable = Comparator::<Option<i32>>::natural_order();
    assert!(nullable.compare_result(None, Some(2)).is_err());
    assert!(nullable.compare_result(Some(2), None).is_err());
    assert!(nullable.compare_result(None, None).is_err());
    assert_eq!(nullable.compare_result(Some(9), Some(2)).unwrap(), 1);
    struct CheckedKey(i32);
    impl JavaTreeKey for CheckedKey {
        fn java_tree_cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.cmp(&other.0) }
        fn java_compare_to_result(&self, other: &Self) -> Result<i32, JavaException> {
            if self.0 < 0 || other.0 < 0 { return Err(JavaException::new("IllegalArgumentException", "negative".into())); }
            Ok(self.0 - other.0)
        }
    }
    let checked = Comparator::<CheckedKey>::natural_order();
    assert_eq!(checked.compare_result(CheckedKey(9), CheckedKey(2)).unwrap(), 7);
    assert!(checked.compare_result(CheckedKey(-1), CheckedKey(2)).is_err());
    assert_eq!(checked.reversed().compare_result(CheckedKey(9), CheckedKey(2)).unwrap(), -7);
}
"#,
        }],
    );
}
