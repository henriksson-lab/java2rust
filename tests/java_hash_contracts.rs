#[path = "common/mod.rs"]
mod common;

#[test]
fn sync_source_key_dispatch_does_not_hold_a_mutex_across_callbacks() {
    let source = format!(
        "{}\n{}",
        include_str!("fixtures/concrete_checked_keys.java"),
        include_str!("fixtures/concrete_checked_keys_sync_root.java"),
    );
    let translation = new_java2rust::translate_source(&source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("JavaMapKey for std::sync::Arc<std::sync::Mutex<ConcreteCheckedKey>>"),
        "{}",
        translation.rust
    );
    common::cargo_test_generated_modules(
        "sync_checked_keys",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn reentrant_key_dispatch_finishes_without_a_retained_mutex_guard() {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let passed = ConcreteCheckedKeys::hash_case().unwrap() == 0 && ConcreteCheckedKeys::linked_case().unwrap() == 0;
        send.send(passed).unwrap();
    });
    assert!(receive.recv_timeout(std::time::Duration::from_secs(5)).expect("key equality retained a mutex guard across Java observe(this)"));
}
"#,
        }],
    );
}

#[test]
fn concrete_key_overrides_and_set_consumers_preserve_checked_dispatch() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/concrete_checked_keys.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "concrete_checked_keys",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn java_oracle() {
    assert_eq!(ConcreteCheckedKeys::hash_case().unwrap(), 0);
    assert_eq!(ConcreteCheckedKeys::linked_case().unwrap(), 0);
}
#[derive(Clone)] struct Key(std::rc::Rc<()>, std::rc::Rc<std::cell::Cell<usize>>);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, _: &Self) -> bool { panic!("must use checked equality") }
    fn java_map_key_identity_eq(&self, other: &Self) -> bool { std::rc::Rc::ptr_eq(&self.0, &other.0) }
    fn java_map_key_eq_result(&self, _: &Self) -> Result<bool, JavaException> {
        self.1.set(self.1.get() + 1);
        Err(JavaException::new("IllegalStateException", "equals"))
    }
}
#[test] fn identity_shortcut_forwards_through_nullable_and_borrowed_keys() {
    let key = Key(std::rc::Rc::new(()), std::rc::Rc::new(std::cell::Cell::new(0)));
    let alias = key.clone();
    let different = Key(std::rc::Rc::new(()), std::rc::Rc::new(std::cell::Cell::new(0)));
    let mut semantic = JavaSemanticHashSet::new();
    semantic.insert_result(Some(key.clone())).unwrap();
    assert!(semantic.contains_result(&Some(alias.clone())).unwrap());
    assert!(semantic.contains_result(&Some(different.clone())).is_err());
    assert!(semantic.remove_result(&Some(alias.clone())).unwrap());
    assert_eq!(key.1.get(), 0);
    assert_eq!(different.1.get(), 1);
    let mut linked = JavaLinkedHashSet::new();
    linked.insert_result(&key).unwrap();
    assert!(linked.contains_result(&&alias).unwrap());
    assert!(linked.contains_result(&&different).is_err());
    assert!(linked.remove_result(&&alias).unwrap());
    assert_eq!(key.1.get(), 0);
    assert_eq!(different.1.get(), 2);
    assert!(key.java_map_key_eq_result(&alias).is_err(), "explicit equals has no identity shortcut");
}
#[test] fn generic_protocol_erases_a_handle_without_cloning_its_type_argument() {
    struct NoClone;
    let ordinary = ConcreteCheckedKeys::generic::<NoClone>(false).unwrap();
    let failing = ConcreteCheckedKeys::generic::<NoClone>(true).unwrap();
    assert!(ordinary.java_map_key_eq_result(&failing).unwrap());
    assert_eq!(failing.java_map_key_eq_result(&ordinary).unwrap_err().class_name(), "IllegalStateException");
}
"#,
        }],
    );
}

#[test]
fn hierarchy_key_equality_preserves_checked_source_dispatch() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/checked_key_equality.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "checked_key_equality",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn checked_key_protocol_uses_the_source_override() {
    assert_eq!(CheckedKeyEquality::run().unwrap(), 0);
    assert_eq!(CheckedKeyEquality::bulk().unwrap(), 0);
    let left = CheckedKeyEquality::make(7, false);
    let right = CheckedKeyEquality::make(7, false);
    assert!(left.java_map_key_eq_result(&right).unwrap());
    assert!(!left.java_map_key_eq_result(&CheckedKeyEquality::make(8, false)).unwrap());
    assert_eq!(CheckedKeyEquality::make(7, true).java_map_key_eq_result(&right).unwrap_err().class_name(), "IllegalStateException");
}
"#,
        }],
    );
}

#[test]
fn inherited_hashing_keeps_root_binders_and_borrows_nullable_fields() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/inherited_hash_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "inherited_hash_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(InheritedHashContracts::run().value(), 0); }
#[test] fn guarded_field_hash_does_not_require_payload_clone() {
    struct NoClone(i32);
    impl JavaHashCode for NoClone {
        fn java_hash_code_result(&self) -> Result<i32, JavaException> { Ok(self.0) }
    }
    let parent = HashParent { value: Some(NoClone(7)), __marker: std::marker::PhantomData };
    assert_eq!(parent.hash_code().value(), 7);
}
"#,
        }],
    );
}

#[test]
fn primitive_wrapper_factories_share_typed_scalar_boundaries() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/primitive_value_factories.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "primitive_value_factories",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(PrimitiveValueFactories::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn java_hashing_dispatches_overrides_and_preserves_checked_behavior() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/java_hash_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "java_hash_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(JavaHashContracts::run().value(), 0); }
"#,
        }],
    );
}
