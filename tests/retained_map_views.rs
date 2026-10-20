#[path = "common/mod.rs"]
mod common;

#[test]
fn source_carrier_map_results_preserve_dispatch_identity_and_null() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_carrier_map_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_carrier_map_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(SourceCarrierMapValues::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn source_key_protocol_requirements_reach_map_callers_and_forwarders() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nested_key_capabilities.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nested_key_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_oracle() { assert_eq!(NestedKeyCapabilities::run().value(), 0); }
#[test] fn observing_map_size_does_not_invoke_the_key_protocol() {
    struct NoClone;
    impl JavaMapKey for NoClone {
        fn java_map_key_eq(&self, _: &Self) -> bool { true }
    }
    let index = ProtocolIndex::<NoClone> {
        entries: Some(std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::new()))),
        __marker: std::marker::PhantomData,
    };
    assert_eq!(index.size_only().value(), 0);
}
"#,
        }],
    );
}

#[test]
fn nested_factory_and_map_storage_preserve_one_element_contract() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nested_map_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nested_map_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_nested_contracts() { assert_eq!(NestedMapContracts::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn map_values_and_put_if_absent_keep_live_reference_contracts() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_map_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_map_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_values_contracts() { assert_eq!(RetainedMapValues::run().value(), 0); }
#[test] fn generic_value_view_does_not_demand_key_clone_or_value_equality() {
    struct NoCloneKey(i32);
    impl JavaMapKey for NoCloneKey {
        fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
    }
    #[derive(Clone)] struct NoEqualityValue;
    let map = std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::<NoCloneKey, Option<NoEqualityValue>>::new()));
    let values = RetainedMapValues::forward_values(&map);
    assert!(values.borrow().is_empty());
}
"#,
        }],
    );
}

#[test]
fn nullable_query_borrows_and_current_self_use_canonical_carriers() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/canonical_carrier_queries.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "canonical_carrier_queries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_query_contracts() { assert_eq!(CanonicalCarrierQueries::run().value(), 0); }
#[test] fn null_query_does_not_require_nullable_key_storage() {
    let mut map = JavaSemanticHashMap::<String, i32>::new();
    map.insert("a".into(), 1);
    assert_eq!(map.get_query(None::<&str>), None);
    assert!(!map.contains_key_query(None::<&String>));
    assert_eq!(map.remove_query(None::<&str>), None);
    assert_eq!(map.get_query(Some("a")), Some(&1));
    assert_eq!(map.len(), 1);
}
"#,
        }],
    );
}

#[test]
fn mixed_map_carrier_forwarding_reaches_a_signature_fixpoint() {
    use new_java2rust::{
        facts::FactSet, parser, representation, resolve, semantic_analysis, signature,
    };
    let parsed = parser::parse_java_source(include_str!("fixtures/retained_map_views.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let profiled = signature::solve_signatures_with_facts_and_support_context_profiled(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
        &resolved.program,
    );
    assert_eq!(
        profiled.profile().diagnostics(),
        0,
        "{:?}",
        profiled.profile()
    );
    assert!(profiled.profile().fixpoint_iterations() < profiled.profile().iteration_budget());
}

#[test]
fn map_children_and_returned_key_views_keep_live_reference_contracts() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_map_views.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_map_views",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_map_view_contracts() { assert_eq!(RetainedMapViews::run().value(), 0); }
"#,
        }],
    );
}
