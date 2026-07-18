use crate::access_lowering::{lower_bare_name, lower_field_access};
use crate::array_lowering::render_array_expr;
use crate::array_plan::plan_array_expr;
use crate::call_lowering::{apply_call_target, apply_new_target_with_facts};
use crate::call_plan::{
    ArgumentLoweringPlan, CallExprPlan, CallStatementPlan, plan_call_arguments_with_facts,
    plan_call_expr, plan_call_statement, plan_constructor_arguments, plan_new_target_with_facts,
};
use crate::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, plan_constructor_delegation,
    plan_constructor_field_values,
};
use crate::conversion_lowering::{
    apply_cast_conversion, apply_instanceof, apply_lowered_slot_expr_conversion,
    apply_slot_expr_conversion,
};
use crate::conversion_plan::{
    ReturnValuePlan, Slot, actual_shape_for_slot_expr, argument_slot,
    assignment_value_plan_for_shape, assignment_value_plan_for_target, condition_slot, index_slot,
    plan_cast_conversion, plan_instanceof, plan_lowered_slot_conversion, plan_slot_conversion,
    return_value_plan,
};
use crate::diagnostics::DiagnosticBag;
use crate::exception_lowering::{
    exception_expr_from_new, fallback_exception_expr, java_exception_support_item, return_stmt,
    throw_stmt,
};
use crate::exception_plan::{
    ProgramExceptionPlan, ThrowExprPlan, plan_program_exceptions, plan_throw_expr,
};
use crate::expr_render::{
    binary_expr, indexed_expr, literal_expr, raw_expr, super_expr, ternary_expr, this_expr,
};
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, learn_stmt_shapes, shape_env_for_constructor, shape_env_for_method,
};
use crate::facts::FactSet;
use crate::item_assembly::{
    constructor_default_field_value_with_facts, constructor_function_shell_with_facts,
    constructor_return_body, delegating_constructor_return_body, impl_item_for_type,
    method_function_shell_with_facts_and_exception_plan, raw_item, struct_item_for_type_with_facts,
};
use crate::java_ir::*;
use crate::loop_lowering::{assemble_for_loop, continue_with_updates};
use crate::loop_plan::{ForBodyStmtPlan, plan_for_body_stmt};
use crate::lowering_diagnostics::{LoweringDiagnosticPlan, collect_lowering_diagnostic_plans};
use crate::operator_lowering::lower_unary_expr;
use crate::rewrite::MethodControlFlowPlan;
use crate::rust_ir::*;
use crate::signature::{LocalPlan, SignaturePlan};
use crate::slot_shape::RustSlotShape;
use crate::statement_render::{
    assignment_stmt, break_stmt, continue_stmt, expr_stmt, if_stmt, local_let_stmt_with_facts,
    raw_stmt, render_block_stmt, render_enhanced_for, while_stmt,
};
use crate::stream::{StreamPlan, build_stream_plan};
use crate::stream_lowering::render_stream_plan;
use crate::string_concat_plan::build_string_concat_plan;
use crate::string_concat_render::render_string_concat;
use crate::switch_lowering::{SwitchRenderCase, render_switch, switch_case_condition};
use crate::switch_plan::{
    SwitchCaseLabelPlan, SwitchPlan, plan_switch, plan_switch_case, rewrite_switch_breaks,
};
use crate::unsupported::{
    plan_unsupported_expression, plan_unsupported_member, plan_unsupported_statement,
    unsupported_constructor_stmt, unsupported_expression_expr,
    unsupported_member_diagnostic_policy, unsupported_statement_stmt,
};
use crate::{
    representation::RepresentationPlan,
    representation_lowering::{
        raw_type_item_for_with_facts_and_exceptions,
        trait_impl_emissions_for_type_with_facts_and_exceptions,
    },
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
    exceptions: &'a ProgramExceptionPlan,
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
    let signatures =
        crate::signature::solve_signatures_with_facts(program, &representations, &usage, facts);
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
    diagnostics: DiagnosticBag,
    facts: &FactSet,
    _usage: &UsageFacts,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
) -> LoweredProgram {
    lower_program_with_semantic_plans(
        program,
        diagnostics,
        facts,
        _usage,
        representations,
        signatures,
        &[],
    )
}

pub fn lower_program_with_semantic_plans(
    program: &Program,
    mut diagnostics: DiagnosticBag,
    facts: &FactSet,
    _usage: &UsageFacts,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
) -> LoweredProgram {
    push_lowering_diagnostic_plans(
        collect_lowering_diagnostic_plans(
            program,
            facts,
            representations,
            signatures,
            control_flow_plans,
        ),
        &mut diagnostics,
    );
    let exceptions = plan_program_exceptions(program, facts);
    let ctx = LowerCtx {
        facts,
        representations,
        signatures,
        current_local_shapes: &[],
        current_return_shape: None,
        exceptions: &exceptions,
    };
    let mut file = RustFile::default();
    if exceptions.support.emit_support_item {
        file.items.push(java_exception_support_item());
    }
    for ty in &program.types {
        lower_type(program, ty, &mut file, &mut diagnostics, ctx);
    }
    LoweredProgram { file, diagnostics }
}

fn push_lowering_diagnostic_plans(
    diagnostic_plans: Vec<LoweringDiagnosticPlan>,
    diagnostics: &mut DiagnosticBag,
) {
    for diagnostic in diagnostic_plans {
        diagnostics.push(
            diagnostic.severity,
            diagnostic.code,
            diagnostic.message,
            None,
        );
    }
}

fn lower_type(
    program: &Program,
    ty: &TypeDecl,
    file: &mut RustFile,
    diagnostics: &mut DiagnosticBag,
    ctx: LowerCtx,
) {
    if let Some(item) = raw_type_item_for_with_facts_and_exceptions(
        ctx.representations,
        ty,
        ctx.signatures,
        ctx.facts,
        ctx.exceptions,
    ) {
        file.items.push(raw_item(item));
        return;
    }

    file.items.push(struct_item_for_type_with_facts(
        ty,
        ctx.signatures,
        ctx.facts,
    ));

    let mut functions = Vec::new();
    for member in &ty.members {
        if let Some(kind) = plan_unsupported_member(member) {
            let diagnostic = unsupported_member_diagnostic_policy(kind);
            diagnostics.unsupported(diagnostic.code, diagnostic.message);
            continue;
        }
        match member {
            Member::Constructor(ctor) => functions.push(lower_constructor(ty, ctor, ctx)),
            Member::Method(method) => functions.push(lower_method(ty, method, ctx)),
            _ => {}
        }
    }
    file.items.push(impl_item_for_type(ty, functions));
    for item in trait_impl_emissions_for_type_with_facts_and_exceptions(
        ctx.representations,
        program,
        ty,
        ctx.signatures,
        ctx.facts,
        ctx.exceptions,
    ) {
        file.items.push(raw_item(item));
    }
}

fn lower_constructor(ty: &TypeDecl, ctor: &ConstructorDecl, ctx: LowerCtx) -> RustFunction {
    let constructor_signature = ctx.signatures.required_constructor_signature(ty, ctor);
    let constructor_ctx = LowerCtx {
        current_local_shapes: constructor_signature.locals.as_slice(),
        ..ctx
    };
    let mut env = shape_env_for_constructor(ty, constructor_signature, ctx.signatures);
    if let Some(body) = &ctor.body
        && let Some(function) = lower_delegating_constructor(ty, ctor, body, &env, constructor_ctx)
    {
        return function;
    }
    if let Some(body) = &ctor.body {
        for stmt in &body.statements {
            learn_stmt_shapes(
                stmt,
                &mut env,
                constructor_ctx.current_local_shapes,
                constructor_ctx.signatures,
            );
        }
    }

    let fields = plan_constructor_field_values(ty, ctor.body.as_ref(), ctx.signatures)
        .into_iter()
        .map(|plan| lower_constructor_field_value_plan(plan, &env, constructor_ctx))
        .collect();

    constructor_function_shell_with_facts(
        ty,
        ctor,
        ctx.signatures,
        constructor_return_body(ty, fields),
        ctx.facts,
    )
}

fn lower_constructor_field_value_plan(
    plan: ConstructorFieldValuePlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> (String, RustExpr) {
    match plan {
        ConstructorFieldValuePlan::Assigned {
            field_name,
            value,
            shape,
        }
        | ConstructorFieldValuePlan::FieldInitializer {
            field_name,
            value,
            shape,
        } => (
            field_name,
            lower_expr_for_slot(
                value,
                env,
                assignment_value_plan_for_shape(&shape).slot,
                ctx,
            ),
        ),
        ConstructorFieldValuePlan::Default { field_name, shape } => {
            constructor_default_field_value_with_facts(field_name, shape, ctx.facts)
        }
    }
}

fn lower_delegating_constructor(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    body: &Block,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustFunction> {
    let plan = plan_constructor_delegation(body)?;

    let body = match plan {
        ConstructorDelegationPlan::SameClassCall { args, resolved } => {
            delegating_constructor_return_body(
                &resolved.rust_name,
                lower_args_with_plan(
                    args,
                    env,
                    plan_constructor_arguments(Some(resolved), args.len(), ctx.signatures),
                    ctx,
                ),
            )
        }
        ConstructorDelegationPlan::Unsupported(kind) => vec![unsupported_constructor_stmt(kind)],
    };

    Some(constructor_function_shell_with_facts(
        ty,
        ctor,
        ctx.signatures,
        body,
        ctx.facts,
    ))
}

fn lower_method(ty: &TypeDecl, method: &MethodDecl, ctx: LowerCtx) -> RustFunction {
    let method_signature = ctx.signatures.required_method_signature(ty, method);
    let method_ctx = LowerCtx {
        current_return_shape: method_signature.return_shape.as_ref(),
        current_local_shapes: method_signature.locals.as_slice(),
        ..ctx
    };
    let mut env = shape_env_for_method(ty, method, method_signature, method_ctx.signatures);
    let exception_plan = method_ctx.exceptions.method_plan(ty, method);
    let wrap_returns_in_ok = exception_plan.wrap_returns_in_result;
    let body = method
        .body
        .as_ref()
        .map(|body| lower_block(body, &mut env, wrap_returns_in_ok, method_ctx))
        .unwrap_or_default();
    method_function_shell_with_facts_and_exception_plan(
        ty,
        method,
        ctx.signatures,
        body,
        ctx.facts,
        exception_plan,
    )
}

fn lower_block(
    block: &Block,
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut body = Vec::new();
    for stmt in &block.statements {
        lower_stmt(stmt, env, wrap_returns_in_ok, ctx, &mut body);
        learn_stmt_shapes(stmt, env, ctx.current_local_shapes, ctx.signatures);
    }
    body
}

fn lower_stmt(
    stmt: &Stmt,
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    out: &mut Vec<RustStmt>,
) {
    if let Some(kind) = plan_unsupported_statement(stmt) {
        out.push(unsupported_statement_stmt(kind));
        return;
    }

    match stmt {
        Stmt::VarDecl {
            name, initializer, ..
        } => {
            let local_plan = ctx
                .signatures
                .required_local_plan(ctx.current_local_shapes, name);
            out.push(local_let_stmt_with_facts(
                local_plan,
                initializer.as_ref().map(|expr| {
                    lower_expr_for_slot(
                        expr,
                        env,
                        assignment_value_plan_for_shape(&local_plan.shape).slot,
                        ctx,
                    )
                }),
                ctx.facts,
            ))
        }
        Stmt::Assign { target, value } => out.push(assignment_stmt(
            lower_expr(target, env, ctx),
            lower_expr_for_slot(
                value,
                env,
                assignment_value_plan_for_target(
                    target,
                    env,
                    ExprShapeContext {
                        signatures: Some(ctx.signatures),
                        representations: Some(ctx.representations),
                        facts: Some(ctx.facts),
                    },
                )
                .slot,
                ctx,
            ),
        )),
        Stmt::Expr(expr) => {
            if let Some(stmt) = lower_trivial_accessor_expr_stmt(expr, env, ctx) {
                out.push(stmt);
            } else {
                out.push(expr_stmt(lower_expr(expr, env, ctx)));
            }
        }
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
        } => out.push(if_stmt(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_block(then_block, &mut env.clone(), wrap_returns_in_ok, ctx),
            else_block
                .as_ref()
                .map(|block| lower_block(block, &mut env.clone(), wrap_returns_in_ok, ctx))
                .unwrap_or_default(),
        )),
        Stmt::While { condition, body } => out.push(while_stmt(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
        )),
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            for init_stmt in init {
                lower_stmt(init_stmt, env, wrap_returns_in_ok, ctx, out);
                learn_stmt_shapes(init_stmt, env, ctx.current_local_shapes, ctx.signatures);
            }
            let mut loop_body =
                lower_block_for_body(body, update, &mut env.clone(), wrap_returns_in_ok, ctx);
            for expr in update {
                loop_body.push(expr_stmt(lower_expr(expr, env, ctx)));
            }
            let condition = condition
                .as_ref()
                .map(|expr| lower_expr_for_slot(expr, env, condition_slot(), ctx));
            out.push(assemble_for_loop(condition, loop_body, label.clone()));
        }
        Stmt::EnhancedFor {
            var_name,
            iterable,
            body,
            ..
        } => out.push(render_enhanced_for(
            var_name,
            &lower_expr(iterable, env, ctx),
            &lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
        )),
        Stmt::Switch { selector, cases } => {
            let plan = plan_switch(selector, cases);
            out.push(raw_stmt(lower_switch(&plan, env, wrap_returns_in_ok, ctx)));
        }
        Stmt::Throw(expr) => out.push(throw_stmt(lower_throw_expr(expr, env, ctx))),
        Stmt::Break(label) => out.push(break_stmt(label.clone())),
        Stmt::Continue(label) => out.push(continue_stmt(label.clone())),
        Stmt::Block(block) => out.push(render_block_stmt(&lower_block(
            block,
            &mut env.clone(),
            wrap_returns_in_ok,
            ctx,
        ))),
        Stmt::Try { .. }
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Unsupported(_) => {
            unreachable!("unsupported statement kinds should be handled before statement lowering")
        }
    }
}

fn lower_switch(
    plan: &SwitchPlan<'_>,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> String {
    let cases = plan
        .cases
        .iter()
        .map(|case| SwitchRenderCase {
            label_condition: switch_label_condition(case, plan, env, ctx),
            statements: lower_switch_case_statements(
                case,
                plan.break_label,
                env,
                wrap_returns_in_ok,
                ctx,
            ),
        })
        .collect();
    render_switch(plan, lower_expr(plan.selector, env, ctx), cases)
}

fn switch_label_condition(
    case: &SwitchCase,
    plan: &SwitchPlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> String {
    let case_plan = plan_switch_case(case);
    let label_exprs = case_plan
        .labels
        .iter()
        .filter_map(|label| match label {
            SwitchCaseLabelPlan::Expr(expr) => Some(lower_expr(expr, env, ctx)),
            SwitchCaseLabelPlan::Default => None,
        })
        .collect::<Vec<_>>();
    switch_case_condition(plan, &case_plan, label_exprs)
}

fn lower_switch_case_statements(
    case: &SwitchCase,
    switch_label: &str,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut case_env = env.clone();
    let mut out = Vec::new();
    for stmt in &case.statements {
        let stmt = rewrite_switch_breaks(stmt, switch_label);
        lower_stmt(&stmt, &mut case_env, wrap_returns_in_ok, ctx, &mut out);
        learn_stmt_shapes(
            &stmt,
            &mut case_env,
            ctx.current_local_shapes,
            ctx.signatures,
        );
    }
    out
}

fn lower_return_stmt(
    expr: Option<&Expr>,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> RustStmt {
    let value = expr.map(|expr| match return_value_plan(ctx.current_return_shape) {
        ReturnValuePlan::Slot(slot) => lower_expr_for_slot(expr, env, slot, ctx),
        ReturnValuePlan::Raw => lower_expr(expr, env, ctx),
    });
    return_stmt(value, wrap_returns_in_ok)
}

fn lower_throw_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    match plan_throw_expr(expr) {
        ThrowExprPlan::NewException { class_name, args } => {
            let args = args.iter().map(|arg| lower_expr(arg, env, ctx)).collect();
            exception_expr_from_new(&class_name, args)
        }
        ThrowExprPlan::FallbackValue(value) => fallback_exception_expr(lower_expr(value, env, ctx)),
    }
}

fn lower_trivial_accessor_expr_stmt(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustStmt> {
    let CallStatementPlan::TrivialAccessorSetter {
        receiver,
        field_name,
        value,
    } = plan_call_statement(expr, ctx.facts)?;
    Some(assignment_stmt(
        lower_accessor_field(receiver, &field_name, env, ctx),
        lower_expr(value, env, ctx),
    ))
}

fn lower_accessor_field(
    receiver: Option<&Expr>,
    field_name: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    receiver
        .map(|receiver| {
            lower_field_access(
                receiver,
                field_name,
                env.type_env(),
                lower_expr(receiver, env, ctx),
            )
        })
        .unwrap_or_else(|| lower_bare_name(field_name, env))
}

fn lower_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        return stream_expr.expr;
    }
    if let Some(format_expr) = lower_string_concat(expr, env, ctx) {
        return format_expr;
    }
    if let Some(array_plan) = plan_array_expr(expr) {
        let children = array_plan
            .child_exprs()
            .iter()
            .map(|child| lower_expr(child, env, ctx))
            .collect();
        return render_array_expr(&array_plan, children);
    }
    if let Some(kind) = plan_unsupported_expression(expr) {
        return unsupported_expression_expr(kind);
    }
    match expr {
        Expr::Literal(lit) => literal_expr(lit),
        Expr::Var(name) => lower_bare_name(name, env),
        Expr::This => this_expr(),
        Expr::Super => super_expr(),
        Expr::FieldAccess(receiver, field) => lower_field_access(
            receiver,
            field,
            env.type_env(),
            lower_expr(receiver, env, ctx),
        ),
        Expr::ArrayAccess(array, index) => indexed_expr(
            lower_expr(array, env, ctx),
            lower_expr_for_slot(index, env, index_slot(), ctx),
        ),
        Expr::ArrayCreation { .. } | Expr::ArrayInitializer(_) => {
            unreachable!("array expressions should be planned before expression lowering")
        }
        Expr::Binary { op, left, right } => {
            binary_expr(*op, lower_expr(left, env, ctx), lower_expr(right, env, ctx))
        }
        Expr::Unary { op, expr } => lower_unary_expr(*op, lower_expr(expr, env, ctx)),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => ternary_expr(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_expr(then_expr, env, ctx),
            lower_expr(else_expr, env, ctx),
        ),
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            let plan = plan_call_expr(
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                ctx.facts,
                env.type_env(),
            );
            let target = match plan {
                CallExprPlan::TrivialAccessorGetter {
                    receiver,
                    field_name,
                } => return lower_accessor_field(receiver, &field_name, env, ctx),
                CallExprPlan::Target(target) => target,
            };
            let rust_args = lower_args_with_plan(
                args,
                env,
                plan_call_arguments_with_facts(
                    receiver.as_deref(),
                    name,
                    resolved.as_ref(),
                    args.len(),
                    ctx.signatures,
                    ctx.facts,
                    env.type_env(),
                ),
                ctx,
            );
            let lowered_receiver = target
                .receiver_expr()
                .map(|receiver| lower_expr(receiver, env, ctx));
            apply_call_target(&target, lowered_receiver, rust_args)
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            let target = plan_new_target_with_facts(
                ty,
                resolved.as_ref(),
                ctx.representations,
                ctx.facts,
                args.len(),
            );
            let rust_args = lower_args_with_plan(
                args,
                env,
                plan_constructor_arguments(resolved.as_ref(), args.len(), ctx.signatures),
                ctx,
            );
            apply_new_target_with_facts(&target, rust_args, ctx.facts)
        }
        Expr::Cast { ty, expr } => {
            let plan = plan_cast_conversion(
                expr,
                ty,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                    facts: Some(ctx.facts),
                },
            );
            apply_cast_conversion(lower_expr(expr, env, ctx), &plan)
        }
        Expr::InstanceOf { expr, ty } => {
            let plan = plan_instanceof(
                expr,
                ty,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                    facts: Some(ctx.facts),
                },
            );
            apply_instanceof(lower_expr(expr, env, ctx), &plan)
        }
        Expr::Lambda { .. } | Expr::MethodReference { .. } | Expr::ClassLiteral(_) => {
            unreachable!(
                "unsupported expression kinds should be handled before expression lowering"
            )
        }
        Expr::Raw(raw) => raw_expr(raw),
    }
}

fn lower_args_with_plan(
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    match plan {
        ArgumentLoweringPlan::Shaped(shapes) => args
            .iter()
            .zip(shapes)
            .map(|(arg, shape)| lower_expr_for_slot(arg, env, argument_slot(shape), ctx))
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => {
            args.iter().map(|arg| lower_expr(arg, env, ctx)).collect()
        }
    }
}

fn lower_stream_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<crate::coercion::LoweredExpr> {
    let plan = build_stream_plan(expr, env.type_env(), ctx.facts)?;
    lower_stream_plan(&plan, env, ctx)
}

fn lower_stream_plan(
    plan: &StreamPlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<crate::coercion::LoweredExpr> {
    render_stream_plan(plan, lower_expr(plan.source.expr, env, ctx))
}

fn lower_expr_for_slot(expr: &Expr, env: &ShapeEnv, slot: Slot, ctx: LowerCtx) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        let plan = plan_lowered_slot_conversion(&stream_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(stream_expr, &plan);
    }
    let actual = actual_shape_for_slot_expr(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let expr = lower_expr(expr, env, ctx);
    let plan = plan_slot_conversion(actual, slot, ctx.facts);
    apply_slot_expr_conversion(expr, &plan)
}

fn lower_string_concat(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    let plan = build_string_concat_plan(expr, env.type_env())?;
    let args = plan
        .args
        .iter()
        .map(|part| lower_expr(part, env, ctx))
        .collect::<Vec<_>>();
    Some(render_string_concat(&plan, args))
}

fn lower_block_for_body(
    block: &Block,
    update: &[Expr],
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut out = Vec::new();
    for stmt in &block.statements {
        lower_stmt_for_body(stmt, update, env, wrap_returns_in_ok, ctx, &mut out);
        learn_stmt_shapes(stmt, env, ctx.current_local_shapes, ctx.signatures);
    }
    out
}

fn lower_stmt_for_body(
    stmt: &Stmt,
    update: &[Expr],
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    out: &mut Vec<RustStmt>,
) {
    match plan_for_body_stmt(stmt, update) {
        ForBodyStmtPlan::ContinueWithUpdates { updates } => {
            let updates = updates
                .iter()
                .map(|expr| lower_expr(expr, env, ctx))
                .collect::<Vec<_>>();
            out.extend(continue_with_updates(&updates));
        }
        ForBodyStmtPlan::If {
            condition,
            then_block,
            else_block,
        } => out.push(if_stmt(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_block_for_body(
                then_block,
                update,
                &mut env.clone(),
                wrap_returns_in_ok,
                ctx,
            ),
            else_block
                .as_ref()
                .map(|block| {
                    lower_block_for_body(block, update, &mut env.clone(), wrap_returns_in_ok, ctx)
                })
                .unwrap_or_default(),
        )),
        ForBodyStmtPlan::While { condition, body } => out.push(while_stmt(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
        )),
        ForBodyStmtPlan::Default { stmt } => lower_stmt(stmt, env, wrap_returns_in_ok, ctx, out),
    }
}
