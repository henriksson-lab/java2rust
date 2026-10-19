#[path = "common/mod.rs"]
mod common;

#[test]
fn isolated_list_and_extrema_consumers_include_their_runtime_support() {
    let sources = [
        (
            "list_only",
            r#"
import java.util.ArrayList;
import java.util.Comparator;
class IsolatedOrdering {
    static int key(int value) { return value; }
    static int check() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(1);
        values.sort(Comparator.comparingInt(IsolatedOrdering::key));
        return values.get(0);
    }
}
"#,
        ),
        (
            "extrema_only",
            r#"
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
class IsolatedOrdering {
    static int key(int value) { return value; }
    static int check() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(1);
        return Collections.min(values, Comparator.comparingInt(IsolatedOrdering::key));
    }
}
"#,
        ),
        (
            "set_extrema",
            r#"
import java.util.HashSet;
import java.util.Collections;
import java.util.Comparator;
class IsolatedOrdering {
    static int key(int value) { return value; }
    static int check() {
        HashSet<Integer> values = new HashSet<>();
        values.add(3);
        values.add(1);
        return Collections.min(values, Comparator.comparingInt(IsolatedOrdering::key));
    }
}
"#,
        ),
        (
            "borrowed_list_sort",
            r#"
import java.util.ArrayList;
import java.util.Comparator;
class IsolatedOrdering {
    static int key(int value) { return value; }
    static void sort(ArrayList<Integer> values, Comparator<Integer> order) {
        values.sort(order);
    }
    static int check() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(1);
        sort(values, Comparator.comparingInt(IsolatedOrdering::key));
        return values.get(0);
    }
}
"#,
        ),
        (
            "source_comparator",
            r#"
import java.util.ArrayList;
import java.util.Comparator;
class IntegerOrdering implements Comparator<Integer> {
    public int compare(Integer left, Integer right) {
        if (left < 0 || right < 0) throw new IllegalArgumentException("comparison");
        return left - right;
    }
}
class IsolatedOrdering {
    static void applyOrder(ArrayList<Integer> values) {
        values.sort(new IntegerOrdering());
    }
    static int check() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(-1);
        try {
            applyOrder(values);
            return -1;
        } catch (IllegalArgumentException error) {
            return 1;
        }
    }
}
"#,
        ),
        (
            "inline_lambda",
            r#"
import java.util.ArrayList;
import java.util.Collections;
class IsolatedOrdering {
    static int check() {
        ArrayList<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(1);
        try {
            return Collections.max(values, (left, right) -> {
                throw new IllegalArgumentException("comparison");
            });
        } catch (IllegalArgumentException error) {
            return 1;
        }
    }
}
"#,
        ),
    ];
    for (label, source) in sources {
        let translation = new_java2rust::translate_source(source);
        assert!(
            !translation.diagnostics.has_errors(),
            "{:?}",
            translation.diagnostics
        );
        let tests = r#"
trait JavaIntOutcome { fn value(self) -> i32; }
impl JavaIntOutcome for i32 { fn value(self) -> i32 { self } }
impl JavaIntOutcome for Result<i32, JavaException> {
    fn value(self) -> i32 { self.expect("unexpected uncaught Java exception") }
}
#[test]
fn isolated_consumer_matches_java() {
    assert_eq!(IsolatedOrdering::check().value(), 1);
}
"#;
        common::cargo_test_generated_modules(
            label,
            vec![common::GeneratedCrateModule {
                name: "fixture",
                rust: translation.rust,
                tests,
            }],
        );
    }
}

#[test]
fn source_ordering_method_names_do_not_select_native_consumers() {
    let translation = new_java2rust::translate_source(
        r#"
class Collections {
    static int min(int left, int right) { return left + right; }
    static int max(int left, int right) { return left - right; }
    static int sort(int left, int right) { return left * right; }
}
class List {
    int sort(int value) { return value + 5; }
}
class SourceOrdering {
    static int check() {
        List list = new List();
        return Collections.min(2, 3) + Collections.max(9, 4)
            + Collections.sort(2, 3) + list.sort(1);
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
        "source_ordering",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_methods_keep_their_meaning() {
    assert_eq!(SourceOrdering::check(), 22);
}
"#,
        }],
    );
}

#[test]
fn native_comparator_consumers_preserve_order_stability_and_checked_errors() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/ordering_consumers.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "ordering_consumers",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
// This behavior oracle accepts either solved effect ABI, but never hides an
// uncaught Java exception. Separate callback-contract tests check ABI identity.
trait JavaIntOutcome { fn value(self) -> i32; }
impl JavaIntOutcome for i32 { fn value(self) -> i32 { self } }
impl JavaIntOutcome for Result<i32, JavaException> {
    fn value(self) -> i32 { self.expect("unexpected uncaught Java exception") }
}
#[test]
fn comparator_consumers_match_java() {
    assert_eq!(OrderingConsumers::list_sorted().value(), 11122122);
    assert_eq!(OrderingConsumers::collections_sorted().value(), 11122122);
    assert_eq!(OrderingConsumers::extrema().value(), 2111);
    assert_eq!(OrderingConsumers::caught_sort().value(), 9);
    assert_eq!(OrderingConsumers::caught_collections_sort().value(), 9);
    assert_eq!(OrderingConsumers::caught_maximum().value(), 9);
    assert_eq!(OrderingConsumers::empty_minimum().value(), 7);
}
"#,
        }],
    );
}
