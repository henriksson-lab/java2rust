#[path = "common/mod.rs"]
mod common;

#[test]
fn default_interface_handle_storage_does_not_require_payload_clone() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/default_interface_handle_storage.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let trait_body = translation
        .rust
        .split("pub trait ViewWrapper<T>")
        .nth(1)
        .unwrap()
        .split("fn as_any")
        .next()
        .unwrap();
    assert!(!trait_body.contains("T: Clone"), "{trait_body}");
    common::cargo_test_generated_modules(
        "default_interface_handle_storage",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn default_dispatch_copies_only_the_interface_handle() {
    struct Opaque;
    let value = EmptyStoredView::<Opaque>::new();
    let wrapper = DefaultViewWrapper::<Opaque>::new();
    let _wrapped = wrapper.wrap(&value);
    let dynamic: Box<dyn ViewWrapper<Opaque>> = Box::new(wrapper);
    let _dynamic_wrapped = dynamic.wrap(&value);
}
"#,
        }],
    );
}

#[test]
fn interface_conversions_include_capabilities_from_methods_not_only_fields() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/interface_method_capability.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "interface_method_capability",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn selected_implementation_equality_matches_java() {
    assert!(InterfaceMethodCapability::create::<String>().test("value".to_string()));
}
"#,
        }],
    );
}

#[test]
fn interface_conversions_reuse_concrete_implementation_capabilities() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/generic_interface_construction.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    // A concrete construction and an interface-handle transfer do not copy T.
    // Requiring Clone there would hide an overly broad conversion rule.
    for name in ["concrete", "handle", "empty"] {
        let header = translation
            .rust
            .lines()
            .find(|line| line.contains(&format!("fn {name}<")))
            .unwrap();
        assert!(!header.contains("Clone"), "{header}");
        assert!(!header.contains("JavaMapKey"), "{header}");
    }
    common::cargo_test_generated_modules(
        "generic_interface_construction",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn factories_match_the_java_oracle() {
    struct Opaque;
    let _concrete = GenericInterfaceConstruction::concrete::<Opaque>();
    let _empty = GenericInterfaceConstruction::empty::<Opaque>();
    assert!(GenericInterfaceConstruction::mixed::<String, Opaque>().values().is_empty());
    assert!(GenericInterfaceConstruction::snapshot::<String>().values().is_empty());
    assert!(!GenericInterfaceConstruction::membership::<String>().contains("absent".to_string()));
    assert!(GenericInterfaceConstruction::local::<String>().values().is_empty());
    let initial = GenericInterfaceConstruction::snapshot::<String>();
    assert!(GenericInterfaceConstruction::assigned(initial).values().is_empty());
    let _envelope = GenericInterfaceConstruction::enveloped::<String>();
    assert!(OwnerInterfaceConstruction::<String>::new().snapshot().values().is_empty());
    assert!(GenericInterfaceConstruction::argument::<String>().values().is_empty());
    assert!(GenericInterfaceConstruction::forwarded::<String>().values().is_empty());
    assert!(GenericInterfaceConstruction::cast::<String>().values().is_empty());
    for first in [false, true] {
        assert!(GenericInterfaceConstruction::conditional::<String>(first).values().is_empty());
        assert!(GenericInterfaceConstruction::nested::<String>(first).values().is_empty());
    }
}
"#,
        }],
    );
}
