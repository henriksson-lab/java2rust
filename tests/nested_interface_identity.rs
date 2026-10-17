use new_java2rust::translate_source;
use new_java2rust::{
    facts::{FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy},
    translate_source_with_facts,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules};

#[test]
fn nested_generic_interface_and_static_member_class_keep_one_abi() {
    check_nested_generic_interface_identity();
}

#[test]
fn source_interface_definition_replaces_external_erased_facade_at_all_slots() {
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Port".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    let translation = translate_source_with_facts(
        "interface Port { int read(); } class FixedPort implements Port { public int read() { return 7; } }",
        &facts,
    );
    assert!(
        translation.rust.contains("pub trait Port"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("pub type Port = Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "source_interface_precedence",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

fn check_nested_generic_interface_identity() {
    let translation = translate_source(
        r#"
interface Container {
    interface Source<T> { T value(); }
    class Value<T> implements Source<T> {
        private final T item;
        Value(T item) { this.item = item; }
        public T value() { return item; }
    }
}

"#,
    );
    assert!(
        translation
            .rust
            .contains("pub type Source<T> = dyn Container_Source<T>;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("Container_Source<T> for Container_Value<T>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("__outer:"),
        "{}",
        translation.rust
    );
    cargo_check_generated_modules(
        "nested_generic_interface_identity",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}
