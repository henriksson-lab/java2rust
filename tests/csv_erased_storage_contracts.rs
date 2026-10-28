#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_array_producers_retain_the_original_owner_through_fields_and_returns() {
    common::verify_java_sources_zero_contract(
        "ErasedStorageOracle",
        &[
            (
                "tests/corpus/csv/ErasedStorageContracts.java",
                include_str!("corpus/csv/ErasedStorageContracts.java"),
            ),
            (
                "tests/corpus/csv/ErasedStorageOracle.java",
                include_str!("corpus/csv/ErasedStorageOracle.java"),
            ),
        ],
    );
    let translated =
        new_java2rust::translate_source(include_str!("corpus/csv/ErasedStorageContracts.java"));
    assert!(
        !translated.diagnostics.has_errors(),
        "{:?}",
        translated.diagnostics
    );
    assert!(
        !translated.rust.contains("unsupported Java array creation"),
        "fresh array producers must emit their actual allocation"
    );
    common::cargo_test_generated_modules(
        "csv_erased_storage",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
#[test] fn original_array_owner_is_preserved() {
    use std::rc::Rc;
    use std::cell::RefCell;
    let values = Rc::new(RefCell::new(vec![Some(JavaString::from("before")), None]));
    let local = ErasedStorageContracts::erase(Some(&values)).unwrap();
    let direct = ErasedStorageContracts::direct(&values);
    let owner = ErasedStorageContracts::new(values.clone());
    values.borrow_mut()[0] = Some(JavaString::from("after"));
    let field = owner.value().unwrap();
    let typed = owner.typed_value().unwrap();
    let typed_local = owner.typed_local().unwrap();
    let via_call = ErasedStorageContracts::via_call(&values);
    let direct_call = ErasedStorageContracts::direct_call(&values);
    for object in [&local, &direct, &field, &typed, &typed_local, &via_call, &direct_call] {
        let erased = object.downcast_ref::<JavaErasedValue>().unwrap();
        let backing = erased.downcast_clone::<Rc<RefCell<Vec<Option<JavaString>>>>>();
        assert!(Rc::ptr_eq(&values, &backing));
        assert_eq!(backing.borrow()[0].as_ref().unwrap().units(), JavaString::from("after").units());
        assert_eq!(java_erased_reference_identity(object.as_ref()), Some(Rc::as_ptr(&values).cast()));
    }
    assert!(java_erased_any_equals(local.as_ref(), direct.as_ref()));
    assert_eq!(java_erased_any_hash_code_result(local.as_ref()).unwrap(), java_erased_any_hash_code_result(direct.as_ref()).unwrap());
    assert!(ErasedStorageContracts::absent().is_none());
    let ints = Rc::new(RefCell::new(vec![1, 2]));
    let primitive = ErasedStorageContracts::primitive(&ints);
    ints.borrow_mut()[0] = 7;
    let recovered = primitive.downcast_ref::<JavaErasedValue>().unwrap()
        .downcast_clone::<Rc<RefCell<Vec<i32>>>>();
    assert!(Rc::ptr_eq(&ints, &recovered));
    assert_eq!(recovered.borrow()[0], 7);
    assert_eq!(java_erased_any_class(primitive.as_ref()).get_name(), "[I");
    ErasedStorageContracts::initialize_shared();
    let shared = ErasedStorageContracts::synchronized_value().unwrap();
    let shared_again = ErasedStorageContracts::synchronized_value().unwrap();
    assert!(java_erased_any_equals(shared.as_ref(), shared_again.as_ref()));
    assert_eq!(java_erased_any_class(shared.as_ref()).get_name(), "[Ljava.lang.String;");
    let recovered = shared.downcast_ref::<JavaErasedValue>().unwrap()
        .downcast_clone::<std::sync::Arc<std::sync::Mutex<Vec<Option<JavaString>>>>>();
    assert_eq!(recovered.lock().unwrap()[0].as_ref().unwrap().units(), JavaString::from("shared").units());
    let checked = ErasedStorageContracts::checked(false).unwrap();
    assert_eq!(java_erased_any_class(checked.as_ref()).get_name(), "[Ljava.lang.String;");
    for object in [ErasedStorageContracts::erased_empty_strings(), ErasedStorageContracts::erased_null_strings()] {
        assert_eq!(java_erased_any_class(object.as_ref()).get_name(), "[Ljava.lang.String;");
    }
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_rows().as_ref()).get_name(), "[[Ljava.lang.String;");
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_null_rows().as_ref()).get_name(), "[[Ljava.lang.String;");
    let null_rows = ErasedStorageContracts::null_rows();
    assert_eq!(null_rows.borrow().len(), 2);
    assert!(null_rows.borrow().iter().all(Option::is_none));
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_units().as_ref()).get_name(), "[C");
    for object in [ErasedStorageContracts::erased_clone_strings(), ErasedStorageContracts::erased_copy_strings().unwrap(), ErasedStorageContracts::erased_range_strings().unwrap()] {
        assert_eq!(java_erased_any_class(object.as_ref()).get_name(), "[Ljava.lang.String;");
    }
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_generic_clone().as_ref()).get_name(), "[Ljava.lang.String;");
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_generic_copy().unwrap().as_ref()).get_name(), "[Ljava.lang.String;");
    ErasedStorageContracts::copy_shared();
    let shared_copy = ErasedStorageContracts::erased_shared_copy().unwrap();
    assert!(!java_erased_any_equals(shared.as_ref(), shared_copy.as_ref()));
    assert_eq!(java_erased_any_class(shared_copy.as_ref()).get_name(), "[Ljava.lang.String;");
    assert_eq!(java_erased_any_class(ErasedStorageContracts::erased_conditional_clone().as_ref()).get_name(), "[Ljava.lang.String;");
    let original = ErasedStorageContracts::null_strings();
    let cloned = ErasedStorageContracts::clone_strings(&original);
    let generic_cloned = ErasedStorageContracts::clone_generic(&original);
    let conditional = ErasedStorageContracts::clone_or_alias(&original, true);
    let alias = ErasedStorageContracts::clone_or_alias(&original, false);
    assert!(Rc::ptr_eq(&original, &alias));
    assert!(!Rc::ptr_eq(&original, &conditional));
    let copied = ErasedStorageContracts::copy_strings(&original).unwrap();
    let ranged = ErasedStorageContracts::range_strings(&original).unwrap();
    for other in [&cloned, &generic_cloned, &copied, &ranged] {
        assert!(!Rc::ptr_eq(&original, other));
        assert_eq!(java_array_runtime_class(Rc::as_ptr(other) as usize), Some("[Ljava.lang.String;"));
    }
    original.borrow_mut()[0] = Some(JavaString::from("changed"));
    assert_eq!(copied.borrow().len(), 5);
    assert_eq!(ranged.borrow().len(), 3);
    for other in [&cloned, &generic_cloned, &copied, &ranged] {
        assert!(other.borrow().iter().all(Option::is_none));
    }
    // Local copies must not acquire the synchronized producer's Send bound.
    #[derive(Clone)] struct LocalOnly(Rc<()>);
    let local_value = Rc::new(());
    let local_array = Rc::new(RefCell::new(vec![Some(LocalOnly(local_value.clone()))]));
    let local_copy = ErasedStorageContracts::clone_generic(&local_array);
    assert!(Rc::ptr_eq(&local_value, &local_copy.borrow()[0].as_ref().unwrap().0));
    let recovered = checked.downcast_ref::<JavaErasedValue>().unwrap()
        .downcast_clone::<Rc<RefCell<Vec<Option<JavaString>>>>>();
    assert_eq!(recovered.borrow()[0].as_ref().unwrap().units(), JavaString::from("checked").units());
    match ErasedStorageContracts::checked(true) {
        Err(error) => assert!(error.is_instance(&["IllegalStateException"])),
        Ok(_) => panic!("producer exception must precede erasure"),
    }
    let copy = java_clone_erased_any_ref(checked.as_ref());
    drop(recovered);
    drop(checked);
    assert_eq!(java_erased_any_class(copy.as_ref()).get_name(), "[Ljava.lang.String;");
}

fn reference_view(value: &dyn std::any::Any) -> JavaReferenceArrayView {
    value.downcast_ref::<JavaErasedValue>().unwrap().reference_array_projection()
        .try_cast().expect("classified reference array").unwrap()
}

#[test] fn erased_array_operations_retain_owner_and_check_stores_before_mutation() {
    use std::rc::Rc;
    use std::cell::RefCell;
    let array = ErasedStorageContracts::null_strings();
    let object = ErasedStorageContracts::direct(&array);
    let view = reference_view(object.as_ref());
    assert_eq!(view.java_reference_identity(), Rc::as_ptr(&array).cast());
    assert_eq!(view.runtime_class(), "[Ljava.lang.String;");
    assert_eq!(view.length(), 3);
    view.set_result(0, Some(Box::new(JavaString::from("changed")))).unwrap();
    assert_eq!(array.borrow()[0].as_ref().unwrap().units(), JavaString::from("changed").units());
    let error = view.set_result(0, Some(Box::new(3_i32))).unwrap_err();
    assert!(error.is_instance(&["ArrayStoreException"]));
    assert_eq!(array.borrow()[0].as_ref().unwrap().units(), JavaString::from("changed").units());
    let error = view.set_result(-1, Some(Box::new(3_i32))).unwrap_err();
    assert!(error.is_instance(&["ArrayIndexOutOfBoundsException"]));
    assert!(view.get_result(3).unwrap_err().is_instance(&["ArrayIndexOutOfBoundsException"]));
    view.set_result(0, None).unwrap();
    assert!(array.borrow()[0].is_none());
    let objects = ErasedStorageContracts::erased_objects(&array);
    let object_view = reference_view(objects.as_ref());
    assert_eq!(java_erased_reference_identity(object_view.get_result(0).unwrap().unwrap().as_ref()), Some(Rc::as_ptr(&array).cast()));
    assert!(object_view.get_result(2).unwrap().is_none());
    object_view.set_result(1, Some(Box::new(7_i32))).unwrap();
    assert_eq!(*object_view.get_result(1).unwrap().unwrap().downcast::<i32>().unwrap(), 7);
    // Object[] accepts an opaque Java reference without asking for ancestry,
    // and reading it copies the owner rather than its non-Clone payload.
    struct Opaque;
    let opaque = JavaErasedValue::new(Opaque);
    let opaque_identity = opaque.java_reference_identity();
    object_view.set_result(1, Some(Box::new(opaque))).unwrap();
    assert_eq!(java_erased_reference_identity(object_view.get_result(1).unwrap().unwrap().as_ref()), Some(opaque_identity));
    // Copies of the view share its owner even after the original slot changes.
    let weak = Rc::downgrade(&array);
    let copy = view.clone();
    drop(object_view); drop(objects); drop(object); drop(array); drop(view);
    assert!(weak.upgrade().is_some());
    assert!(copy.get_result(0).unwrap().is_none());
    drop(copy);
    assert!(weak.upgrade().is_none());

    ErasedStorageContracts::initialize_shared();
    let shared = ErasedStorageContracts::synchronized_value().unwrap();
    let shared_view = reference_view(shared.as_ref());
    shared_view.set_result(0, Some(Box::new(JavaString::from("shared-change")))).unwrap();
    let owner = shared.downcast_ref::<JavaErasedValue>().unwrap()
        .downcast_clone::<std::sync::Arc<std::sync::Mutex<Vec<Option<JavaString>>>>>();
    assert_eq!(shared_view.java_reference_identity(), std::sync::Arc::as_ptr(&owner).cast());
    assert_eq!(owner.lock().unwrap()[0].as_ref().unwrap().units(), JavaString::from("shared-change").units());
    let primitive = ErasedStorageContracts::primitive(&Rc::new(RefCell::new(vec![1])));
    assert_eq!(primitive.downcast_ref::<JavaErasedValue>().unwrap().reference_array_projection().type_test(), Some(false));
    let unknown = ErasedStorageContracts::direct(&Rc::new(RefCell::new(Vec::<Option<JavaString>>::new())));
    assert_eq!(unknown.downcast_ref::<JavaErasedValue>().unwrap().reference_array_projection().type_test(), None);
}

#[test] fn array_class_metadata_has_weak_owner_lifetimes_and_no_element_bounds() {
    use std::rc::Rc;
    use std::cell::RefCell;
    // Construction and metadata do not require Clone on array elements.
    struct NotClone;
    let owner = Rc::new(RefCell::new(vec![NotClone]));
    let identity = Rc::as_ptr(&owner) as usize;
    let weak = Rc::downgrade(&owner);
    let owner = java_register_local_array_class(owner, "[LNotClone;");
    assert_eq!(Rc::strong_count(&owner), 1);
    assert_eq!(java_array_runtime_class(identity), Some("[LNotClone;"));
    // No array loan is needed to resolve a runtime class.
    let loan = owner.borrow_mut();
    assert_eq!(java_array_runtime_class(identity), Some("[LNotClone;"));
    drop(loan);
    drop(owner);
    assert!(weak.upgrade().is_none());
    assert_eq!(java_array_runtime_class(identity), None);
    // The synchronized registry follows the allocation across threads.
    let owner = std::sync::Arc::new(std::sync::Mutex::new(vec![NotClone]));
    let identity = std::sync::Arc::as_ptr(&owner) as usize;
    let owner = java_register_sync_array_class(owner, "[LNotClone;");
    assert_eq!(std::sync::Arc::strong_count(&owner), 1);
    std::thread::spawn(move || {
        let _loan = owner.lock().unwrap();
        assert_eq!(java_array_runtime_class(identity), Some("[LNotClone;"));
    }).join().unwrap();
    assert_eq!(java_array_runtime_class(identity), None);
    // Unknown evidence must not masquerade as a proved component type.
    let external = Rc::new(RefCell::new(vec![None::<JavaString>]));
    assert_eq!(java_array_runtime_class(Rc::as_ptr(&external) as usize), None);
}

#[test] fn array_view_codecs_run_after_payload_loans_end() {
    use std::rc::Rc;
    let owner = ErasedStorageContracts::null_strings();
    let encode_owner = owner.clone();
    let decode_owner = owner.clone();
    let view = JavaReferenceArrayView::from_local_array(owner.clone(), Clone::clone,
        move |value| {
            encode_owner.borrow_mut()[1] = Some(JavaString::from("encoded"));
            java_reference_array_encode_typed(value)
        },
        move |value| {
            decode_owner.borrow_mut()[2] = Some(JavaString::from("decoded"));
            java_reference_array_decode_typed(value)
        }).unwrap().unwrap();
    assert!(view.get_result(0).unwrap().is_none());
    assert!(owner.borrow()[1].is_some());
    assert!(view.set_result(0, Some(Box::new(3_i32))).is_err());
    assert!(owner.borrow()[2].is_none());
    view.set_result(0, Some(Box::new(JavaString::from("value")))).unwrap();
    assert!(owner.borrow()[2].is_some());
    assert_eq!(view.java_reference_identity(), Rc::as_ptr(&owner).cast());

    ErasedStorageContracts::initialize_shared();
    let erased = ErasedStorageContracts::synchronized_value().unwrap();
    let owner = erased.downcast_ref::<JavaErasedValue>().unwrap()
        .downcast_clone::<std::sync::Arc<std::sync::Mutex<Vec<Option<JavaString>>>>>();
    let encode_owner = owner.clone();
    let decode_owner = owner.clone();
    let view = JavaReferenceArrayView::from_shared_array(owner.clone(), Clone::clone,
        move |value| {
            assert!(encode_owner.try_lock().is_ok());
            java_reference_array_encode_typed(value)
        },
        move |value| {
            assert!(decode_owner.try_lock().is_ok());
            java_reference_array_decode_typed(value)
        }).unwrap().unwrap();
    assert!(view.get_result(0).unwrap().is_some());
    view.set_result(0, None).unwrap();
    assert!(owner.lock().unwrap()[0].is_none());
    // Incomplete ancestry stays unclassified except for a proved final target.
    assert!(!java_runtime_class_is_assignable("java.lang.String", "unregistered.Actual").unwrap());
    assert!(java_runtime_class_is_assignable("unregistered.Target", "unregistered.Actual").is_err());
}
"#,
        }],
    );
}
