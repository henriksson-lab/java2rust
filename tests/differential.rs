//! Differential tests: run the Java and run the translation, compare output.
//!
//! Compile errors only catch idioms that fail loudly. The defects that cost the
//! most — arithmetic that wraps the wrong way, a `StringBuilder` that silently
//! returns `""`, a `static` block that never runs, `Integer.MAX_VALUE` emitted
//! as `0` — all produce Rust that compiles cleanly and computes the wrong
//! answer. Nothing but running both sides finds those.
//!
//! Each fixture in `tests/differential/` is a self-contained class exposing
//! `public static String run()` and a `main` that prints it. Java's stdout is
//! the oracle; the translation must produce the same bytes.
//!
//! Requires `javac`, `java` and `rustc` on PATH. Missing prerequisites are
//! failures, because a skipped behavioural oracle is indistinguishable from a
//! passing one in the gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXPECTED_DIFFERENTIAL_FIXTURES: usize = 437;
const EXPECTED_KNOWN_GAP_FIXTURES: usize = 0;

#[test]
fn generic_source_erased_display_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/GenericErasedDisplay.java");
    compare_fixture(&translator, &fixture, "GenericErasedDisplay").unwrap();
}

#[test]
fn typed_map_entries_erased_to_object_keep_key_value_display() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedMapEntryTypedDisplay.java");
    compare_fixture(&translator, &fixture, "ErasedMapEntryTypedDisplay").unwrap();
}

#[test]
fn source_map_entry_values_keep_virtual_methods_and_aliases() {
    let translator = translator_binary().expect("translator binary built");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    for name in [
        "SourceMapEntryValue",
        "SourceMapEntryImported",
        "SourceMapEntryNullableValue",
        "SourceMapEntryDirect",
    ] {
        compare_fixture(&translator, &root.join(format!("{name}.java")), name).unwrap();
    }
}

#[test]
fn mixed_source_and_native_map_entries_keep_live_dispatch() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/MixedSourceAndNativeMapEntry.java");
    compare_fixture(&translator, &fixture, "MixedSourceAndNativeMapEntry").unwrap();
}

#[test]
fn mixed_source_and_hash_map_entries_keep_live_dispatch() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/MixedSourceAndHashMapEntryAlias.java");
    compare_fixture(&translator, &fixture, "MixedSourceAndHashMapEntryAlias").unwrap();
}

#[test]
fn mixed_source_and_tree_map_entries_keep_live_sorted_dispatch() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/MixedSourceAndTreeMapEntryAlias.java");
    compare_fixture(&translator, &fixture, "MixedSourceAndTreeMapEntryAlias").unwrap();
}

#[test]
fn tree_map_entry_parameter_mutation_reaches_caller() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/TreeMapEntryParameterMutation.java");
    compare_fixture(&translator, &fixture, "TreeMapEntryParameterMutation").unwrap();
}

#[test]
fn hash_and_linked_map_entry_parameter_mutations_reach_callers() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/HashAndLinkedMapEntryParameterMutation.java");
    compare_fixture(
        &translator,
        &fixture,
        "HashAndLinkedMapEntryParameterMutation",
    )
    .unwrap();
}

#[test]
fn nullable_map_entry_parameter_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapEntryParameterMutation.java");
    compare_fixture(&translator, &fixture, "NullableMapEntryParameterMutation").unwrap();
}

#[test]
fn interface_map_entry_parameter_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/InterfaceMapEntryParameterMutation.java");
    compare_fixture(&translator, &fixture, "InterfaceMapEntryParameterMutation").unwrap();
}

#[test]
fn interface_map_field_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/InterfaceMapFieldEntryMutation.java");
    compare_fixture(&translator, &fixture, "InterfaceMapFieldEntryMutation").unwrap();
}

#[test]
fn interface_map_local_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/InterfaceMapLocalEntryMutation.java");
    compare_fixture(&translator, &fixture, "InterfaceMapLocalEntryMutation").unwrap();
}

#[test]
fn map_field_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/MapFieldEntryMutation.java");
    compare_fixture(&translator, &fixture, "MapFieldEntryMutation").unwrap();
}

#[test]
fn nullable_map_field_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapFieldEntryMutation.java");
    compare_fixture(&translator, &fixture, "NullableMapFieldEntryMutation").unwrap();
}

#[test]
fn static_map_field_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/StaticMapFieldEntryMutation.java");
    compare_fixture(&translator, &fixture, "StaticMapFieldEntryMutation").unwrap();
}

#[test]
fn nullable_static_map_field_entry_mutation_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableStaticMapFieldEntryMutation.java");
    compare_fixture(&translator, &fixture, "NullableStaticMapFieldEntryMutation").unwrap();
}

#[test]
fn nullable_hash_map_value_concat_matches_java() {
    let translator = translator_binary().expect("translator binary built");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    for name in ["MapPutNullValue", "NullableHashMapValueConcat"] {
        compare_fixture(&translator, &root.join(format!("{name}.java")), name).unwrap();
    }
}

#[test]
fn mixed_source_and_hash_map_entry_views_follow_live_values() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/MixedSourceAndHashMapEntryViews.java");
    compare_fixture(&translator, &fixture, "MixedSourceAndHashMapEntryViews").unwrap();
}

#[test]
fn hash_map_inline_entry_argument_keeps_live_set_value() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/HashMapInlineEntrySetValue.java");
    compare_fixture(&translator, &fixture, "HashMapInlineEntrySetValue").unwrap();
}

#[test]
fn nested_map_entry_mutation_keeps_live_backing_map() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NestedMapEntryMutation.java");
    compare_fixture(&translator, &fixture, "NestedMapEntryMutation").unwrap();
}

#[test]
fn assigned_map_entry_alias_mutation_keeps_live_backing_map() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/AssignedMapEntryAliasMutation.java");
    compare_fixture(&translator, &fixture, "AssignedMapEntryAliasMutation").unwrap();
}

#[test]
fn nested_assigned_map_entry_alias_mutation_keeps_live_backing_map() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NestedAssignedMapEntryAliasMutation.java");
    compare_fixture(&translator, &fixture, "NestedAssignedMapEntryAliasMutation").unwrap();
}

#[test]
fn nullable_map_get_switch_selectors_raise_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapGetSwitchCatch.java");
    compare_fixture(&translator, &fixture, "NullableMapGetSwitchCatch").unwrap();
}

#[test]
fn nullable_map_get_primitive_contexts_raise_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapGetPrimitiveContexts.java");
    compare_fixture(&translator, &fixture, "NullableMapGetPrimitiveContexts").unwrap();
}

#[test]
fn inline_array_temporary_elements_survive_checked_access() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/InlineArrayTemporaryAccess.java");
    compare_fixture(&translator, &fixture, "InlineArrayTemporaryAccess").unwrap();
}

#[test]
fn wildcard_map_entry_access_and_display_match_java() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/WildcardMapEntryAccess.java");
    compare_fixture(&translator, &fixture, "WildcardMapEntryAccess").unwrap();
}

#[test]
fn nullable_hash_map_entry_unboxing_raises_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableHashMapEntryUnboxingCatch.java");
    compare_fixture(&translator, &fixture, "NullableHashMapEntryUnboxingCatch").unwrap();
}

#[test]
fn nullable_map_entry_primitive_contexts_raise_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapEntryPrimitiveReturnCatch.java");
    compare_fixture(
        &translator,
        &fixture,
        "NullableMapEntryPrimitiveReturnCatch",
    )
    .unwrap();
}

#[test]
fn nullable_map_entry_boolean_condition_raises_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapEntryBooleanConditionCatch.java");
    compare_fixture(
        &translator,
        &fixture,
        "NullableMapEntryBooleanConditionCatch",
    )
    .unwrap();
}

#[test]
fn nullable_map_entry_string_switch_raises_catchable_null_pointer() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapEntryStringSwitchCatch.java");
    compare_fixture(&translator, &fixture, "NullableMapEntryStringSwitchCatch").unwrap();
}

#[test]
fn source_map_entry_nullable_accessors_compare_to_null() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/SourceMapEntryNullComparison.java");
    compare_fixture(&translator, &fixture, "SourceMapEntryNullComparison").unwrap();
}

#[test]
fn generic_source_map_entry_keeps_virtual_methods_and_aliases() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/GenericSourceMapEntry.java");
    compare_fixture(&translator, &fixture, "GenericSourceMapEntry").unwrap();
}

#[test]
fn nullable_map_entry_value_deref_is_catchable() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapEntryValueCatch.java");
    compare_fixture(&translator, &fixture, "NullableMapEntryValueCatch").unwrap();
}

#[test]
fn nullable_map_key_variable_is_not_collapsed_to_empty_string() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapKeyVariable.java");
    compare_fixture(&translator, &fixture, "NullableMapKeyVariable").unwrap();
}

#[test]
fn nullable_map_key_parameter_is_not_collapsed_to_empty_string() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapKeyParameter.java");
    compare_fixture(&translator, &fixture, "NullableMapKeyParameter").unwrap();
}

#[test]
fn nullable_map_key_constructor_parameter_is_not_collapsed_to_empty_string() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableMapKeyConstructor.java");
    compare_fixture(&translator, &fixture, "NullableMapKeyConstructor").unwrap();
}

#[test]
fn source_map_entry_erased_to_object_keeps_dynamic_display() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/SourceMapEntryErased.java");
    compare_fixture(&translator, &fixture, "SourceMapEntryErased").unwrap();
}

#[test]
fn source_map_entry_erased_display_exception_is_catchable() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/SourceMapEntryErasedThrow.java");
    compare_fixture(&translator, &fixture, "SourceMapEntryErasedThrow").unwrap();
}

#[test]
fn objects_to_string_erased_throw_is_catchable() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ObjectsToStringErasedThrow.java");
    compare_fixture(&translator, &fixture, "ObjectsToStringErasedThrow").unwrap();
}

#[test]
fn erased_object_array_elements_keep_their_dynamic_display() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedObjectArrayElements.java");
    compare_fixture(&translator, &fixture, "ErasedObjectArrayElements").unwrap();
}

#[test]
fn erased_object_array_round_trip_keeps_elements_and_length() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedObjectArrayRoundTrip.java");
    compare_fixture(&translator, &fixture, "ErasedObjectArrayRoundTrip").unwrap();
}

#[test]
fn erased_source_argument_keeps_dynamic_display() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedSourceArgumentDisplay.java");
    compare_fixture(&translator, &fixture, "ErasedSourceArgumentDisplay").unwrap();
}

#[test]
fn nullable_boxed_field_getter_preserves_optional_carrier() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableBoxedFieldGetter.java");
    compare_fixture(&translator, &fixture, "NullableBoxedFieldGetter").unwrap();
}

#[test]
fn nullable_erased_to_string_raises_catchable_npe() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableErasedToStringCatch.java");
    compare_fixture(&translator, &fixture, "NullableErasedToStringCatch").unwrap();
}

#[test]
fn nullable_receiver_matrix_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableReceiverMatrix.java");
    compare_fixture(&translator, &fixture, "NullableReceiverMatrix").unwrap();
}

#[test]
fn nullable_enhanced_for_receivers_raise_catchable_npe() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/NullableEnhancedForCatch.java");
    compare_fixture(&translator, &fixture, "NullableEnhancedForCatch").unwrap();
    for class in ["EnhancedForArray", "EnhancedForCollection"] {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/differential")
            .join(format!("{class}.java"));
        compare_fixture(&translator, &fixture, class).unwrap();
    }
}

#[test]
fn generic_nullable_field_display_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/GenericNullableFieldDisplay.java");
    compare_fixture(&translator, &fixture, "GenericNullableFieldDisplay").unwrap();
}

#[test]
fn generic_nullable_stringification_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/GenericNullableStringification.java");
    compare_fixture(&translator, &fixture, "GenericNullableStringification").unwrap();
}

#[test]
fn generic_nullable_formatting_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/GenericNullableFormatting.java");
    compare_fixture(&translator, &fixture, "GenericNullableFormatting").unwrap();
}

#[test]
fn string_format_locale_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/StringFormatLocale.java");
    compare_fixture(&translator, &fixture, "StringFormatLocale").unwrap();
}

#[test]
fn generic_objects_to_string_default_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/GenericObjectsToStringDefault.java");
    compare_fixture(&translator, &fixture, "GenericObjectsToStringDefault").unwrap();
}

#[test]
fn objects_to_string_default_evaluation_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ObjectsToStringNullDefault.java");
    compare_fixture(&translator, &fixture, "ObjectsToStringNullDefault").unwrap();
}

#[test]
fn source_objects_to_string_shadows_java_utility() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/SourceObjectsToStringShadow.java");
    compare_fixture(&translator, &fixture, "SourceObjectsToStringShadow").unwrap();
}

#[test]
fn erased_string_identity_equals_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedStringIdentityEquals.java");
    compare_fixture(&translator, &fixture, "ErasedStringIdentityEquals").unwrap();
}

#[test]
fn erased_string_alias_display_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedStringAliasDisplay.java");
    compare_fixture(&translator, &fixture, "ErasedStringAliasDisplay").unwrap();
}

#[test]
fn nullable_objects_equals_matches_java_output() {
    let translator = translator_binary().expect("translator binary built");
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/NullableObjectsEquals.java");
    compare_fixture(&translator, &fixture, "NullableObjectsEquals").unwrap();
}

#[test]
fn translated_output_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let fixtures = fixture_paths();
    assert_eq!(
        fixtures.len(),
        EXPECTED_DIFFERENTIAL_FIXTURES,
        "differential fixture count changed; update EXPECTED_DIFFERENTIAL_FIXTURES intentionally"
    );

    let mut failures = Vec::new();
    for fixture in &fixtures {
        let class = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("fixture has a class name");
        match compare_fixture(&translator, fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "translated output differs from Java for {} of {} fixtures:\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn string_reference_identity_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/StringEqualsVsEq.java");
    compare_fixture(&translator, &fixture, "StringEqualsVsEq").unwrap();
}

#[test]
fn generic_getter_concat_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/Generics.java");
    compare_fixture(&translator, &fixture, "Generics").unwrap();
}

#[test]
fn linked_node_optional_shared_receiver_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/LinkedNode.java");
    compare_fixture(&translator, &fixture, "LinkedNode").unwrap();
}

#[test]
fn synchronized_instance_method_reentry_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/SynchronizedMethodReentry.java");
    compare_fixture(&translator, &fixture, "SynchronizedMethodReentry").unwrap();
}

#[test]
fn synchronized_static_method_reentry_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/SynchronizedStaticMethodReentry.java");
    compare_fixture(&translator, &fixture, "SynchronizedStaticMethodReentry").unwrap();
}

#[test]
fn erased_display_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ErasedAtomicBooleanDisplay",
        "ErasedAtomicIntegerDisplay",
        "ErasedAtomicLongDisplay",
        "ErasedArrayBlockingQueueDisplay",
        "ErasedArrayBlockingQueueBooleanDisplay",
        "ErasedArrayBlockingQueueDoubleDisplay",
        "ErasedArrayBlockingQueueFloatDisplay",
        "ErasedArrayBlockingQueueLongDisplay",
        "ErasedArrayListDisplay",
        "ErasedArraysAsListNullDisplay",
        "ErasedBoxedPrimitiveDisplay",
        "ErasedBitSetDisplay",
        "ErasedClassDisplay",
        "ErasedFileDisplay",
        "ErasedFloatingPointDisplay",
        "ErasedHashMapDisplay",
        "ErasedHashMapIntegerKeyDisplay",
        "ErasedHashSetDisplay",
        "ErasedInputStreamDisplay",
        "ErasedJarEntryDisplay",
        "ErasedLinkedHashMapBooleanDisplay",
        "ErasedLinkedHashMapDisplay",
        "ErasedLinkedHashMapDoubleDisplay",
        "ErasedLinkedHashMapIntegerKeyDisplay",
        "ErasedLinkedHashSetBooleanDisplay",
        "ErasedLinkedHashSetDisplay",
        "ErasedLinkedHashSetDoubleDisplay",
        "ErasedLinkedHashSetFloatDisplay",
        "ErasedLinkedHashSetLongDisplay",
        "ErasedLocaleDisplay",
        "ErasedMapEntryBooleanDisplay",
        "ErasedMapEntryDisplay",
        "ErasedMapEntryDoubleDisplay",
        "ErasedMapEntryIntegerDisplay",
        "ErasedMapEntryTypedDisplay",
        "ErasedNullableArrayListDisplay",
        "ErasedNullableBooleanArrayListDisplay",
        "ErasedNullableByteArrayListDisplay",
        "ErasedNullableCharacterArrayListDisplay",
        "ErasedNullableDoubleArrayListDisplay",
        "ErasedNullableFloatArrayListDisplay",
        "ErasedNullableIntegerArrayListDisplay",
        "ErasedNullableLongArrayListDisplay",
        "ErasedNullableShortArrayListDisplay",
        "ErasedOptionalCollectionDisplay",
        "ErasedOptionalDisplay",
        "ErasedOptionalObjectDisplay",
        "ErasedObjectArrayElements",
        "ErasedObjectArrayRoundTrip",
        "ErasedPathDisplay",
        "ErasedPatternDisplay",
        "ErasedPlatformDisplay",
        "ErasedPriorityQueueDisplay",
        "ErasedPriorityQueueFloatDisplay",
        "ErasedPriorityQueueLongDisplay",
        "ErasedRandomDisplay",
        "ErasedSimpleImmutableEntryDisplay",
        "ErasedSourceArgumentDisplay",
        "ErasedStringAliasDisplay",
        "ErasedStringBuilderDisplay",
        "ErasedStringTokenizerDisplay",
        "ErasedTreeMapBooleanDisplay",
        "ErasedTreeMapDisplay",
        "ErasedTreeMapDoubleDisplay",
        "ErasedTreeMapIntegerKeyDisplay",
        "ErasedTreeSetBooleanDisplay",
        "ErasedTreeSetDisplay",
        "ErasedUriDisplay",
        "ErasedUrlDisplay",
        "ErasedZipEntryDisplay",
        "GenericErasedDisplay",
        "GenericNullableFieldDisplay",
        "GenericNullableStringification",
        "SourceMapEntryErased",
        "SourceMapEntryErasedThrow",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "erased display fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn erased_hash_map_integer_key_display_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedHashMapIntegerKeyDisplay.java");
    compare_fixture(&translator, &fixture, "ErasedHashMapIntegerKeyDisplay")
        .expect("ErasedHashMapIntegerKeyDisplay should match Java output");
}

#[test]
fn erased_string_builder_display_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/differential/ErasedStringBuilderDisplay.java");
    compare_fixture(&translator, &fixture, "ErasedStringBuilderDisplay")
        .expect("ErasedStringBuilderDisplay should match Java output");
}

#[test]
fn aliasing_shared_local_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    compare_fixture(
        &translator,
        &root.join("AliasingLocalField.java"),
        "AliasingLocalField",
    )
    .unwrap();
    compare_fixture(
        &translator,
        &root.join("AliasingCallbackParam.java"),
        "AliasingCallbackParam",
    )
    .unwrap();
    compare_fixture(&translator, &root.join("Aliasing.java"), "Aliasing").unwrap();
}

#[test]
fn anonymous_functional_interface_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    compare_fixture(
        &translator,
        &root.join("AnonymousSupplier.java"),
        "AnonymousSupplier",
    )
    .unwrap();
}

#[test]
fn nullable_map_get_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["MapGetNull2", "MapPutNullValue"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "nullable Map.get fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn null_representation_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ErasedObjectArrayElements",
        "ErasedObjectArrayRoundTrip",
        "InlineArrayTemporaryAccess",
        "NullBasics",
        "NullFieldDefault",
        "NullObjectReturn",
        "NullCtorArg",
        "NullArgLiteral",
        "NullArgCatch",
        "NullCallArgCatch",
        "NullDeref",
        "NullDefaultReturn",
        "NullFieldCtorOverload",
        "NullInList",
        "NullInstanceof",
        "NullConcat",
        "NullLiteralConcat",
        "NullStringArray",
        "BoxedPrimitiveOperatorNullCatch",
        "NullableArrayLengthCatch",
        "NullableBoxedFieldGetter",
        "NullableErasedToStringCatch",
        "NullableEnhancedForCatch",
        "NullableFieldReceiverCatch",
        "NullableMapEntryValueCatch",
        "NullableObjectsEquals",
        "NullableReceiverMatrix",
        "NullableUnboxSwitchCatch",
        "NullableUnboxArrayDimensionCatch",
        "NullableUnboxArrayIndexCatch",
        "NullableUnboxStringCharAtCatch",
        "NullableUnboxCatch",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "null-representation fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn nullable_unbox_catch_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    compare_fixture(
        &translator,
        &root.join("NullableUnboxCatch.java"),
        "NullableUnboxCatch",
    )
    .unwrap();
}

#[test]
fn instanceof_pattern_expression_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["InstanceofPatternExpression"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "instanceof-pattern expression fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn record_canonical_constructor_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["RecordCanonicalConstructor"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "record canonical-constructor fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn qualified_inner_capture_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "InnerCapture",
        "QualifiedInnerCapture",
        "NestedInnerCapture",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "qualified inner-capture fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn super_call_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "SuperMethodCall",
        "SuperMutatingCall",
        "SuperMutatingCallAssignedLocal",
        "SuperMutatingCallArrayAccess",
        "SuperMutatingCallConditional",
        "SuperMutatingCallDoWhileLoop",
        "SuperMutatingCallEnhancedFor",
        "SuperMutatingCallForAssignmentUpdate",
        "SuperMutatingCallForLoop",
        "SuperMutatingCallIfElseAssignedLocal",
        "SuperMutatingCallLabeledLoopControl",
        "SuperMutatingCallLocalArray",
        "SuperMutatingCallLocalArrayForUpdate",
        "SuperMutatingCallLocalArrayUnary",
        "SuperMutatingCallLocalArrayWrite",
        "SuperMutatingCallLoopControl",
        "SuperMutatingCallMathStaticCall",
        "SuperMutatingCallTernaryLocal",
        "SuperMutatingCallUnaryField",
        "SuperMutatingCallUnaryLocal",
        "SuperMutatingCallUnaryParam",
        "SuperMutatingCallWhileLoop",
        "SuperMutatingCallWithArg",
        "SuperMutatingCallWithTemp",
        "SuperMutatingCallReturning",
        "SuperMutatingCallSwitch",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "super-call fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn inheritance_upcast_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ErasedCarrierIdentity",
        "ErasedObjectSupertypeCast",
        "ExplicitInheritanceUpcast",
        "InheritanceUpcast",
        "NestedInheritanceUpcast",
        "PolymorphicListUpcast",
        "PatternInstanceofSupertypeUsed",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "inheritance-upcast fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn priority_queue_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "PriorityQueueHeadOrder",
        "PriorityQueueOfferHeadOrder",
        "PriorityQueuePollBooleanConcat",
        "PriorityQueuePollDoubleConcat",
        "PriorityQueuePollIntegerConcat",
        "PriorityQueueComparatorConstructor",
        "PriorityQueueComparatorFactoryConstructor",
        "PriorityQueueComparatorFactoryLocalConstructor",
        "PriorityQueueBlockLambdaComparatorConstructor",
        "PriorityQueueLambdaKeywordComparatorConstructor",
        "PriorityQueueLambdaComparatorConstructor",
        "PriorityQueueNamedComparatorConstructor",
        "PriorityQueueStaticCompareComparatorConstructor",
        "PriorityQueueSubtractionComparatorConstructor",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "priority queue fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn queue_poll_fixture_matches_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = ["QueuePollNull", "QueuePollAliases"];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        if let Err(report) = compare_fixture(&translator, &fixture, class) {
            failures.push(format!("{class}: {report}"));
        }
    }

    assert!(
        failures.is_empty(),
        "queue poll fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn map_view_contract_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "CollectionAddAllReturn",
        "MapEntryDetachedContract",
        "MapEntryDisplay",
        "MapEntryFieldDisplay",
        "MapEntryNextDisplay",
        "MapEntrySetBulkContract",
        "MapEntrySetAddAllUnsupported",
        "MapEntrySetAddUnsupported",
        "MapEntrySetClear",
        "MapEntrySetContainsEntryLocal",
        "MapEntrySetContainsNext",
        "MapEntrySetContainsNextLocal",
        "MapEntrySetContainsParam",
        "MapEntrySetContainsSimpleEntry",
        "MapEntrySetEnhancedForSetValue",
        "MapEntrySetRemove",
        "MapEntrySetRemoveEntryLocal",
        "MapEntrySetRemoveNextLocal",
        "MapEntrySetRemoveParam",
        "MapEntrySetRemoveSimpleEntry",
        "MapEntrySetSetValue",
        "MapEntrySimpleImmutableEntry",
        "MapKeySetAddAllUnsupported",
        "MapKeySetAddUnsupported",
        "MapKeySetAliasRemove",
        "MapKeySetBulkContract",
        "MapKeySetClear",
        "MapKeySetRemove",
        "MapKeySetRemoveAll",
        "MapValuesAddUnsupported",
        "MapValuesAliasRemove",
        "MapValuesBulkContract",
        "MapValuesClear",
        "MapValuesContains",
        "MapValuesRemove",
        "MapValuesRemoveAll",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "map-view contract fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn borrowed_element_discharge_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "NestedMapValueRead",
        "EnumValuesElementIntoObject",
        "BorrowedElementIntoPrimitiveLocal",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "borrowed-element discharge fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn string_join_null_display_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "StringJoinNullDelimiter",
        "StringJoinNullElement",
        "StringJoinVarargsNullElement",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "String.join null-display fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn list_exception_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "ArrayCompoundWriteException",
        "ArrayIndexException",
        "ArrayNegativeSizeCatch",
        "ArrayWriteException",
        "ArraysBinarySearchRangeCatch",
        "ArraysCompareMismatchRangeCatch",
        "ArraysCopyOfCatch",
        "ArraysCopyOfRangeCatch",
        "ArraysRangeCatch",
        "ArraysStreamRangeCatch",
        "ArrayBlockingQueueAdd",
        "ArrayBlockingQueueBadCapacity",
        "ArrayBlockingQueueNull",
        "ArrayBlockingQueueNullVariablePrecedence",
        "ArrayBlockingQueueOffer",
        "ArrayBlockingQueuePut",
        "ArrayDequeFirstPopCatch",
        "ArrayDequeOffer",
        "BitSetClearFlipCatch",
        "BitSetCtorCatch",
        "BitSetGetCatch",
        "BitSetSetCatch",
        "CatchException",
        "CatchOrder",
        "CatchSupertype",
        "CollectionsSortComparatorThrowCatch",
        "CollectionsNCopiesCatch",
        "EnhancedForConcurrentModification",
        "ExceptionHierarchy",
        "Exceptions",
        "ExceptionsCustom",
        "FinallySwitchBreak",
        "FloatingParseNull",
        "IntegerParseNull",
        "InputStreamReadBoundsCatch",
        "IntegralParseRadix",
        "IteratorNextCatch",
        "LinkedListQueueHeadCatch",
        "ListAddIndexCatch",
        "ListElementReceiverNullCatch",
        "ListGetCatch",
        "ListRemoveIndexCatch",
        "ListSetCatch",
        "ListSubListInvalidRangeCatch",
        "LongParseNull",
        "MapEntrySetAddAllUnsupported",
        "MapEntrySetAddUnsupported",
        "MapEntryIteratorNextCatch",
        "MapGetArithmeticNullCatch",
        "MapKeySetAddAllUnsupported",
        "MapKeySetAddUnsupported",
        "MapValuesAddUnsupported",
        "MathFloorDivModCatch",
        "MathToIntExactCatch",
        "NarrowParseNull",
        "NullArgCatch",
        "NullCallArgCatch",
        "NullableArrayLengthCatch",
        "NullableErasedToStringCatch",
        "NullableFieldReceiverCatch",
        "NullableReceiverMatrix",
        "NullableUnboxSwitchCatch",
        "NullableUnboxArrayDimensionCatch",
        "NullableUnboxArrayIndexCatch",
        "NullableUnboxStringCharAtCatch",
        "NullableUnboxCatch",
        "OptionalFactoryGetCatch",
        "OptionalOfNullCatch",
        "QueueElementCatch",
        "QueuePollAliases",
        "QueuePollNull",
        "QueueRemoveCatch",
        "RandomNextIntCatch",
        "RaggedArrayNegativeSizeCatch",
        "StringBoundsException",
        "StringBuilderCapacityCatch",
        "StringBuilderCharAtCatch",
        "StringBuilderDeleteInsertCatch",
        "StringBuilderSetLengthCatch",
        "StringBuilderSubstringCatch",
        "StringByteRangeCtorCatch",
        "StringTokenizerCharAtBoundsCatch",
        "StringTokenizerNextCatch",
        "StringTokenizerNextElementCatch",
        "ThreadSleepCatch",
        "ThreadSleepNanosCatch",
        "TreeMapFirstLastKeyCatch",
        "UnmodifiableCollectionMutationCatch",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "list exception fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn finally_completion_fixtures_match_java_output() {
    let translator = translator_binary().unwrap_or_else(|| {
        panic!("translator binary not built; run `cargo build --release` before differential tests")
    });
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required differential test tool `{tool}` is not available"
        );
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let fixtures = [
        "FinallyReturnOverride",
        "CatchFinallyBreak",
        "CatchFinallyContinue",
        "CatchFinallyReturnOverride",
        "FinallyBreakOverridesReturn",
        "FinallyBreakOverridesCatchReturn",
        "FinallyContinueOverridesBreak",
        "FinallyNestedContinue",
        "FinallyBlockContinue",
        "FinallyDoWhileContinue",
        "FinallyLabeledContinue",
        "FinallyOnBreak",
        "FinallyOnContinue",
        "FinallyPaths",
        "FinallySwitchBreak",
        "FinallySwitchFallsThrough",
        "FinallyThrowOverride",
        "FinallyWhileContinue",
        "CatchFinallyLabeledContinue",
        "TryCatchFinallyBodyBreak",
        "TryCatchReturnFinally",
        "TryWithResourcesFinally",
        "TryWithResourcesFinallyBodyBreak",
        "TryWithResourcesFinallyBodyContinue",
        "TryWithResourcesCatchFinally",
        "TryWithResourcesCatchFinallyBreak",
        "TryWithResourcesCatchFinallyBodyBreak",
        "TryWithResourcesCatchFinallyBodyContinue",
        "TryWithResourcesFinallyBreakOverridesCatchReturn",
        "TryWithResourcesFinallyContinueOverridesCatchReturn",
        "TryWithResourcesCloseThrowFinally",
        "TryWithResourcesMultipleCloseSuppressed",
        "TryWithResourcesPrimaryExceptionCatchFinally",
        "TryWithResourcesSuppressedPrimary",
        "TryWithResourcesCatchBreak",
        "TryWithResourcesCatchContinue",
        "TryWithResourcesBodyBreak",
        "TryWithResourcesBodyContinue",
    ];
    let mut failures = Vec::new();
    for class in fixtures {
        let fixture = root.join(format!("{class}.java"));
        match compare_fixture(&translator, &fixture, class) {
            Ok(()) => {}
            Err(report) => failures.push(format!("{class}: {report}")),
        }
    }

    assert!(
        failures.is_empty(),
        "finally completion fixture output differs from Java:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn known_gap_fixtures_are_valid_java_and_match_expected_failure_mode() {
    for tool in ["javac", "java", "rustc"] {
        assert!(
            have_tool(tool),
            "required known-gap differential test tool `{tool}` is not available"
        );
    }

    let actual_paths = known_gap_fixture_paths()
        .into_iter()
        .map(|path| {
            path.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual_paths.len(),
        EXPECTED_KNOWN_GAP_FIXTURES,
        "known-gap fixture count changed; update EXPECTED_KNOWN_GAP_FIXTURES intentionally"
    );
}

fn compare_fixture(translator: &Path, fixture: &Path, class: &str) -> Result<(), String> {
    let work = std::env::temp_dir().join(format!("new_java2rust_differential_{class}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|err| format!("temp dir: {err}"))?;

    let java_source = work.join(format!("{class}.java"));
    std::fs::copy(fixture, &java_source).map_err(|err| format!("copy fixture: {err}"))?;

    let expected = run_java(&work, &java_source, class)?;
    let actual = run_translated(translator, &work, &java_source, class)?;

    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "java produced {expected:?}, translation produced {actual:?}"
        ))
    }
}

fn known_gap_fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential/known-gaps");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn run_java(work: &Path, source: &Path, class: &str) -> Result<String, String> {
    let compile = Command::new("javac")
        .arg("-d")
        .arg(work)
        .arg(source)
        .output()
        .map_err(|err| format!("javac: {err}"))?;
    if !compile.status.success() {
        return Err(format!(
            "javac failed: {}",
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new("java")
        .arg("-cp")
        .arg(work)
        .arg(class)
        .output()
        .map_err(|err| format!("java: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "java failed: {}",
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

fn run_translated(
    translator: &Path,
    work: &Path,
    source: &Path,
    class: &str,
) -> Result<String, String> {
    let translated = Command::new(translator)
        .arg(source)
        .output()
        .map_err(|err| format!("translator: {err}"))?;
    if !translated.status.success() {
        return Err(format!(
            "translation failed: {}",
            String::from_utf8_lossy(&translated.stderr)
        ));
    }
    let mut rust = String::from_utf8_lossy(&translated.stdout).into_owned();
    rust.push_str(&harness_main(&rust, class));

    let rust_path = work.join(format!("{class}.rs"));
    std::fs::write(&rust_path, &rust).map_err(|err| format!("write rust: {err}"))?;

    let binary = work.join(class);
    let compile = Command::new("rustc")
        .args(["--edition", "2021", "--crate-name"])
        .arg(class.to_lowercase())
        .arg(&rust_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .map_err(|err| format!("rustc: {err}"))?;
    if !compile.status.success() {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        let errors = stderr
            .lines()
            .filter(|line| line.starts_with("error"))
            .take(5)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("generated Rust did not compile: {errors}"));
    }
    let run = Command::new(&binary)
        .output()
        .map_err(|err| format!("run: {err}"))?;
    if !run.status.success() {
        return Err(format!(
            "translated program failed: {}",
            String::from_utf8_lossy(&run.stderr).trim_end()
        ));
    }
    Ok(String::from_utf8_lossy(&run.stdout).trim_end().to_string())
}

/// `run()` is lowered as fallible whenever the analysis says it can throw, so
/// the harness has to accept both shapes.
fn harness_main(rust: &str, class: &str) -> String {
    let fallible = rust.contains(&format!("pub fn run() -> Result<"));
    if fallible {
        format!(
            "\nfn main() {{\n    match {class}::run() {{\n        Ok(value) => println!(\"{{value}}\"),\n        Err(err) => println!(\"EXCEPTION:{{}}\", err.class_name()),\n    }}\n}}\n"
        )
    } else {
        format!("\nfn main() {{\n    println!(\"{{}}\", {class}::run());\n}}\n")
    }
}

fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential");
    let mut paths = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "java"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

fn translator_binary() -> Option<PathBuf> {
    if let Some(path) = option_env!("CARGO_BIN_EXE_new-java2rust") {
        return Some(PathBuf::from(path));
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    ["release", "debug"]
        .iter()
        .map(|profile| root.join("target").join(profile).join("new-java2rust"))
        .find(|path| path.exists())
}

fn have_tool(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
