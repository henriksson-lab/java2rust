use crate::diagnostics::DiagnosticBag;
use crate::facts::{CaptureOwnerKind, FactSet};
use crate::java_ir::*;
use crate::rust_ir::*;
use crate::types::{
    TypeEnv, default_rust_literal_for_java_type, infer_expr_type, java_type_to_rust,
    rust_return_type,
};

#[derive(Debug, Clone)]
pub struct LoweredProgram {
    pub file: RustFile,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone, Copy)]
struct LowerCtx<'a> {
    facts: &'a FactSet,
}

pub fn lower_program(program: &Program, diagnostics: DiagnosticBag) -> LoweredProgram {
    let facts = FactSet::default();
    lower_program_with_facts(program, diagnostics, &facts)
}

pub fn lower_program_with_facts(
    program: &Program,
    mut diagnostics: DiagnosticBag,
    facts: &FactSet,
) -> LoweredProgram {
    let ctx = LowerCtx { facts };
    let mut file = RustFile::default();
    if program_uses_java_exception(program) {
        file.items.push(RustItem::Raw(java_exception_support()));
    }
    for ty in &program.types {
        lower_type(ty, &mut file, &mut diagnostics, ctx);
    }
    LoweredProgram { file, diagnostics }
}

fn program_uses_java_exception(program: &Program) -> bool {
    program.types.iter().any(type_uses_java_exception)
}

fn type_uses_java_exception(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => !method.throws.is_empty(),
        Member::Constructor(constructor) => !constructor.throws.is_empty(),
        Member::Type(ty) => type_uses_java_exception(ty),
        _ => false,
    })
}

fn java_exception_support() -> String {
    r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message }
    }
}"#
    .to_string()
}

fn lower_type(ty: &TypeDecl, file: &mut RustFile, diagnostics: &mut DiagnosticBag, ctx: LowerCtx) {
    if ty.extends.is_some() {
        diagnostics.unsupported(
            "unsupported_inheritance",
            format!(
                "{} uses inheritance, which is outside the first slice",
                ty.name
            ),
        );
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(RustField {
                name: field.name.clone(),
                ty: crate::coercion::rust_storage_type(&field.ty),
                is_pub: false,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    file.items.push(RustItem::Struct(RustStruct {
        name: ty.name.clone(),
        fields,
    }));

    let mut functions = Vec::new();
    for member in &ty.members {
        match member {
            Member::Constructor(ctor) => functions.push(lower_constructor(ty, ctor, ctx)),
            Member::Method(method) => functions.push(lower_method(ty, method, ctx)),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    file.items.push(RustItem::Impl(RustImpl {
        target: ty.name.clone(),
        functions,
    }));
}

fn lower_constructor(ty: &TypeDecl, ctor: &ConstructorDecl, ctx: LowerCtx) -> RustFunction {
    let mut env = env_for_type(ty);
    for param in &ctor.params {
        env.insert_local(param.name.clone(), param.ty.clone());
    }
    if let Some(body) = &ctor.body
        && let Some(function) = lower_delegating_constructor(ty, ctor, body, &env, ctx)
    {
        return function;
    }
    let mut assigned = Vec::new();
    if let Some(body) = &ctor.body {
        for stmt in &body.statements {
            if let Stmt::Assign {
                target: Expr::FieldAccess(receiver, field),
                value,
            } = stmt
                && matches!(receiver.as_ref(), Expr::This)
            {
                let target_ty = env
                    .field_type(field)
                    .cloned()
                    .unwrap_or(JavaType::Unknown(field.clone()));
                assigned.push((
                    field.clone(),
                    lower_expr_for_slot(
                        value,
                        &env,
                        crate::coercion::Slot::Assignment(target_ty),
                        ctx,
                    ),
                ));
            }
            env.learn_stmt(stmt);
        }
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => {
                let value = assigned
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| value.clone())
                    .or_else(|| {
                        field.initializer.as_ref().map(|expr| {
                            lower_expr_for_slot(
                                expr,
                                &env,
                                crate::coercion::Slot::Assignment(field.ty.clone()),
                                ctx,
                            )
                        })
                    })
                    .unwrap_or_else(|| default_expr_for_type(&field.ty));
                Some((field.name.clone(), value))
            }
            _ => None,
        })
        .collect();

    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        params: ctor
            .params
            .iter()
            .map(|p| RustParam {
                name: p.name.clone(),
                ty: crate::coercion::rust_param_slot_type(&p.ty, p.is_varargs),
            })
            .collect(),
        return_type: Some(RustType::Named(ty.name.clone())),
        body: vec![RustStmt::Return(Some(RustExpr::StructLit {
            name: ty.name.clone(),
            fields,
        }))],
        is_pub: true,
        receiver: None,
    }
}

fn lower_delegating_constructor(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    body: &Block,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> Option<RustFunction> {
    let Some(Stmt::ConstructorInvocation {
        kind,
        args,
        resolved,
    }) = body.statements.first()
    else {
        return None;
    };

    let body = match kind {
        ConstructorInvocationKind::This => {
            if body.statements.len() != 1 {
                vec![RustStmt::Raw(
                    "compile_error!(\"unsupported Java constructor: this(...) followed by additional statements\");"
                        .to_string(),
                )]
            } else if let Some(resolved) = resolved {
                vec![RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::Raw(format!("Self::{}", resolved.rust_name))),
                    args: args.iter().map(|arg| lower_expr(arg, env, ctx)).collect(),
                }))]
            } else {
                vec![RustStmt::Raw(
                    "compile_error!(\"unresolved Java constructor delegation\");".to_string(),
                )]
            }
        }
        ConstructorInvocationKind::Super => vec![RustStmt::Raw(
            "compile_error!(\"unsupported Java constructor: super(...) requires inheritance lowering\");"
                .to_string(),
        )],
    };

    Some(RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        params: ctor
            .params
            .iter()
            .map(|p| RustParam {
                name: p.name.clone(),
                ty: crate::coercion::rust_param_slot_type(&p.ty, p.is_varargs),
            })
            .collect(),
        return_type: Some(RustType::Named(ty.name.clone())),
        body,
        is_pub: true,
        receiver: None,
    })
}

fn lower_method(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> RustFunction {
    let mutable_receiver = method.body.as_ref().is_some_and(method_mutates_self);
    let mut env = if method.modifiers.contains(&Modifier::Static) {
        TypeEnv::new()
    } else {
        env_for_type(ty)
    };
    for param in &method.params {
        env.insert_local(param.name.clone(), param.ty.clone());
    }
    RustFunction {
        name: method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
        params: method
            .params
            .iter()
            .map(|p| RustParam {
                name: p.name.clone(),
                ty: crate::coercion::rust_param_slot_type(&p.ty, p.is_varargs),
            })
            .collect(),
        return_type: if method.throws.is_empty() {
            rust_return_type(&method.return_type)
        } else {
            Some(exception_result_type(&method.return_type))
        },
        body: method
            .body
            .as_ref()
            .map(|body| {
                lower_block(
                    body,
                    &mut env,
                    Some(&method.return_type),
                    !method.throws.is_empty(),
                    ctx,
                )
            })
            .unwrap_or_default(),
        is_pub: true,
        receiver: if method.modifiers.contains(&Modifier::Static) {
            None
        } else if mutable_receiver {
            Some(RustReceiver::Mutable)
        } else {
            Some(RustReceiver::Shared)
        },
    }
}

fn exception_result_type(return_type: &JavaType) -> RustType {
    let ok_type = rust_return_type(return_type).unwrap_or(RustType::Unit);
    RustType::Raw(format!(
        "Result<{}, JavaException>",
        crate::emit::emit_type(&ok_type)
    ))
}

fn lower_block(
    block: &Block,
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut body = Vec::new();
    for stmt in &block.statements {
        lower_stmt(stmt, env, return_type, wrap_returns_in_ok, ctx, &mut body);
        env.learn_stmt(stmt);
    }
    body
}

fn lower_stmt(
    stmt: &Stmt,
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    out: &mut Vec<RustStmt>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => out.push(RustStmt::Let {
            mutable: block_reassigns_name(name, stmt),
            name: name.clone(),
            ty: Some(java_type_to_rust(ty)),
            value: initializer.as_ref().map(|expr| {
                lower_expr_for_slot(
                    expr,
                    env,
                    crate::coercion::Slot::Assignment(ty.clone()),
                    ctx,
                )
            }),
        }),
        Stmt::Assign { target, value } => out.push(RustStmt::Assign {
            target: lower_expr(target, env, ctx),
            value: lower_expr_for_slot(value, env, assignment_slot_for_target(target, env), ctx),
        }),
        Stmt::Expr(expr) => out.push(RustStmt::Expr(lower_expr(expr, env, ctx))),
        Stmt::Return(expr) => out.push(lower_return_stmt(
            expr.as_ref(),
            env,
            return_type,
            wrap_returns_in_ok,
            ctx,
        )),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => out.push(RustStmt::If {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            then_body: lower_block(
                then_block,
                &mut env.clone(),
                return_type,
                wrap_returns_in_ok,
                ctx,
            ),
            else_body: else_block
                .as_ref()
                .map(|block| {
                    lower_block(
                        block,
                        &mut env.clone(),
                        return_type,
                        wrap_returns_in_ok,
                        ctx,
                    )
                })
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            body: lower_block(body, &mut env.clone(), return_type, wrap_returns_in_ok, ctx),
        }),
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            for init_stmt in init {
                lower_stmt(init_stmt, env, return_type, wrap_returns_in_ok, ctx, out);
                env.learn_stmt(init_stmt);
            }
            let mut loop_body = lower_block_for_body(
                body,
                update,
                &mut env.clone(),
                return_type,
                wrap_returns_in_ok,
                ctx,
            );
            for expr in update {
                loop_body.push(RustStmt::Expr(lower_expr(expr, env, ctx)));
            }
            out.push(RustStmt::While {
                condition: condition
                    .as_ref()
                    .map(|expr| {
                        lower_expr_for_slot(expr, env, crate::coercion::Slot::Condition, ctx)
                    })
                    .unwrap_or_else(|| RustExpr::Literal("true".to_string())),
                body: if label.is_some() {
                    vec![RustStmt::Loop {
                        label: label.clone(),
                        body: loop_body,
                    }]
                } else {
                    loop_body
                },
            });
        }
        Stmt::EnhancedFor {
            var_name,
            iterable,
            body,
            ..
        } => out.push(RustStmt::Raw(format!(
            "for {var_name} in {}.iter() {{\n{}\n}}",
            crate::emit::emit_expr(&lower_expr(iterable, env, ctx)),
            crate::emit::emit_statements(
                &lower_block(body, &mut env.clone(), return_type, wrap_returns_in_ok, ctx),
                1
            )
        ))),
        Stmt::Switch { selector, cases } => {
            out.push(RustStmt::Raw(lower_switch(
                selector,
                cases,
                env,
                return_type,
                wrap_returns_in_ok,
                ctx,
            )));
        }
        Stmt::Try { .. } => out.push(RustStmt::Raw(
            "compile_error!(\"unsupported Java statement: try/catch/finally\");".to_string(),
        )),
        Stmt::Throw(expr) => out.push(RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw("Err".to_string())),
            args: vec![lower_throw_expr(expr, env, ctx)],
        }))),
        Stmt::LocalType(_) => out.push(RustStmt::Raw(
            "compile_error!(\"unsupported Java statement: local type declaration\");".to_string(),
        )),
        Stmt::ConstructorInvocation { .. } => out.push(RustStmt::Raw(
            "compile_error!(\"unsupported Java statement: constructor delegation\");".to_string(),
        )),
        Stmt::Break(label) => out.push(RustStmt::Break(label.clone())),
        Stmt::Continue(label) => out.push(RustStmt::Continue(label.clone())),
        Stmt::Block(block) => out.push(RustStmt::Raw(format!(
            "{{\n{}\n}}",
            crate::emit::emit_statements(
                &lower_block(
                    block,
                    &mut env.clone(),
                    return_type,
                    wrap_returns_in_ok,
                    ctx
                ),
                1
            )
        ))),
        Stmt::Unsupported(reason) => out.push(RustStmt::Raw(format!(
            "compile_error!({:?});",
            format!("unsupported Java statement: {reason}")
        ))),
    }
}

fn lower_switch(
    selector: &Expr,
    cases: &[SwitchCase],
    env: &TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> String {
    let switch_label = "java_switch";
    let switch_value = "__java_switch_value";
    let switch_matched = "__java_switch_matched";
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "    let {switch_value} = {};\n",
        crate::emit::emit_expr(&lower_expr(selector, env, ctx))
    ));
    out.push_str(&format!("    let mut {switch_matched} = false;\n"));
    out.push_str(&format!("    '{switch_label}: loop {{\n"));
    for case in cases {
        let label_condition = switch_label_condition(case, switch_value, switch_matched, env, ctx);
        out.push_str(&format!("        if {label_condition} {{\n"));
        out.push_str(&format!("            {switch_matched} = true;\n"));
        out.push_str("        }\n");
        out.push_str(&format!("        if {switch_matched} {{\n"));
        let statements = lower_switch_case_statements(
            case,
            switch_label,
            env,
            return_type,
            wrap_returns_in_ok,
            ctx,
        );
        out.push_str(&crate::emit::emit_statements(&statements, 3));
        out.push_str("        }\n");
    }
    out.push_str(&format!("        break '{switch_label};\n"));
    out.push_str("    }\n");
    out.push('}');
    out
}

fn switch_label_condition(
    case: &SwitchCase,
    switch_value: &str,
    switch_matched: &str,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> String {
    if case
        .labels
        .iter()
        .any(|label| matches!(label, SwitchLabel::Default))
    {
        return format!("!{switch_matched}");
    }
    let comparisons = case
        .labels
        .iter()
        .filter_map(|label| match label {
            SwitchLabel::Expr(expr) => Some(format!(
                "{switch_value} == {}",
                crate::emit::emit_expr(&lower_expr(expr, env, ctx))
            )),
            SwitchLabel::Default => None,
        })
        .collect::<Vec<_>>();
    if comparisons.is_empty() {
        "false".to_string()
    } else {
        format!("!{switch_matched} && ({})", comparisons.join(" || "))
    }
}

fn lower_switch_case_statements(
    case: &SwitchCase,
    switch_label: &str,
    env: &TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut case_env = env.clone();
    let mut out = Vec::new();
    for stmt in &case.statements {
        let stmt = rewrite_switch_breaks(stmt, switch_label);
        lower_stmt(
            &stmt,
            &mut case_env,
            return_type,
            wrap_returns_in_ok,
            ctx,
            &mut out,
        );
        case_env.learn_stmt(&stmt);
    }
    out
}

fn lower_return_stmt(
    expr: Option<&Expr>,
    env: &TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> RustStmt {
    let value = expr.map(|expr| {
        if let Some(return_type) = return_type {
            lower_expr_for_slot(
                expr,
                env,
                crate::coercion::Slot::Return(return_type.clone()),
                ctx,
            )
        } else {
            lower_expr(expr, env, ctx)
        }
    });
    if wrap_returns_in_ok {
        RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw("Ok".to_string())),
            args: vec![value.unwrap_or_else(|| RustExpr::Raw("()".to_string()))],
        }))
    } else {
        RustStmt::Return(value)
    }
}

fn lower_throw_expr(expr: &Expr, env: &TypeEnv, ctx: LowerCtx) -> RustExpr {
    match expr {
        Expr::New { ty, args, .. } => {
            let class_name = match ty {
                JavaType::Named { name, .. } => name.clone(),
                JavaType::Unknown(raw) => raw.clone(),
                _ => "JavaException".to_string(),
            };
            let message = args
                .first()
                .map(|arg| lower_expr(arg, env, ctx))
                .unwrap_or_else(|| RustExpr::Raw("String::new()".to_string()));
            RustExpr::Call {
                function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
                args: vec![RustExpr::Literal(format!("{class_name:?}")), message],
            }
        }
        _ => RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
            args: vec![
                RustExpr::Literal("\"JavaException\"".to_string()),
                RustExpr::Raw(format!(
                    "format!(\"{{:?}}\", {})",
                    crate::emit::emit_expr(&lower_expr(expr, env, ctx))
                )),
            ],
        },
    }
}

fn rewrite_switch_breaks(stmt: &Stmt, switch_label: &str) -> Stmt {
    match stmt {
        Stmt::Break(None) => Stmt::Break(Some(switch_label.to_string())),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => Stmt::If {
            condition: condition.clone(),
            then_block: rewrite_switch_breaks_in_block(then_block, switch_label),
            else_block: else_block
                .as_ref()
                .map(|block| rewrite_switch_breaks_in_block(block, switch_label)),
        },
        Stmt::Block(block) => Stmt::Block(rewrite_switch_breaks_in_block(block, switch_label)),
        _ => stmt.clone(),
    }
}

fn rewrite_switch_breaks_in_block(block: &Block, switch_label: &str) -> Block {
    Block {
        statements: block
            .statements
            .iter()
            .map(|stmt| rewrite_switch_breaks(stmt, switch_label))
            .collect(),
    }
}

fn lower_expr(expr: &Expr, env: &TypeEnv, ctx: LowerCtx) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        return stream_expr.expr;
    }
    if let Some(format_expr) = lower_string_concat(expr, env, ctx) {
        return format_expr;
    }
    match expr {
        Expr::Literal(lit) => RustExpr::Literal(literal_to_rust(lit)),
        Expr::Var(name) => {
            if env.local_type(name).is_none() && env.field_type(name).is_some() {
                RustExpr::Field(Box::new(RustExpr::Var("self".to_string())), name.clone())
            } else {
                RustExpr::Var(name.clone())
            }
        }
        Expr::This => RustExpr::Var("self".to_string()),
        Expr::Super => RustExpr::Raw("super".to_string()),
        Expr::FieldAccess(receiver, field) => {
            if field == "length" {
                return RustExpr::Raw(format!(
                    "{}.len() as i32",
                    crate::emit::emit_expr(&lower_expr(receiver, env, ctx))
                ));
            }
            RustExpr::Field(Box::new(lower_expr(receiver, env, ctx)), field.clone())
        }
        Expr::ArrayAccess(array, index) => RustExpr::Index(
            Box::new(lower_expr(array, env, ctx)),
            Box::new(lower_expr_for_slot(
                index,
                env,
                crate::coercion::Slot::Index,
                ctx,
            )),
        ),
        Expr::Binary { op, left, right } => RustExpr::Binary {
            op: binary_op_to_rust(*op).to_string(),
            left: Box::new(lower_expr(left, env, ctx)),
            right: Box::new(lower_expr(right, env, ctx)),
        },
        Expr::Unary { op, expr } => match op {
            UnaryOp::PostInc => RustExpr::Raw(format!(
                "{} += 1",
                crate::emit::emit_expr(&lower_expr(expr, env, ctx))
            )),
            UnaryOp::PostDec => RustExpr::Raw(format!(
                "{} -= 1",
                crate::emit::emit_expr(&lower_expr(expr, env, ctx))
            )),
            UnaryOp::Not => RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(lower_expr(expr, env, ctx)),
            },
            UnaryOp::Neg => RustExpr::Unary {
                op: "-".to_string(),
                expr: Box::new(lower_expr(expr, env, ctx)),
            },
            UnaryOp::PreInc | UnaryOp::PreDec => {
                RustExpr::Raw(crate::emit::emit_expr(&lower_expr(expr, env, ctx)))
            }
        },
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => RustExpr::If {
            condition: Box::new(lower_expr_for_slot(
                condition,
                env,
                crate::coercion::Slot::Condition,
                ctx,
            )),
            then_expr: Box::new(lower_expr(then_expr, env, ctx)),
            else_expr: Box::new(lower_expr(else_expr, env, ctx)),
        },
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let rust_args = args.iter().map(|arg| lower_expr(arg, env, ctx)).collect();
            if let Some(call) = resolved {
                match (&call.kind, receiver) {
                    (CallKind::Static, _) => RustExpr::Call {
                        function: Box::new(RustExpr::Raw(format!("Self::{}", call.rust_name))),
                        args: rust_args,
                    },
                    (CallKind::Instance, Some(receiver)) => RustExpr::MethodCall {
                        receiver: Box::new(lower_expr(receiver, env, ctx)),
                        method: call.rust_name.clone(),
                        args: rust_args,
                    },
                    (CallKind::Instance, None) => RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("self".to_string())),
                        method: call.rust_name.clone(),
                        args: rust_args,
                    },
                }
            } else if let Some(receiver) = receiver {
                RustExpr::MethodCall {
                    receiver: Box::new(lower_expr(receiver, env, ctx)),
                    method: crate::parser::to_snake_case(name),
                    args: rust_args,
                }
            } else {
                RustExpr::Call {
                    function: Box::new(RustExpr::Var(crate::parser::to_snake_case(name))),
                    args: rust_args,
                }
            }
        }
        Expr::New { ty, args, .. } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!(
                "{}::new",
                crate::emit::emit_type(&java_type_to_rust(ty))
            ))),
            args: args.iter().map(|arg| lower_expr(arg, env, ctx)).collect(),
        },
        Expr::Cast { ty: _, expr } => lower_expr(expr, env, ctx),
        Expr::InstanceOf { .. } => {
            RustExpr::Raw("compile_error!(\"unsupported Java expression: instanceof\")".to_string())
        }
        Expr::Lambda { .. } => {
            RustExpr::Raw("compile_error!(\"unsupported Java expression: lambda\")".to_string())
        }
        Expr::MethodReference { .. } => RustExpr::Raw(
            "compile_error!(\"unsupported Java expression: method reference\")".to_string(),
        ),
        Expr::ClassLiteral(_) => RustExpr::Raw(
            "compile_error!(\"unsupported Java expression: class literal\")".to_string(),
        ),
        Expr::Raw(raw) => RustExpr::Raw(raw.clone()),
    }
}

fn lower_stream_expr(
    expr: &Expr,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> Option<crate::coercion::LoweredExpr> {
    let plan = build_stream_plan(expr, env, ctx)?;
    lower_stream_plan(&plan, env, ctx)
}

fn stream_lambdas_are_capture_free(ctx: LowerCtx) -> bool {
    ctx.facts
        .captures
        .owners
        .iter()
        .filter(|owner| owner.kind == CaptureOwnerKind::Lambda)
        .all(|owner| owner.captures.is_empty())
}

fn is_collectors_to_list(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "toList"
            && args.is_empty()
            && matches!(receiver.as_ref(), Expr::Var(owner) if owner == "Collectors" || owner == "collectors")
    )
}

#[derive(Debug, Clone, Copy)]
enum StreamLambdaValue {
    FilterBorrowed,
    BorrowedItem,
}

struct StreamPlan<'a> {
    source: StreamSource<'a>,
    ops: Vec<StreamOp<'a>>,
    terminal: StreamTerminal<'a>,
    element: StreamElement,
}

struct StreamSource<'a> {
    expr: &'a Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamElement {
    ty: JavaType,
    value_category: StreamValueCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamValueCategory {
    Borrowed,
}

enum StreamOp<'a> {
    Filter(LambdaPlan<'a>),
    Map(LambdaPlan<'a>),
    MapToPrimitive(LambdaPlan<'a>),
}

enum StreamTerminal<'a> {
    Sum,
    CollectToList,
    FindFirst,
    AnyMatch(LambdaPlan<'a>),
}

struct LambdaPlan<'a> {
    param_name: String,
    body: &'a Expr,
}

fn build_stream_plan<'a>(expr: &'a Expr, env: &TypeEnv, ctx: LowerCtx) -> Option<StreamPlan<'a>> {
    if !stream_lambdas_are_capture_free(ctx) {
        return None;
    }
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };

    let terminal = match name.as_str() {
        "sum" if args.is_empty() => StreamTerminal::Sum,
        "collect" if args.len() == 1 && is_collectors_to_list(&args[0]) => {
            StreamTerminal::CollectToList
        }
        "findFirst" if args.is_empty() => StreamTerminal::FindFirst,
        "anyMatch" if args.len() == 1 => StreamTerminal::AnyMatch(lambda_plan(&args[0])?),
        _ => return None,
    };
    let (source, ops) = collect_stream_chain(receiver)?;
    let element_ty = stream_source_element_type(source, env)?;
    Some(StreamPlan {
        source: StreamSource { expr: source },
        ops,
        terminal,
        element: StreamElement {
            ty: element_ty,
            value_category: StreamValueCategory::Borrowed,
        },
    })
}

fn collect_stream_chain<'a>(expr: &'a Expr) -> Option<(&'a Expr, Vec<StreamOp<'a>>)> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };

    match name.as_str() {
        "stream" if args.is_empty() => receiver.as_deref().map(|source| (source, Vec::new())),
        "filter" if args.len() == 1 => {
            let (source, mut ops) = collect_stream_chain(receiver.as_deref()?)?;
            ops.push(StreamOp::Filter(lambda_plan(&args[0])?));
            Some((source, ops))
        }
        "map" if args.len() == 1 => {
            let (source, mut ops) = collect_stream_chain(receiver.as_deref()?)?;
            ops.push(StreamOp::Map(lambda_plan(&args[0])?));
            Some((source, ops))
        }
        "mapToInt" if args.len() == 1 => {
            let (source, mut ops) = collect_stream_chain(receiver.as_deref()?)?;
            ops.push(StreamOp::MapToPrimitive(lambda_plan(&args[0])?));
            Some((source, ops))
        }
        _ => None,
    }
}

fn lambda_plan(lambda: &Expr) -> Option<LambdaPlan<'_>> {
    let Expr::Lambda { params, body } = lambda else {
        return None;
    };
    let param = params.first()?.name.clone();
    let LambdaBody::Expr(body) = body else {
        return None;
    };
    Some(LambdaPlan {
        param_name: param,
        body,
    })
}

fn stream_source_element_type(source: &Expr, env: &TypeEnv) -> Option<JavaType> {
    match infer_expr_type(source, env) {
        JavaType::Named { name, args } if name == "List" && args.len() == 1 => {
            Some(normalize_stream_element_type(args[0].clone()))
        }
        JavaType::Array(inner) => Some(*inner),
        _ => None,
    }
}

fn normalize_stream_element_type(ty: JavaType) -> JavaType {
    match ty {
        JavaType::Named { name, args } if name == "Integer" && args.is_empty() => {
            JavaType::Primitive(PrimitiveType::Int)
        }
        other => other,
    }
}

fn lower_stream_plan(
    plan: &StreamPlan<'_>,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> Option<crate::coercion::LoweredExpr> {
    let mut out = format!(
        "{}.iter()",
        crate::emit::emit_expr(&lower_expr(plan.source.expr, env, ctx))
    );
    let mut element_actual = plan.element.ty.clone();
    for op in &plan.ops {
        match op {
            StreamOp::Filter(lambda) => {
                out.push_str(&format!(
                    ".filter(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::FilterBorrowed
                    )?
                ));
            }
            StreamOp::Map(lambda) => {
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::BorrowedItem
                    )?
                ));
                element_actual = stream_lambda_body_type(lambda.body, &plan.element.ty);
            }
            StreamOp::MapToPrimitive(lambda) => {
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::BorrowedItem
                    )?
                ));
                element_actual = JavaType::Primitive(PrimitiveType::Int);
            }
        }
    }

    let actual = match &plan.terminal {
        StreamTerminal::Sum => {
            out.push_str(".sum::<i32>()");
            crate::coercion::LoweredType::Java(JavaType::Primitive(PrimitiveType::Int))
        }
        StreamTerminal::CollectToList => {
            out.push_str(".collect::<Vec<_>>()");
            if element_actual == JavaType::String
                && plan
                    .ops
                    .iter()
                    .any(|op| matches!(op, StreamOp::Map(lambda) if stream_lambda_body_is_borrowed_string(lambda.body)))
            {
                crate::coercion::LoweredType::VecBorrowedString
            } else {
                crate::coercion::LoweredType::Java(JavaType::Named {
                    name: "List".to_string(),
                    args: vec![element_actual],
                })
            }
        }
        StreamTerminal::FindFirst => {
            out.push_str(".next()");
            crate::coercion::LoweredType::OptionalBorrowed(Box::new(element_actual))
        }
        StreamTerminal::AnyMatch(lambda) => {
            out.push_str(&format!(
                ".any(|{}| {})",
                lambda.param_name,
                emit_stream_lambda_body(
                    lambda.body,
                    &lambda.param_name,
                    StreamLambdaValue::BorrowedItem
                )?
            ));
            crate::coercion::LoweredType::Java(JavaType::Primitive(PrimitiveType::Boolean))
        }
    };
    Some(crate::coercion::LoweredExpr {
        expr: RustExpr::Raw(out),
        actual,
    })
}

fn emit_stream_lambda_body(expr: &Expr, param: &str, value: StreamLambdaValue) -> Option<String> {
    match expr {
        Expr::Literal(lit) => Some(literal_to_rust(lit)),
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed => format!("**{param}"),
            StreamLambdaValue::BorrowedItem => format!("*{param}"),
        }),
        Expr::Var(name) => Some(name.clone()),
        Expr::Binary { op, left, right } => Some(format!(
            "({} {} {})",
            emit_stream_lambda_body(left, param, value)?,
            binary_op_to_rust(*op),
            emit_stream_lambda_body(right, param, value)?
        )),
        Expr::Unary { op, expr } => {
            let op = match op {
                UnaryOp::Not => "!",
                UnaryOp::Neg => "-",
                _ => return None,
            };
            Some(format!(
                "{op}{}",
                emit_stream_lambda_body(expr, param, value)?
            ))
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.is_empty() => {
            let receiver = emit_stream_lambda_receiver(receiver, param, value)?;
            match name.as_str() {
                "isEmpty" => Some(format!("{receiver}.is_empty()")),
                "length" => Some(format!("{receiver}.len() as i32")),
                "trim" => Some(format!("{receiver}.trim()")),
                _ => Some(format!(
                    "{}.{}()",
                    receiver,
                    crate::parser::to_snake_case(name)
                )),
            }
        }
        _ => None,
    }
}

fn emit_stream_lambda_receiver(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
) -> Option<String> {
    match expr {
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed | StreamLambdaValue::BorrowedItem => {
                param.to_string()
            }
        }),
        _ => emit_stream_lambda_body(expr, param, value),
    }
}

fn stream_lambda_body_type(expr: &Expr, default: &JavaType) -> JavaType {
    if stream_lambda_body_is_borrowed_string(expr) {
        JavaType::String
    } else {
        default.clone()
    }
}

fn stream_lambda_body_is_borrowed_string(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            name,
            args,
            ..
        } if name == "trim" && args.is_empty()
    )
}

fn lower_expr_for_slot(
    expr: &Expr,
    env: &TypeEnv,
    slot: crate::coercion::Slot,
    ctx: LowerCtx,
) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        return crate::coercion::apply_lowered_expr_to_slot(stream_expr, &slot);
    }
    let actual = infer_expr_type(expr, env);
    let expr = lower_expr(expr, env, ctx);
    let plan = crate::coercion::plan_coercion(&actual, &slot);
    crate::coercion::apply_coercions(expr, &plan)
}

fn lower_string_concat(expr: &Expr, env: &TypeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    if !crate::types::is_string_concat(expr, env) {
        return None;
    }
    let mut parts = Vec::new();
    crate::types::flatten_add(expr, &mut parts);
    let format_string = parts
        .iter()
        .map(|part| match part {
            Expr::Literal(Literal::String(s)) => s.replace('{', "{{").replace('}', "}}"),
            _ => "{}".to_string(),
        })
        .collect::<String>();
    let args = parts
        .iter()
        .filter(|part| !matches!(part, Expr::Literal(Literal::String(_))))
        .map(|part| crate::emit::emit_expr(&lower_expr(part, env, ctx)))
        .collect::<Vec<_>>();
    let mut raw = format!("format!({format_string:?}");
    for arg in args {
        raw.push_str(", ");
        raw.push_str(&arg);
    }
    raw.push(')');
    Some(RustExpr::Raw(raw))
}

fn literal_to_rust(lit: &Literal) -> String {
    match lit {
        Literal::Null => "None".to_string(),
        Literal::Bool(value) => value.to_string(),
        Literal::Int(value) => value.to_string(),
        Literal::Long(value) => format!("{value}i64"),
        Literal::Float(value) => format!("{value}f32"),
        Literal::Double(value) => format!("{value}f64"),
        Literal::String(value) => format!("{value:?}.to_string()"),
        Literal::Char(value) => format!("{value:?}"),
    }
}

fn lower_block_for_body(
    block: &Block,
    update: &[Expr],
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut out = Vec::new();
    for stmt in &block.statements {
        lower_stmt_for_body(
            stmt,
            update,
            env,
            return_type,
            wrap_returns_in_ok,
            ctx,
            &mut out,
        );
        env.learn_stmt(stmt);
    }
    out
}

fn lower_stmt_for_body(
    stmt: &Stmt,
    update: &[Expr],
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    out: &mut Vec<RustStmt>,
) {
    match stmt {
        Stmt::Continue(None) => {
            for expr in update {
                out.push(RustStmt::Expr(lower_expr(expr, env, ctx)));
            }
            out.push(RustStmt::Continue(None));
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => out.push(RustStmt::If {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            then_body: lower_block_for_body(
                then_block,
                update,
                &mut env.clone(),
                return_type,
                wrap_returns_in_ok,
                ctx,
            ),
            else_body: else_block
                .as_ref()
                .map(|block| {
                    lower_block_for_body(
                        block,
                        update,
                        &mut env.clone(),
                        return_type,
                        wrap_returns_in_ok,
                        ctx,
                    )
                })
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            body: lower_block(body, &mut env.clone(), return_type, wrap_returns_in_ok, ctx),
        }),
        other => lower_stmt(other, env, return_type, wrap_returns_in_ok, ctx, out),
    }
}

fn binary_op_to_rust(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

fn default_expr_for_type(ty: &JavaType) -> RustExpr {
    RustExpr::Raw(default_rust_literal_for_java_type(ty).to_string())
}

fn env_for_type(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    env
}

fn assignment_slot_for_target(target: &Expr, env: &TypeEnv) -> crate::coercion::Slot {
    match target {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .map(crate::coercion::Slot::Assignment)
            .unwrap_or(crate::coercion::Slot::Value),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => env
            .field_type(field)
            .cloned()
            .map(crate::coercion::Slot::Assignment)
            .unwrap_or(crate::coercion::Slot::Value),
        Expr::ArrayAccess(array, _) => {
            let array_ty = infer_expr_type(array, env);
            if let JavaType::Array(inner) = array_ty {
                crate::coercion::Slot::Assignment(*inner)
            } else {
                crate::coercion::Slot::Value
            }
        }
        _ => crate::coercion::Slot::Value,
    }
}

fn method_mutates_self(block: &Block) -> bool {
    block.statements.iter().any(stmt_mutates_self)
}

fn stmt_mutates_self(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Assign { target, .. } => target_is_self_field(target),
        Stmt::Expr(Expr::Unary { expr, .. }) => target_is_self_field(expr),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            method_mutates_self(then_block) || else_block.as_ref().is_some_and(method_mutates_self)
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::EnhancedFor { body, .. } => {
            method_mutates_self(body)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_mutates_self)),
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            method_mutates_self(body)
                || catches.iter().any(|catch| method_mutates_self(&catch.body))
                || finally_block.as_ref().is_some_and(method_mutates_self)
        }
        _ => false,
    }
}

fn target_is_self_field(expr: &Expr) -> bool {
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This))
}

fn block_reassigns_name(_name: &str, _stmt: &Stmt) -> bool {
    true
}
