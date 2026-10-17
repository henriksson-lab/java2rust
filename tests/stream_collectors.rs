use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn wildcard_imported_ordered_collector_preserves_the_native_owner() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.*;
class Use {
    static String run() {
        LinkedHashSet<String> unique = Arrays.asList("b", "a", "b").stream()
            .collect(Collectors.toCollection(LinkedHashSet::new));
        return unique.toString();
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
        "wildcard_imported_ordered_collector",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn ordered_collection_executes() { assert_eq!(Use::run(), \"[b, a]\"); }",
        }],
    );
}

#[test]
fn linked_set_constructor_collector_preserves_encounter_order_and_owned_elements() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.Collectors;
class Use {
    static <T> LinkedHashSet<T> gather(List<T> values) {
        return values.stream().collect(Collectors.toCollection(LinkedHashSet::new));
    }
    static String borrowed() {
        List<String> values = Arrays.asList("b", "a", "b", "c");
        LinkedHashSet<String> unique = gather(values);
        unique.add("d");
        return unique.toString() + ":" + values.toString();
    }
    static String mapped() {
        LinkedHashSet<String> unique = Arrays.asList("bb", "a", "bb").stream()
            .map(s -> s + "!")
            .collect(java.util.stream.Collectors.toCollection(java.util.LinkedHashSet::new));
        return unique.toString();
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
        "linked_set_stream_collector",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn ordered_collection_executes() {
    assert_eq!(Use::borrowed(), "[b, a, c, d]:[b, a, b, c]");
    assert_eq!(Use::mapped(), "[bb!, a!]");
}
#[derive(Debug)]
struct Key(i32);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, other: &Self) -> bool {
        self.0.abs() == other.0.abs()
    }
}
#[test]
fn ordered_set_reuses_java_key_equality_without_structural_equality() {
    let keys = [Key(-2), Key(1), Key(2)];
    let set: JavaLinkedHashSet<&Key> = keys.iter().collect();
    assert_eq!(set.iter().map(|key| key.0).collect::<Vec<_>>(), vec![-2, 1]);
    let reordered = [Key(-1), Key(2)];
    let other: JavaLinkedHashSet<&Key> = reordered.iter().collect();
    assert_eq!(set, other);
}
"#,
        }],
    );
}

#[test]
fn joining_overloads_preserve_delimiters_and_empty_stream_affixes() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.Collectors;
class Use {
    static String plain() {
        return Arrays.asList("a", "b").stream().collect(Collectors.joining());
    }
    static String separated() {
        return Arrays.asList("a", "b").stream().collect(Collectors.joining(","));
    }
    static String bracketed() {
        return Arrays.asList("a", "b").stream().collect(Collectors.joining(",", "[", "]"));
    }
    static String empty() {
        List<String> values = new ArrayList<String>();
        return values.stream().collect(Collectors.joining(",", "[", "]"));
    }
    static String qualified() {
        return Arrays.asList("a", "b").stream()
            .collect(java.util.stream.Collectors.joining(",", "[", "]"));
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
        "stream_joining_overloads",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn overloads() {
    assert_eq!(Use::plain(), "ab");
    assert_eq!(Use::separated(), "a,b");
    assert_eq!(Use::bracketed(), "[a,b]");
    assert_eq!(Use::empty(), "[]");
    assert_eq!(Use::qualified(), "[a,b]");
}
"#,
        }],
    );
}

#[test]
fn collection_constructor_collectors_produce_mutable_lists() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.Collectors;
class Use {
    static String run() {
        List<String> values = Arrays.asList("a", "", "b").stream()
            .filter(s -> !s.isEmpty()).collect(Collectors.toCollection(ArrayList::new));
        values.add("tail");
        return values.stream().collect(Collectors.joining(","));
    }
    static int setSize() {
        Set<String> values = Arrays.asList("a", "a", "b").stream()
            .collect(Collectors.toCollection(HashSet::new));
        values.add("c");
        return values.size();
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
        "stream_collection_constructor",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn collected_list_is_mutable_and_ordered() {
    assert_eq!(Use::run(), "a,b,tail");
    assert_eq!(Use::set_size(), 3);
}
"#,
        }],
    );
}

#[test]
fn joining_arguments_are_evaluated_before_lazy_mapping() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.Collectors;
class Use {
    private static int order;
    static String mark(int digit, String value) {
        order = order * 10 + digit;
        return value;
    }
    static String run() {
        order = 0;
        String joined = Arrays.asList("x", "y").stream().map(s -> mark(4, s))
            .collect(Collectors.joining(mark(1, ","), mark(2, "["), mark(3, "]")));
        return joined + ":" + order;
    }
    static String single() {
        order = 0;
        String joined = Arrays.asList("x", "y").stream().map(s -> mark(2, s))
            .collect(Collectors.joining(mark(1, ",")));
        return joined + ":" + order;
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
        "stream_joining_evaluation_order",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn collector_arguments_precede_stream_traversal() {
    assert_eq!(Use::run(), "[x,y]:12344");
    assert_eq!(Use::single(), "x,y:122");
}
"#,
        }],
    );
}
