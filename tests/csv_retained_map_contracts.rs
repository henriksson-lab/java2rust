#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_map_bulk_copy_and_callbacks_preserve_live_aliases() {
    common::verify_java_fixture_zero_contract(
        "csv_retained_maps",
        "RetainedMapContracts",
        "tests/corpus/csv/RetainedMapContracts.java",
        include_str!("corpus/csv/RetainedMapContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn maps() { assert_eq!(RetainedMapContracts::run().value(), 0); }
#[test] fn entry_copy_preserves_non_clone_erased_references_and_nulls() {
    use std::{rc::Rc, cell::RefCell, any::Any};
    struct Opaque;
    fn copy(value: &Option<Box<dyn Any>>) -> Option<Box<dyn Any>> {
        value.as_ref().map(|value| java_clone_erased_any_ref(value.as_ref()))
    }
    let object = Rc::new(Opaque);
    let source = Rc::new(RefCell::new(JavaSemanticHashMap::new()));
    source.borrow_mut().insert(JavaString::from("object"), Some(Box::new(JavaErasedValue::new(object.clone())) as Box<dyn Any>));
    source.borrow_mut().insert(JavaString::from("null"), None);
    let destination = Rc::new(RefCell::new(JavaSemanticHashMap::new()));
    java_native_pure_key_map_put_all_result(&destination, &source, Clone::clone, copy).unwrap();
    assert_eq!(destination.borrow().len(), 2);
    assert!(destination.borrow().get(&JavaString::from("null")).unwrap().is_none());
    let copied = destination.borrow().get(&JavaString::from("object")).unwrap().as_ref().unwrap()
        .downcast_ref::<JavaErasedValue>().unwrap().downcast_clone::<Rc<Opaque>>();
    assert!(Rc::ptr_eq(&copied, &object));
    java_native_pure_key_map_put_all_result(&source, &source, Clone::clone, copy).unwrap();
    assert_eq!(source.borrow().len(), 2);
}
"#,
    );
}
