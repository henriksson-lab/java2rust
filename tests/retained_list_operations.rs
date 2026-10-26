#[path = "common/mod.rs"]
mod common;

#[test]
fn boxed_indices_unbox_once_at_retained_collection_boundaries() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/boxed_collection_indices.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "boxed_collection_indices",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_boxed_indices() { assert_eq!(BoxedCollectionIndices::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn list_cursor_storage_tracks_backing_and_nullable_writes() {
    use new_java2rust::{
        facts::FactSet, java_ir::Member, parser, representation, resolve, semantic_analysis,
        signature,
    };
    let parsed = parser::parse_java_source(include_str!("fixtures/retained_list_operations.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let owner = &resolved.program.types[0];
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = plan.method_signature(owner, method).unwrap();
    for name in ["backed", "copied", "nullable"] {
        let shape = signature
            .locals()
            .iter()
            .find(|local| local.name() == name)
            .unwrap()
            .shape();
        assert!(shape.native_allocation.shared(), "{name}: {shape:?}");
    }
    for name in ["cursor", "writer"] {
        let shape = signature
            .locals()
            .iter()
            .find(|local| local.name() == name)
            .unwrap()
            .shape();
        let new_java2rust::java_ir::JavaType::Named { args, .. } = &shape.java_ty else {
            panic!("{shape:?}")
        };
        assert!(
            new_java2rust::types::optional_element_type(&args[0]).is_some(),
            "{name}: {shape:?}"
        );
    }
    let references = signature
        .locals()
        .iter()
        .find(|local| local.name() == "references")
        .unwrap()
        .shape();
    assert!(
        references
            .nested_native_allocations
            .iter()
            .any(|(path, placement)| path == &[0] && placement.shared()),
        "{references:?}"
    );
}

#[test]
fn retained_lists_keep_live_cursors_reversal_and_distinct_copies() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_list_operations.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_list_operations",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn java_list_contracts() { assert_eq!(RetainedListOperations::run().value(), 23); }
#[test] fn projected_reference_identity_and_operation_access_share_the_root() {
    struct NoClone(i32);
    let source = JavaLocalNativeRef::new(std::cell::RefCell::new(NoClone(1)));
    let view = JavaLocalNativeRef::projected(&source, std::cell::RefCell::new(source.clone()));
    let source_loan = source.borrow_mut();
    let view_loan = view.borrow_mut();
    assert!(java_reference_equals(Some(&source), Some(&view)));
    drop((source_loan, view_loan));
    view.java_read(|owner| owner.java_write(|value| value.0 = 9));
    assert_eq!(source.java_read(|value| value.0), 9);
    let sync = JavaSyncNativeRef::new(std::sync::Mutex::new(NoClone(3)));
    let sync_view = JavaSyncNativeRef::projected(&sync, std::sync::Mutex::new(sync.clone()));
    let source_guard = sync.lock().unwrap();
    let view_guard = sync_view.lock().unwrap();
    assert!(java_reference_equals(Some(&sync), Some(&sync_view)));
    drop((source_guard, view_guard));
    sync_view.java_read(|owner| owner.java_write(|value| value.0 = 11));
    assert_eq!(sync.java_read(|value| value.0), 11);
}
#[test] fn probes_and_reversal_do_not_require_payload_clone() {
    struct NoClone(i32);
    let cursor = JavaListIterator::<NoClone>::from_fallible_operations(
        || false, || false, || Ok(NoClone(1)), || Ok(NoClone(2)), || 0,
        || Ok(()), |_| Ok(()), |_| Ok(()),
    );
    let alias = cursor.clone();
    assert_eq!(cursor, alias);
    assert!(!alias.has_next());
    let values = std::rc::Rc::new(std::cell::RefCell::new(vec![NoClone(1), NoClone(2)]));
    let mut view = JavaNativeCollection::from_local_array(values.clone());
    view.reverse_result().unwrap();
    assert_eq!(values.borrow()[0].0, 2);
}
"#,
        }],
    );
}
