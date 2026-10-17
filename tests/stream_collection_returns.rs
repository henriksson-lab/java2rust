use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn generic_set_parameters_preserve_java_equality_at_concrete_call_sites() {
    let translation = translate_source(
        r#"
import java.util.*;
class Use {
    static <T> boolean has(Set<T> values, T value) { return values.contains(value); }
    static <T> boolean erase(Set<T> values, T value) { return values.remove(value); }
    static String run() {
        Set<String> values = new LinkedHashSet<String>(Arrays.asList("b", "a"));
        boolean found = has(values, "b");
        boolean removed = erase(values, "a");
        return found + ":" + removed + ":" + values.toString();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_set_parameter_boundary",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn concrete_caller_keeps_its_set() { assert_eq!(Use::run(), \"true:true:[b]\"); }",
        }],
    );
}

#[test]
fn empty_generic_set_factories_require_java_keys_without_structural_hashing() {
    let translation = translate_source(
        r#"
import java.util.*;
class Factory<T> {
    Set<T> empty() { return new HashSet<T>(); }
    static <U> Set<U> emptyMethod() { return new HashSet<U>(); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_empty_set_factories",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct Key;
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, _: &Self) -> bool { true }
}
#[test]
fn empty_factories_accept_java_key_only_types() {
    let factory = Factory::<Key>::new();
    assert!(factory.empty().is_empty());
    assert!(Factory::<Key>::empty_method::<Key>().is_empty());
}
"#,
        }],
    );
}

#[test]
fn ordered_collector_from_generic_source_field_is_planned_as_a_stream() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.*;
interface Provider<T> { Set<T> values(); }
class Filter<T> {
    Provider<T> provider;
    Set<T> allowed = new LinkedHashSet<T>();
    Set<T> get() {
        return provider.values().stream().filter(value -> allowed.contains(value))
            .collect(Collectors.toCollection(LinkedHashSet::new));
    }
}
"#,
    );
    assert!(
        !translation.rust.contains("Collectors::"),
        "native collector was left unresolved:\n{}",
        translation.rust
    );
    common::cargo_check_generated_modules(
        "source_field_stream_collection_return",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn ordered_collector_can_return_through_a_set_interface_slot() {
    let translation = translate_source(
        r#"
import java.util.*;
import java.util.stream.*;
class Use {
    static <T> Set<T> gather(List<T> values) {
        return values.stream().collect(Collectors.toCollection(LinkedHashSet::new));
    }
    static String run() {
        return gather(Arrays.asList("b", "a", "b")).toString();
    }
}
"#,
    );
    assert!(
        !translation.rust.contains("Collectors::"),
        "native collector was left unresolved:\n{}",
        translation.rust
    );
    common::cargo_test_generated_modules(
        "stream_set_interface_return",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct Key(i32);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
#[test]
fn generic_set_return_uses_java_keys_and_retains_encounter_order() {
    let result = Use::gather(&[Key(2), Key(1), Key(2)]);
    assert_eq!(result.iter().map(|key| key.0).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(Use::run(), "[b, a]");
}
"#,
        }],
    );
}
