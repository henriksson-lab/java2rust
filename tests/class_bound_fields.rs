#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_field_stores_preserve_assignment_values_and_rhs_before_null_check() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/class_bound_writes.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "class_bound_writes",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(ClassBoundWrites::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn generated_class_bounds_expose_field_capabilities_and_generic_reads() {
    let translation = new_java2rust::translate_source(
        r#"
class BoundCounter {
    int count;
    private int hidden;
    static int global;
    final int fixed = 3;
}
class BoundChildCounter extends BoundCounter {}
class BoundPrivateChild extends BoundCounter { private int count; }
class BoundFieldRead {
    static <T extends BoundCounter> int read(T value) { return value.count; }
    static <T extends BoundPrivateChild> int fixed(T value) { return value.fixed; }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("fn __java_field_count("));
    assert!(!translation.rust.contains("fn __java_field_hidden("));
    assert!(!translation.rust.contains("fn __java_field_global("));
    assert!(!translation.rust.contains("fn __java_set_field_fixed("));
    let private_bound = translation
        .rust
        .split("pub trait BoundPrivateChildBound:")
        .nth(1)
        .expect("private child bound")
        .split("\n}")
        .next()
        .unwrap();
    assert!(!private_bound.contains("fn __java_field_count("));
    common::cargo_test_generated_modules(
        "class_bound_fields",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test]
fn concrete_and_inherited_storage_share_the_bound_abi() {
    let mut root = BoundCounter::new();
    BoundCounterBound::__java_set_field_count(&mut root, 7);
    assert_eq!(BoundCounterBound::__java_field_count(&root), 7);
    assert_eq!(BoundFieldRead::read(&root).value(), 7);
    let mut child = BoundChildCounter::new();
    BoundCounterBound::__java_set_field_count(&mut child, 11);
    assert_eq!(BoundFieldRead::read(&child).value(), 11);
    let mut erased = JavaErasedValue::new(child);
    BoundCounterBound::__java_set_field_count(&mut erased, 13);
    assert_eq!(BoundCounterBound::__java_field_count(&erased), 13);
    let mut private_child = BoundPrivateChild::new();
    BoundCounterBound::__java_set_field_count(&mut private_child, 17);
    assert_eq!(BoundCounterBound::__java_field_count(&private_child), 17);
    assert_eq!(BoundFieldRead::fixed(&private_child).value(), 3);
}
"#,
        }],
    );
}

#[test]
fn bound_field_capabilities_mutate_retained_erased_handles_without_copying_objects() {
    let translation = new_java2rust::translate_source(
        r#"
class SharedFieldCounter {
    int count;
    SharedFieldCounter next;
    Object info;
    void inc() { count++; }
}
class SharedFieldHolder {
    SharedFieldCounter value;
    SharedFieldHolder(SharedFieldCounter value) { this.value = value; }
}
class SharedFieldRead {
    static <T extends SharedFieldCounter> int read(T value) { return value.count; }
    static <T extends SharedFieldCounter> SharedFieldCounter next(T value) { return value.next; }
    static <T extends SharedFieldCounter> void clear(T value) { value.next = null; }
    static <T extends SharedFieldCounter> SharedFieldCounter assign(T value, SharedFieldCounter next) {
        return value.next = next;
    }
    static <T extends SharedFieldCounter> SharedFieldCounter assignNullable(T value, SharedFieldCounter next) {
        return value.next = (next == null ? null : next);
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains(
        "SharedFieldCounterBound for std::rc::Rc<std::cell::RefCell<SharedFieldCounter>>"
    ));
    common::cargo_test_generated_modules(
        "shared_class_bound_fields",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test]
fn aliased_erasure_writes_the_same_object() {
    let mut counter = std::rc::Rc::new(std::cell::RefCell::new(SharedFieldCounter::new()));
    let original = counter.clone();
    SharedFieldCounterBound::__java_set_field_count(&mut counter, 7);
    assert_eq!(SharedFieldRead::read(&counter).value(), 7);
    let mut erased = JavaErasedValue::new(counter);
    let erased_alias = erased.clone();
    SharedFieldCounterBound::__java_set_field_count(&mut erased, 13);
    assert_eq!(SharedFieldCounterBound::__java_field_count(&erased_alias), 13);
    assert_eq!(original.borrow().count, 13);
    let next = std::rc::Rc::new(std::cell::RefCell::new(SharedFieldCounter::new()));
    let mut handle = original.clone();
    SharedFieldCounterBound::__java_set_field_next(&mut handle, Some(next.clone()));
    let child = SharedFieldCounterBound::__java_field_next(&handle).unwrap();
    assert!(std::rc::Rc::ptr_eq(&child, &next));
    SharedFieldCounterBound::__java_set_field_info(&mut handle, Some(Box::new(next.clone())));
    let info = SharedFieldCounterBound::__java_field_info(&handle).unwrap();
    let info = info.downcast::<std::rc::Rc<std::cell::RefCell<SharedFieldCounter>>>().unwrap();
    assert!(std::rc::Rc::ptr_eq(&info, &next));
    SharedFieldRead::clear(&mut handle);
    assert!(SharedFieldRead::next(&handle).optional_value().is_none());
    let returned = SharedFieldRead::assign(&mut handle, next.clone()).reference_value();
    assert!(std::rc::Rc::ptr_eq(&returned, &next));
    let stored = SharedFieldRead::next(&handle).optional_value().unwrap();
    assert!(std::rc::Rc::ptr_eq(&stored, &next));
    assert!(SharedFieldRead::assign_nullable(&mut handle, None).optional_value().is_none());
    assert!(SharedFieldRead::next(&handle).optional_value().is_none());
    let returned = SharedFieldRead::assign_nullable(&mut handle, Some(next.clone())).optional_value().unwrap();
    assert!(std::rc::Rc::ptr_eq(&returned, &next));
}
trait OptionalValue<T> { fn optional_value(self) -> Option<T>; }
impl<T> OptionalValue<T> for Option<T> { fn optional_value(self) -> Option<T> { self } }
impl<T, E: std::fmt::Debug> OptionalValue<T> for Result<Option<T>, E> {
    fn optional_value(self) -> Option<T> { self.unwrap() }
}
trait ReferenceValue<T> { fn reference_value(self) -> std::rc::Rc<std::cell::RefCell<T>>; }
impl<T> ReferenceValue<T> for std::rc::Rc<std::cell::RefCell<T>> {
    fn reference_value(self) -> std::rc::Rc<std::cell::RefCell<T>> { self }
}
impl<T, E: std::fmt::Debug> ReferenceValue<T> for Result<std::rc::Rc<std::cell::RefCell<T>>, E> {
    fn reference_value(self) -> std::rc::Rc<std::cell::RefCell<T>> { self.unwrap() }
}
"#,
        }],
    );
}
