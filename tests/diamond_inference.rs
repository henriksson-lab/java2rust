use new_java2rust::{
    java_ir::{Expr, JavaType, Member, Stmt},
    parser::parse_java_source,
    resolve::resolve_program,
    translate_source,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules};

fn assert_contextual_new(expr: &Expr, expected: &str) {
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = expr
    else {
        panic!("expected resolved construction: {expr:?}");
    };
    assert_eq!(
        constructor.constructed_type,
        new_java2rust::parser::parse_hierarchy_type(expected)
    );
}

#[test]
fn contextual_diamond_preserves_argument_witnesses_and_wildcard_variance() {
    let parsed = parse_java_source(
        r#"
class Envelope<T> { Envelope(T value) {} }
class EnvelopeUse {
    Envelope<? super String> lower(Object value) { return new Envelope<>(value); }
    Envelope<? extends Object> upper(String value) { return new Envelope<>(value); }
    Envelope<?> any(String value) { return new Envelope<>(value); }
    Object neutral(String value) { return new Envelope<>(value); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "EnvelopeUse")
        .unwrap();
    for (name, expected) in [
        ("lower", "Envelope<Object>"),
        ("upper", "Envelope<String>"),
        ("any", "Envelope<String>"),
        ("neutral", "Envelope<String>"),
    ] {
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        let Stmt::Return(Some(expr)) = &method.body.as_ref().unwrap().statements[0] else {
            panic!()
        };
        assert_contextual_new(expr, expected);
    }
}

#[test]
fn contextual_diamond_projects_reordered_superclass_arguments() {
    let parsed = parse_java_source(
        r#"
class Parent<Left, Right> {}
class Child<A, B> extends Parent<B, A> {}
class ParentUse { Parent<String, Integer> make() { return new Child<>(); } }
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ParentUse")
        .unwrap();
    let Member::Method(method) = &owner.members[0] else {
        panic!()
    };
    let Stmt::Return(Some(expr)) = &method.body.as_ref().unwrap().statements[0] else {
        panic!()
    };
    assert_contextual_new(expr, "Child<Integer, String>");
}

#[test]
fn contextual_diamond_targets_follow_returns_assignments_and_callable_boundaries() {
    let parsed = parse_java_source(
        r#"
class Envelope<T> { Envelope() {} }
interface Maker<R> { R make(); }
class EnvelopeScopes {
    Envelope<String> branches(boolean flag) { if (flag) { return new Envelope<>(); } return new Envelope<>(); }
    Envelope<String> conditional(boolean flag) { return flag ? new Envelope<>() : new Envelope<>(); }
    Envelope<String> assigned() { Envelope<String> result = null; result = new Envelope<>(); return result; }
    Maker<Envelope<String>> lambda() { return () -> new Envelope<>(); }
    Envelope<Object> outer() { return use(() -> { return new Envelope<>(); }); }
    static Envelope<Object> use(Maker<Envelope<String>> action) { return null; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "EnvelopeScopes")
        .unwrap();
    let method = |name| {
        owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap()
    };
    let branches = &method("branches").body.as_ref().unwrap().statements;
    let Stmt::If { then_block, .. } = &branches[0] else {
        panic!()
    };
    for statement in [&then_block.statements[0], &branches[1]] {
        let Stmt::Return(Some(expr)) = statement else {
            panic!()
        };
        assert_contextual_new(expr, "Envelope<String>");
    }
    let Stmt::Return(Some(Expr::Ternary {
        then_expr,
        else_expr,
        ..
    })) = &method("conditional").body.as_ref().unwrap().statements[0]
    else {
        panic!()
    };
    assert_contextual_new(then_expr, "Envelope<String>");
    assert_contextual_new(else_expr, "Envelope<String>");
    let Stmt::Assign { value, .. } = &method("assigned").body.as_ref().unwrap().statements[1]
    else {
        panic!()
    };
    assert_contextual_new(value, "Envelope<String>");
    let Stmt::Return(Some(Expr::Lambda {
        body: new_java2rust::java_ir::LambdaBody::Expr(expr),
        ..
    })) = &method("lambda").body.as_ref().unwrap().statements[0]
    else {
        panic!()
    };
    assert_contextual_new(expr, "Envelope<String>");
    let Stmt::Return(Some(Expr::Call { args, .. })) =
        &method("outer").body.as_ref().unwrap().statements[0]
    else {
        panic!()
    };
    let Expr::Lambda {
        body: new_java2rust::java_ir::LambdaBody::Block(body),
        ..
    } = &args[0]
    else {
        panic!()
    };
    let Stmt::Return(Some(expr)) = &body.statements[0] else {
        panic!()
    };
    assert_contextual_new(expr, "Envelope<String>");
}

#[test]
fn nested_diamond_under_wildcard_return_keeps_constructor_witnesses() {
    let translation = translate_source(
        r#"
interface Store<Key, Value> {}
class ConcreteStore<Key, Value> implements Store<Key, Value> {
    ConcreteStore(Value value) {}
}
class StoreBuilder<Key, Value, Actual extends Store<Key, Value>> {
    Actual store;
    StoreBuilder(Actual store) { this.store = store; }
}
class StoreFactory {
    static <Key, Value> StoreBuilder<Key, Value, ? extends ConcreteStore<Key, Value>> make(Value value) {
        return new StoreBuilder<>(new ConcreteStore<>(value));
    }
}
"#,
    );
    let invalid_patterns = new_java2rust::output_markers::count_invalid_patterns(&translation.rust);
    cargo_check_generated_modules(
        "nested_diamond_wildcard_return",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
    assert_eq!(
        invalid_patterns, 0,
        "source constructor witnesses must not become erased casts"
    );
}

#[test]
fn nested_diamond_preserves_payload_capabilities_through_source_bounds() {
    let translation = translate_source(
        r#"
interface ValueStore<Key, Value> { Value read(); }
class PayloadStore<Key, Value> implements ValueStore<Key, Value> {
    Value value;
    PayloadStore(Value value) { this.value = value; }
    public Value read() { return value; }
}
class PayloadBuilder<Key, Value, Actual extends ValueStore<Key, Value>> {
    Actual store;
    PayloadBuilder(Actual store) { this.store = store; }
}
class PayloadFactory {
    static <Key, Value> PayloadBuilder<Key, Value, ? extends PayloadStore<Key, Value>> make(Value value) {
        return new PayloadBuilder<>(new PayloadStore<>(value));
    }
}
interface SizedStore<Item> { int size(); }
class HandleStore<Item> implements SizedStore<Item> {
    SizedStore<Item> delegate;
    HandleStore(SizedStore<Item> delegate) { this.delegate = delegate; }
    public int size() { return delegate.size(); }
}
class HandleBuilder<Item, Actual extends SizedStore<Item>> {
    Actual store;
    HandleBuilder(Actual store) { this.store = store; }
}
class HandleFactory {
    static <Item> HandleBuilder<Item, ? extends HandleStore<Item>> make(SizedStore<Item> delegate) {
        return new HandleBuilder<>(new HandleStore<>(delegate));
    }
}
"#,
    );
    cargo_check_generated_modules(
        "nested_diamond_payload_capabilities",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NonClone;
fn payload_bounds<Key: 'static, Value: Clone + 'static>(value: Value)
    -> PayloadBuilder<Key, Value, PayloadStore<Key, Value>> {
    PayloadFactory::make(value)
}
fn handle_bounds<Item: 'static>(delegate: &dyn SizedStore<Item>)
    -> HandleBuilder<Item, HandleStore<Item>> {
    HandleFactory::make(delegate)
}
fn no_phantom_or_handle_payload_clone() {
    let _ = payload_bounds::<NonClone, String>;
    let _ = handle_bounds::<NonClone>;
}
"#,
        }],
    );
}

#[test]
fn chained_diamond_factory_preserves_generic_interface_return() {
    let translation = translate_source(
        r#"
import java.util.List;
interface Bundle<T> { int size(); }
class Factory<T> {
    public Factory(List<T> input) {}
    Bundle<T> result() { return null; }
}
class FactoryUse {
    static Bundle<String> forward(List<String> input) {
        return new Factory<>(input).result();
    }
}
"#,
    );
    assert!(
        !translation
            .rust
            .contains("unsupported Java dispatch coercion"),
        "{}",
        translation
            .rust
            .lines()
            .filter(|line| line.contains("fn forward") || line.contains("return "))
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .join("\n")
    );
    cargo_check_generated_modules(
        "chained_diamond_factory",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn diamond_and_raw_constructor_syntax_remain_distinct_after_resolution() {
    let parsed = parse_java_source(
        r#"
class Parcel<T> { Parcel(T input) {} }
class ParcelUse {
    Object diamond(String input) { return new Parcel<>(input); }
    Object raw(String input) { return new Parcel(input); }
}

"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ParcelUse")
        .unwrap();
    for (name, diamond, arguments) in [
        ("diamond", true, vec![JavaType::String]),
        ("raw", false, vec![]),
    ] {
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        let Stmt::Return(Some(Expr::New {
            infer_type_arguments,
            resolved: Some(constructor),
            ..
        })) = &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("expected resolved constructor");
        };
        assert_eq!(*infer_type_arguments, diamond);
        assert_eq!(
            constructor.constructed_type,
            JavaType::Named {
                name: "Parcel".into(),
                args: arguments
            }
        );
    }
}

#[test]
fn diamond_argument_constraints_are_joined_independently_of_argument_order() {
    let parsed = parse_java_source(
        r#"
class Joined<T> { Joined(T first, T second) {} }
class JoinedUse {
    Object left(String text, Object object) { return new Joined<>(text, object); }
    Object right(String text, Object object) { return new Joined<>(object, text); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "JoinedUse")
        .unwrap();
    for method in owner.members.iter().filter_map(|member| match member {
        Member::Method(method) => Some(method),
        _ => None,
    }) {
        let Stmt::Return(Some(Expr::New {
            resolved: Some(constructor),
            ..
        })) = &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("expected resolved constructor");
        };
        assert_eq!(
            constructor.constructed_type,
            JavaType::Named {
                name: "Joined".into(),
                args: vec![JavaType::Named {
                    name: "Object".into(),
                    args: vec![]
                }],
            },
            "{}",
            method.name
        );
    }
}
