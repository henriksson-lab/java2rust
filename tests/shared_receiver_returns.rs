use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn shared_receivers_return_generic_field_values_without_borrowing_a_guard() {
    let translation = translate_source(
        r#"
class ReadOnlyHolder<T> {
    private T value;
    ReadOnlyHolder(T value) { this.value = value; }
    ReadOnlyHolder<T> same() { return this; }
    T get() { return value; }
}
class MutableHolder<T> {
    private T value;
    MutableHolder(T value) { this.value = value; }
    MutableHolder<T> same() { return this; }
    T get() { return value; }
    void set(T value) { this.value = value; }
    void setDistinct(T incoming) { this.value = incoming; }
}
class BaseHolder<T> {
    protected T value;
    T get() { return value; }
}
class DerivedHolder<T> extends BaseHolder<T> {
    DerivedHolder<T> same() { return this; }
    void set(T incoming) { value = incoming; }
}
class TextHolder {
    private String value;
    int setAndLength(String incoming) {
        value = incoming;
        return incoming.length();
    }
    int length() { return value.length(); }
}
class Use {
    static int readOnlyProbe() {
        ReadOnlyHolder<String> first = new ReadOnlyHolder<String>("abc");
        ReadOnlyHolder<String> alias = first.same();
        return alias.get().length();
    }
    static int mutableProbe() {
        MutableHolder<String> first = new MutableHolder<String>("abc");
        MutableHolder<String> alias = first.same();
        alias.set("changed");
        alias.setDistinct("changed");
        return first.get().length();
    }
    static int inheritedProbe() {
        DerivedHolder<String> first = new DerivedHolder<String>();
        DerivedHolder<String> alias = first.same();
        alias.set("inherited");
        return first.get().length();
    }
    static int reusedProbe() {
        TextHolder holder = new TextHolder();
        return holder.setAndLength("keep") + holder.length();
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
        "shared_receiver_returns",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn generic_getters_preserve_values_and_shared_mutation() {
    assert_eq!(Use::read_only_probe().unwrap(), 3);
    assert_eq!(Use::mutable_probe().unwrap(), 7);
    assert_eq!(Use::inherited_probe().unwrap(), 9);
    assert_eq!(Use::reused_probe().unwrap(), 8);
}

#[test]
fn last_use_setters_do_not_require_clone() {
    struct NotClone(i32);
    let mut holder = MutableHolder::new(NotClone(1));
    holder.set(NotClone(2));
    holder.set_distinct(NotClone(3));
    assert_eq!(holder.value.as_ref().unwrap().0, 3);
}
"#,
        }],
    );
}
