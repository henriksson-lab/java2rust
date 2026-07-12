use new_java2rust::java_ir::{CallKind, Expr, Member, Stmt};
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
            } => Some((call.rust_name.as_str(), call.kind)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert!(names.contains(&("choose_int", CallKind::Instance)));
    assert!(names.contains(&("choose_long", CallKind::Instance)));
    assert!(names.contains(&("choose_string", CallKind::Instance)));
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
