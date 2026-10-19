#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_covariant_stream_elements_keep_their_typed_read_domain() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_covariant_stream_values.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_covariant_stream_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn typed_values_match_java() { assert_eq!(CovariantStreamValues::<String>::run().value(), 1); }
"#,
        }],
    );
}

#[test]
fn retained_stream_callbacks_reuse_solved_source_and_nullable_field_contracts() {
    let parsed = new_java2rust::parser::parse_java_source(include_str!(
        "fixtures/retained_stream_callback_contracts.java"
    ));
    let resolved = new_java2rust::resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = new_java2rust::semantic_analysis::analyze_program(
        &resolved.program,
        &resolved.facts,
        &new_java2rust::facts::FactSet::default(),
    );
    let reps =
        new_java2rust::representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = new_java2rust::signature::solve_signatures_with_facts(
        &resolved.program,
        &reps,
        &semantic.usage,
        &semantic.facts,
    );
    for owner in &resolved.program.types {
        if owner.name != "RetainedStreamCallbackContracts" {
            continue;
        }
        for member in &owner.members {
            if let new_java2rust::java_ir::Member::Method(method) = member {
                let signature = signatures.method_signature(owner, method).unwrap();
                if method.name == "run" {
                    let selected = signature
                        .locals()
                        .iter()
                        .find(|local| local.name() == "selected")
                        .unwrap();
                    let matches = signature
                        .locals()
                        .iter()
                        .find(|local| local.name() == "matches")
                        .unwrap();
                    assert_eq!(
                        selected.shape().sequence_element_type(),
                        Some(new_java2rust::java_ir::JavaType::String),
                        "return constraints must not cross between unrelated filter methods"
                    );
                    assert_eq!(
                        matches
                            .shape()
                            .sequence_element_type()
                            .and_then(|ty| new_java2rust::types::optional_element_type(&ty)),
                        Some(new_java2rust::java_ir::JavaType::String)
                    );
                }
            }
        }
    }
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/retained_stream_callback_contracts.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_stream_callback_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_and_field_callbacks_match_java() {
    assert_eq!(RetainedStreamCallbackContracts::run().value(), 22);
}
"#,
        }],
    );
}

#[test]
fn retained_stream_collectors_transfer_fresh_allocations_and_keep_java_equality() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_stream_collectors.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_stream_collectors",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[derive(Clone)] struct Key(i32);
impl JavaMapKey for Key { fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 } }
#[test] fn fresh_collection_results_match_java() {
    assert_eq!(RetainedStreamCollectors::run().value(), 3134);
    assert_eq!(RetainedStreamCollectors::nullable().value(), 21);
    assert_eq!(RetainedStreamCollectors::checked().value(), 7);
    assert_eq!(RetainedStreamCollectors::list_policies().value(), 11);
    let source = vec![Key(1), Key(2), Key(1)];
    let nullable = source.iter().cloned().map(Some).collect::<Vec<_>>();
    let result = RetainedStreamCollectors::set(&nullable);
    assert_eq!(result.borrow().len(), 2);
    let ordered = RetainedStreamCollectors::ordered(&source);
    assert_eq!(ordered.borrow().len(), 2);
    let copy = RetainedStreamCollectors::add_null_to_copy(&source).unwrap();
    assert_eq!(copy.borrow().len(), 3);
    assert_eq!(source.len(), 3);
}
#[test] fn set_equality_remains_interleaved_with_stream_callbacks() {
    #[derive(Clone)] struct EventKey(i32, std::rc::Rc<std::cell::RefCell<Vec<String>>>);
    impl JavaMapKey for EventKey {
        fn java_map_key_eq(&self, other: &Self) -> bool {
            self.1.borrow_mut().push("e".into());
            self.0 == other.0
        }
    }
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let values = vec![EventKey(1, log.clone()), EventKey(2, log.clone()), EventKey(1, log.clone())];
    let callback = Function::from_fn(|value: EventKey| {
        value.1.borrow_mut().push(format!("m{}", value.0));
        value
    });
    let result = RetainedStreamCollectors::mapped(&values, callback).unwrap();
    assert_eq!(result.borrow().len(), 2);
    assert_eq!(&*log.borrow(), &["m1", "m2", "e", "m1", "e"]);
}
"#,
        }],
    );
}

#[test]
fn map_entry_arrays_own_the_pair_payloads() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/map_entry_arrays.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "map_entry_arrays",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn boxed_entries_outlive_the_map_borrow() {
    let mut values = std::collections::HashMap::new();
    values.insert("key".to_string(), "value".to_string());
    let entries = MapEntryArrays::erased(&values);
    values.clear();
    assert_eq!(entries.len(), 1);
    let entry = entries[0].as_ref().unwrap().downcast_ref::<(String, String)>().unwrap();
    assert_eq!(entry, &("key".to_string(), "value".to_string()));
}
"#,
        }],
    );
}

#[test]
fn generic_set_collectors_use_the_solved_java_equality_carrier() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/stream_set_carriers.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "stream_set_carriers",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct Key(i32);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
#[test]
fn collectors_do_not_require_rust_hash_or_eq() {
    let values = vec![Key(1), Key(2), Key(1)];
    for set in [StreamSetCarriers::copied(&values), StreamSetCarriers::filtered(&values),
        StreamSetCarriers::mapped(&values), StreamSetCarriers::checked(&values, false).unwrap()] {
        assert_eq!(set.len(), 2);
        assert!(set.contains(&Key(1)) && set.contains(&Key(2)));
    }
    assert!(StreamSetCarriers::checked::<Key>(&[], true).unwrap().is_empty());
    let error = match StreamSetCarriers::checked(&values, true) {
        Err(error) => error,
        Ok(_) => panic!("callback failure was swallowed"),
    };
    assert!(error.is_instance(&["IllegalArgumentException"]));
}
"#,
        }],
    );
}

#[test]
fn map_set_snapshots_copy_only_the_selected_element_type() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/map_set_carriers.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "map_set_carriers",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct Key(i32);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
#[test]
fn snapshots_use_java_equality_and_do_not_copy_unrelated_values() {
    let mut map = JavaSemanticHashMap::new();
    map.insert(Key(1), Key(7));
    map.insert(Key(2), Key(7));
    let keys = MapSetCarriers::keys(&map);
    let copy = MapSetCarriers::copied_keys(&map);
    let values = MapSetCarriers::copied_values(&map);
    assert_eq!(keys.len(), 2);
    assert_eq!(copy.len(), 2);
    assert_eq!(values.len(), 1);
    assert!(values.contains(&Key(7)));
    assert!(MapSetCarriers::maybe_keys(&map, true).is_none());
    assert_eq!(MapSetCarriers::maybe_keys(&map, false).unwrap().len(), 2);
    assert_eq!(MapSetCarriers::copied_size(&map), 2);
    map.insert(Key(3), Key(8));
    assert_eq!(copy.len(), 2);
    assert_eq!(values.len(), 1);
    // This is an explicit snapshot test, not a claim of live keySet aliasing.
    struct Opaque;
    let mut opaque = JavaSemanticHashMap::new();
    opaque.insert(Key(1), Opaque);
    assert_eq!(MapSetCarriers::copied_keys(&opaque).len(), 1);
    assert_eq!(MapSetCarriers::forwarded_keys(&opaque).len(), 1);
    struct NoCloneKey(i32);
    impl JavaMapKey for NoCloneKey {
        fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
    }
    let mut counted = JavaSemanticHashMap::new();
    counted.insert(NoCloneKey(1), Opaque);
    assert_eq!(MapSetCarriers::key_count(&counted), 1);
    assert_eq!(MapSetCarriers::source_name(&MapLike::<NoCloneKey>::new()), 7);
}
"#,
        }],
    );
}
