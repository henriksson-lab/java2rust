use new_java2rust::java_ir::{ConstructorInvocationKind, Expr, Member, Stmt, UnaryOp};
use new_java2rust::parser::parse_java_source;

fn fixture(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new("examples/java").join(path)).unwrap()
}

#[test]
fn all_fixtures_parse_without_reader_fallback_nodes() {
    let mut paths = Vec::new();
    collect_java_files(std::path::Path::new("examples/java"), &mut paths);
    paths.sort();

    let mut failures = Vec::new();
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let parsed = parse_java_source(&source);
        let mut fallbacks = Vec::new();
        for ty in &parsed.program.types {
            for member in &ty.members {
                collect_member_fallbacks(member, &mut fallbacks);
            }
        }
        if !fallbacks.is_empty() {
            failures.push(format!("{}: {}", path.display(), fallbacks.join(", ")));
        }
    }

    assert!(
        failures.is_empty(),
        "reader fallback nodes remain:\n{}",
        failures.join("\n")
    );
}

#[test]
fn parses_package_and_imports_from_fixture() {
    let parsed = parse_java_source(&fixture("06-stdlib/CollectionsUse.java"));
    assert_eq!(parsed.program.package.as_deref(), Some("examples.stdlib"));
    assert!(
        parsed
            .program
            .imports
            .iter()
            .any(|import| import.path == "java.util.HashMap")
    );
    assert!(
        parsed
            .program
            .imports
            .iter()
            .any(|import| import.path == "java.util.List")
    );
}

#[test]
fn parses_simple_class_members() {
    let parsed = parse_java_source(&fixture("04-objects/SimpleClass.java"));
    let ty = &parsed.program.types[0];
    assert_eq!(ty.name, "SimpleClass");
    assert_eq!(
        ty.members
            .iter()
            .filter(|member| matches!(member, Member::Field(_)))
            .count(),
        2
    );
    assert_eq!(
        ty.members
            .iter()
            .filter(|member| matches!(member, Member::Constructor(_)))
            .count(),
        1
    );
    assert_eq!(
        ty.members
            .iter()
            .filter(|member| matches!(member, Member::Method(_)))
            .count(),
        2
    );
}

#[test]
fn parses_ternary_return_expression() {
    let parsed = parse_java_source(&fixture("01-expressions/ExpressionPrecedence.java"));
    let method = parsed.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let body = method.body.as_ref().unwrap();
    assert!(body.statements.iter().any(|stmt| {
        matches!(
            stmt,
            Stmt::Return(Some(Expr::Ternary {
                condition: _,
                then_expr: _,
                else_expr: _
            }))
        )
    }));
}

#[test]
fn parses_prefix_unary_expressions() {
    let negative = parse_java_source(&fixture("02-control-flow/LabeledContinue.java"));
    assert!(program_has_unary(&negative.program, UnaryOp::Neg));

    let not = parse_java_source(&fixture("06-stdlib/StreamsCollect.java"));
    assert!(program_has_unary(&not.program, UnaryOp::Not));
}

fn program_has_unary(program: &new_java2rust::java_ir::Program, op: UnaryOp) -> bool {
    program.types.iter().any(|ty| {
        ty.members.iter().any(|member| match member {
            Member::Method(method) => method
                .body
                .as_ref()
                .is_some_and(|body| block_has_unary(body, op)),
            Member::Constructor(constructor) => constructor
                .body
                .as_ref()
                .is_some_and(|body| block_has_unary(body, op)),
            Member::Type(ty) => program_has_unary(
                &new_java2rust::java_ir::Program {
                    package: None,
                    imports: Vec::new(),
                    types: vec![ty.clone()],
                },
                op,
            ),
            _ => false,
        })
    })
}

#[test]
fn parses_classic_for_loop() {
    let parsed = parse_java_source(&fixture("02-control-flow/LoopsAndBranches.java"));
    let method = parsed.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    assert!(
        method
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .any(|stmt| matches!(stmt, Stmt::For { .. }))
    );
}

#[test]
fn emits_syntax_based_wildcard_and_throws_diagnostics() {
    let wildcard = parse_java_source(&fixture("05-types/WildcardBounds.java"));
    assert!(wildcard.diagnostics.has_code("unsupported_wildcard"));

    let checked = parse_java_source(&fixture("06-stdlib/CheckedException.java"));
    assert!(checked.diagnostics.has_code("unsupported_exceptions"));
}

#[test]
fn parses_switch_statement_cases() {
    let parsed = parse_java_source(&fixture("02-control-flow/SwitchFallthrough.java"));
    let method = first_method(&parsed.program.types[0]);
    assert!(
        method
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .any(|stmt| matches!(stmt, Stmt::Switch { cases, .. } if cases.len() == 4))
    );
}

#[test]
fn parses_try_catch_finally_statement() {
    let parsed = parse_java_source(&fixture("06-stdlib/TryCatchFinally.java"));
    let method = first_method(&parsed.program.types[0]);
    assert!(
        method
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .any(|stmt| matches!(
                stmt,
                Stmt::Try {
                    catches,
                    finally_block: Some(_),
                    ..
                } if catches.len() == 1
            ))
    );
}

#[test]
fn parses_lambda_arguments_in_stream_fixture() {
    let parsed = parse_java_source(&fixture("06-stdlib/StreamsUse.java"));
    let method = first_method(&parsed.program.types[0]);
    assert!(block_has_lambda(method.body.as_ref().unwrap()));
}

#[test]
fn parses_interface_members_and_implements_list() {
    let parsed = parse_java_source(&fixture("04-objects/InterfaceDefault.java"));
    let interface = &parsed.program.types[0];
    assert_eq!(interface.name, "Named");
    assert!(
        interface
            .members
            .iter()
            .any(|member| matches!(member, Member::Method(method) if method.name == "name"))
    );

    let class = &parsed.program.types[1];
    assert_eq!(class.implements, vec!["Named"]);
}

#[test]
fn parses_local_class_declaration() {
    let parsed = parse_java_source(&fixture("04-objects/LocalClassCapture.java"));
    let method = first_method(&parsed.program.types[0]);
    assert!(
        method
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .any(|stmt| matches!(stmt, Stmt::LocalType(local) if local.name == "Adder"))
    );
}

#[test]
fn parses_constructor_delegation() {
    let parsed = parse_java_source(&fixture("03-methods/ConstructorOverloads.java"));
    let constructor = parsed.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(constructor)
                if constructor.body.as_ref().is_some_and(|body| {
                    body.statements.iter().any(|stmt| {
                        matches!(
                            stmt,
                            Stmt::ConstructorInvocation {
                                kind: ConstructorInvocationKind::This,
                                ..
                            }
                        )
                    })
                }) =>
            {
                Some(constructor)
            }
            _ => None,
        })
        .unwrap();
    assert!(constructor.params.is_empty());
}

#[test]
fn parses_anonymous_class_body() {
    let parsed = parse_java_source(&fixture("04-objects/AnonymousInterface.java"));
    let method = first_method(&parsed.program.types[0]);
    assert!(
        method
            .body
            .as_ref()
            .unwrap()
            .statements
            .iter()
            .any(|stmt| matches!(
                stmt,
                Stmt::Return(Some(Expr::New {
                    body: Some(members),
                    ..
                })) if members.iter().any(|member| matches!(
                    member,
                    Member::Method(method) if method.name == "test"
                ))
            ))
    );
}

fn first_method(ty: &new_java2rust::java_ir::TypeDecl) -> &new_java2rust::java_ir::MethodDecl {
    ty.members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap()
}

fn collect_java_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_java_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "java") {
            out.push(path);
        }
    }
}

fn collect_member_fallbacks(member: &Member, out: &mut Vec<String>) {
    match member {
        Member::Field(field) => {
            if let Some(expr) = &field.initializer {
                collect_expr_fallbacks(expr, out);
            }
        }
        Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_block_fallbacks(body, out);
            }
        }
        Member::Constructor(constructor) => {
            if let Some(body) = &constructor.body {
                collect_block_fallbacks(body, out);
            }
        }
        Member::Type(ty) => {
            for member in &ty.members {
                collect_member_fallbacks(member, out);
            }
        }
        Member::Unsupported { reason, .. } => out.push(format!("member unsupported: {reason}")),
    }
}

fn collect_block_fallbacks(block: &new_java2rust::java_ir::Block, out: &mut Vec<String>) {
    for stmt in &block.statements {
        collect_stmt_fallbacks(stmt, out);
    }
}

fn collect_stmt_fallbacks(stmt: &Stmt, out: &mut Vec<String>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                collect_expr_fallbacks(expr, out);
            }
        }
        Stmt::Assign { target, value } => {
            collect_expr_fallbacks(target, out);
            collect_expr_fallbacks(value, out);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_fallbacks(expr, out);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_fallbacks(condition, out);
            collect_block_fallbacks(then_block, out);
            if let Some(block) = else_block {
                collect_block_fallbacks(block, out);
            }
        }
        Stmt::While { condition, body } => {
            collect_expr_fallbacks(condition, out);
            collect_block_fallbacks(body, out);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_fallbacks(stmt, out);
            }
            if let Some(expr) = condition {
                collect_expr_fallbacks(expr, out);
            }
            for expr in update {
                collect_expr_fallbacks(expr, out);
            }
            collect_block_fallbacks(body, out);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_fallbacks(iterable, out);
            collect_block_fallbacks(body, out);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_fallbacks(selector, out);
            for case in cases {
                for label in &case.labels {
                    if let new_java2rust::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_expr_fallbacks(expr, out);
                    }
                }
                for stmt in &case.statements {
                    collect_stmt_fallbacks(stmt, out);
                }
            }
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            collect_block_fallbacks(body, out);
            for catch in catches {
                collect_block_fallbacks(&catch.body, out);
            }
            if let Some(block) = finally_block {
                collect_block_fallbacks(block, out);
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                collect_member_fallbacks(member, out);
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
        }
        Stmt::Block(block) => collect_block_fallbacks(block, out),
        Stmt::Unsupported(reason) => out.push(format!("stmt unsupported: {reason}")),
        _ => {}
    }
}

fn collect_expr_fallbacks(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Raw(raw) => out.push(format!("expr raw: {raw}")),
        Expr::FieldAccess(receiver, _) => collect_expr_fallbacks(receiver, out),
        Expr::ArrayAccess(array, index) => {
            collect_expr_fallbacks(array, out);
            collect_expr_fallbacks(index, out);
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_fallbacks(left, out);
            collect_expr_fallbacks(right, out);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_fallbacks(expr, out);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_fallbacks(condition, out);
            collect_expr_fallbacks(then_expr, out);
            collect_expr_fallbacks(else_expr, out);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_fallbacks(receiver, out);
            }
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_fallbacks(arg, out);
            }
            if let Some(members) = body {
                for member in members {
                    collect_member_fallbacks(member, out);
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            new_java2rust::java_ir::LambdaBody::Expr(expr) => collect_expr_fallbacks(expr, out),
            new_java2rust::java_ir::LambdaBody::Block(block) => collect_block_fallbacks(block, out),
        },
        _ => {}
    }
}

fn block_has_lambda(block: &new_java2rust::java_ir::Block) -> bool {
    block.statements.iter().any(stmt_has_lambda)
}

fn block_has_unary(block: &new_java2rust::java_ir::Block, op: UnaryOp) -> bool {
    block.statements.iter().any(|stmt| stmt_has_unary(stmt, op))
}

fn stmt_has_unary(stmt: &Stmt, op: UnaryOp) -> bool {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        }
        | Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr) => expr_has_unary(expr, op),
        Stmt::Assign { target, value } => expr_has_unary(target, op) || expr_has_unary(value, op),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_unary(condition, op)
                || block_has_unary(then_block, op)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_has_unary(block, op))
        }
        Stmt::While { condition, body } => {
            expr_has_unary(condition, op) || block_has_unary(body, op)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| stmt_has_unary(stmt, op))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_has_unary(expr, op))
                || update.iter().any(|expr| expr_has_unary(expr, op))
                || block_has_unary(body, op)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_unary(iterable, op) || block_has_unary(body, op)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_unary(selector, op)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(|stmt| stmt_has_unary(stmt, op)))
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            block_has_unary(body, op)
                || catches.iter().any(|catch| block_has_unary(&catch.body, op))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_has_unary(block, op))
        }
        Stmt::LocalType(ty) => ty.members.iter().any(|member| match member {
            Member::Method(method) => method
                .body
                .as_ref()
                .is_some_and(|body| block_has_unary(body, op)),
            _ => false,
        }),
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(|expr| expr_has_unary(expr, op))
        }
        Stmt::Block(block) => block_has_unary(block, op),
        _ => false,
    }
}

fn expr_has_unary(expr: &Expr, op: UnaryOp) -> bool {
    match expr {
        Expr::Unary { op: actual, expr } => *actual == op || expr_has_unary(expr, op),
        Expr::FieldAccess(receiver, _) => expr_has_unary(receiver, op),
        Expr::ArrayAccess(array, index) => expr_has_unary(array, op) || expr_has_unary(index, op),
        Expr::Binary { left, right, .. } => expr_has_unary(left, op) || expr_has_unary(right, op),
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => expr_has_unary(expr, op),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_has_unary(condition, op)
                || expr_has_unary(then_expr, op)
                || expr_has_unary(else_expr, op)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|expr| expr_has_unary(expr, op))
                || args.iter().any(|expr| expr_has_unary(expr, op))
        }
        Expr::New { args, .. } => args.iter().any(|expr| expr_has_unary(expr, op)),
        Expr::Lambda { body, .. } => match body {
            new_java2rust::java_ir::LambdaBody::Expr(expr) => expr_has_unary(expr, op),
            new_java2rust::java_ir::LambdaBody::Block(block) => block_has_unary(block, op),
        },
        _ => false,
    }
}

fn stmt_has_lambda(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        }
        | Stmt::Expr(expr)
        | Stmt::Return(Some(expr))
        | Stmt::Throw(expr) => expr_has_lambda(expr),
        Stmt::Assign { target, value } => expr_has_lambda(target) || expr_has_lambda(value),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_has_lambda(condition)
                || block_has_lambda(then_block)
                || else_block.as_ref().is_some_and(block_has_lambda)
        }
        Stmt::While { condition, body } => expr_has_lambda(condition) || block_has_lambda(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_has_lambda)
                || condition.as_ref().is_some_and(expr_has_lambda)
                || update.iter().any(expr_has_lambda)
                || block_has_lambda(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_has_lambda(iterable) || block_has_lambda(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_has_lambda(selector)
                || cases
                    .iter()
                    .any(|case| case.statements.iter().any(stmt_has_lambda))
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            block_has_lambda(body)
                || catches.iter().any(|catch| block_has_lambda(&catch.body))
                || finally_block.as_ref().is_some_and(block_has_lambda)
        }
        Stmt::Block(block) => block_has_lambda(block),
        _ => false,
    }
}

fn expr_has_lambda(expr: &Expr) -> bool {
    match expr {
        Expr::Lambda { .. } => true,
        Expr::FieldAccess(receiver, _) => expr_has_lambda(receiver),
        Expr::ArrayAccess(array, index) => expr_has_lambda(array) || expr_has_lambda(index),
        Expr::Binary { left, right, .. } => expr_has_lambda(left) || expr_has_lambda(right),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_has_lambda(expr)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => expr_has_lambda(condition) || expr_has_lambda(then_expr) || expr_has_lambda(else_expr),
        Expr::Call { receiver, args, .. } => {
            receiver.as_ref().is_some_and(|expr| expr_has_lambda(expr))
                || args.iter().any(expr_has_lambda)
        }
        Expr::New { args, .. } => args.iter().any(expr_has_lambda),
        _ => false,
    }
}
