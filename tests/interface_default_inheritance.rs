#[path = "common/mod.rs"]
mod common;

#[test]
fn fallible_void_defaults_share_method_completion_and_exception_rules() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/default_void_fallibility.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "default_void_fallibility",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct Step {
    calls: std::rc::Rc<std::cell::Cell<i32>>,
    fail: bool,
}
impl FallibleStep for Step {
    fn execute(&self) -> Result<(), JavaException> {
        self.calls.set(self.calls.get() + 1);
        if self.fail { Err(JavaException::new("Exception", "step".to_string())) }
        else { Ok(()) }
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn FallibleStep> { Box::new(self.clone()) }
}
#[test]
fn defaults_complete_successfully_and_preserve_exception_paths() {
    for fail in [false, true] {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let step: Box<dyn FallibleStep> = Box::new(Step { calls: calls.clone(), fail });
        assert_eq!(step.run().is_err(), fail);
        assert_eq!(calls.get(), 1);
        assert!(step.guarded(false).is_ok());
        assert_eq!(calls.get(), 1);
        assert_eq!(step.guarded(true).is_err(), fail);
        assert_eq!(calls.get(), 2);
        assert!(step.branches(false).is_err());
        assert_eq!(calls.get(), 2);
        assert_eq!(step.branches(true).is_err(), fail);
        assert_eq!(calls.get(), 3);
        assert!(step.recovered().is_ok());
        assert_eq!(calls.get(), 4);
        assert_eq!(step.__java_default_run().is_err(), fail);
        assert_eq!(step.__java_default_fallible_step_run().is_err(), fail);
        assert_eq!(calls.get(), 6);
    }
}
"#,
        }],
    );
}

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
