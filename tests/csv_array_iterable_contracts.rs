#[path = "common/mod.rs"]
mod common;

#[test]
fn array_backed_iterables_preserve_identity_list_projection_and_reentry() {
    common::verify_java_fixture_zero_contract(
        "csv_array_iterable",
        "ArrayIterableContracts",
        "tests/corpus/csv/ArrayIterableContracts.java",
        include_str!("corpus/csv/ArrayIterableContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn iterables() { assert_eq!(ArrayIterableContracts::run().value(), 0); }

#[test] fn typed_erased_cursor_reentry_and_non_clone_values() {
    use std::rc::Rc;
    use std::cell::{Cell, RefCell};
    struct ValueWithoutClone(Rc<Cell<i32>>);
    let state = Rc::new(Cell::new(7));
    let source = state.clone();
    let holder = Rc::new(RefCell::new(None::<JavaErasedIterator>));
    let weak = Rc::downgrade(&holder);
    let position = Rc::new(Cell::new(false));
    let has = position.clone();
    let next = position.clone();
    let typed = JavaIterator::new_live(move || Ok(!has.get()), move || {
        let alias = weak.upgrade().unwrap().borrow().as_ref().unwrap().clone();
        assert!(alias.has_next_result()?);
        next.set(true);
        Ok(ValueWithoutClone(source.clone()))
    }, || Ok(()));
    let cursor = JavaErasedIterator::from_typed(typed, |value| Some(Box::new(value)));
    *holder.borrow_mut() = Some(cursor.clone());
    let value = cursor.next_result().unwrap().unwrap().downcast::<ValueWithoutClone>().unwrap();
    assert!(Rc::ptr_eq(&state, &value.0));
    assert!(!cursor.has_next_result().unwrap());
}

#[test] fn snapshot_removal_callbacks_run_outside_cursor_loans() {
    use std::rc::Rc;
    use std::cell::{Cell, RefCell};
    let holder = Rc::new(RefCell::new(None::<JavaErasedIterator>));
    let weak = Rc::downgrade(&holder);
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let typed = JavaIterator::new_with_remove(vec![1i32, 2], move |index| {
        assert_eq!(index, 0);
        let alias = weak.upgrade().unwrap().borrow().as_ref().unwrap().clone();
        assert!(alias.has_next_result()?);
        count.set(count.get() + 1);
        Ok(if count.get() == 1 { JavaIteratorRemoveStatus::Deferred } else { JavaIteratorRemoveStatus::Applied })
    });
    let cursor = JavaErasedIterator::from_typed(typed, |value| Some(Box::new(value)));
    *holder.borrow_mut() = Some(cursor.clone());
    assert_eq!(*cursor.next_result().unwrap().unwrap().downcast::<i32>().unwrap(), 1);
    cursor.remove_result().unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(*cursor.next_result().unwrap().unwrap().downcast::<i32>().unwrap(), 2);
    assert_eq!(calls.get(), 2);
    assert!(!cursor.has_next_result().unwrap());
}

#[test] fn native_erased_collection_copies_retain_non_clone_payloads_and_array_owners() {
    use std::{any::Any, cell::{Cell, RefCell}, rc::Rc, sync::{Arc, Mutex}};
    type Item = Option<Box<dyn Any>>;
    struct WithoutClone(Rc<Cell<i32>>);
    let state = Rc::new(Cell::new(7));
    let original = JavaErasedValue::new(WithoutClone(state.clone()));
    let identity = original.java_reference_identity();
    let item = || Some(Box::new(original.clone()) as Box<dyn Any>);
    let backing = Rc::new(RefCell::new(vec![item(), None]));
    let collection = Rc::new(RefCell::new(JavaNativeCollection::from_local_array(backing.clone())));
    let iterable = Iterable::from_native_collection_with_copy(collection.clone(), java_clone_optional_erased_object_ref);
    assert_eq!(iterable.list_type_test(), Some(true));
    let mut cursor = iterable.iterator_result().unwrap();
    let first = cursor.next_result().unwrap().unwrap();
    assert_eq!(first.as_ref().java_reference_identity(), identity);
    first.downcast_ref::<JavaErasedValue>().unwrap().as_any().downcast_ref::<WithoutClone>().unwrap().0.set(11);
    assert_eq!(state.get(), 11);
    // Simulate a Java callback changing the next array slot after next().
    backing.borrow_mut()[1] = item();
    assert_eq!(cursor.next_result().unwrap().unwrap().as_ref().java_reference_identity(), identity);
    assert!(cursor.next_result().is_err());
    assert!(cursor.remove_result().is_err());
    let list = iterable.list_view_result().unwrap();
    assert_eq!(list.get_result(0).unwrap().unwrap().as_ref().java_reference_identity(), identity);
    assert!(list.set_result(0, None).unwrap().is_some());
    assert!(backing.borrow()[0].is_none());
    assert!(list.add_result(item()).is_err());
    let mut second = iterable.iterator_result().unwrap();
    assert!(second.next_result().unwrap().is_none());
    let shared: Arc<Mutex<Vec<Item>>> = Arc::new(Mutex::new(vec![item(), None]));
    let view = Iterable::from_native_collection_with_copy(Rc::new(RefCell::new(JavaNativeCollection::from_sync_array(shared.clone()))), java_clone_optional_erased_object_ref);
    let mut cursor = view.iterator_result().unwrap();
    assert_eq!(cursor.next_result().unwrap().unwrap().as_ref().java_reference_identity(), identity);
    shared.lock().unwrap()[1] = item();
    assert_eq!(cursor.next_result().unwrap().unwrap().as_ref().java_reference_identity(), identity);
    assert!(Arc::strong_count(&shared) > 1);
    assert!(Rc::ptr_eq(&collection, &iterable.native_collection().unwrap()));
}
"#,
    );
}
