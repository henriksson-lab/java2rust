#[path = "common/mod.rs"]
mod common;

#[test]
fn bulk_membership_preserves_live_sources_and_checked_mutation() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/mixed_bulk_membership.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "mixed_bulk_membership",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(MixedBulkMembership::run().value(), 0); }
#[test] fn live_bulk_source_releases_guards_and_observes_updates() {
    let source = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![1, 2], true)));
    let mut observed = Vec::new();
    assert!(java_native_collection_all_result(&source, |value| {
        observed.push(*value);
        source.borrow_mut().list_set_result(1, 3)?;
        Ok(true)
    }).unwrap());
    assert_eq!(observed, vec![1, 3]);
    let invalidated = java_native_collection_all_result(&source, |_| {
        source.borrow_mut().list_add_result(4)?;
        Ok(true)
    }).unwrap_err();
    assert_eq!(invalidated.class_name(), "ConcurrentModificationException");
    let mut calls = 0;
    assert!(!java_native_collection_all_result(&source, |_| {
        calls += 1;
        source.borrow_mut().list_add_result(5)?;
        Ok(false)
    }).unwrap());
    assert_eq!(calls, 1);
}
#[test] fn retain_keeps_partial_progress_and_uses_live_identity() {
    let mut partial = vec![1, 2, 3, 4];
    let mut calls = 0;
    let error = java_collection_retain_all_result(
        |keep| partial.retain(keep),
        |value| {
            calls += 1;
            if *value == 2 { Err(JavaException::new("Failure", "membership")) }
            else { Ok(false) }
        }).unwrap_err();
    assert_eq!(error.class_name(), "Failure");
    assert_eq!(partial, vec![2, 3, 4]);
    assert_eq!(calls, 2);
    let source = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![1, 2], true)));
    assert!(!java_native_collection_retain_all_projected_result(&source, &source, |a, b| a == b).unwrap());
    assert_eq!(source.borrow().cloned_items(), vec![1, 2]);
    let only_two = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![2], true)));
    assert!(java_native_collection_retain_all_projected_result(&source, &only_two, |a, b| a == b).unwrap());
    assert_eq!(source.borrow().cloned_items(), vec![2]);
    let fixed = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![1, 2], false)));
    assert_eq!(java_native_collection_retain_all_projected_result(&fixed, &only_two, |a, b| a == b)
        .unwrap_err().class_name(), "UnsupportedOperationException");
    assert_eq!(fixed.borrow().cloned_items(), vec![1, 2]);
}
#[test] fn checked_projection_preserves_errors_partial_progress_and_reentrancy() {
    let source = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![2], true)));
    for keep in [false, true] {
        let first = if keep { 1 } else { 2 };
        let receiver = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![first, 3, 4], true)));
        let mut calls = 0;
        let equal = |query: &i32, stored: &i32| {
            calls += 1;
            // Both allocations must be released before a Java callback.
            let last = receiver.borrow().len() - 1;
            receiver.borrow_mut().list_set_result(last as i32, 9)?;
            source.borrow_mut().list_set_result(0, 2)?;
            if *query == 3 { Err(JavaException::new("IllegalStateException", "equals")) }
            else { Ok(query == stored) }
        };
        let error = if keep {
            java_native_collection_retain_all_projected_checked_result(&receiver, &source, equal)
        } else {
            java_native_collection_remove_all_projected_checked_result(&receiver, &source, equal)
        }.unwrap_err();
        assert_eq!(error.class_name(), "IllegalStateException");
        assert_eq!(receiver.borrow().cloned_items(), vec![3, 9]);
        assert_eq!(calls, 2);
    }
    let mut calls = 0;
    assert!(java_collection_any_result([1, 2, 3], |item| {
        calls += 1;
        if item == 3 { panic!("must short circuit") }
        Ok(item == 2)
    }).unwrap());
    assert_eq!(calls, 2);
    assert_eq!(java_collection_any_result([1], |_| Err(JavaException::new("Failure", "equals")))
        .unwrap_err().class_name(), "Failure");
}
"#,
        }],
    );
}
