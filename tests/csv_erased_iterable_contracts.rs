#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_iterable_operations_retain_identity_and_live_factories() {
    let translated =
        new_java2rust::translate_source(include_str!("corpus/csv/ErasedStorageContracts.java"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    common::cargo_test_generated_modules(
        "csv_erased_iterable_operations",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
#[test] fn operations_are_live_lazy_and_preserve_cursor_aliases() {
    use std::rc::Rc;
    use std::cell::{Cell, RefCell};
    let values = Rc::new(RefCell::new(vec![Some(JavaString::from("before")), None]));
    let factories = Rc::new(Cell::new(0));
    let source = values.clone();
    let count = factories.clone();
    let view = JavaErasedIterableView::from_factory(Rc::as_ptr(&values).cast(), move || {
        count.set(count.get() + 1);
        let position = Rc::new(Cell::new(0));
        let has_source = source.clone();
        let has_position = position.clone();
        let next_source = source.clone();
        let next_position = position.clone();
        Ok(JavaErasedIterator::from_operations(
            move || Ok(has_position.get() < has_source.borrow().len()),
            move || {
                let index = next_position.get();
                let value = next_source.borrow().get(index).cloned().ok_or_else(|| JavaException::new("NoSuchElementException", "end".to_string()))?;
                next_position.set(index + 1);
                Ok(value.map(|value| Box::new(value) as Box<dyn std::any::Any>))
            },
            || Err(JavaException::new("UnsupportedOperationException", "fixed size".to_string())),
        ))
    });
    let object = JavaErasedValue::from_native_reference(values.clone())
        .with_iterable_projection(JavaErasedIterableProjection::Present(view));
    let field = object.clone();
    assert!(java_erased_iterable_type_test_result(Some(&object)).unwrap());
    let recovered = java_erased_iterable_cast_result(Some(&field)).unwrap().unwrap();
    assert_eq!(factories.get(), 0);
    assert_eq!(recovered.java_reference_identity(), Rc::as_ptr(&values).cast());
    assert_eq!(object.java_reference_identity(), recovered.java_reference_identity());
    values.borrow_mut()[0] = Some(JavaString::from("late"));
    let cursor = recovered.iterator_result().unwrap();
    let cursor_alias = cursor.clone();
    assert_eq!(factories.get(), 1);
    assert!(cursor.has_next_result().unwrap());
    let first = cursor.next_result().unwrap().unwrap().downcast::<JavaString>().unwrap();
    assert_eq!(first.units(), JavaString::from("late").units());
    // Mutate the same backing allocation between operations, as callbacks do.
    values.borrow_mut()[1] = Some(JavaString::from("callback"));
    let second = cursor_alias.next_result().unwrap().unwrap().downcast::<JavaString>().unwrap();
    assert_eq!(second.units(), JavaString::from("callback").units());
    assert!(!cursor.has_next_result().unwrap());
    assert!(cursor.next_result().unwrap_err().is_instance(&["NoSuchElementException"]));
    assert!(cursor.remove_result().unwrap_err().is_instance(&["UnsupportedOperationException"]));
    values.borrow_mut()[0] = None;
    let next_cursor = recovered.iterator_result().unwrap();
    assert_eq!(factories.get(), 2);
    assert!(next_cursor.next_result().unwrap().is_none());
}

#[test] fn iterator_factory_reentry_propagates_the_original_exception() {
    use std::rc::Rc;
    use std::cell::{Cell, RefCell};
    let holder = Rc::new(RefCell::new(None::<JavaErasedIterableView>));
    let weak = Rc::downgrade(&holder);
    let count = Rc::new(Cell::new(0));
    let calls = count.clone();
    let view = JavaErasedIterableView::from_factory(Rc::as_ptr(&holder).cast(), move || {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            let nested = weak.upgrade().unwrap().borrow().as_ref().unwrap().clone();
            return nested.iterator_result();
        }
        Err(JavaException::new("IOException", "factory failure".to_string()))
    });
    *holder.borrow_mut() = Some(view.clone());
    let object = JavaErasedValue::from_native_reference(holder.clone())
        .with_iterable_projection(JavaErasedIterableProjection::Present(view));
    let view = java_erased_iterable_cast_result(Some(&object)).unwrap().unwrap();
    assert_eq!(count.get(), 0);
    assert!(view.iterator_result().err().unwrap().is_instance(&["IOException"]));
    assert_eq!(count.get(), 2);
}

#[test] fn negative_null_and_unknown_capabilities_remain_distinct() {
    assert!(!java_erased_iterable_type_test_result(None).unwrap());
    assert!(java_erased_iterable_cast_result(None).unwrap().is_none());
    let absent = JavaErasedValue::new(3i32).with_iterable_projection(JavaErasedIterableProjection::Absent);
    assert!(!java_erased_iterable_type_test_result(Some(&absent)).unwrap());
    assert!(java_erased_iterable_cast_result(Some(&absent)).unwrap_err().is_instance(&["ClassCastException"]));
    struct Unknown;
    assert!(java_erased_iterable_type_test_result(Some(&Unknown)).unwrap_err().is_instance(&["UnsupportedOperationException"]));
    assert!(java_erased_iterable_cast_result(Some(&Unknown)).unwrap_err().is_instance(&["UnsupportedOperationException"]));
}
"#,
        }],
    );
}

#[test]
fn erased_reference_array_and_raw_iterable_tests_use_java_type_evidence() {
    common::verify_java_fixture_zero_contract(
        "csv_erased_capability_types",
        "ErasedCapabilityTypeContracts",
        "tests/corpus/csv/ErasedCapabilityTypeContracts.java",
        include_str!("corpus/csv/ErasedCapabilityTypeContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn type_tests() { assert_eq!(ErasedCapabilityTypeContracts::run().value(), 0); }
"#,
    );
}
