use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn primitive_extrema_keep_width_and_optional_empty_semantics() {
    let translation = translate_source(
        r#"
import java.util.*;
class Use {
    static int maximum() {
        return Arrays.asList("a", "abcd", "xy").stream().mapToInt(String::length).max().getAsInt();
    }
    static int minimum() {
        OptionalInt value = Arrays.asList("a", "abcd", "xy").stream().mapToInt(String::length).min();
        return value.orElse(9);
    }
    static long large() {
        return Arrays.stream(new long[] { 3000000000L, 7000000000L }).max().getAsLong();
    }
    static long mappedLong() {
        return Arrays.stream(new long[] { 4L, 9L }).filter(n -> n > 0L).map(n -> 1).max().getAsLong();
    }
    static double fractional() {
        return Arrays.stream(new double[] { 0.25, -0.75, 0.5 }).min().getAsDouble();
    }
    static int emptyDefault() {
        List<String> values = new ArrayList<String>();
        return values.stream().mapToInt(String::length).max().orElse(7);
    }
    static boolean emptyPresent() {
        List<String> values = new ArrayList<String>();
        return values.stream().mapToInt(String::length).min().isPresent();
    }
    static int emptyThrows() {
        List<String> values = new ArrayList<String>();
        try { return values.stream().mapToInt(String::length).max().getAsInt(); }
        catch (NoSuchElementException ex) { return 11; }
    }
    static int emptyIdentity() {
        List<String> values = new ArrayList<String>();
        OptionalInt maximum = values.stream().mapToInt(String::length).max();
        try { return Objects.requireNonNull(maximum).orElse(7); }
        catch (NullPointerException ex) { return -1; }
    }
    static double negativeZero() {
        return Arrays.stream(new double[] { 0.0, -0.0 }).min().getAsDouble();
    }
    static double positiveZero() {
        return Arrays.stream(new double[] { -0.0, 0.0 }).max().getAsDouble();
    }
    static double nanMinimum() {
        return Arrays.stream(new double[] { 1.0, 0.0 / 0.0, 2.0 }).min().getAsDouble();
    }
    static double nanMaximum() {
        return Arrays.stream(new double[] { 1.0, 0.0 / 0.0, 2.0 }).max().getAsDouble();
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "stream_primitive_extrema",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn extrema_and_empty_results() {
    assert_eq!(Use::maximum(), 4);
    assert_eq!(Use::minimum(), 1);
    assert_eq!(Use::large(), 7000000000);
    assert_eq!(Use::mapped_long(), 1);
    assert_eq!(Use::fractional(), -0.75);
    assert_eq!(Use::empty_default(), 7);
    assert!(!Use::empty_present());
    assert_eq!(Use::empty_throws(), 11);
    assert_eq!(Use::empty_identity(), 7);
    assert_eq!(Use::negative_zero().to_bits(), 1u64 << 63);
    assert_eq!(Use::positive_zero().to_bits(), 0);
    assert!(Use::nan_minimum().is_nan());
    assert!(Use::nan_maximum().is_nan());
}
"#,
        }],
    );
}
