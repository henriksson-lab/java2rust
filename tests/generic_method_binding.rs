use new_java2rust::{
    java_ir::{Expr, JavaType, Member, ResolvedCall, Stmt},
    parser::parse_java_source,
    resolve::resolve_program,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

fn returned_call(source: &str) -> ResolvedCall {
    let parsed = parse_java_source(source);
    assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected resolved return call");
    };
    call.clone()
}

#[test]
fn source_owner_contract_shadowed_method_binder_beside_a_reified_wildcard() {
    let translation = new_java2rust::translate_source(
        r#"
interface Reader<Key, Unused> { int read(Key value); }
class TextReader implements Reader<String, Integer> {
    public int read(String value) { return value.length(); }
}
class Factory<Key, Unused> {
    static <Key> int count(Reader<Key, ?> reader, Key value) {
        return reader.read(value);
    }
}
class Bridge {
    static <Key, Payload> int invoke(Reader<Key, Payload> reader, Key value) {
        return Factory.count(reader, value);
    }
    static <LocalKey, LocalPayload> int relay(Reader<LocalKey, LocalPayload> reader, LocalKey value) {
        return invoke(reader, value);
    }
    static int run() { return relay(new TextReader(), "abc"); }
}
"#,
    );
    cargo_test_generated_modules(
        "shadowed_binder_next_to_wildcard",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct BorrowedTextReader;
impl<'a> Reader<&'a str, i32> for BorrowedTextReader {
    fn read(&self, value: &'a str) -> i32 { value.len() as i32 }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn Reader<&'a str, i32>> { Box::new(self.clone()) }
}
fn read_borrowed<'a>(text: &'a str) -> i32 {
    // Only the captured wildcard payload needs Any. Its ordinary sibling
    // must remain usable with a non-static borrow through both callers.
    Bridge::relay(&BorrowedTextReader, text)
}
#[test]
fn caller_frame_does_not_leak_the_callee_binder() {
    assert_eq!(Bridge::run(), 3);
    let text = "borrowed".to_string();
    assert_eq!(read_borrowed(&text), 8);
}
"#,
        }],
    );
}

#[test]
fn inherited_call_binds_the_receiver_frame_after_argument_reordering() {
    let call = returned_call(
        r#"
class Parent<Key, Value> { Key first(Key value) { return value; } }
class Flipped<A, B> extends Parent<B, A> {}
class Use { String make(Flipped<Integer, String> value) { return value.first("ok"); } }
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn inherited_argument_substitution_is_simultaneous_when_binder_names_repeat() {
    let call = returned_call(
        r#"
class Parent<A, B> { B second(B value) { return value; } }
class Flipped<A, B> extends Parent<B, A> {}
class Use { Integer make(Flipped<Integer, String> value) { return value.second(1); } }
"#,
    );
    let integer = JavaType::Named {
        name: "Integer".to_string(),
        args: vec![],
    };
    assert_eq!(call.return_type, integer);
    assert_eq!(call.arg_target_types, vec![integer]);
}

#[test]
fn inherited_repeated_frames_compose_without_stale_ancestor_aliases() {
    let call = returned_call(
        r#"
class Parent<A, B> { A first(A value) { return value; } }
class Middle<A, B> extends Parent<B, A> {}
class Leaf<A, B> extends Middle<B, A> {}
class Use { Integer make(Leaf<Integer, String> value) { return value.first(1); } }
"#,
    );
    let integer = JavaType::Named {
        name: "Integer".to_string(),
        args: vec![],
    };
    assert_eq!(call.return_type, integer);
    assert_eq!(call.arg_target_types, vec![integer]);
}

#[test]
fn inherited_method_binder_shadows_its_declaring_owner_binder() {
    let call = returned_call(
        r#"
class Parent<T> { <T> T echo(T value) { return value; } }
class Child extends Parent<Integer> {}
class Use { String make(Child value) { return value.echo("ok"); } }
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn inherited_method_bounds_are_specialized_without_erasing_the_constraint() {
    for (actual, allowed) in [("Lower", true), ("Other", false)] {
        let source = format!(
            r#"
class Upper {{}}
class Lower extends Upper {{}}
class Other {{}}
class Parent<Value> {{ <Item extends Value> Item retain(Item value) {{ return value; }} }}
class Child<A> extends Parent<A> {{}}
class Use {{ Object make(Child<Upper> child) {{ return child.retain(new {actual}()); }} }}
"#
        );
        let parsed = parse_java_source(&source);
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert_eq!(
            !resolved.diagnostics.has_errors(),
            allowed,
            "{actual}: {:?}",
            resolved.diagnostics
        );
    }
}

#[test]
fn inherited_reordered_calls_execute_with_the_declared_java_types() {
    let translated = new_java2rust::translate_source(
        r#"
class Parent<A, B> {
    A first(A value) { return value; }
    B second(B value) { return value; }
}
class Flipped<A, B> extends Parent<B, A> {}
class Use {
    static String classify(Integer value) { return "integer"; }
    static String classify(String value) { return "string"; }
    static String run() {
        Flipped<Integer, String> value = new Flipped<Integer, String>();
        return value.first("ok") + ":" + classify(value.second(7));
    }
}
"#,
    );
    cargo_test_generated_modules(
        "inherited_reordered_call",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: "#[test] fn inherited_call_keeps_types() { assert_eq!(Use::run(), \"ok:integer\"); }",
        }],
    );
}

#[test]
fn method_inference_projects_all_generic_arguments_through_source_interfaces() {
    let call = returned_call(
        r#"
interface Pair<A, B> {}
interface Flipped<X, Y> extends Pair<Y, X> {}
class Value implements Flipped<Integer, String> {}
class Use {
    static <A, B> A first(Pair<A, B> value) { return null; }
    String make() { return first(new Value()); }
}
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(
        call.arg_target_types,
        vec![JavaType::Named {
            name: "Pair".to_string(),
            args: vec![
                JavaType::String,
                JavaType::Named {
                    name: "Integer".to_string(),
                    args: vec![]
                }
            ],
        }]
    );
}

#[test]
fn method_binder_shadows_receiver_binder_during_overload_selection() {
    let call = returned_call(
        r#"
class Owner<T> {
    <T> T identity(T value) { return value; }
    long identity(long value) { return value; }
}
class Use { String make(Owner<Integer> owner) { return owner.identity("text"); } }
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn receiver_substitution_does_not_overwrite_inferred_method_result() {
    let call = returned_call(
        r#"
class Owner<T> { <T> T identity(T value) { return value; } }
class Use { String make(Owner<Integer> owner) { return owner.identity("text"); } }
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn a_static_generic_factory_keeps_its_own_binders() {
    let call = returned_call(
        r#"
import java.util.function.Supplier;
class Factory<V, E> {
    static <V, E> Factory<V, E> create(Class<? extends E> edgeClass) { return null; }
    static <V, E> Factory<V, E> create(Supplier<E> supplier) { return null; }
}
class Use<V, E> {
    Supplier<E> supplier;
    Factory<V, E> make() { return Factory.<V, E>create(supplier); }
}
"#,
    );
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "Supplier" || name.ends_with(".Supplier")),
        "{call:?}"
    );
}

#[test]
fn overload_applicability_infers_declared_mixed_case_method_parameters() {
    let call = returned_call(
        r#"
class Owner<T> {
    <Element> Element identity(Element value) { return value; }
    long identity(long value) { return value; }
}
class Use { String make(Owner<Integer> owner) { return owner.identity("text"); } }
"#,
    );
    assert_eq!(call.return_type, JavaType::String);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn factory_overloads_infer_their_declared_payload_parameter() {
    let call = returned_call(
        r#"
import java.util.function.Supplier;
class Factory<T> {
    static <Element> Factory<Element> create(Class<? extends Element> type) { return null; }
    static <Element> Factory<Element> create(Supplier<Element> supplier) { return null; }
}
class Use<E> {
    Supplier<E> supplier;
    Factory<E> make() { return Factory.create(supplier); }
}
"#,
    );
    assert!(
        matches!(&call.return_type, JavaType::Named { name, args }
        if name == "Factory" && args == &vec![JavaType::Named { name: "E".into(), args: vec![] }]),
        "{call:?}"
    );
}

#[test]
fn untyped_lambda_is_not_concrete_generic_inference_evidence() {
    let call = returned_call(
        r#"
interface Action { int apply(String value); }
class Owner<T> {
    <Element> String choose(Element value) { return "generic"; }
    String choose(Action value) { return "action"; }
}
class Use { String make(Owner<Integer> owner) { return owner.choose(value -> 1); } }
"#,
    );
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "Action"),
        "{call:?}"
    );
}

#[test]
fn an_exact_non_generic_overload_keeps_precedence() {
    let call = returned_call(
        r#"
class Owner<T> {
    <Element> Element identity(Element value) { return value; }
    String identity(String value) { return "specialized"; }
}
class Use { String make(Owner<Integer> owner) { return owner.identity("text"); } }
"#,
    );
    assert_eq!(call.rust_name, "identity_string");
}

#[test]
fn translated_shadowed_generic_method_executes_with_its_own_type() {
    let translation = new_java2rust::translate_source(
        r#"
class Owner<T> {
    <T> T identity(T value) { return value; }
    long identity(long value) { return value; }
}
class Use {
    static int probe() {
        Owner<Integer> owner = new Owner<Integer>();
        String value = owner.identity("text");
        return value.length();
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
        "generic_method_binding",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn caller_does_not_use_the_receivers_integer_binder() {
    assert_eq!(Use::probe(), 4);
}
"#,
        }],
    );
}

#[test]
fn inferred_method_arguments_must_satisfy_declared_bounds() {
    let call = returned_call(
        r#"
class Owner<T> {
    <Element extends Number> String choose(Element value) { return "number"; }
    String choose(Object value) { return "object"; }
}
class Use { String make(Owner<Integer> owner) { return owner.choose("text"); } }
"#,
    );
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "Object"),
        "{call:?}"
    );
}

#[test]
fn scoped_method_arguments_use_their_proven_upper_bounds() {
    let call = returned_call(
        r#"
class Owner<T> {
    <Element extends Number> Element identity(Element value) { return value; }
}
class Use<N extends Number> {
    N make(Owner<Integer> owner, N value) { return owner.identity(value); }
}
"#,
    );
    assert_eq!(
        call.return_type,
        JavaType::Named {
            name: "N".into(),
            args: vec![]
        }
    );
}

#[test]
fn an_unbounded_caller_variable_is_not_known_to_be_a_number() {
    let call = returned_call(
        r#"
class Owner<T> {
    <Element extends Number> String choose(Element value) { return "number"; }
    String choose(Object value) { return "object"; }
}
class Use<N> { String make(Owner<Integer> owner, N value) { return owner.choose(value); } }
"#,
    );
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "Object"),
        "{call:?}"
    );
}
