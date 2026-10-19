#[path = "common/mod.rs"]
mod common;

#[test]
fn stored_and_argument_callbacks_preserve_nullable_result_elements() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/stored_nullable_callback_results.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "stored_nullable_callback_results",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn stored_callbacks_observe_nulls_and_post_capture_mutation() {
    assert_eq!(StoredNullableCallbackResults::run().unwrap(), 612);
}
"#,
        }],
    );
}

#[test]
fn shared_interface_adapter_bounds_are_scoped_to_the_selected_contract() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/scoped_adapter_capabilities.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "scoped_adapter_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NotClone;
#[test] fn constructing_and_cloning_a_cursor_does_not_clone_its_payload() {
    let cursor = ScopedCursorFactory::make::<NotClone, NotClone>();
    let _alias = cursor.clone();
    assert_eq!(ScopedCursorFactory::run().unwrap(), 3);
}
"#,
        }],
    );
}

#[test]
fn nullable_constructor_sam_slots_keep_cursor_creation_capabilities() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_sam_constructor.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_sam_constructor",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn nullable_callback_abi_does_not_hide_payload_cursor_demand() {
    assert_eq!(NullableSamConstructor::run().unwrap(), 1);
}
"#,
        }],
    );
}

#[test]
fn inherited_cursor_contracts_use_applied_binders_and_live_storage() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/inherited_iterable_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "inherited_iterable_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NotClone;
#[test] fn inherited_cursor_observes_mutation_and_only_requires_its_payload_binder() {
    assert_eq!(InheritedIterableProbe::run().unwrap(), 2);
    let _rows = SwappedRows::<String, NotClone>::new().rows();
}
"#,
        }],
    );
}

#[test]
fn nullable_captured_elements_survive_native_sam_result_boundaries() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_iterable_supplier.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "nullable_iterable_supplier",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn returned_iterator_preserves_null_and_late_mutation() {
    assert_eq!(NullableIterableSupplier::run().unwrap(), 102);
}
"#,
        }],
    );
}

#[test]
fn iterable_return_boundaries_propagate_cursor_payload_capabilities() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/iterable_return_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "iterable_return_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn returned_factories_keep_live_shared_allocations() {
    assert_eq!(IterableReturnProbe::run().unwrap(), 14);
}
"#,
        }],
    );
}

#[test]
fn nullable_stream_destination_does_not_change_non_null_source_elements() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class DirectedStreamCopy {
    static void copy(List<String> source, List<String> target) {
        target.add(null);
        source.stream().forEach(target::add);
    }
    static int run() {
        List<String> source = new ArrayList<>();
        source.add("x");
        List<String> target = new ArrayList<>();
        copy(source, target);
        return source.size() * 10 + target.size();
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
        "directed_stream_copy",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn copied_elements_keep_the_source_domain() {
    assert_eq!(DirectedStreamCopy::run().unwrap(), 12);
}
"#,
        }],
    );
}

#[test]
fn native_stream_without_explicit_iterable_or_iterator_types_runs() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class NativeOnlyStreams {
    List<Integer> values = new ArrayList<>();
    void pin(List<List<Integer>> target) { target.add(values); }
    long count() { return values.stream().count(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "native_only_stream",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn checked_count_runs_without_explicit_iterator_types() {
    assert_eq!(NativeOnlyStreams::new().count().unwrap(), 0);
}
"#,
        }],
    );
}

#[test]
fn generic_stream_null_tests_and_identity_casts_preserve_typed_payloads() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/stream_generic_values.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "generic_stream_values",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn null_and_non_null_payloads_follow_java() {
    assert_eq!(GenericStreamProbe::run().unwrap(), 1112);
}
"#,
        }],
    );
}

#[test]
fn retained_streams_survive_shared_runtime_assembly() {
    common::cargo_run_translated_batch_sources(
        "retained_stream_assembly",
        &[
            new_java2rust::BatchSource::new(
                "RetainedStreams.java",
                include_str!("fixtures/retained_streams.java"),
            ),
            new_java2rust::BatchSource::new(
                "RetainedStreamPeer.java",
                r#"
import java.util.*;
class RetainedStreamPeer {
    static int run() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(4);
        return (int) values.stream().count();
    }
}
"#,
            ),
            new_java2rust::BatchSource::new(
                "GenericStreamValues.java",
                include_str!("fixtures/stream_generic_values.java"),
            ),
        ],
        r#"fn main() {
            assert_eq!(translated::RetainedStreams::matches().unwrap(), 11);
            assert_eq!(translated::RetainedStreams::mapped().unwrap(), 25);
            assert_eq!(translated::RetainedStreams::for_each().unwrap(), 3);
            assert_eq!(translated::RetainedStreams::fail_fast().unwrap(), 7);
            assert_eq!(translated::RetainedStreams::nested_reference().unwrap(), 2);
            assert_eq!(translated::RetainedStreams::nullable_source(false).unwrap(), 1);
            assert_eq!(translated::RetainedStreams::nullable_source(true).unwrap(), 9);
            assert_eq!(translated::RetainedStreams::nested_sizes().unwrap(), 2);
            assert_eq!(translated::RetainedStreams::nullable_parameter_call(false).unwrap(), 2);
            assert_eq!(translated::RetainedStreams::nullable_parameter_call(true).unwrap(), 9);
            assert_eq!(translated::RetainedStreamPeer::run().unwrap(), 1);
            assert_eq!(translated::GenericStreamProbe::run().unwrap(), 1112);
        }"#,
    );
}

#[test]
fn constructor_only_stream_use_emits_its_runtime_dependencies() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class ConstructorStreams {
    ConstructorStreams(List<Integer> values) {
        Iterable<Integer> alias = values;
        values.stream().count();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_check_generated_modules(
        "constructor_stream_dependencies",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn retained_streams_use_checked_owned_traversal_and_physical_child_layout() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/retained_streams.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "retained_streams",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
trait ProbeLong { fn long_value(self) -> i64; }
impl ProbeLong for i64 { fn long_value(self) -> i64 { self } }
impl<E: std::fmt::Debug> ProbeLong for Result<i64, E> { fn long_value(self) -> i64 { self.unwrap() } }
#[test] fn stream_results_match_java() {
    assert_eq!(RetainedStreams::matches().value(), 11);
    assert_eq!(RetainedStreams::mapped().value(), 25);
    assert_eq!(RetainedStreams::for_each().value(), 3);
    assert_eq!(RetainedStreams::fail_fast().value(), 7);
    assert_eq!(RetainedStreams::nested_reference().value(), 2);
    assert_eq!(RetainedStreams::nullable_source(false).value(), 1);
    assert_eq!(RetainedStreams::nullable_source(true).value(), 9);
    assert_eq!(RetainedStreams::nested_sizes().value(), 2);
    assert_eq!(RetainedStreams::nullable_parameter_call(false).value(), 2);
    assert_eq!(RetainedStreams::nullable_parameter_call(true).value(), 9);
    struct NonClone;
    assert_eq!(GenericRowStreams::<NonClone>::new().count().long_value(), 0);
}
"#,
        }],
    );
}
