#[path = "common/mod.rs"]
mod common;

#[test]
fn erased_value_conversions_preserve_carriers_and_checked_failures_without_boxes() {
    let translation = new_java2rust::translate_source(
        "class ErasedValueProbe { static Object keep(Object value) { return value; } }",
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "checked_erased_generic_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct AllocationCounter;
std::thread_local! {
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
unsafe impl std::alloc::GlobalAlloc for AllocationCounter {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let value = unsafe { std::alloc::GlobalAlloc::alloc(&std::alloc::System, layout) };
        if !value.is_null() && COUNTING.try_with(|flag| flag.get()).unwrap_or(false) {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
        value
    }
    unsafe fn dealloc(&self, value: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::GlobalAlloc::dealloc(&std::alloc::System, value, layout); }
    }
}
#[global_allocator] static ALLOCATOR: AllocationCounter = AllocationCounter;
struct Payload(std::cell::Cell<i32>);
#[test]
fn retained_value_reads_and_same_type_casts_allocate_nothing() {
    let original = std::rc::Rc::new(Payload(std::cell::Cell::new(7)));
    let erased = JavaErasedValue::new(original.clone());
    ALLOCATIONS.with(|value| value.set(0));
    COUNTING.with(|value| value.set(true));
    for _ in 0..1000 {
        let retained = java_retarget_erased_generic_value_result::<_, JavaErasedValue>(erased.clone()).unwrap();
        let recovered = java_retarget_erased_generic_value_result::<_, std::rc::Rc<Payload>>(retained).unwrap();
        let same = java_retarget_erased_generic_value_result::<_, std::rc::Rc<Payload>>(recovered).unwrap();
        assert!(std::rc::Rc::ptr_eq(&original, &same));
    }
    COUNTING.with(|value| value.set(false));
    assert_eq!(ALLOCATIONS.with(|value| value.get()), 0);
    let copied: std::rc::Rc<Payload> = erased.downcast_clone();
    copied.0.set(13);
    assert_eq!(original.0.get(), 13);
}
#[test]
fn checked_cast_failure_is_a_java_exception_and_keeps_the_source() {
    let original = std::rc::Rc::new(Payload(std::cell::Cell::new(5)));
    let erased = JavaErasedValue::new(original.clone());
    let result = java_retarget_erased_generic_value_result::<_, i32>(erased.clone());
    assert_eq!(result.unwrap_err().class_name(), "ClassCastException");
    let recovered: std::rc::Rc<Payload> = erased.try_downcast_clone().unwrap();
    assert!(std::rc::Rc::ptr_eq(&original, &recovered));
    let null = JavaErasedValue::new(None::<std::rc::Rc<Payload>>);
    let recovered: Option<std::rc::Rc<Payload>> = java_retarget_erased_generic_value_result(null).unwrap();
    assert!(recovered.is_none());
    let opaque = JavaErasedValue::new(Payload(std::cell::Cell::new(17)));
    assert_eq!(opaque.clone().as_any().downcast_ref::<Payload>().unwrap().0.get(), 17);
}
"#,
        }],
    );
}
