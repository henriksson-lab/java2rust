#[path = "common/mod.rs"]
mod common;

use new_java2rust::{facts::FactSet, java_ir::JavaType};

fn named(name: &str) -> JavaType {
    JavaType::Named {
        name: name.into(),
        args: Vec::new(),
    }
}

fn assert_reaches(facts: &FactSet, actual: &str, target: &str, expected: bool) {
    let index = facts.hierarchy_lookup_index();
    for _ in 0..2 {
        assert_eq!(
            facts
                .hierarchy_subtype_policy(&named(actual), &named(target))
                .reaches(),
            expected,
            "direct {actual} -> {target}"
        );
        assert_eq!(
            index
                .hierarchy_subtype_policy(&named(actual), &named(target))
                .reaches(),
            expected,
            "indexed {actual} -> {target}"
        );
    }
}

#[test]
fn platform_collection_hierarchy_preserves_namespace_and_source_precedence() {
    let facts = FactSet::default();
    for actual in [
        "ArrayList",
        "LinkedHashSet",
        "AbstractList",
        "AbstractSequentialList",
        "java.util.AbstractList",
    ] {
        assert_reaches(&facts, actual, "Iterable", true);
        assert_reaches(&facts, actual, "java.lang.Iterable", true);
    }
    for actual in [
        "Iterator",
        "Spliterator",
        "Enumeration",
        "other.AbstractList",
        "other.LinkedHashSet",
    ] {
        assert_reaches(&facts, actual, "Iterable", false);
    }
    assert_reaches(&facts, "AbstractList", "other.Iterable", false);
    let parsed = new_java2rust::parser::parse_java_source(
        "abstract class Root<T> extends java.util.AbstractSequentialList<T> {} abstract class Child extends Root<Integer> {}",
    );
    let resolved = new_java2rust::resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert_reaches(&resolved.facts, "Child", "Iterable", true);
    assert_reaches(&resolved.facts, "Child", "List", true);
    let parsed = new_java2rust::parser::parse_java_source(
        "class AbstractList {} class Child extends AbstractList {} interface Iterable {}",
    );
    let resolved = new_java2rust::resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert_reaches(&resolved.facts, "Child", "Iterable", false);
    assert_reaches(&resolved.facts, "AbstractList", "java.lang.Iterable", false);
    assert_reaches(&resolved.facts, "java.util.AbstractList", "Iterable", false);
    assert_reaches(
        &resolved.facts,
        "java.util.AbstractList",
        "java.lang.Iterable",
        true,
    );
    let mut overridden = FactSet::default();
    overridden
        .hierarchy_overrides
        .push(new_java2rust::facts::HierarchyOverride {
            type_name: "AbstractList".into(),
            strategy: new_java2rust::facts::HierarchyStrategy::ConcreteStruct,
            provenance: new_java2rust::facts::FactProvenance::UserOverride,
        });
    assert_reaches(&overridden, "AbstractList", "Iterable", false);
}

#[test]
fn inherited_iterable_adapter_retains_the_source_allocation() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/inherited_iterable_identity.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "inherited_iterable_identity",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> {
    fn checked_value(self) -> i32 { self.unwrap() }
}
#[test]
fn inherited_interface_observes_source_mutation() {
    assert_eq!(InheritedIterableIdentity::run().checked_value(), 9);
}
"#,
        }],
    );
}
