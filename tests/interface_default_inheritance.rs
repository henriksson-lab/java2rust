#[path = "common/mod.rs"]
mod common;

#[test]
fn nested_generic_defaults_resolve_inherited_calls_in_their_lexical_owner() {
    let translation = new_java2rust::translate_source(
        r#"
class Cursors {
    static class Cell<E> {
        E value;
        Cell(E value) { this.value = value; }
        E getValue() { return value; }
    }
    interface Cursor<E> {
        Cell<E> nextCell();
        default E next() { return nextCell().getValue(); }
    }
    interface Bidirectional<E> extends Cursor<E> {
        Cell<E> previousCell();
        default E next() { return nextCell().getValue(); }
        default E previous() { return previousCell().getValue(); }
    }
    static class Words implements Bidirectional<String> {
        public Cell<String> nextCell() { return new Cell<String>("next"); }
        public Cell<String> previousCell() { return new Cell<String>("previous"); }
    }
    static int run() {
        Bidirectional<String> words = new Words();
        return words.next().length() + words.previous().length();
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
        "nested_generic_default_cursor",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn nested_inherited_default_calls_execute() { assert_eq!(Cursors::run(), 12); }",
        }],
    );
}

#[test]
fn generic_interface_with_default_preserves_nullable_return_contract() {
    let translation = new_java2rust::translate_source(
        r#"
interface Maybe<T> {
    T read();
    default T copy() { T local = read(); return local; }
    default int marker() { return 1; }
}
class Empty implements Maybe<String> {
    public String read() { return null; }
}
class Use {
    static int run() {
        Maybe<String> value = new Empty();
        return value.read() == null && value.copy() == null ? value.marker() : 0;
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
        "default_interface_nullable_generic_return",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn nullable_virtual_result_executes() { assert_eq!(Use::run(), 1); }",
        }],
    );
}

#[test]
fn inherited_defaults_specialize_bodies_and_preserve_most_specific_dispatch() {
    let translation = new_java2rust::translate_source(
        r#"
interface Parent<T> {
    T read();
    default T copy() { T local = read(); return local; }
    default int marker() { return 2; }
}
interface Child<X> extends Parent<X> { default int marker() { return Parent.super.marker() + 7; } }
interface TextChild extends Child<String> {}
class Value implements TextChild { public String read() { return "four"; } }
class Use {
    static int run() {
        TextChild value = new Value();
        Parent<String> parent = value;
        return value.copy().length() + parent.marker();
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
        "inherited_default_specialization",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn inherited_defaults_and_parent_upcast_execute() { assert_eq!(Use::run(), 13); }",
        }],
    );
}

#[test]
fn generic_child_default_and_inherited_contracts_share_one_trait_surface() {
    let translation = new_java2rust::translate_source(
        r#"
interface Parent<T> { int measure(T value); }
interface Child<T, U> extends Parent<T> { default int extra() { return 3; } }
class Concrete implements Child<String, String> {
    public int measure(String value) { return value.length(); }
}
class Use {
    static int run() {
        Child<String, String> child = new Concrete();
        return child.measure("four") + child.extra();
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
        "interface_default_inheritance",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn inherited_and_default_dispatch_execute() { assert_eq!(Use::run(), 7); }",
        }],
    );
}
