use new_java2rust::{
    java_ir::{Expr, JavaType, Member, ResolvedCall, Stmt},
    parser::parse_java_source,
    resolve::resolve_program,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn explicit_static_witnesses_survive_chaining_and_exception_binder_erasure() {
    let translation = new_java2rust::translate_source(
        r#"
class ExplicitStaticWitness {
    static <Value, Unused> Value keep(Value value) { return value; }
    static <Failure extends RuntimeException, Value, Unused> Value checked(Value value) throws Failure { return value; }
    static int run() {
        return ExplicitStaticWitness.<String, Integer>keep("ok").length()
            + ExplicitStaticWitness.<IllegalArgumentException, String, Integer>checked("abc").length();
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
        "explicit_static_witness",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test] fn explicit_unused_witness_and_erased_throwable_binder_match_the_emitted_signature() {
    assert_eq!(ExplicitStaticWitness::run().unwrap(), 5);
}
"#,
        }],
    );
}

#[test]
fn source_result_nullability_is_not_copied_from_generic_argument() {
    use new_java2rust::{facts::FactSet, representation, semantic_analysis, signature};
    let parsed = parse_java_source(include_str!("fixtures/source_result_contract.java"));
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let signatures = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let owner = &resolved.program.types[0];
    for (name, nullable_return) in [("reject", false), ("maybe", true)] {
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        let signature = signatures.required_method_signature(owner, method);
        assert!(
            signature.params()[0]
                .shape()
                .optional_element_type()
                .is_some()
        );
        assert_eq!(
            signature
                .return_shape()
                .unwrap()
                .optional_element_type()
                .is_some(),
            nullable_return
        );
    }
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_result_contract.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "source_result_contract",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeString { fn value(self) -> String; }
impl ProbeString for String { fn value(self) -> String { self } }
impl<E: std::fmt::Debug> ProbeString for Result<String, E> { fn value(self) -> String { self.unwrap() } }
#[test] fn source_result_contract_matches_java() { assert_eq!(CheckedResults::run().value(), "ok:null:chosen:caught:null:0"); }
"#,
        }],
    );
}

#[test]
fn emitted_source_return_contract_beats_conflicting_external_nullability() {
    use new_java2rust::facts::{
        ExternalApiEffectFact, ExternalApiRole, FactProvenance, FactSet, NullabilityState,
        ReceiverMutabilityFact,
    };
    let mut facts = FactSet::default();
    for (name, arity, nullability) in [
        ("value", 0, NullabilityState::MaybeNull),
        ("optional", 1, NullabilityState::DefinitelyNonNull),
    ] {
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: name.into(),
            receiver_type: Some("DeclaredResults".into()),
            arg_count: Some(arity),
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::String),
            return_nullability: nullability,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    }
    let translation = new_java2rust::translate_source_with_facts(
        r#"
class DeclaredResults {
    static String value() { return "source"; }
    static String optional(boolean missing) { if (missing) { return null; } return "present"; }
    static String run() { String nonNull = value(); String missing = optional(true); return nonNull + ":" + missing; }
}
"#,
        &facts,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "source_external_return_contract",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"#[test] fn source_contract_wins() { assert_eq!(DeclaredResults::run(), "source:null"); }"#,
        }],
    );
}

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
fn explicit_method_witness_types_a_null_argument_and_shadows_owner_binders() {
    let call = returned_call(
        r#"
class Box<T> { <T> T echo(T value) { return value; } }
class Use { String make(Box<Integer> box) { return box.<String>echo(null); } }
"#,
    );
    assert_eq!(call.type_args, vec![JavaType::String]);
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
    assert_eq!(call.return_type, JavaType::String);
}

#[test]
fn explicit_witnesses_preserve_overload_choice_and_non_generic_precedence() {
    let prefix = r#"
class Factory {
    static <T> String choose(T value) { return "generic"; }
    static String choose(String value) { return "string"; }
}
"#;
    let generic = returned_call(&format!(
        "{prefix} class Use {{ String make() {{ return Factory.<Integer>choose(3); }} }}"
    ));
    assert_eq!(
        generic.type_args,
        vec![new_java2rust::parser::parse_type("Integer")]
    );
    let specific = returned_call(&format!(
        "{prefix} class Use {{ String make() {{ return Factory.<String>choose(\"x\"); }} }}"
    ));
    assert!(specific.type_args.is_empty());
    assert_ne!(generic.rust_name, specific.rust_name);
}

#[test]
fn explicit_witnesses_are_checked_against_bounds_arity_and_value_arguments() {
    for call in [
        "Factory.<String>echo(null)",
        "Factory.<Good, Good>echo(null)",
        "Factory.<Good>echo(\"bad\")",
    ] {
        let source = format!(
            r#"
class Bound {{}}
class Good extends Bound {{}}
class Factory {{ static <T extends Bound> T echo(T value) {{ return value; }} }}
class Use {{ Object make() {{ return {call}; }} }}
"#
        );
        let parsed = parse_java_source(&source);
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            resolved.diagnostics.has_errors(),
            "{call}: {:?}",
            resolved.diagnostics
        );
    }
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
#[test]
fn exact_bounded_source_calls_keep_declaration_binders() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/exact_bounded_source_calls.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "exact_bounded_source_calls",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
fn use_owned_sink<T>(mut sink: Box<dyn ValueSink<T>>, value: T) -> bool { sink.put(value) }
#[derive(Clone)] struct BorrowedSink;
impl<'a> ValueSink<&'a str> for BorrowedSink {
    fn put(&mut self, _: &'a str) -> bool { true }
    fn first(&self) -> Option<&'a str> { None }
    fn count(&self) -> i32 { 0 }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn ValueSink<&'a str>> { Box::new(self.clone()) }
}
fn use_borrowed_argument<'a>(value: &'a str) -> bool {
    use_owned_sink::<&'a str>(Box::new(BorrowedSink), value)
}
#[test] fn exact_capture_matches_java() {
    assert_eq!(ExactBoundedSourceCalls::run().value(), 23);
    let local = String::from("not static");
    assert!(use_borrowed_argument(&local));
}
"#,
        }],
    );
}
