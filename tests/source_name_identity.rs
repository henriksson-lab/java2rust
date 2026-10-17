use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules};

#[test]
fn constructor_type_parameter_shadows_a_lexical_sibling_type() {
    use new_java2rust::{
        java_ir::{Expr, Member, Stmt},
        parser::parse_java_source,
        resolve::resolve_program,
    };
    let parsed = parse_java_source(
        r#"
class Owner {
    static class Element {}
    static class Box {
        <Element> Box(Element value) {}
        Box(int value) {}
    }
}
class Consumer {
    Owner.Box make(String value) { return new Owner.Box(value); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Consumer")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(constructor),
        ..
    })) = &method.body.as_ref().unwrap().statements[0]
    else {
        panic!("generic constructor must resolve");
    };
    assert!(!format!("{:?}", constructor.params).contains("Owner.Element"));
}

#[test]
fn nested_type_and_field_with_same_rust_spelling_keep_distinct_java_identity() {
    let translation = translate_source(
        r#"
class Metadata {
    Bounds[] bounds;
    static final class Bounds { public static final int UNMAPPED = 7; }
    int run() { return Bounds.UNMAPPED; }
}
"#,
    );
    cargo_check_generated_modules(
        "type_field_spelling_identity",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}

#[test]
fn type_like_local_binding_keeps_its_instance_receiver() {
    let translation = translate_source(
        r#"
class Snapshot { int value; }
class Holder { Snapshot retained; }
class Reader {
    static int read(Snapshot Input) { return Input.value; }
    static int write(Snapshot Input) { Input.value = 3; return Input.value; }
}
"#,
    );
    cargo_check_generated_modules(
        "type_like_local_binding",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}
