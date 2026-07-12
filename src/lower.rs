use crate::diagnostics::DiagnosticBag;
use crate::facts::{CaptureOwnerKind, FactSet};
use crate::java_ir::*;
use crate::rust_ir::*;
use crate::signature::{
    LocalPlan, ParamPlan, ReceiverPlan, SignaturePlan, default_param_plan, default_receiver_plan,
    default_return_shape,
};
use crate::slot_shape::RustSlotShape;
use crate::types::{
    ExprShapeContext, TypeEnv, infer_assignment_target_shape_with_context,
    infer_expr_shape_with_context, infer_stream_lambda_body_shape, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape,
};
use crate::{
    representation::{RepresentationPlan, RepresentationTypeEmission},
    usage::UsageFacts,
};

#[derive(Debug, Clone)]
pub struct LoweredProgram {
    pub file: RustFile,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone, Copy)]
struct LowerCtx<'a> {
    facts: &'a FactSet,
    representations: &'a RepresentationPlan,
    signatures: &'a SignaturePlan,
    current_local_shapes: &'a [LocalPlan],
    current_return_shape: Option<&'a RustSlotShape>,
}

pub fn lower_program(program: &Program, diagnostics: DiagnosticBag) -> LoweredProgram {
    let facts = FactSet::default();
    lower_program_with_facts(program, diagnostics, &facts)
}

pub fn lower_program_with_facts(
    program: &Program,
    diagnostics: DiagnosticBag,
    facts: &FactSet,
) -> LoweredProgram {
    let usage = crate::usage::analyze_usage(program);
    let representations = crate::representation::plan_representations(program, facts);
    let signatures = crate::signature::solve_signatures(program, &representations, &usage);
    lower_program_with_plans(
        program,
        diagnostics,
        facts,
        &usage,
        &representations,
        &signatures,
    )
}

pub fn lower_program_with_plans(
    program: &Program,
    mut diagnostics: DiagnosticBag,
    facts: &FactSet,
    _usage: &UsageFacts,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
) -> LoweredProgram {
    for diagnostic in representations.diagnostics_for_program(program) {
        diagnostics.unsupported(diagnostic.code, diagnostic.message);
    }
    let ctx = LowerCtx {
        facts,
        representations,
        signatures,
        current_local_shapes: &[],
        current_return_shape: None,
    };
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
    if let RepresentationTypeEmission::RawRustItem(item) =
        ctx.representations.type_emission_for(&ty.name)
    {
        file.items.push(RustItem::Raw(item));
        return;
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(RustField {
                name: field.name.clone(),
                ty: field_shape_or_storage_default(ctx, &ty.name, &field.name, &field.ty)
                    .rust_type(),
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
    let constructor_local_shapes = ctx.signatures.constructor_local_shapes_or_empty(ty, ctor);
    let constructor_ctx = LowerCtx {
        current_local_shapes: constructor_local_shapes,
        ..ctx
    };
    let mut env = env_for_type(ty, ctx);
    if let Some(signature) = ctx.signatures.constructor_signature(ty, ctor) {
        for param in &signature.params {
            env.insert_local_shape(param.name.clone(), param.shape.clone());
        }
    } else {
        for param in &ctor.params {
            env.insert_local(param.name.clone(), param.ty.clone());
        }
    }
    if let Some(body) = &ctor.body
        && let Some(function) = lower_delegating_constructor(ty, ctor, body, &env, constructor_ctx)
    {
        return function;
    }
    let mut assigned = Vec::new();
    if let Some(body) = &ctor.body {
        for stmt in &body.statements {
            if let Stmt::Assign { target, value } = stmt
                && let Expr::FieldAccess(receiver, field) = target
                && matches!(receiver.as_ref(), Expr::This)
            {
                assigned.push((
                    field.clone(),
                    lower_expr_for_slot(
                        value,
                        &env,
                        assignment_slot_for_target(target, &env, constructor_ctx),
                        constructor_ctx,
                    ),
                ));
            }
            learn_stmt_shapes(stmt, &mut env, constructor_ctx);
        }
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => {
                let field_shape =
                    field_shape_or_storage_default(ctx, &ty.name, &field.name, &field.ty);
                let value = assigned
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| value.clone())
                    .or_else(|| {
                        field.initializer.as_ref().map(|expr| {
                            lower_expr_for_slot(
                                expr,
                                &env,
                                crate::coercion::Slot::Assignment(field_shape.clone()),
                                constructor_ctx,
                            )
                        })
                    })
                    .unwrap_or_else(|| default_expr_for_shape(&field_shape));
                Some((field.name.clone(), value))
            }
            _ => None,
        })
        .collect();

    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        params: constructor_params(ty, ctor, constructor_ctx),
        return_type: Some(constructor_return_type(ty, ctor, ctx)),
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
                    args: lower_constructor_call_args(args, env, resolved, ctx),
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
        params: constructor_params(ty, ctor, ctx),
        return_type: Some(constructor_return_type(ty, ctor, ctx)),
        body,
        is_pub: true,
        receiver: None,
    })
}

fn constructor_params(ty: &TypeDecl, ctor: &ConstructorDecl, ctx: LowerCtx) -> Vec<RustParam> {
    constructor_param_plans(ty, ctor, ctx)
        .into_iter()
        .map(|param| RustParam {
            name: param.name.clone(),
            ty: ctx.signatures.param_rust_type(&param),
        })
        .collect()
}

fn constructor_return_type(ty: &TypeDecl, ctor: &ConstructorDecl, ctx: LowerCtx) -> RustType {
    ctx.signatures
        .constructor_return_type(ty, ctor)
        .unwrap_or_else(|| RustType::Named(ty.name.clone()))
}

fn constructor_param_plans(ty: &TypeDecl, ctor: &ConstructorDecl, ctx: LowerCtx) -> Vec<ParamPlan> {
    ctx.signatures
        .constructor_param_plans(ty, ctor)
        .map(|params| params.to_vec())
        .unwrap_or_else(|| ctor.params.iter().map(default_param_plan).collect())
}

fn method_return_type(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> Option<RustType> {
    method_return_shape(ty, method, ctx).map(|shape| shape.rust_type())
}

fn method_return_shape(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> Option<RustSlotShape> {
    ctx.signatures
        .method_return_shape(ty, method)
        .or_else(|| default_return_shape(&method.return_type))
}

fn method_ok_type_or_unit(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> RustType {
    method_return_type(ty, method, ctx).unwrap_or(RustType::Unit)
}

fn method_receiver(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> ReceiverPlan {
    ctx.signatures
        .method_receiver(ty, method)
        .unwrap_or_else(|| default_receiver_plan(method))
}

fn lower_method(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> RustFunction {
    let method_signature = ctx.signatures.method_signature(ty, method);
    let method_ctx = LowerCtx {
        current_return_shape: method_signature
            .and_then(|signature| signature.return_shape.as_ref()),
        current_local_shapes: ctx.signatures.method_local_shapes_or_empty(ty, method),
        ..ctx
    };
    let mut env = if method.modifiers.contains(&Modifier::Static) {
        TypeEnv::new()
    } else {
        env_for_type(ty, method_ctx)
    };
    if let Some(signature) = method_signature {
        for param in &signature.params {
            env.insert_local_shape(param.name.clone(), param.shape.clone());
        }
    } else {
        for param in &method.params {
            env.insert_local(param.name.clone(), param.ty.clone());
        }
    }
    RustFunction {
        name: method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
        params: ctx
            .signatures
            .method_param_plans(ty, method)
            .map(|params| params.to_vec())
            .unwrap_or_else(|| method.params.iter().map(default_param_plan).collect())
            .into_iter()
            .map(|param| RustParam {
                name: param.name.clone(),
                ty: ctx.signatures.param_rust_type(&param),
            })
            .collect(),
        return_type: if method.throws.is_empty() {
            method_return_type(ty, method, ctx)
        } else {
            Some(exception_result_type(method_ok_type_or_unit(
                ty, method, ctx,
            )))
        },
        body: method
            .body
            .as_ref()
            .map(|body| lower_block(body, &mut env, !method.throws.is_empty(), method_ctx))
            .unwrap_or_default(),
        is_pub: true,
        receiver: lower_receiver(method_receiver(ty, method, ctx)),
    }
}

fn lower_receiver(receiver: ReceiverPlan) -> Option<RustReceiver> {
    match receiver {
        ReceiverPlan::None => None,
        ReceiverPlan::Shared => Some(RustReceiver::Shared),
        ReceiverPlan::Mutable => Some(RustReceiver::Mutable),
    }
}

fn exception_result_type(ok_type: RustType) -> RustType {
    RustType::Raw(format!(
        "Result<{}, JavaException>",
        crate::emit::emit_type(&ok_type)
    ))
}

fn lower_block(
    block: &Block,
    env: &mut TypeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut body = Vec::new();
    for stmt in &block.statements {
        lower_stmt(stmt, env, wrap_returns_in_ok, ctx, &mut body);
        learn_stmt_shapes(stmt, env, ctx);
    }
    body
}

fn lower_stmt(
    stmt: &Stmt,
    env: &mut TypeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    out: &mut Vec<RustStmt>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => {
            let target_shape = local_shape_or_storage_default(ctx, name, ty);
            out.push(RustStmt::Let {
                mutable: ctx
                    .signatures
                    .local_binding_mutability(ctx.current_local_shapes, name)
                    .unwrap_or(false),
                name: name.clone(),
                ty: Some(target_shape.rust_type()),
                value: initializer.as_ref().map(|expr| {
                    lower_expr_for_slot(
                        expr,
                        env,
                        crate::coercion::Slot::Assignment(target_shape.clone()),
                        ctx,
                    )
                }),
            })
        }
        Stmt::Assign { target, value } => out.push(RustStmt::Assign {
            target: lower_expr(target, env, ctx),
            value: lower_expr_for_slot(
                value,
                env,
                assignment_slot_for_target(target, env, ctx),
                ctx,
            ),
        }),
        Stmt::Expr(expr) => out.push(RustStmt::Expr(lower_expr(expr, env, ctx))),
        Stmt::Return(expr) => out.push(lower_return_stmt(
            expr.as_ref(),
            env,
            wrap_returns_in_ok,
            ctx,
        )),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => out.push(RustStmt::If {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            then_body: lower_block(then_block, &mut env.clone(), wrap_returns_in_ok, ctx),
            else_body: else_block
                .as_ref()
                .map(|block| lower_block(block, &mut env.clone(), wrap_returns_in_ok, ctx))
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            body: lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
        }),
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            for init_stmt in init {
                lower_stmt(init_stmt, env, wrap_returns_in_ok, ctx, out);
                learn_stmt_shapes(init_stmt, env, ctx);
            }
            let mut loop_body =
                lower_block_for_body(body, update, &mut env.clone(), wrap_returns_in_ok, ctx);
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
                &lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
                1
            )
        ))),
        Stmt::Switch { selector, cases } => {
            out.push(RustStmt::Raw(lower_switch(
                selector,
                cases,
                env,
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
                &lower_block(block, &mut env.clone(), wrap_returns_in_ok, ctx),
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
        let statements =
            lower_switch_case_statements(case, switch_label, env, wrap_returns_in_ok, ctx);
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
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut case_env = env.clone();
    let mut out = Vec::new();
    for stmt in &case.statements {
        let stmt = rewrite_switch_breaks(stmt, switch_label);
        lower_stmt(&stmt, &mut case_env, wrap_returns_in_ok, ctx, &mut out);
        learn_stmt_shapes(&stmt, &mut case_env, ctx);
    }
    out
}

fn lower_return_stmt(
    expr: Option<&Expr>,
    env: &TypeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> RustStmt {
    let value = expr.map(|expr| {
        if let Some(return_shape) = ctx.current_return_shape {
            lower_expr_for_slot(
                expr,
                env,
                crate::coercion::Slot::Return(return_shape.clone()),
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
            if let Some(call) = resolved {
                let rust_args = lower_call_args(args, env, call, ctx);
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
                let rust_args = args.iter().map(|arg| lower_expr(arg, env, ctx)).collect();
                RustExpr::MethodCall {
                    receiver: Box::new(lower_expr(receiver, env, ctx)),
                    method: crate::parser::to_snake_case(name),
                    args: rust_args,
                }
            } else {
                let rust_args = args.iter().map(|arg| lower_expr(arg, env, ctx)).collect();
                RustExpr::Call {
                    function: Box::new(RustExpr::Var(crate::parser::to_snake_case(name))),
                    args: rust_args,
                }
            }
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            let rust_name = resolved
                .as_ref()
                .map(|resolved| resolved.rust_name.as_str())
                .unwrap_or("new");
            let owner_type = ctx.representations.constructor_owner_type_for_new(
                ty,
                resolved
                    .as_ref()
                    .map(|resolved| resolved.declaring_type.as_str()),
            );
            RustExpr::Call {
                function: Box::new(RustExpr::Raw(format!(
                    "{}::{rust_name}",
                    crate::emit::emit_type(&owner_type)
                ))),
                args: resolved
                    .as_ref()
                    .map(|resolved| lower_constructor_call_args(args, env, resolved, ctx))
                    .unwrap_or_else(|| args.iter().map(|arg| lower_expr(arg, env, ctx)).collect()),
            }
        }
        Expr::Cast { ty, expr } => {
            let actual = infer_expr_shape_with_context(
                expr,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                },
            );
            let target = ctx.representations.value_shape_for_java_type(ty.clone());
            crate::coercion::apply_expr_to_shape(lower_expr(expr, env, ctx), &actual, &target)
        }
        Expr::InstanceOf { expr, ty } => {
            let actual = infer_expr_shape_with_context(
                expr,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                },
            );
            let target = ctx.representations.value_shape_for_java_type(ty.clone());
            crate::coercion::apply_instanceof_check(lower_expr(expr, env, ctx), &actual, &target)
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

fn lower_call_args(
    args: &[Expr],
    env: &TypeEnv,
    call: &ResolvedCall,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            ctx.signatures
                .method_arg_shape_by_rust_name(&call.declaring_type, &call.rust_name, index)
                .map(|shape| {
                    lower_expr_for_slot(arg, env, crate::coercion::Slot::Argument(shape), ctx)
                })
                .unwrap_or_else(|| lower_expr(arg, env, ctx))
        })
        .collect()
}

fn lower_constructor_call_args(
    args: &[Expr],
    env: &TypeEnv,
    resolved: &ResolvedConstructor,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            ctx.signatures
                .constructor_arg_shape_by_rust_name(
                    &resolved.declaring_type,
                    &resolved.rust_name,
                    index,
                )
                .map(|shape| {
                    lower_expr_for_slot(arg, env, crate::coercion::Slot::Argument(shape), ctx)
                })
                .unwrap_or_else(|| lower_expr(arg, env, ctx))
        })
        .collect()
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
    shape: RustSlotShape,
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
    let element_shape = infer_stream_source_element_shape(source, env)?;
    Some(StreamPlan {
        source: StreamSource { expr: source },
        ops,
        terminal,
        element: StreamElement {
            shape: element_shape,
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

fn lower_stream_plan(
    plan: &StreamPlan<'_>,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> Option<crate::coercion::LoweredExpr> {
    let mut out = format!(
        "{}.iter()",
        crate::emit::emit_expr(&lower_expr(plan.source.expr, env, ctx))
    );
    let mut element_actual = plan.element.shape.clone();
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
                let input_shape = element_actual.clone();
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::BorrowedItem
                    )?
                ));
                element_actual = infer_stream_lambda_body_shape(lambda.body, &input_shape);
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
                element_actual = infer_stream_map_to_int_shape(lambda.body);
            }
        }
    }

    let actual = match &plan.terminal {
        StreamTerminal::Sum => {
            out.push_str(".sum::<i32>()");
            crate::coercion::LoweredType::stream_sum()
        }
        StreamTerminal::CollectToList => {
            out.push_str(".collect::<Vec<_>>()");
            crate::coercion::LoweredType::stream_collect(element_actual)
        }
        StreamTerminal::FindFirst => {
            out.push_str(".next()");
            crate::coercion::LoweredType::stream_find_first(element_actual)
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
            crate::coercion::LoweredType::stream_any_match()
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

fn lower_expr_for_slot(
    expr: &Expr,
    env: &TypeEnv,
    slot: crate::coercion::Slot,
    ctx: LowerCtx,
) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        return crate::coercion::apply_lowered_expr_to_slot(stream_expr, &slot);
    }
    if let Expr::Cast { ty, .. } = expr {
        let actual = ctx.representations.value_shape_for_java_type(ty.clone());
        let expr = lower_expr(expr, env, ctx);
        return crate::coercion::apply_expr_to_slot(expr, actual, &slot);
    }
    let actual = infer_expr_shape_with_context(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
        },
    );
    let expr = lower_expr(expr, env, ctx);
    crate::coercion::apply_expr_to_slot(expr, actual, &slot)
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
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut out = Vec::new();
    for stmt in &block.statements {
        lower_stmt_for_body(stmt, update, env, wrap_returns_in_ok, ctx, &mut out);
        learn_stmt_shapes(stmt, env, ctx);
    }
    out
}

fn lower_stmt_for_body(
    stmt: &Stmt,
    update: &[Expr],
    env: &mut TypeEnv,
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
                wrap_returns_in_ok,
                ctx,
            ),
            else_body: else_block
                .as_ref()
                .map(|block| {
                    lower_block_for_body(block, update, &mut env.clone(), wrap_returns_in_ok, ctx)
                })
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition, ctx),
            body: lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
        }),
        other => lower_stmt(other, env, wrap_returns_in_ok, ctx, out),
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

fn default_expr_for_shape(shape: &RustSlotShape) -> RustExpr {
    shape.default_expr()
}

fn env_for_type(ty: &TypeDecl, ctx: LowerCtx) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            if let Some(shape) = ctx.signatures.field_shape(&ty.name, &field.name) {
                env.insert_field_shape(field.name.clone(), shape.clone());
            } else {
                env.insert_field(field.name.clone(), field.ty.clone());
            }
        }
    }
    env
}

fn learn_stmt_shapes(stmt: &Stmt, env: &mut TypeEnv, ctx: LowerCtx) {
    match stmt {
        Stmt::VarDecl { ty, name, .. } => {
            let shape = local_shape_or_storage_default(ctx, name, ty);
            env.insert_local_shape(name.clone(), shape);
        }
        Stmt::For { init, .. } => {
            for stmt in init {
                learn_stmt_shapes(stmt, env, ctx);
            }
        }
        Stmt::EnhancedFor {
            var_type, var_name, ..
        } => {
            let shape = local_shape_or_storage_default(ctx, var_name, var_type);
            env.insert_local_shape(var_name.clone(), shape);
        }
        Stmt::Try { body, .. } => {
            for stmt in &body.statements {
                learn_stmt_shapes(stmt, env, ctx);
            }
        }
        _ => {}
    }
}

fn local_shape_or_storage_default(
    ctx: LowerCtx,
    local_name: &str,
    fallback: &JavaType,
) -> RustSlotShape {
    ctx.signatures
        .local_shape(ctx.current_local_shapes, local_name)
        .cloned()
        .unwrap_or_else(|| RustSlotShape::for_storage(fallback.clone()))
}

fn assignment_slot_for_target(
    target: &Expr,
    env: &TypeEnv,
    ctx: LowerCtx,
) -> crate::coercion::Slot {
    infer_assignment_target_shape_with_context(
        target,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
        },
    )
    .map(crate::coercion::Slot::Assignment)
    .unwrap_or(crate::coercion::Slot::Value)
}
