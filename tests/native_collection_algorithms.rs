#[path = "common/mod.rs"]
mod common;

#[test]
fn collections_reverse_mutates_the_existing_list_including_parameter_calls() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class Use {
    static void flip(List<String> values) { java.util.Collections.reverse(values); }
    static String run() {
        List<String> values = new ArrayList<String>(Arrays.asList("a", "b", "c"));
        Collections.reverse(values);
        String first = values.toString();
        flip(values);
        return first + ":" + values.toString();
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
        "collections_reverse",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn reverse_is_in_place() { assert_eq!(Use::run(), \"[c, b, a]:[a, b, c]\"); }",
        }],
    );
}

#[test]
fn source_collections_name_does_not_become_a_native_mutation() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.List;
import java.util.ArrayList;
import java.util.Arrays;
class Collections {
    static int calls;
    static void reverse(List<String> values) { calls++; }
}
class Use {
    static int run() {
        List<String> values = new ArrayList<String>(Arrays.asList("a", "b"));
        Collections.reverse(values);
        return Collections.calls;
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
        "source_collections_reverse",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn source_owner_keeps_its_behavior() { assert_eq!(Use::run(), 1); }",
        }],
    );
}

#[test]
fn collections_reverse_respects_unmodifiable_views_and_trivial_sizes() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
class Use {
    static int run() {
        List<String> values = Collections.unmodifiableList(Arrays.asList("a", "b"));
        try { Collections.reverse(values); return 0; }
        catch (UnsupportedOperationException ex) { return 1; }
    }
    static String trivial() {
        List<String> values = Collections.unmodifiableList(Arrays.asList("a"));
        Collections.reverse(values);
        return values.toString();
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
        "unmodifiable_collections_reverse",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn reverse_throws_only_when_a_swap_is_required() { assert_eq!(Use::run(), 1); assert_eq!(Use::trivial(), \"[a]\"); }",
        }],
    );
}
