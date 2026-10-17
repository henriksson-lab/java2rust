#[path = "common/mod.rs"]
mod common;

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
