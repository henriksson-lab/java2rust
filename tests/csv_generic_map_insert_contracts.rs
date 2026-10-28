#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_map_insertion_preserves_caller_identity_and_present_null_values() {
    common::verify_java_fixture_zero_contract(
        "csv_generic_map_insert",
        "GenericMapInsertContracts",
        "tests/corpus/csv/GenericMapInsertContracts.java",
        include_str!("corpus/csv/GenericMapInsertContracts.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn insert() { assert_eq!(GenericMapInsertContracts::run().value(), 0); }
#[test] fn retained_insert_capability_does_not_require_value_clone() {
    use std::{rc::Rc, cell::{Cell, RefCell}};
    struct ValueWithoutClone(Rc<Cell<i32>>);
    let state = Rc::new(Cell::new(7));
    let mut map = Rc::new(RefCell::new(JavaSemanticHashMap::new()));
    let alias = map.clone();
    JavaMapInsert::insert(&mut map, JavaString::from("value"), Some(ValueWithoutClone(state.clone())));
    JavaMapInsert::insert(&mut map, JavaString::from("null"), None);
    assert!(Rc::ptr_eq(&map, &alias));
    assert!(Rc::ptr_eq(&map.borrow().get(&JavaString::from("value")).unwrap().as_ref().unwrap().0, &state));
    assert!(alias.borrow().contains_key(&JavaString::from("null")));
    assert!(alias.borrow().get(&JavaString::from("null")).unwrap().is_none());
}
"#,
    );
}
