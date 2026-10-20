#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_erasure_preserves_canonical_handles_through_adapters() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/interface_object_erasure.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "interface_object_erasure",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(InterfaceObjectErasure::run().value(), 0); }

// These raw payloads are deliberately not Clone. Neither interface erasure
// nor adapter cloning may clone a shared object's payload or borrow its cell.
struct NoClone;
struct Payload<T>(std::marker::PhantomData<fn() -> T>);
impl<T: 'static> ObjectCarrierTag<T> for Payload<T> {
    fn code(&self) -> i32 { 9 }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_any(&self) -> Box<dyn std::any::Any> { panic!("must clone the handle") }
    fn clone_box(&self) -> Box<dyn ObjectCarrierTag<T>> { panic!("must clone the handle") }
}
#[test] fn direct_and_cell_adapters_clone_only_canonical_handles() {
    let local = std::rc::Rc::new(Payload::<NoClone>(std::marker::PhantomData));
    let carrier: __JavaDirectObjectCarrierTagLocalCarrier<Payload<NoClone>> = local.clone().into();
    let erased = ObjectCarrierTag::<NoClone>::clone_any(&carrier);
    assert!(std::rc::Rc::ptr_eq(&local, erased.downcast_ref().unwrap()));
    assert!(std::rc::Rc::ptr_eq(&local, ObjectCarrierTag::<NoClone>::as_any(&carrier).downcast_ref().unwrap()));
    let sync = std::sync::Arc::new(Payload::<NoClone>(std::marker::PhantomData));
    let carrier: __JavaDirectObjectCarrierTagSyncCarrier<Payload<NoClone>> = sync.clone().into();
    assert!(std::sync::Arc::ptr_eq(&sync, ObjectCarrierTag::<NoClone>::clone_any(&carrier).downcast_ref().unwrap()));
    assert!(std::sync::Arc::ptr_eq(&sync, ObjectCarrierTag::<NoClone>::as_any(&carrier).downcast_ref().unwrap()));
    let cell = std::rc::Rc::new(std::cell::RefCell::new(Payload::<NoClone>(std::marker::PhantomData)));
    let _loan = cell.borrow_mut();
    assert!(std::rc::Rc::ptr_eq(&cell, ObjectCarrierTag::<NoClone>::clone_any(&cell).downcast_ref().unwrap()));
    let mutex = std::sync::Arc::new(std::sync::Mutex::new(Payload::<NoClone>(std::marker::PhantomData)));
    let _loan = mutex.lock().unwrap();
    assert!(std::sync::Arc::ptr_eq(&mutex, ObjectCarrierTag::<NoClone>::clone_any(&mutex).downcast_ref().unwrap()));
}
#[test] fn generic_retargeting_exposes_the_original_object_not_the_view() {
    let local = std::rc::Rc::new(Payload::<i32>(std::marker::PhantomData));
    let carrier: __JavaDirectObjectCarrierTagLocalCarrier<Payload<i32>> = local.clone().into();
    let inner: Box<dyn ObjectCarrierTag<i32>> = Box::new(carrier);
    let adapted = __JavaGenericRetargetObjectCarrierTag::<i32, String>::new(inner);
    assert!(std::rc::Rc::ptr_eq(&local, ObjectCarrierTag::<String>::clone_any(&adapted).downcast_ref().unwrap()));
    assert!(std::rc::Rc::ptr_eq(&local, ObjectCarrierTag::<String>::as_any(&adapted).downcast_ref().unwrap()));
    let boxed: Box<dyn ObjectCarrierTag<String>> = Box::new(adapted);
    assert!(std::rc::Rc::ptr_eq(&local, ObjectCarrierTag::<String>::clone_any(&boxed).downcast_ref().unwrap()));
}
"#,
        }],
    );
}
