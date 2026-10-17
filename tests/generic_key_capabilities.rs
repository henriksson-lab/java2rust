use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn receiver_only_key_requirements_reach_callers_and_interface_forwarders() {
    let translated = translate_source(
        r#"
import java.util.*;
interface CountingJob<Data, Key> { int calculate(); }
class ShadowedField<Key> {
    Key key;
    Key consume(Key key) { return key; }
    Key throughArgument(Key key) { return consume(key); }
    void consumeVoid(Key key) {}
    void throughVoid(Key key) { consumeVoid(key); }
}
class KeySetBuilder<Key> {
    Key key;
    KeySetBuilder(Key key) { this.key = key; }
    int count() {
        Set<Key> keys = new HashSet<Key>();
        keys.add(key);
        return keys.size();
    }
}
class LocalKeyJob<Data, Key> implements CountingJob<Data, Key> {
    Key key;
    LocalKeyJob(Key key) { this.key = key; }
    public int calculate() { return new KeySetBuilder<Key>(key).count(); }
    int indirect() { return calculate(); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "receiver_only_key_requirements",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
struct NonKeyPayload;
fn copy_shadowed<P: Clone + 'static>(owner: &ShadowedField<P>, key: &P) {
    let _ = owner.through_argument(key);
}
fn move_shadowed<P: 'static>(owner: &ShadowedField<P>, key: &P) {
    owner.through_void(key);
}
#[derive(Clone)]
struct OnlyJavaKey;
impl JavaMapKey for OnlyJavaKey {
    fn java_map_key_eq(&self, _: &Self) -> bool { true }
}
fn boxed_job<P: 'static, K: Clone + JavaMapKey + 'static>(value: LocalKeyJob<P, K>) -> Box<dyn CountingJob<P, K>> { Box::new(value) }
#[test]
fn key_capability_is_forwarded_without_constraining_payload() {
    let _: fn(&ShadowedField<NonKeyPayload>, &NonKeyPayload) = move_shadowed::<NonKeyPayload>;
    let _: fn(&ShadowedField<OnlyJavaKey>, &OnlyJavaKey) = copy_shadowed::<OnlyJavaKey>;
    let mut job = LocalKeyJob::<NonKeyPayload, OnlyJavaKey>::new(OnlyJavaKey);
    assert_eq!(job.indirect(), 1);
    let mut boxed = boxed_job(job);
    assert_eq!(boxed.calculate(), 1);
}
"#,
        }],
    );
}

#[test]
fn receiver_class_key_capabilities_flow_through_generic_wrappers() {
    let translated = translate_source(
        r#"
import java.util.*;
class Payload {}
class Table<Value, Key> {
    int remember(Key key, Value payload) {
        Set<Key> seen = new HashSet<Key>();
        seen.add(key);
        return seen.size();
    }
    int indirect(Key key, Value payload) { return remember(key, payload); }
}
class Use {
    static <Data, Item> int relay(Table<Data, Item> table, Item key, Data payload) {
        return table.indirect(key, payload);
    }
    static int run() { return relay(new Table<Payload, String>(), "key", new Payload()); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "receiver_class_key_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: "#[test] fn receiver_capability_is_preserved() { assert_eq!(Use::run(), 1); }",
        }],
    );
}

#[test]
fn generic_wrappers_forward_key_capabilities_without_constraining_payloads() {
    let translated = translate_source(
        r#"
import java.util.*;
class Use {
    static <Key, Payload> int direct(Key key, Payload payload) {
        Set<Key> keys = new HashSet<Key>();
        keys.add(key);
        return keys.size();
    }
    static <Element, Value> int relay(Element key, Value payload) {
        return direct(key, payload);
    }
    static <Item, Data> int outer(Item key, Data payload) {
        return relay(key, payload);
    }
    static int run() { return outer("key", 17); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_key_capability_forwarding",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
struct OnlyJavaKey;
impl JavaMapKey for OnlyJavaKey {
    fn java_map_key_eq(&self, _: &Self) -> bool { true }
}
struct NonKeyPayload;
#[test]
fn generic_calls_preserve_behavior() {
    assert_eq!(Use::run(), 1);
    assert_eq!(Use::outer(OnlyJavaKey, &NonKeyPayload), 1);
}
"#,
        }],
    );
}
