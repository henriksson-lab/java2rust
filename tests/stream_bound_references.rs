use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn bound_receivers_use_instance_calls_and_primitive_stream_widths() {
    let translation = translate_source(
        r#"
package neutral;
import java.util.*;
class Measure {
    int score(Object value) { return 1000; }
    int score(String value) { return value.length(); }
    double weight(String value) { return value.length() * 0.25; }
    long large(String value) { return 3000000000L + value.length(); }
    boolean accept(String value) { return !value.isEmpty(); }
    double checkedWeight(String value) {
        if (value.isEmpty()) throw new IllegalArgumentException("empty");
        return value.length() * 0.25;
    }
}

class Use {
    static int integers() {
        Measure measure = new Measure();
        return Arrays.asList("ab", "", "c").stream()
            .filter(measure::accept).mapToInt(measure::score).sum();
    }
    static double doubles() {
        Measure measure = new Measure();
        return Arrays.asList("ab", "c").stream().mapToDouble(measure::weight).sum();
    }
    static long longs() {
        Measure measure = new Measure();
        return Arrays.asList("ab", "c").stream().mapToLong(measure::large).sum();
    }
    static int nativeReceiver() {
        String expected = "x";
        return Arrays.asList("x", "y", "x").stream().filter(expected::equals)
            .mapToInt(String::length).sum();
    }
    static int collision() {
        Measure value = new Measure();
        return Arrays.asList("ab", "c").stream().mapToInt(value::score).sum();
    }
    static long widening() {
        return Arrays.asList("ab", "c").stream().mapToLong(String::length).sum();
    }
    static int wrapping() {
        return Arrays.asList("a", "b").stream().mapToInt(s -> 2147483647).sum();
    }
    static double exceptional() {
        Measure measure = new Measure();
        try {
            return Arrays.asList("ab", "", "c").stream().mapToDouble(measure::checkedWeight).sum();
        } catch (IllegalArgumentException ex) {
            return -1.0;
        }
    }
    static int nullReceiverBeforeEmptyTraversal() {
        String expected = null;
        List<String> values = new ArrayList<String>();
        try {
            return (int) values.stream().filter(expected::equals).count();
        } catch (NullPointerException ex) {
            return 7;
        }
    }
    static int nullReceiverBeforeEmptyTerminal() {
        String expected = null;
        List<String> values = new ArrayList<String>();
        try {
            return values.stream().anyMatch(expected::equals) ? 0 : 1;
        } catch (NullPointerException ex) {
            return 7;
        }
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
        "stream_bound_primitive_references",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn bound_calls_and_numeric_terminals() {
    assert_eq!(Use::integers(), 3);
    assert!((Use::doubles() - 0.75).abs() < 1e-12);
    assert_eq!(Use::longs(), 6000000003);
    assert_eq!(Use::native_receiver(), 2);
    assert_eq!(Use::collision(), 3);
    assert_eq!(Use::widening(), 3);
    assert_eq!(Use::wrapping(), -2);
    // Consumed callbacks now participate in the conservative fallibility
    // graph. Still require the Java catch to handle the predicate exception.
    assert_eq!(Use::exceptional().unwrap(), -1.0);
    assert_eq!(Use::null_receiver_before_empty_traversal(), 7);
    assert_eq!(Use::null_receiver_before_empty_terminal(), 7);
}
"#,
        }],
    );
}

#[test]
fn bound_generic_source_interface_calls_keep_their_argument_and_result_abi() {
    let translation = translate_source(
        r#"
import java.util.*;
interface Scorer<T> { double weight(T value); }
class TextScorer implements Scorer<String> {
    public double weight(String value) { return value.length() * 0.25; }
}
class Use {
    static double run(Scorer<String> scorer) {
        return Arrays.asList("ab", "c").stream().mapToDouble(scorer::weight).sum();
    }
    static double probe() { return run(new TextScorer()); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "stream_bound_generic_interface",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn interface_dispatch() { assert!((Use::probe() - 0.75).abs() < 1e-12); }
"#,
        }],
    );
}
