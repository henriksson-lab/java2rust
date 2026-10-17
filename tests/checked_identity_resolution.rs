use new_java2rust::{
    java_ir::{Expr, JavaType, Member, PrimitiveType, Stmt},
    parser::parse_java_source,
    resolve::{ResolvedProgram, resolve_program},
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

fn resolve(source: &str) -> ResolvedProgram {
    let parsed = parse_java_source(source);
    assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    resolved
}

fn returned_expr(program: &ResolvedProgram) -> &Expr {
    let owner = program
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
    let Stmt::Return(Some(expr)) = &method.body.as_ref().unwrap().statements[0] else {
        panic!("expected return expression");
    };
    expr
}

#[test]
fn checked_source_receiver_preserves_generic_collection_return_for_overloads() {
    let program = resolve(
        r#"
import java.util.Collection;
import java.util.List;
import java.util.Objects;
interface Nodes<V> { Collection<V> values(); }
class Index<V> { Index(List<V> values) {} Index(Collection<V> values) {} }
class Use {
    static <V> Index<V> make(Nodes<V> nodes) {
        return new Index<V>(Objects.requireNonNull(nodes).values());
    }
}
"#,
    );
    let Expr::New {
        resolved: Some(constructor),
        ..
    } = returned_expr(&program)
    else {
        panic!("expected resolved constructor");
    };
    assert!(
        matches!(&constructor.params[0].ty, JavaType::Named { name, .. }
        if name == "Collection"),
        "{constructor:?}"
    );
}

#[test]
fn checked_source_receiver_keeps_source_method_identity() {
    let program = resolve(
        r#"
import java.util.Objects;
class Token { int score() { return 7; } }
class Use { int make(Token value) { return Objects.requireNonNull(value, "value").score(); } }
"#,
    );
    let Expr::Call {
        resolved: Some(call),
        ..
    } = returned_expr(&program)
    else {
        panic!("expected resolved source score method");
    };
    assert_eq!(call.declaring_type, "Token");
}

#[test]
fn objects_non_null_is_a_boolean_predicate_not_an_identity() {
    let program = resolve(
        r#"
import java.util.Objects;
class Use {
    int choose(boolean value) { return 1; }
    int choose(String value) { return 2; }
    int make(String value) { return choose(Objects.nonNull(value)); }
}
"#,
    );
    let Expr::Call {
        resolved: Some(call),
        ..
    } = returned_expr(&program)
    else {
        panic!("expected resolved choose call");
    };
    assert_eq!(
        call.arg_target_types,
        vec![JavaType::Primitive(PrimitiveType::Boolean)]
    );
}

#[test]
fn checked_optional_argument_keeps_its_declared_java_type() {
    let program = resolve(
        r#"
import java.util.Objects;
import java.util.Optional;
class Token {}
class Use {
    int choose(Optional<Token> value) { return 1; }
    int choose(Token value) { return 2; }
    int make(Optional<Token> value) { return choose(Objects.requireNonNull(value)); }
}
"#,
    );
    let Expr::Call {
        resolved: Some(call),
        ..
    } = returned_expr(&program)
    else {
        panic!("expected resolved choose call");
    };
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "Optional"),
        "{call:?}"
    );
}

#[test]
fn source_objects_definition_keeps_precedence_over_native_identity() {
    let program = resolve(
        r#"
class Token {}
class Objects { static String requireNonNull(Token value) { return "custom"; } }
class Use {
    int choose(String value) { return 1; }
    int choose(Token value) { return 2; }
    int make(Token value) { return choose(Objects.requireNonNull(value)); }
}
"#,
    );
    let Expr::Call {
        resolved: Some(call),
        ..
    } = returned_expr(&program)
    else {
        panic!("expected resolved choose call");
    };
    assert_eq!(call.arg_target_types, vec![JavaType::String]);
}

#[test]
fn source_objects_utility_keeps_its_runtime_body() {
    let translation = new_java2rust::translate_source(
        r#"
class Token {}
class Objects {
    static String requireNonNull(Token value) { return "custom"; }
}
class Use {
    static int make() { return Objects.requireNonNull(new Token()).length(); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "checked_identity_source_precedence",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_utility_is_not_a_native_identity() {
    assert_eq!(Use::make(), 6);
}
"#,
        }],
    );
}

#[test]
fn checked_source_calls_preserve_predicate_dispatch_and_exception_order() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.Objects;
import java.util.Optional;
class Token { int score() { return 7; } }
class Counter { int count; void increment() { count++; } }
class Use {
    static int stamp;
    static Token missing() { stamp = 1; return null; }
    static String message() {
        if (stamp == 1) stamp = 12;
        else stamp = 99;
        return "missing";
    }
    static int choose(boolean value) { return value ? 1 : 0; }
    static int choose(String value) { return 2; }
    static int make(Token value) { return Objects.requireNonNull(value, "value").score(); }
    static int probe() { return make(new Token()) * 10 + choose(Objects.nonNull("present")); }
    static int nullProbe() {
        stamp = 0;
        try {
            Objects.requireNonNull(missing(), message()).score();
            return -1;
        } catch (NullPointerException error) {
            return stamp;
        }
    }
    static int oneArgumentNullProbe() {
        stamp = 0;
        try {
            Objects.requireNonNull(missing()).score();
            return -1;
        } catch (NullPointerException error) {
            return stamp;
        }
    }
    static int presentMessageProbe() {
        stamp = 1;
        int score = Objects.requireNonNull(new Token(), message()).score();
        return stamp * 10 + score;
    }
    static int optionalProbe() {
        Optional<String> value = Optional.empty();
        return Objects.requireNonNull(value).isPresent() ? 1 : 0;
    }
    static int inlineOptionalProbe() {
        return Objects.requireNonNull(Optional.of("present")).isPresent() ? 1 : 0;
    }
    static int emptyOptionalWithMessageProbe() {
        Optional<String> value = Optional.empty();
        return Objects.requireNonNull(value, "optional").isPresent() ? 1 : 0;
    }
    static int aliasProbe() {
        Counter value = new Counter();
        Objects.requireNonNull(value).increment();
        return value.count;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        !translation
            .rust
            .contains("imported static method require_non_null"),
        "native checked identities must not emit unmodelled facade placeholders"
    );
    cargo_test_generated_modules(
        "checked_identity_source",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn checked_calls_preserve_source_behavior() {
    assert_eq!(Use::probe(), 71);
    assert_eq!(Use::null_probe(), 12);
    assert_eq!(Use::one_argument_null_probe(), 1);
    assert_eq!(Use::present_message_probe(), 127);
    assert_eq!(Use::optional_probe(), 0);
    assert_eq!(Use::inline_optional_probe(), 1);
    assert_eq!(Use::empty_optional_with_message_probe(), 0);
    assert_eq!(Use::alias_probe(), 1);
}
"#,
        }],
    );
}
