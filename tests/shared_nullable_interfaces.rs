#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_interface_loans_borrow_shared_handles_not_their_payloads() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/shared_nullable_interfaces.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "shared_nullable_interfaces",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn forwarding_keeps_aliases_and_releases_receiver_loans() {
    use std::{rc::Rc, cell::RefCell};
    assert!(MutableNetwork::<String, i32>::aliases());
    let original = Rc::new(RefCell::new(MutableNetwork::<String, i32>::new(5)));
    let erased = MutableNetwork::duplicate_with_outer(original.clone()).unwrap().unwrap();
    let duplicate = erased.downcast::<Rc<RefCell<MutableNetwork<String, i32>>>>().unwrap();
    assert!(!Rc::ptr_eq(&original, &duplicate));
    assert_eq!(MutableNetwork::size_with_outer(original.clone()), 5);
    assert_eq!(MutableNetwork::size_with_outer((*duplicate).clone()), 5);
    MutableNetwork::increment_with_outer(original.clone());
    assert_eq!(MutableNetwork::size_with_outer(original.clone()), 6);
    assert_eq!(MutableNetwork::size_with_outer((*duplicate).clone()), 5);
}
"#,
        }],
    );
}
