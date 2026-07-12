use new_java2rust::java_ir::{CallKind, ConstructorInvocationKind, Expr, Member, Stmt};
use new_java2rust::parser::parse_java_source;
use new_java2rust::resolve::resolve_program;

#[test]
fn resolve_annotates_same_class_overload_call_targets() {
    let source =
        std::fs::read_to_string("examples/java/03-methods/OverloadSameArity.java").unwrap();
    let parsed = parse_java_source(&source);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let call_all = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "callAll" => method.body.as_ref(),
            _ => None,
        })
        .unwrap();

    let Stmt::Return(Some(expr)) = &call_all.statements[0] else {
        panic!("expected return expression");
    };

    let mut calls = Vec::new();
    collect_calls(expr, &mut calls);
    let names = calls
        .iter()
        .filter_map(|expr| match expr {
            Expr::Call {
                resolved: Some(call),
                ..
            } => Some((
                call.declaring_type.as_str(),
                call.rust_name.as_str(),
                call.kind,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&("OverloadSameArity", "choose_int", CallKind::Instance)));
    assert!(names.contains(&("OverloadSameArity", "choose_long", CallKind::Instance)));
    assert!(names.contains(&("OverloadSameArity", "choose_string", CallKind::Instance)));
}

#[test]
fn resolve_annotates_same_class_constructor_delegation_targets() {
    let source =
        std::fs::read_to_string("examples/java/03-methods/ConstructorOverloads.java").unwrap();
    let parsed = parse_java_source(&source);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let ty = &resolved.program.types[0];
    let delegations = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(constructor) => constructor.body.as_ref(),
            _ => None,
        })
        .flat_map(|body| &body.statements)
        .filter_map(|stmt| match stmt {
            Stmt::ConstructorInvocation {
                kind: ConstructorInvocationKind::This,
                resolved: Some(resolved),
                ..
            } => Some((
                resolved.declaring_type.as_str(),
                resolved.rust_name.as_str(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        delegations,
        vec![
            ("ConstructorOverloads", "new_string_int"),
            ("ConstructorOverloads", "new_string_int")
        ]
    );
}

#[test]
fn resolve_annotates_new_expression_constructor_targets() {
    let parsed = parse_java_source(
        r#"
class Boxed {
    Boxed(String value) {}
}

class MakeBoxed {
    Boxed make(String value) {
        return new Boxed(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );

    let maker = &resolved.program.types[1];
    let body = maker
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => method.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::New {
        resolved: Some(resolved),
        ..
    })) = &body.statements[0]
    else {
        panic!("expected resolved new expression");
    };

    assert_eq!(resolved.declaring_type, "Boxed");
    assert_eq!(resolved.rust_name, "new");
}

fn collect_calls<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    match expr {
        Expr::Call { receiver, args, .. } => {
            out.push(expr);
            if let Some(receiver) = receiver {
                collect_calls(receiver, out);
            }
            for arg in args {
                collect_calls(arg, out);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_calls(left, out);
            collect_calls(right, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_calls(condition, out);
            collect_calls(then_expr, out);
            collect_calls(else_expr, out);
        }
        Expr::FieldAccess(receiver, _) => collect_calls(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_calls(array, out);
            collect_calls(index, out);
        }
        Expr::Unary { expr, .. } => collect_calls(expr, out),
        Expr::New { args, .. } => {
            for arg in args {
                collect_calls(arg, out);
            }
        }
        _ => {}
    }
}
