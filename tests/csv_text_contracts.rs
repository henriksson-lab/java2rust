#[path = "common/mod.rs"]
mod common;

#[test]
fn csv_character_and_array_text_preserve_java_units_and_whitespace() {
    common::verify_java_fixture_zero_contract(
        "csv_text_contracts",
        "CsvTextContracts",
        "tests/corpus/csv/CsvTextContracts.java",
        include_str!("corpus/csv/CsvTextContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn text_contract() { assert_eq!(CsvTextContracts::run().value(), 0); }

#[derive(Clone)]
struct ReentrantElement { value: i32, action: std::rc::Rc<dyn Fn()> }
impl JavaArraysCompareElement for ReentrantElement {
    fn java_arrays_compare(&self, other: &Self) -> i32 {
        (self.action)();
        self.value - other.value
    }
}
impl JavaArraysHashElement for ReentrantElement {
    fn java_arrays_hash_code(&self) -> i32 { (self.action)(); self.value }
}
#[test]
fn retained_array_operations_read_later_elements_after_callbacks() {
    use std::rc::Rc;
    use std::cell::RefCell;
    let left = Rc::new(RefCell::new(Vec::<ReentrantElement>::new()));
    let weak = Rc::downgrade(&left);
    let action: Rc<dyn Fn()> = Rc::new(move || { weak.upgrade().unwrap().borrow_mut()[1].value = 9; });
    let idle: Rc<dyn Fn()> = Rc::new(|| {});
    left.borrow_mut().extend([
        ReentrantElement { value: 1, action },
        ReentrantElement { value: 2, action: idle.clone() },
    ]);
    let right = Rc::new(RefCell::new(vec![
        ReentrantElement { value: 1, action: idle.clone() },
        ReentrantElement { value: 9, action: idle },
    ]));
    assert!(java_arrays_equals(&left, &left));
    assert_eq!(left.borrow()[1].value, 2); // Same reference bypasses element callbacks.
    assert!(java_arrays_equals(&left, &right)); // First comparison mutates the next element.
    left.borrow_mut()[1].value = 2;
    assert_eq!(java_arrays_hash_code(&left), 31 * 32 + 9);
    let sync = std::sync::Arc::new(std::sync::Mutex::new(vec![Some(JavaString::from_units(vec![0xd800])), None]));
    assert!(java_arrays_equals(&sync, &sync));
    assert_eq!(java_arrays_hash_code(Some(&sync)), 31 * (31 + 0xd800));
    assert_eq!(java_arrays_to_text(&sync).units(), &[91, 0xd800, 44, 32, 110, 117, 108, 108, 93]);
    let units = std::sync::Arc::new(std::sync::Mutex::new(vec![JavaCodeUnit(0xd800), JavaCodeUnit(120)]));
    let snapshot = JavaString::fresh(&units);
    units.lock().unwrap()[1] = JavaCodeUnit(121);
    assert_eq!(snapshot.units(), &[0xd800, 120]);
    assert_eq!(JavaString::fresh(&units).units(), &[0xd800, 121]);
}
"#,
    );
}
