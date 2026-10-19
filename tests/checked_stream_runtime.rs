#[path = "common/mod.rs"]
mod common;

#[test]
fn checked_stream_runtime_is_late_bound_single_use_and_callback_safe() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
import java.util.stream.*;
class StreamRuntimeDependencies {
    Stream<Integer> stream;
    Iterable<Integer> iterable;
    Iterator<Integer> iterator;
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "checked_stream_runtime",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn native_stream(source: &Rc<RefCell<JavaNativeCollection<i32>>>) -> JavaStream<i32> {
    let source = source.clone();
    JavaStream::from_factory_result(move || {
        java_native_collection_iterator_result(&source).map(|cursor| cursor.into_results())
    })
}

#[test]
fn checked_iterator_adapter_is_fused_and_preserves_errors() {
    assert_eq!(JavaIterator::new(vec![1, 2]).into_results().collect::<Result<Vec<_>, _>>().unwrap(), vec![1, 2]);
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let cursor = JavaIterator::<i32>::new_live(move || {
        counted.set(counted.get() + 1);
        Err(JavaException::new("ConcurrentModificationException", String::new()))
    }, || panic!("next must not run after failed hasNext"), || Ok(()));
    let mut checked = cursor.into_results();
    assert_eq!(checked.next().unwrap().unwrap_err().class_name(), "ConcurrentModificationException");
    assert!(checked.next().is_none());
    assert!(checked.next().is_none());
    assert_eq!(calls.get(), 1);
    let cursor = JavaIterator::<i32>::new_live(|| Ok(true), || Err(JavaException::new("IllegalStateException", String::new())), || Ok(()));
    let mut checked = cursor.into_results();
    assert_eq!(checked.next().unwrap().unwrap_err().class_name(), "IllegalStateException");
    assert!(checked.next().is_none());
}

#[test]
fn late_binding_and_aliases_match_java_stream_contracts() {
    let values = Rc::new(RefCell::new(JavaNativeCollection::list(vec![1, 2], true)));
    let stream = native_stream(&values);
    let alias = stream.clone();
    values.borrow_mut().list_add_result(3).unwrap();
    assert_eq!(stream.collect_result(()).unwrap(), vec![1, 2, 3]);
    assert_eq!(alias.count_result().unwrap_err().class_name(), "IllegalStateException");
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let original = native_stream(&values);
    let derived = original.map_result(move |value| { counted.set(counted.get() + 1); Ok(value + 1) }).unwrap();
    let filtered = derived.filter_result(|value| Ok(*value > 2)).unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(original.count_result().unwrap_err().class_name(), "IllegalStateException");
    assert_eq!(derived.count_result().unwrap_err().class_name(), "IllegalStateException");
    assert_eq!(filtered.collect_result(()).unwrap(), vec![3, 4]);
    assert_eq!(calls.get(), 3);
    // Taking ownership of a traversal must not bind the cursor before the
    // terminal's callback arguments have been evaluated.
    let mut traversal = native_stream(&values).into_iter_once_result().unwrap();
    values.borrow_mut().list_add_result(4).unwrap();
    assert_eq!(traversal.by_ref().collect::<Result<Vec<_>, _>>().unwrap(), vec![1, 2, 3, 4]);
}

#[test]
fn errors_and_reentrant_callbacks_do_not_hold_state_or_source_borrows() {
    let values = Rc::new(RefCell::new(JavaNativeCollection::list(vec![1, 2], true)));
    let stream = native_stream(&values);
    let source = values.clone();
    let error = stream.for_each_ordered_result(move |_| { source.borrow_mut().list_add_result(9)?; Ok(()) }).unwrap_err();
    assert_eq!(error.class_name(), "ConcurrentModificationException");
    assert_eq!(stream.count_result().unwrap_err().class_name(), "IllegalStateException");
    let stream = JavaStream::from_factory_result(|| -> Result<std::vec::IntoIter<Result<i32, JavaException>>, JavaException> {
        Err(JavaException::new("IllegalArgumentException", String::new()))
    });
    assert_eq!(stream.count_result().unwrap_err().class_name(), "IllegalArgumentException");
    assert_eq!(stream.count_result().unwrap_err().class_name(), "IllegalStateException");
    let slot = Rc::new(RefCell::new(None::<JavaStream<i32>>));
    let reentrant = slot.clone();
    let stream = JavaStream::from_factory_result(move || {
        assert_eq!(reentrant.borrow().as_ref().unwrap().count_result().unwrap_err().class_name(), "IllegalStateException");
        Ok(vec![Ok(7)].into_iter())
    });
    *slot.borrow_mut() = Some(stream.clone());
    assert_eq!(stream.count_result().unwrap(), 1);
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let stream = JavaStream::from_iter(vec![1, 2, 3]).filter_result(move |_| {
        counted.set(counted.get() + 1);
        Err(JavaException::new("IllegalArgumentException", String::new()))
    }).unwrap();
    let mut traversal = stream.into_iter_once_result().unwrap();
    assert_eq!(traversal.next().unwrap().unwrap_err().class_name(), "IllegalArgumentException");
    assert!(traversal.next().is_none());
    assert_eq!(calls.get(), 1);
}

#[test]
fn traversal_short_circuits_and_never_requires_payload_clone() {
    struct Payload(i32);
    let stream = JavaStream::from_iter(vec![Payload(1), Payload(2)]);
    let alias = stream.clone();
    let mapped = stream.map_result(|value| Ok(Payload(value.0 + 1))).unwrap();
    let values = mapped.collect_result(()).unwrap();
    assert_eq!(values.into_iter().map(|value| value.0).collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(alias.count_result().unwrap_err().class_name(), "IllegalStateException");
    let text = String::from("borrowed");
    let borrowed = JavaStream::from_iter(vec![text.as_str()]);
    assert_eq!(borrowed.collect_result(()).unwrap(), vec!["borrowed"]);
    let calls = Cell::new(0);
    assert!(JavaStream::from_iter(vec![1, 2, 3]).any_match_result(|value| { calls.set(calls.get() + 1); Ok(value == 2) }).unwrap());
    assert_eq!(calls.get(), 2);
    let calls = Cell::new(0);
    assert!(!JavaStream::from_iter(vec![1, 2, 3]).all_match_result(|value| { calls.set(calls.get() + 1); Ok(value < 2) }).unwrap());
    assert_eq!(calls.get(), 2);
}
"#,
        }],
    );
}
