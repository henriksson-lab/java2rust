#[path = "common/mod.rs"]
mod common;

#[test]
fn stream_callbacks_can_bind_instance_collection_fields() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/stream_instance_fields.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "stream_instance_fields",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn field_reference_and_explicit_this_have_the_same_receiver() {
    assert_eq!(FieldCallbacks::probe().unwrap(), 2);
    assert!(FieldCallbacks::matches());
    assert!(!FieldCallbacks::misses());
}
"#,
        }],
    );
}

#[test]
fn fallible_intermediates_survive_production_crate_assembly() {
    common::cargo_run_translated_batch_sources(
        "fallible_stream_pipeline_assembly",
        &[new_java2rust::BatchSource::new(
            "FallibleStreamPipeline.java",
            include_str!("fixtures/fallible_stream_pipeline.java"),
        )],
        r#"fn main() {
            assert!(translated::FallibleStreamPipeline::any(&[0, 2, -1]).unwrap());
            assert!(translated::FallibleStreamPipeline::any(&[0, -1, 2]).is_err());
            assert!(translated::FallibleStreamPipeline::bound(&[0, 2, -1]).unwrap());
            assert_eq!(translated::FallibleStreamPipeline::first(&[0, 2, -1]).unwrap(), 4);
            assert_eq!(translated::FallibleStreamPipeline::non_null(&[0, 1, 0, 2]).unwrap(), 2);
            assert_eq!(translated::FallibleStreamPipeline::non_null_plain(&[0, 1, 0, 2]), 2);
            translated::FallibleStreamPipeline::reset();
            translated::FallibleStreamPipeline::each_block(&[0, 1, 2]).unwrap();
            assert_eq!(translated::FallibleStreamPipeline::consumed(), 6);
            assert!(!translated::FallibleStreamPipeline::mutating_null(&[]).unwrap());
            assert!(translated::FallibleStreamPipeline::mutating_null(&[1]).is_err());
        }"#,
    );
}

#[test]
fn intermediate_callbacks_preserve_laziness_errors_and_payload_shapes() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/fallible_stream_pipeline.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "fallible_stream_pipeline",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn intermediate_errors_stop_before_later_callbacks_or_items() {
    FallibleStreamPipeline::reset();
    assert!(FallibleStreamPipeline::any(&[0, 2, -1]).unwrap());
    assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::mapped()), (2, 1));
    FallibleStreamPipeline::reset();
    assert!(FallibleStreamPipeline::any(&[0, -1, 2]).is_err());
    assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::mapped()), (2, 0));
    FallibleStreamPipeline::reset();
    assert!(FallibleStreamPipeline::any(&[13, 2]).is_err());
    assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::mapped()), (1, 1));
    assert!(!FallibleStreamPipeline::any(&[]).unwrap());
    FallibleStreamPipeline::reset();
    assert!(FallibleStreamPipeline::bound(&[0, 2, -1]).unwrap());
    assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::mapped()), (2, 2));
    assert!(!FallibleStreamPipeline::bound(&[]).unwrap());
    assert!(FallibleStreamPipeline::bound(&[-1, 2]).is_err());
    assert!(FallibleStreamPipeline::all(&[]).unwrap());
    assert!(!FallibleStreamPipeline::all(&[2, -1]).unwrap());
    assert!(FallibleStreamPipeline::all(&[-1, 2]).is_err());
    assert_eq!(FallibleStreamPipeline::sum(&[0, 1, 2]).unwrap(), 6);
    assert!(FallibleStreamPipeline::sum(&[1, -1, 2]).is_err());
    assert_eq!(FallibleStreamPipeline::count(&[0, 1, 2]).unwrap(), 2);
    assert!(FallibleStreamPipeline::count(&[1, -1, 2]).is_err());
    FallibleStreamPipeline::reset();
    assert_eq!(FallibleStreamPipeline::first(&[0, 2, -1]).unwrap(), 4);
    assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::mapped()), (2, 1));
    assert_eq!(FallibleStreamPipeline::first(&[]).unwrap(), -1);
    assert!(FallibleStreamPipeline::first(&[-1, 2]).is_err());
    assert_eq!(FallibleStreamPipeline::collected(&[0, 1, 2]).unwrap(), vec![2, 4]);
    assert!(FallibleStreamPipeline::collected(&[1, -1, 2]).is_err());
    for each in [FallibleStreamPipeline::each, FallibleStreamPipeline::each_block] {
        FallibleStreamPipeline::reset();
        each(&[0, 1, 2]).unwrap();
        assert_eq!(FallibleStreamPipeline::consumed(), 6);
        FallibleStreamPipeline::reset();
        assert!(each(&[1, 13, 2]).is_err());
        assert_eq!((FallibleStreamPipeline::visits(), FallibleStreamPipeline::consumed()), (2, 2));
    }
    assert_eq!(FallibleStreamPipeline::non_null(&[0, 1, 0, 2]).unwrap(), 2);
    assert!(FallibleStreamPipeline::non_null(&[0, -1, 2]).is_err());
    assert_eq!(FallibleStreamPipeline::non_null_plain(&[0, 1, 0, 2]), 2);
    assert_eq!(FallibleStreamPipeline::non_null_plain(&[0, 0]), 0);
    assert!(!FallibleStreamPipeline::caught(&[-1]).unwrap());
    assert!(!FallibleStreamPipeline::mutating_null(&[]).unwrap());
    assert!(FallibleStreamPipeline::mutating_null(&[1]).is_err());
    let retained_set = |values: Vec<i32>| Some(std::rc::Rc::new(std::cell::RefCell::new(
        JavaNativeCollection::set(values, JavaMapKey::java_map_key_eq)
    )));
    assert!(FallibleStreamPipeline::mutating(&[1], retained_set(vec![])).unwrap());
    assert!(!FallibleStreamPipeline::mutating(&[1, 3], retained_set(vec![1, 3])).unwrap());
}
"#,
        }],
    );
}

#[test]
fn nullable_native_captures_are_checked_only_when_the_predicate_runs() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/nullable_stream_capture.java"));
    common::cargo_test_generated_modules(
        "nullable_native_stream_capture",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn captured_nulls_preserve_lazy_evaluation_and_java_exception_boundaries() {
    assert!(!NullableStreamCapture::any_null(&[]).unwrap());
    assert!(NullableStreamCapture::all_null(&[]).unwrap());
    for operation in [NullableStreamCapture::any_null, NullableStreamCapture::all_null] {
        let error = operation(&[0]).unwrap_err();
        assert!(error.is_instance(&["NullPointerException"]));
    }
    let accepted = || Some(std::rc::Rc::new(std::cell::RefCell::new(
        JavaNativeCollection::set(vec![1, 3], JavaMapKey::java_map_key_eq)
    )));
    assert!(NullableStreamCapture::any(&[0, 1], accepted()).unwrap());
    assert!(!NullableStreamCapture::any(&[0, 2], accepted()).unwrap());
    assert!(NullableStreamCapture::all(&[1, 3], accepted()).unwrap());
    assert!(!NullableStreamCapture::all(&[1, 2], accepted()).unwrap());
    NullableStreamCapture::reset();
    assert!(!NullableStreamCapture::ordered_null(&[]).unwrap());
    assert_eq!(NullableStreamCapture::get_calls(), 0);
    assert!(NullableStreamCapture::ordered_null(&[0]).is_err());
    assert_eq!(NullableStreamCapture::get_calls(), 1);
    let argument_error = NullableStreamCapture::argument_failure_null(&[0]).unwrap_err();
    assert!(argument_error.is_instance(&["IllegalArgumentException"]));
    assert!(!NullableStreamCapture::argument_failure_null(&[]).unwrap());
    assert!(!NullableStreamCapture::any_nullable(&[]).unwrap());
    assert!(NullableStreamCapture::all_nullable(&[]).unwrap());
    assert!(NullableStreamCapture::any_nullable(&[None]).is_err());
    assert!(NullableStreamCapture::all_nullable(&[None]).is_err());
    assert!(NullableStreamCapture::any_nullable(&[Some(String::new()), None]).unwrap());
    assert!(!NullableStreamCapture::all_nullable(&[Some("x".to_string()), None]).unwrap());
    assert!(!NullableStreamCapture::caught_nullable(&[None]));
    assert!(!NullableStreamCapture::caught_nullable(&[]));
    assert!(NullableStreamCapture::static_reference(&[None]));
    assert!(!NullableStreamCapture::static_reference(&[]));
    assert!(NullableStreamCapture::shadowed(&[1], accepted()).unwrap());
    assert!(NullableStreamCapture::shadowed_null(&[0]).is_err());
    assert!(NullableStreamCapture::lambda_reference(&[None]));
    assert!(!NullableStreamCapture::lambda_reference(&[]));
    assert!(!NullableStreamCapture::filtered_reference(&[None]));
    assert!(NullableStreamCapture::filtered_reference(&[None, Some(String::new())]));
}
"#,
        }],
    );
}

#[test]
fn stream_callback_matching_propagates_predicate_errors_without_eager_evaluation() {
    let translation = new_java2rust::translate_source(
        r#"
package neutral.callbacks;
import java.util.Arrays;
class GenericEffects<Unused> {
    static boolean positive(int value) { return MatchEffects.positive(value); }
}
class MatchEffects {
    static boolean positive(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value > 0;
    }
    static boolean anyLambda(int[] values) {
        return Arrays.stream(values).anyMatch(value -> positive(value));
    }
    static boolean allLambda(int[] values) {
        return Arrays.stream(values).allMatch(value -> positive(value));
    }
    static boolean anyReference(int[] values) {
        return Arrays.stream(values).anyMatch(MatchEffects::positive);
    }
    static boolean allReference(int[] values) {
        return Arrays.stream(values).allMatch(MatchEffects::positive);
    }
    static boolean anyNested(int[] values) {
        return Arrays.stream(values).anyMatch(value -> positive(value) && value > 0);
    }
    static boolean allQualified(int[] values) {
        return Arrays.stream(values).allMatch(value -> MatchEffects.positive(value) == true);
    }
    static int checkedValue(int value) {
        if (value < 0) throw new IllegalArgumentException("negative");
        return value;
    }
    static boolean anyMapped(int[] values) {
        return Arrays.stream(values).map(value -> checkedValue(value)).anyMatch(value -> value > 0);
    }
    static boolean allMapped(int[] values) {
        return Arrays.stream(values).map(MatchEffects::checkedValue).allMatch(value -> value > 0);
    }
    static boolean anyGenericReference(int[] values) {
        return Arrays.stream(values).anyMatch(GenericEffects::positive);
    }
    static boolean allGenericLambda(int[] values) {
        return Arrays.stream(values).allMatch(value -> GenericEffects.positive(value));
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "fallible_stream_matching",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn predicates_keep_error_and_short_circuit_boundaries() {
    for any in [MatchEffects::any_lambda, MatchEffects::any_reference, MatchEffects::any_nested, MatchEffects::any_mapped, MatchEffects::any_generic_reference] {
        assert!(!any(&[]).unwrap());
        assert!(any(&[0, 1, -1]).unwrap());
        assert!(any(&[0, -1, 1]).is_err());
    }
    for all in [MatchEffects::all_lambda, MatchEffects::all_reference, MatchEffects::all_qualified, MatchEffects::all_mapped, MatchEffects::all_generic_lambda] {
        assert!(all(&[]).unwrap());
        assert!(!all(&[1, 0, -1]).unwrap());
        assert!(all(&[1, -1, 0]).is_err());
    }
}
"#,
        }],
    );
}
