#[path = "common/mod.rs"]
mod common;

fn check(source: &str, name: &str, tests: &'static str) {
    let translation = new_java2rust::translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        name,
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests,
        }],
    );
}

#[test]
fn native_container_comparisons_keep_allocation_and_value_equality_distinct() {
    check(
        include_str!("fixtures/native_collection_identity.java"),
        "native_collection_identity",
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(NativeCollectionIdentity::run().value(), 0); }
#[test] fn erased_collection_elements_reuse_object_equality() {
    fn erased<T: std::any::Any>(value: T) -> Box<dyn std::any::Any> { Box::new(value) }
    assert!(erased(7_i32).java_map_key_eq(&erased(7_i32)));
    assert!(!erased(7_i32).java_map_key_eq(&erased(7_i64)));
    assert!(erased(String::from("value")).java_map_key_eq(&erased(String::from("value"))));
    assert!(erased(f64::NAN).java_map_key_eq(&erased(f64::from_bits(0x7ff8_0000_0000_0001))));
    assert!(!erased(0_f64).java_map_key_eq(&erased(-0_f64)));
    assert!(erased(erased(7_i32)).java_map_key_eq(&erased(7_i32)));
    assert!(erased(7_i32).java_map_key_eq(&erased(erased(7_i32))));
    struct Unsupported;
    assert!(std::panic::catch_unwind(|| erased(Unsupported).java_map_key_eq(&erased(Unsupported))).is_err());
}
#[test] fn reference_comparison_does_not_require_payload_traits() {
    struct Opaque;
    let first = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::set(Vec::<Opaque>::new(), |_, _| false)));
    let second = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::set(Vec::<Opaque>::new(), |_, _| false)));
    assert!(NativeCollectionIdentity::same_set(Some(&first), Some(&first)));
    assert!(!NativeCollectionIdentity::same_set(Some(&first), Some(&second)));
    let map = std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::<Opaque, Opaque>::new()));
    let other_map = std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::<Opaque, Opaque>::new()));
    assert!(NativeCollectionIdentity::same_map(&map, &map));
    assert!(!NativeCollectionIdentity::same_map(&map, &other_map));
}
"#,
    );
}

#[test]
fn inherited_dispatch_and_interface_forwarding_share_capability_contracts() {
    check(
        include_str!("fixtures/inherited_dispatch_capabilities.java"),
        "inherited_dispatch_capabilities",
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(InheritedDispatchCapabilities::run().value(), 0); }
#[test] fn unrelated_interface_does_not_inherit_identity_or_clone_obligations() {
    struct NoIdentity;
    fn require<T: 'static, R: DispatchSize<T>>() {}
    require::<NoIdentity, std::rc::Rc<std::cell::RefCell<DispatchCapabilityImpl<NoIdentity>>>>();
}
"#,
    );
}

#[test]
fn generic_identity_preserves_handles_erasure_strings_and_retained_fields() {
    check(
        include_str!("fixtures/generic_reference_identity.java"),
        "generic_reference_identity",
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(GenericReferenceIdentity::run().value(), 0); }
#[test] fn shared_sync_kernel_compares_allocations_not_handle_slots() {
    let first = std::sync::Arc::new(std::sync::Mutex::new(7));
    let alias = first.clone();
    let second = std::sync::Arc::new(std::sync::Mutex::new(7));
    assert!(java_reference_equals(Some(&first), Some(&alias)));
    assert!(!java_reference_equals(Some(&first), Some(&second)));
    assert!(!java_reference_equals(None::<&std::sync::Arc<std::sync::Mutex<i32>>>, Some(&first)));
    *alias.lock().unwrap() = 9;
    assert!(java_reference_equals(Some(&first), Some(&alias)));
    let boxed = Box::new(first.clone());
    assert!(java_reference_equals(Some(&first), Some(&boxed)));
}
"#,
    );
}

#[test]
fn generic_identity_composes_with_bound_fields_and_inherited_returns() {
    check(
        include_str!("fixtures/generic_reference_boundaries.java"),
        "generic_reference_boundaries",
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(GenericReferenceBoundaries::run().value(), 0); }
"#,
    );
}

#[test]
fn nested_capabilities_and_bulk_results_preserve_reference_contracts() {
    check(
        include_str!("fixtures/scoped_reference_contracts.java"),
        "scoped_reference_contracts",
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(ScopedReferenceContracts::run().value(), 0); }
#[test] fn retained_unmodifiable_backends_keep_live_data_and_policies() {
    fn rejected<T>(result: Result<T, JavaException>) {
        match result {
            Err(error) => assert_eq!(error.class_name(), "UnsupportedOperationException"),
            Ok(_) => panic!("unmodifiable mutation succeeded"),
        }
    }
    fn check<B: JavaNativeCollectionBacking<i32>>(view: &mut JavaNativeCollectionStorage<i32, B>) {
        rejected(view.list_set_result(-1, 8));
        rejected(view.list_add_result(8));
        rejected(view.add_all_result(vec![]));
        rejected(view.list_remove_object_result(&99, |a, b| a == b));
        rejected(view.remove_result(&99, |a, b| a == b));
        rejected(view.clear_result());
    }
    let source = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::list(vec![1], true)));
    let mut view = JavaNativeCollection::unmodifiable_from_local_collection(source.clone());
    check(&mut view);
    source.borrow_mut().list_set_result(0, 3).unwrap();
    assert_eq!(view.cloned_items(), vec![3]);
    let wrapped = std::rc::Rc::new(std::cell::RefCell::new(view));
    assert!(!std::rc::Rc::ptr_eq(&source, &wrapped));
    let mut nested = JavaNativeCollection::unmodifiable_from_local_collection(wrapped.clone());
    check(&mut nested);
    source.borrow_mut().list_add_result(4).unwrap();
    assert_eq!(nested.cloned_items(), vec![3, 4]);
    assert_eq!(nested.modification_count(), source.borrow().modification_count());
    let sync_source = std::sync::Arc::new(std::sync::Mutex::new(JavaSyncNativeCollection::list(vec![1], true)));
    let mut sync_view = JavaSyncNativeCollection::unmodifiable_from_sync_collection(sync_source.clone());
    let local_view = JavaNativeCollection::unmodifiable_from_sync_collection(sync_source.clone());
    check(&mut sync_view);
    sync_source.lock().unwrap().list_add_result(2).unwrap();
    assert_eq!(sync_view.cloned_items(), vec![1, 2]);
    assert_eq!(local_view.cloned_items(), vec![1, 2]);
    assert_eq!(sync_view.modification_count(), sync_source.lock().unwrap().modification_count());
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<JavaSyncNativeCollection<i32>>();
    let set = std::rc::Rc::new(std::cell::RefCell::new(JavaNativeCollection::set([1, 2], |a, b| a == b)));
    let mut set_view = JavaNativeCollection::unmodifiable_from_local_collection(set.clone());
    assert!(!set_view.is_list());
    assert!(set_view.contains_by(&2, |a, b| a == b));
    rejected(set_view.insert_result(3));
    rejected(set_view.remove_result(&99, |a, b| a == b));
    set.borrow_mut().insert_result(3).unwrap();
    assert_eq!(set_view.len(), 3);
    assert!(set_view.java_map_key_eq(&set.borrow()));
    assert!(set.borrow().java_map_key_eq(&set_view));
}
"#,
    );
}
