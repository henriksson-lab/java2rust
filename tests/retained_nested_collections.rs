#[path = "common/mod.rs"]
mod common;

#[test]
fn native_entry_object_receivers_recover_shared_handles_without_copying_payloads() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/native_entry_object_receiver.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "native_entry_object_receiver",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn entry_receivers_preserve_aliases_and_null_exceptions() {
    assert_eq!(NativeEntryObjectReceiver::run().value(), 10);
    assert_eq!(NativeEntryObjectReceiver::null_receiver().value(), 7);
    assert_eq!(NativeEntryObjectReceiver::null_mutation().value(), 9);
    assert_eq!(NativeEntryObjectReceiver::key_receiver().value(), 14);
    assert_eq!(NativeEntryObjectReceiver::null_key().value(), 11);
}
"#,
        }],
    );
}

#[test]
fn concrete_entry_setter_preserves_nullable_result_and_owned_tuple_storage() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/concrete_entry_setter.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "entry_setter_nullable_storage",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"#[test] fn setter_returns_old_value_and_updates_entry() { assert_eq!(EntrySetter::run(), "1,a=2"); }"#,
        }],
    );
}

#[test]
fn retained_native_enhanced_for_uses_checked_live_cursor_and_solved_element_capabilities() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_native_iteration.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_native_iteration",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn checked_iteration_matches_java() {
    assert_eq!(RetainedNativeIteration::list().checked_value(), 3);
    assert_eq!(RetainedNativeIteration::set().checked_value(), 5);
    assert_eq!(RetainedNativeIteration::fail_fast().checked_value(), 7);
    assert_eq!(RetainedNativeIteration::null_source().checked_value(), 5);
    assert_eq!(RetainedNativeIteration::field_replacement().checked_value(), 31);
    assert_eq!(RetainedNativeIteration::array_source().checked_value(), 4);
    assert_eq!(RetainedNativeIteration::null_element().checked_value(), 6);
    assert_eq!(RetainedNativeIteration::nullable_element().checked_value(), 7);
    assert_eq!(RetainedNativeIteration::forwarded_unboxing().checked_value(), 8);
    assert_eq!(GenericIteration::<i32>::new().count().checked_value(), 0);
    assert_eq!(GenericIteration::<i32>::new().forward().checked_value(), 0);
    let mut generic = GenericIteration::<i32>::new();
    generic.add(3).unwrap();
    assert_eq!(generic.count_non_null().checked_value(), 1);
    struct NonClone;
    let mut rows = GenericRows::<NonClone>::new();
    assert_eq!(rows.count().checked_value(), 0);
    assert_eq!(rows.forward().checked_value(), 0);
}
"#,
        }],
    );
}

#[test]
fn source_factory_owner_takes_precedence_without_hiding_qualified_jdk_factory() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class Collections {
    static Set<Integer> singleton(Integer value) {
        Set<Integer> result = new LinkedHashSet<>();
        result.add(value);
        return result;
    }
}
class ShadowFactories {
    static int run() {
        Set<Integer> source = Collections.singleton(1);
        Iterable<Integer> sourceView = source;
        source.add(2);
        Set<Integer> platform = java.util.Collections.singleton(3);
        Iterable<Integer> platformView = platform;
        int score = source.size();
        try { platform.add(4); }
        catch (UnsupportedOperationException failure) { score += 10; }
        return score;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_factory_precedence",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn source_and_jdk_policies_remain_distinct() { assert_eq!(ShadowFactories::run().checked_value(), 12); }
"#,
        }],
    );
}

#[test]
fn retained_immutable_factories_keep_mutation_policy_through_generic_calls() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_immutable_factories.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_immutable_factories",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn immutable_factories_match_java() {
    assert_eq!(RetainedImmutableFactories::sets().checked_value(), 111111);
    assert_eq!(RetainedImmutableFactories::lists().checked_value(), 1111111);
    assert_eq!(RetainedImmutableFactories::nested().checked_value(), 44);
    assert_eq!(RetainedImmutableFactories::unique().checked_value(), 1);
    assert_eq!(RetainedImmutableFactories::nullable().checked_value(), 1);
    assert_eq!(RetainedImmutableFactories::effects().checked_value(), 11);
    assert_eq!(RetainedImmutableFactories::generic_reuse().checked_value(), 11);
}
"#,
        }],
    );
}

#[test]
fn retained_native_override_contract_preserves_aliases_through_interface_dispatch() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_override_storage.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_override_storage",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn interface_aliases_match_java() { assert_eq!(OverrideStorage::run().checked_value(), 21); }
"#,
        }],
    );
}

#[test]
fn retained_bulk_add_preserves_aliases_and_set_equality() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_bulk_collections.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_bulk",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn bulk_aliases_match_java() {
    assert_eq!(RetainedBulk::run().checked_value(), 19);
    assert_eq!(RetainedBulk::null_source().checked_value(), 1);
    assert_eq!(RetainedBulk::empty_invalidation().checked_value(), 11);
    assert_eq!(RetainedBulk::receiver_replacement().checked_value(), 101);
    assert_eq!(RetainedBulk::same_owner_replacement().checked_value(), 101);
}
"#,
        }],
    );
}

#[test]
fn retained_generic_field_preserves_its_handle_during_semantic_hash_projection() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_generic_field.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_generic_field",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn checked_value(self) -> i32; }
impl ProbeInt for i32 { fn checked_value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn checked_value(self) -> i32 { self.unwrap() } }
#[test]
fn generic_field_alias_is_live() { assert_eq!(SharedGenericSet::<i32>::run().checked_value(), 22); }
"#,
        }],
    );
}

#[test]
fn retained_generic_factory_preserves_allocation_and_equality_capability() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.LinkedHashSet;
import java.util.Set;
class RetainedFactory {
    static <T> Set<T> fresh() { return new LinkedHashSet<>(); }
    static <T> Set<T> forwarded() { return RetainedFactory.<T>fresh(); }
    static int run() {
        Set<Integer> values = RetainedFactory.<Integer>fresh();
        Iterable<Integer> view = values;
        values.add(4);
        values.add(4);
        int result = 0;
        for (int value : view) { result += value; }
        return result;
    }
    static int forwardedRun() {
        Set<Integer> values = RetainedFactory.<Integer>forwarded();
        Iterable<Integer> view = values;
        values.add(7);
        int result = 0;
        for (int value : view) { result += value; }
        return result;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_generic_factory",
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
fn factory_alias_matches_java() {
    assert_eq!(RetainedFactory::run().checked_value(), 4);
    assert_eq!(RetainedFactory::forwarded_run().checked_value(), 7);
}
"#,
        }],
    );
}

#[test]
fn retained_nested_collections_preserve_aliases_through_elements_fields_and_factories() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_nested_collections.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_nested_collections",
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
fn nested_aliases_match_java() {
    assert_eq!(RetainedNestedCollections::map_alias().checked_value(), 63);
    assert_eq!(RetainedNestedCollections::list_alias().checked_value(), 63);
    assert_eq!(RetainedNestedCollections::factory_alias().checked_value(), 4);
    assert_eq!(RetainedNestedCollections::field_alias().checked_value(), 63);
    assert_eq!(RetainedNestedCollections::plain_alias().checked_value(), 33);
    assert_eq!(RetainedNestedCollections::extracted_alias().checked_value(), 22);
    assert_eq!(RetainedNestedCollections::cross_owner_alias().checked_value(), 33);
    assert_eq!(RetainedNestedCollections::deep_alias().checked_value(), 22);
    assert_eq!(RetainedNestedCollections::nullable_row().checked_value(), -1);
    assert_eq!(RetainedNestedCollections::call_boundary().checked_value(), 22);
}
"#,
        }],
    );
}
