use new_java2rust::{
    java_ir::{Expr, JavaType, Member, ResolvedConstructor, Stmt, TypeDecl},
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

fn returned_constructor<'a>(program: &'a ResolvedProgram, owner: &str) -> &'a ResolvedConstructor {
    fn find_type<'a>(ty: &'a TypeDecl, owner: &str) -> Option<&'a TypeDecl> {
        if ty.name == owner {
            return Some(ty);
        }
        ty.members.iter().find_map(|member| match member {
            Member::Type(nested) => find_type(nested, owner),
            _ => None,
        })
    }
    let owner = program
        .program
        .types
        .iter()
        .find_map(|ty| find_type(ty, owner))
        .unwrap();
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(constructor),
        ..
    })) = &method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected a resolved constructor return");
    };
    constructor
}

fn assert_collection_parameter(constructor: &ResolvedConstructor, name: &str) {
    assert!(
        matches!(&constructor.params[0].ty, JavaType::Named { name: actual, .. }
            if actual == name || actual.ends_with(&format!(".{name}"))),
        "{constructor:?}",
    );
}

#[test]
fn captured_outer_parameter_is_not_reinferred_for_each_constructor_overload() {
    let program = resolve(
        r#"
import java.util.Collection;
class Owner<T> {
    class Cell {
        Cell(Collection<T> values) {}
        Cell(T value) {}
    }
    Cell make(Collection<T> values) { return new Cell(values); }
}
"#,
    );
    assert_collection_parameter(returned_constructor(&program, "Owner"), "Collection");
}

#[test]
fn deeply_nested_capture_arguments_keep_the_full_lexical_owner() {
    let program = resolve(
        r#"
import java.util.Collection;
class Owner<T> {
    class Group {
        class Cell {
            Cell(Collection<T> values) {}
            Cell(T value) {}
        }
        Cell make(Collection<T> values) { return new Cell(values); }
    }
}
"#,
    );
    let constructor = returned_constructor(&program, "Owner.Group");
    assert_collection_parameter(constructor, "Collection");
    assert!(
        matches!(&constructor.constructed_type, JavaType::Named { name, args }
        if name == "Owner.Group.Cell" && args.len() == 1),
        "{constructor:?}"
    );
}

#[test]
fn explicit_constructed_arguments_constrain_constructor_overload_selection() {
    let program = resolve(
        r#"
import java.util.Collection;
class Choice<T> {
    Choice(Collection<T> values) {}
    Choice(T value) {}
}
class Use {
    Choice<String> make(Collection<String> values) { return new Choice<String>(values); }
}
"#,
    );
    assert_collection_parameter(returned_constructor(&program, "Use"), "Collection");
}

#[test]
fn constructor_type_parameter_can_shadow_a_fixed_class_parameter() {
    let program = resolve(
        r#"
class Choice<T> {
    <T> Choice(T value) {}
    Choice(int value, int marker) {}
}
class Use { Choice<String> make(int value) { return new Choice<String>(value); } }
"#,
    );
    assert_eq!(returned_constructor(&program, "Use").params.len(), 1);
}

#[test]
fn constructor_delegation_selects_the_more_specific_collection_contract() {
    let program = resolve(
        r#"
import java.util.Collection;
import java.util.List;
import java.util.ArrayList;
class Choice<T> {
    Choice(List<T> values) {}
    Choice(Collection<T> values) { this(new ArrayList<>(values)); }
}
"#,
    );
    let delegation = program.program.types[0]
        .members
        .iter()
        .find_map(|member| {
            let Member::Constructor(ctor) = member else {
                return None;
            };
            let Stmt::ConstructorInvocation {
                resolved: Some(constructor),
                ..
            } = ctor.body.as_ref()?.statements.first()?
            else {
                return None;
            };
            Some(constructor)
        })
        .unwrap();
    assert_collection_parameter(delegation, "List");
}

#[test]
fn methods_and_constructors_share_collection_specificity() {
    let program = resolve(
        r#"
import java.util.Collection;
import java.util.List;
import java.util.ArrayList;
class Use {
    int choose(List<String> values) { return 1; }
    int choose(Collection<String> values) { return 2; }
    int run() { return choose(new ArrayList<String>()); }
}

"#,
    );
    let run = program.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &run.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected resolved choose call");
    };
    assert!(
        matches!(&call.arg_target_types[0], JavaType::Named { name, .. }
        if name == "List" || name.ends_with(".List")),
        "{call:?}"
    );
}

#[test]
fn unrelated_null_overloads_and_unknown_actuals_remain_ambiguous() {
    for source in [
        r#"
import java.util.List;
import java.util.Set;
class Choice {
    Choice(List<String> values) {}
    Choice(Set<String> values) {}
    static Choice make() { return new Choice(null); }
}
"#,
        r#"
import java.util.List;
import java.util.Collection;
class Choice {
    Choice(List<String> values) {}
    Choice(Collection<String> values) {}
    static Choice make() { return new Choice(missing()); }
}
"#,
    ] {
        let parsed = parse_java_source(source);
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(resolved.diagnostics.has_errors(), "{source}");
        assert!(format!("{:?}", resolved.diagnostics).contains("resolve_constructor_ambiguous"));
    }
}

#[test]
fn constructor_selection_and_delegation_preserve_runtime_choice() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.List;
import java.util.Collection;
import java.util.ArrayList;
class Choice<T> {
    int marker;
    Choice(List<T> values) { marker = 1; }
    Choice(Collection<T> values) { this(new ArrayList<>(values)); }
    Choice(T value) { marker = 2; }
}
class Use {
    static int probe() {
        Collection<String> values = new ArrayList<String>();
        Choice<String> choice = new Choice<String>(values);
        return choice.marker;
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
        "constructor_overload_scope",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn collection_constructor_delegates_to_list_constructor() {
    assert_eq!(Use::probe(), 1);
}
"#,
        }],
    );
}

#[test]
fn scoped_constructor_arguments_preserve_upper_bounds_and_wildcards() {
    resolve(
        r#"
import java.util.Collection;
import java.util.List;
class Sink<T> { Sink(T value) {} }
class Use<T, U extends T> {
    Sink<T> variable(U value) { return new Sink<T>(value); }
    Sink<Object> upper(T value) { return new Sink<Object>(value); }
    Sink<Collection<?>> any(List<T> values) { return new Sink<Collection<?>>(values); }
    Sink<Collection<? extends T>> covariant(List<U> values) {
        return new Sink<Collection<? extends T>>(values);
    }
    Sink<Collection<? super T>> contravariant(List<Object> values) {
        return new Sink<Collection<? super T>>(values);
    }
}
"#,
    );
}

#[test]
fn a_fixed_variable_does_not_absorb_incompatible_arguments() {
    for source in [
        r#"
class Sink<T> { Sink(T value) {} }
class Use<T> { Sink<T> make(String value) { return new Sink<T>(value); } }
"#,
        r#"
import java.util.Collection;
import java.util.List;
class Sink<T> { Sink(T value) {} }
class Use<T> {
    Sink<Collection<T>> make(List<String> values) { return new Sink<Collection<T>>(values); }
}
"#,
    ] {
        let parsed = parse_java_source(source);
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        assert!(resolved.diagnostics.has_errors(), "{source}");
    }
}
