#[path = "common/mod.rs"]
mod common;

#[test]
fn list_projection_crosses_generic_calls_without_losing_aliases() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/list_projection_boundaries.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "list_projection_boundaries",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn generic_views_match_java() {
    assert_eq!(ListProjectionBoundaries::generic_element().value(), 10);
    assert_eq!(ListProjectionBoundaries::aliases().value(), 34);
    assert_eq!(ListProjectionBoundaries::null_cast().value(), 7);
    assert_eq!(ListProjectionBoundaries::rejected_set().value(), 7);
    assert_eq!(ListProjectionBoundaries::nullable_element().value(), 110);
}
#[test] fn cast_does_not_require_cloneable_elements() {
    struct NotClone;
    assert!(ListProjectionBoundaries::cast::<NotClone>(None).unwrap().is_none());
}
"#,
        }],
    );
}

#[test]
fn source_list_projection_preserves_virtual_operations_and_mutation() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_list_projection.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_list_projection",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn source_list_operations_match_java() {
    assert_eq!(SourceListProjection::live_read().value(), 47);
    assert_eq!(SourceListProjection::mutation_through_cast().value(), 28);
}
"#,
        }],
    );
}

#[test]
fn retained_bulk_operands_preserve_domains_aliases_and_changed_results() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_remove_all.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_remove_all",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn retained_remove_all_matches_java() {
    assert_eq!(RetainedRemoveAll::plain_receiver().value(), 21);
    assert_eq!(RetainedRemoveAll::retained_receiver().value(), 111);
    assert_eq!(RetainedRemoveAll::plain_list().value(), 111);
    assert_eq!(RetainedRemoveAll::self_removal().value(), 10);
}
"#,
        }],
    );
}

#[test]
fn retained_collection_searches_use_java_equality_and_preserve_aliases() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_collection_search.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_collection_search",
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
fn retained_search_matches_java() {
    assert_eq!(RetainedCollectionSearch::run().checked_value(), 132);
}
"#,
        }],
    );
}

#[test]
fn retained_collection_iterables_keep_aliases_protocol_and_runtime_type() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_collection_iterable.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}\nset alias output: {:?}",
        translation.diagnostics,
        translation
            .rust
            .lines()
            .skip_while(|line| !line.contains("fn set_alias"))
            .take(18)
            .collect::<Vec<_>>()
    );
    common::cargo_test_generated_modules(
        "retained_collection_iterable",
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
fn retained_views_match_java() {
    assert_eq!(RetainedCollectionIterable::list_alias().checked_value(), 20);
    assert_eq!(RetainedCollectionIterable::set_alias().checked_value(), 7);
    assert_eq!(RetainedCollectionIterable::supplier_alias().checked_value(), 23);
    assert_eq!(RetainedCollectionIterable::iterator_removal().checked_value(), 14);
    assert_eq!(RetainedCollectionIterable::iterator_fail_fast().checked_value(), -7);
    assert_eq!(RetainedCollectionIterable::list_downcast().checked_value(), 4);
    assert_eq!(RetainedCollectionIterable::non_list_branch().checked_value(), 6);
}
"#,
        }],
    );
}
