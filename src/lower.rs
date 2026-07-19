use std::collections::HashSet;

use crate::access_lowering::{lower_bare_name, lower_field_access};
use crate::array_lowering::render_array_expr;
use crate::array_plan::plan_array_expr;
use crate::call_lowering::{
    apply_call_target, apply_class_literal_token, apply_collection_remove_if,
    apply_native_collection_extend, apply_new_target_with_facts, borrowed_sequence_extend_arg,
    propagate_result_expr,
};
use crate::call_plan::{
    ArgumentLoweringPlan, CallExceptionPropagationPlan, CallExprPlan, CallStatementPlan,
    CallTargetPlan, NativeCollectionExtendArgPlan, NativeCollectionMethod,
    NativeCollectionNewTarget, apply_fallback_receiver_rust_name, external_call_lowered_type,
    native_collection_lowered_type, plan_call_arguments, plan_call_arguments_with_facts_and_args,
    plan_call_exception_propagation, plan_call_expr, plan_call_statement,
    plan_constructor_arguments, plan_native_collection_extend_arg,
    plan_new_target_for_args_with_facts,
};
use crate::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, DelegatedConstructorTrailingStmt,
    plan_constructor_delegation_for_type, plan_constructor_field_values_with_inherited_fields,
};
use crate::control_flow_lowering::{TailRecursiveLoopRender, render_tail_recursive_loop_body};
use crate::conversion_lowering::{
    apply_cast_conversion, apply_instanceof, apply_lowered_slot_expr_conversion,
    apply_slot_expr_conversion,
};
use crate::conversion_plan::{
    ReturnValuePlan, Slot, actual_shape_for_slot_expr_with_slot, argument_slot,
    assignment_statement_plan_for_target, assignment_value_plan_for_shape,
    assignment_value_plan_for_target, condition_slot, index_slot, plan_cast_conversion,
    plan_instanceof, plan_lowered_slot_conversion, plan_slot_conversion, return_value_plan,
};
use crate::diagnostics::DiagnosticBag;
use crate::emit;
use crate::exception_lowering::{
    CatchRenderArm, exception_cause_or_self_if_instance_expr,
    exception_expr_from_new_with_cause_hint, exception_try_ok_type, fallback_exception_expr,
    java_exception_support_item, return_stmt, throw_stmt, try_catch_stmt, try_finally_scope_stmt,
    try_with_resources_scope_stmt,
};
use crate::exception_plan::{
    ProgramExceptionPlan, ThrowExprPlan, TryBlockCompletion, TryStmtPlan, catch_match_class_names,
    plan_block_exception_completion, plan_program_exceptions, plan_throw_expr, plan_try_stmt,
    try_resource_names,
};
use crate::expr_render::{
    binary_expr, indexed_expr, literal_expr, raw_expr, super_expr, ternary_expr, this_expr,
};
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, fallback_local_shape_from_declared_type,
    infer_expr_shape_with_context, learn_stmt_shapes, shape_env_for_constructor,
    shape_env_for_method,
};
use crate::facts::{FactSet, OwnedRecoveryCapability};
use crate::item_assembly::{
    constructor_default_field_value_with_facts, constructor_function_shell_with_facts,
    constructor_return_body, delegating_constructor_return_body, impl_item_for_type,
    method_function_shell_with_facts_and_exception_plan, raw_item,
    struct_item_for_type_with_facts_and_support_context,
};
use crate::java_ir::*;
use crate::loop_lowering::{assemble_for_loop, assignment_condition_loop, continue_with_updates};
use crate::loop_plan::{ForBodyStmtPlan, WhileStmtPlan, plan_for_body_stmt, plan_while_stmt};
use crate::lowering_diagnostics::{LoweringDiagnosticPlan, collect_lowering_diagnostic_plans};
use crate::method_reference_lowering::render_method_reference;
use crate::method_reference_plan::plan_method_reference;
use crate::nullability_plan::{TernaryNullRefinementPlan, plan_ternary_null_refinement};
use crate::operator_lowering::lower_equality_comparison_expr_for_op;
use crate::operator_lowering::{
    lower_equality_comparison_expr, lower_numeric_binary_expr, plan_equality_comparison,
    plan_equality_comparison_with_facts, plan_numeric_binary_promotion,
};
use crate::operator_lowering::{lower_null_comparison_expr, lower_unary_expr};
use crate::rewrite::{MethodControlFlowPlan, method_has_tail_recursive_loop_rewrite};
use crate::rust_ir::*;
use crate::signature::{LocalPlan, ReceiverPlan, SignaturePlan, SolvedSlotShape};
use crate::slot_shape::RustSlotShape;
use crate::slot_shape::{DispatchShape, OwnershipShape, ViewShape};
use crate::statement_render::{
    assignment_stmt, break_stmt, continue_stmt, expr_stmt, if_stmt, local_let_stmt_with_facts,
    raw_stmt, render_block_stmt, render_enhanced_for, while_stmt,
};
use crate::stdlib::{
    external_api_facade_support_items_with_local_type_program,
    external_static_string_constant_support_items_with_local_type_program, java_class_support_item,
    java_platform_facade_support_item, java_string_format_support_item,
    java_string_split_support_item, program_uses_java_class_tokens,
    program_uses_java_platform_facades, program_uses_java_string_format,
    program_uses_java_string_split,
};
use crate::stream::{StreamPlan, build_stream_plan};
use crate::stream_lowering::{StreamRenderContext, render_stream_plan_with_context};
use crate::stream_shape::infer_stream_source_element_shape_with_facts;
use crate::string_concat_plan::build_string_concat_plan;
use crate::string_concat_render::render_string_concat;
use crate::switch_lowering::{
    SwitchMatchRenderCase, SwitchRenderCase, render_simple_return_match_switch, render_switch,
    switch_case_condition, switch_match_pattern_expr,
};
use crate::switch_plan::{
    SimpleReturnSwitchPlan, SwitchCaseLabelPlan, SwitchPlan, plan_simple_return_switch,
    plan_simple_return_switch_case, plan_switch, plan_switch_case, rewrite_switch_breaks,
};
use crate::unsupported::{
    plan_unsupported_expression, plan_unsupported_member, plan_unsupported_statement,
    unsupported_constructor_stmt, unsupported_expression_expr,
    unsupported_member_diagnostic_policy, unsupported_statement_stmt,
    unsupported_try_catch_finally_stmt,
};
use crate::{
    representation::RepresentationPlan,
    representation_lowering::{
        raw_type_item_for_program_with_facts_and_exceptions,
        trait_impl_emissions_for_type_with_facts_and_exceptions,
    },
    types,
    usage::{UsageConstructorKey, UsageFacts, UsageMethodKey},
};

#[derive(Debug, Clone)]
pub struct LoweredProgram {
    pub file: RustFile,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone, Copy)]
struct LowerCtx<'a> {
    facts: &'a FactSet,
    usage: &'a UsageFacts,
    representations: &'a RepresentationPlan,
    signatures: &'a SignaturePlan,
    current_type: Option<&'a str>,
    current_method: Option<&'a str>,
    current_constructor: Option<&'a str>,
    current_receiver: Option<ReceiverPlan>,
    current_local_shapes: &'a [LocalPlan],
    current_return_shape: Option<&'a SolvedSlotShape>,
    exceptions: &'a ProgramExceptionPlan,
    control_flow_plans: &'a [MethodControlFlowPlan],
    support_local_type_program: &'a Program,
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
    diagnostics: DiagnosticBag,
    facts: &FactSet,
    usage: &UsageFacts,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
) -> LoweredProgram {
    lower_program_with_semantic_plans_and_support_context(
        program,
        diagnostics,
        facts,
        usage,
        representations,
        signatures,
        control_flow_plans,
        program,
    )
}

pub fn lower_program_with_semantic_plans_and_support_context(
    program: &Program,
    mut diagnostics: DiagnosticBag,
    facts: &FactSet,
    usage: &UsageFacts,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
    support_local_type_program: &Program,
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
        usage,
        representations,
        signatures,
        current_type: None,
        current_method: None,
        current_constructor: None,
        current_receiver: None,
        current_local_shapes: &[],
        current_return_shape: None,
        exceptions: &exceptions,
        control_flow_plans,
        support_local_type_program,
    };
    let mut file = RustFile::default();
    if exceptions.support.emit_support_item {
        file.items.push(java_exception_support_item());
    }
    if program_uses_java_class_tokens(program) {
        file.items.push(java_class_support_item());
    }
    if program_uses_java_platform_facades(program) {
        file.items.push(java_platform_facade_support_item());
    }
    if program_uses_java_string_split(program) {
        file.items.push(java_string_split_support_item());
    }
    if program_uses_java_string_format(program) {
        file.items.push(java_string_format_support_item());
    }
    file.items
        .extend(external_api_facade_support_items_with_local_type_program(
            program,
            facts,
            support_local_type_program,
        ));
    file.items.extend(
        external_static_string_constant_support_items_with_local_type_program(
            program,
            facts,
            support_local_type_program,
        ),
    );
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

fn seed_imported_static_type_aliases(env: &mut ShapeEnv, program: &Program) {
    for import in &program.imports {
        if import.is_static && !import.is_wildcard {
            if let Some((owner, member)) = import.path.rsplit_once('.') {
                env.insert_static_member_import(member.to_string(), owner.to_string());
                env.insert_static_member_import(crate::parser::to_snake_case(member), owner);
            }
        }
        if import.is_wildcard && !import.is_static {
            continue;
        }
        let Some(type_name) = imported_static_owner_type_name(import) else {
            continue;
        };
        let simple = type_name
            .split('.')
            .filter(|segment| !segment.is_empty())
            .next_back()
            .unwrap_or(type_name.as_str());
        env.insert_static_type_alias(simple.to_string(), type_name.clone());
        env.insert_static_type_alias(crate::parser::to_snake_case(simple), type_name);
    }
}

fn seed_support_type_fields(env: &mut ShapeEnv, program: &Program) {
    for ty in &program.types {
        seed_support_type_fields_for_type(env, ty);
    }
}

fn seed_support_type_fields_for_type(env: &mut ShapeEnv, ty: &TypeDecl) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                let shape = RustSlotShape::for_storage(field.ty.clone());
                env.insert_type_field_shape(&ty.name, field.name.clone(), shape.clone());
                if field.modifiers.contains(&Modifier::Static)
                    && field.modifiers.contains(&Modifier::Final)
                {
                    env.insert_static_field_shape(field.name.clone(), shape);
                }
            }
            Member::Type(nested) => seed_support_type_fields_for_type(env, nested),
            _ => {}
        }
    }
}

fn seed_inherited_instance_fields(
    env: &mut ShapeEnv,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    support_program: &Program,
) {
    for inherited in
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
    {
        let shape = signatures.required_field_shape(&ty.name, &inherited.field.name);
        env.insert_type_field_shape(&ty.name, inherited.field.name.clone(), shape.clone());
        env.insert_field_shape(inherited.field.name, shape);
    }
}

fn imported_static_owner_type_name(import: &ImportDecl) -> Option<String> {
    if import.path.is_empty() {
        return None;
    }
    if !import.is_static {
        return Some(import.path.clone());
    }
    if import.is_wildcard {
        return Some(import.path.clone());
    }
    let owner = import.path.rsplit_once('.')?.0;
    (!owner.is_empty()).then(|| owner.to_string())
}

fn lower_type(
    program: &Program,
    ty: &TypeDecl,
    file: &mut RustFile,
    diagnostics: &mut DiagnosticBag,
    ctx: LowerCtx,
) {
    for member in &ty.members {
        if let Member::Type(nested) = member {
            lower_type(program, nested, file, diagnostics, ctx);
        }
    }

    if let Some(item) = raw_type_item_for_program_with_facts_and_exceptions(
        ctx.representations,
        program,
        ty,
        ctx.signatures,
        ctx.facts,
        ctx.exceptions,
    ) {
        file.items.push(raw_item(item));
        return;
    }

    file.items
        .push(struct_item_for_type_with_facts_and_support_context(
            ty,
            ctx.signatures,
            ctx.facts,
            ctx.support_local_type_program,
        ));

    let mut functions = Vec::new();
    for member in &ty.members {
        if let Some(kind) = plan_unsupported_member(member) {
            let diagnostic = unsupported_member_diagnostic_policy(kind);
            diagnostics.unsupported(diagnostic.code, diagnostic.message);
            continue;
        }
        match member {
            Member::Constructor(ctor) => functions.push(lower_constructor(program, ty, ctor, ctx)),
            Member::Method(method) => functions.push(lower_method(program, ty, method, ctx)),
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

fn lower_constructor(
    program: &Program,
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    ctx: LowerCtx,
) -> RustFunction {
    let constructor_signature = ctx.signatures.required_constructor_signature(ty, ctor);
    let constructor_ctx = LowerCtx {
        current_type: Some(&ty.name),
        current_method: None,
        current_constructor: Some(ctor.rust_name.as_deref().unwrap_or("new")),
        current_receiver: None,
        current_local_shapes: constructor_signature.locals.as_slice(),
        ..ctx
    };
    let mut env = shape_env_for_constructor(ty, constructor_signature, ctx.signatures);
    seed_imported_static_type_aliases(&mut env, program);
    seed_support_type_fields(&mut env, ctx.support_local_type_program);
    seed_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.support_local_type_program);
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

    let fields = plan_constructor_field_values_with_inherited_fields(
        ty,
        ctor.body.as_ref(),
        ctx.signatures,
        ctx.support_local_type_program,
    )
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
    let plan = plan_constructor_delegation_for_type(Some(ty), body)?;

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
        ConstructorDelegationPlan::SameClassCallWithTrailing {
            args,
            resolved,
            trailing,
        } => delegated_constructor_with_trailing_body(ty, resolved, args, &trailing, env, ctx),
        ConstructorDelegationPlan::ExternalExceptionSuperclass { .. } => {
            let fields = plan_constructor_field_values_with_inherited_fields(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
            )
            .into_iter()
            .map(|plan| lower_constructor_field_value_plan(plan, env, ctx))
            .collect();
            constructor_return_body(ty, fields)
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

fn delegated_constructor_with_trailing_body(
    ty: &TypeDecl,
    resolved: &ResolvedConstructor,
    args: &[Expr],
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut body = vec![RustStmt::Let {
        mutable: true,
        name: "__java_self".to_string(),
        ty: None,
        value: Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("Self::{}", resolved.rust_name))),
            args: lower_args_with_plan(
                args,
                env,
                plan_constructor_arguments(Some(resolved), args.len(), ctx.signatures),
                ctx,
            ),
        }),
    }];
    for stmt in trailing {
        body.push(lower_delegated_constructor_trailing_stmt(
            ty, stmt, env, ctx,
        ));
    }
    body.push(RustStmt::Return(Some(RustExpr::Var(
        "__java_self".to_string(),
    ))));
    body
}

fn lower_delegated_constructor_trailing_stmt(
    ty: &TypeDecl,
    stmt: &DelegatedConstructorTrailingStmt<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustStmt {
    match stmt {
        DelegatedConstructorTrailingStmt::FieldAssignment { field_name, value } => {
            let shape = ctx.signatures.required_field_shape(&ty.name, field_name);
            assignment_stmt(
                RustExpr::Raw(format!("__java_self.{field_name}")),
                lower_expr_for_slot(
                    value,
                    env,
                    assignment_value_plan_for_shape(&shape).slot,
                    ctx,
                ),
            )
        }
        DelegatedConstructorTrailingStmt::InstanceCall { resolved, args } => {
            expr_stmt(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("__java_self".to_string())),
                method: resolved.rust_name.clone(),
                args: lower_args_with_plan(
                    args,
                    env,
                    plan_call_arguments(Some(resolved), args.len(), ctx.signatures),
                    ctx,
                ),
            })
        }
    }
}

fn lower_method(
    program: &Program,
    ty: &TypeDecl,
    method: &MethodDecl,
    ctx: LowerCtx,
) -> RustFunction {
    let method_signature = ctx.signatures.required_method_signature(ty, method);
    let method_ctx = LowerCtx {
        current_type: Some(&ty.name),
        current_method: Some(&method.name),
        current_constructor: None,
        current_receiver: Some(method_signature.receiver),
        current_return_shape: method_signature.return_shape.as_ref(),
        current_local_shapes: method_signature.locals.as_slice(),
        ..ctx
    };
    let mut env = shape_env_for_method(ty, method, method_signature, method_ctx.signatures);
    seed_imported_static_type_aliases(&mut env, program);
    seed_support_type_fields(&mut env, method_ctx.support_local_type_program);
    seed_inherited_instance_fields(
        &mut env,
        ty,
        method_ctx.signatures,
        method_ctx.support_local_type_program,
    );
    let exception_plan = method_ctx.exceptions.method_plan(ty, method);
    let wrap_returns_in_ok = exception_plan.wrap_returns_in_result;
    let mut body = if !wrap_returns_in_ok
        && method_has_tail_recursive_loop_rewrite(
            method_ctx.control_flow_plans,
            &ty.name,
            &method.name,
        ) {
        method
            .body
            .as_ref()
            .and_then(|body| lower_tail_recursive_loop_method_body(method, body, &env, method_ctx))
            .unwrap_or_else(|| {
                lower_block(method.body.as_ref().unwrap(), &mut env, false, method_ctx)
            })
    } else {
        method
            .body
            .as_ref()
            .map(|body| lower_block(body, &mut env, wrap_returns_in_ok, method_ctx))
            .unwrap_or_default()
    };
    if wrap_returns_in_ok
        && method_signature
            .return_shape
            .as_ref()
            .is_none_or(|shape| shape.java_ty == JavaType::Void)
        && method.body.as_ref().is_some_and(|body| {
            matches!(
                plan_block_exception_completion(body),
                TryBlockCompletion::FallsThroughUnit
            )
        })
    {
        body.push(return_stmt(None, true));
    }
    method_function_shell_with_facts_and_exception_plan(
        ty,
        method,
        ctx.signatures,
        body,
        ctx.facts,
        exception_plan,
    )
}

fn lower_tail_recursive_loop_method_body(
    method: &MethodDecl,
    body: &Block,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<Vec<RustStmt>> {
    let [
        Stmt::If {
            condition,
            then_block,
            else_block: None,
        },
        Stmt::Return(Some(Expr::Call {
            receiver: None,
            name,
            args,
            ..
        })),
    ] = body.statements.as_slice()
    else {
        return None;
    };
    if name != &method.name || args.len() != method.params.len() {
        return None;
    }
    let [Stmt::Return(Some(base_expr))] = then_block.statements.as_slice() else {
        return None;
    };

    Some(vec![render_tail_recursive_loop_body(
        TailRecursiveLoopRender {
            params: method
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect(),
            condition: lower_expr_for_slot(condition, env, condition_slot(), ctx),
            base_return: lower_expr(base_expr, env, ctx),
            next_args: args
                .iter()
                .zip(method.params.iter())
                .map(|(arg, param)| {
                    lower_expr_for_slot(
                        arg,
                        env,
                        assignment_value_plan_for_target(
                            &Expr::Var(param.name.clone()),
                            env,
                            ExprShapeContext {
                                signatures: Some(ctx.signatures),
                                representations: Some(ctx.representations),
                                facts: Some(ctx.facts),
                            },
                        )
                        .slot,
                        ctx,
                    )
                })
                .collect(),
        },
    )])
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

fn lower_lambda_block(block: &Block, env: &mut ShapeEnv, ctx: LowerCtx) -> Vec<RustStmt> {
    let mut body = Vec::new();
    for stmt in &block.statements {
        lower_stmt(stmt, env, false, ctx, &mut body);
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                env.insert_local_shape(name.clone(), fallback_local_shape_from_declared_type(ty));
            }
            _ => learn_stmt_shapes(stmt, env, ctx.current_local_shapes, ctx.signatures),
        }
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
            ty,
            name,
            initializer,
        } => {
            let fallback_local_plan;
            let local_plan = if let Some(local_plan) =
                local_plan_for_lowered_var(ctx.current_local_shapes, name)
            {
                local_plan
            } else {
                fallback_local_plan = conservative_lambda_local_plan(name, ty);
                &fallback_local_plan
            };
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
        Stmt::Assign { target, value } => {
            let shape_ctx = ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            };
            if assignment_statement_plan_for_target(target, value, env, shape_ctx)
                == crate::conversion_plan::AssignmentStatementPlan::ClearOptionalTarget
            {
                out.push(expr_stmt(RustExpr::MethodCall {
                    receiver: Box::new(lower_expr(target, env, ctx)),
                    method: "take".to_string(),
                    args: Vec::new(),
                }));
            } else {
                out.push(assignment_stmt(
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
                ));
            }
        }
        Stmt::Expr(expr) => {
            if let Some(stmt) = lower_native_collection_extend_expr_stmt(expr, env, ctx) {
                out.push(stmt);
            } else if let Some(stmt) = lower_planned_call_expr_stmt(expr, env, ctx) {
                out.push(stmt);
            } else if let Some(stmt) =
                lower_throwing_call_expr_stmt(expr, env, wrap_returns_in_ok, ctx)
            {
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
        Stmt::While { condition, body } => match plan_while_stmt(condition, body) {
            WhileStmtPlan::AssignmentInCondition {
                local_name,
                value,
                null_test,
                body,
            } => {
                let local = Expr::Var(local_name.to_string());
                let assigned_value = lower_expr_for_slot(
                    value,
                    env,
                    assignment_value_plan_for_target(
                        &local,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    )
                    .slot,
                    ctx,
                );
                out.push(assignment_condition_loop(
                    local_name,
                    assigned_value,
                    null_test,
                    lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
                ));
            }
            WhileStmtPlan::Plain { condition, body } => out.push(while_stmt(
                lower_expr_for_slot(condition, env, condition_slot(), ctx),
                lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
            )),
        },
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
        } => out.push(lower_enhanced_for_stmt(
            var_name,
            iterable,
            body,
            env,
            wrap_returns_in_ok,
            ctx,
        )),
        Stmt::Switch { selector, cases } => {
            if let Some(plan) = plan_simple_return_switch(selector, cases) {
                out.push(raw_stmt(lower_simple_return_switch(
                    &plan,
                    env,
                    wrap_returns_in_ok,
                    ctx,
                )));
            } else {
                let plan = plan_switch(selector, cases);
                out.push(raw_stmt(lower_switch(&plan, env, wrap_returns_in_ok, ctx)));
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => out.push(lower_try_stmt(
            resources,
            body,
            catches,
            finally_block.as_ref(),
            env,
            wrap_returns_in_ok,
            ctx,
        )),
        Stmt::Throw(expr) => out.push(throw_stmt(lower_throw_expr(expr, env, ctx))),
        Stmt::Break(label) => out.push(break_stmt(label.clone())),
        Stmt::Continue(label) => out.push(continue_stmt(label.clone())),
        Stmt::Block(block) => out.push(render_block_stmt(&lower_block(
            block,
            &mut env.clone(),
            wrap_returns_in_ok,
            ctx,
        ))),
        Stmt::LocalType(_) | Stmt::ConstructorInvocation { .. } | Stmt::Unsupported(_) => {
            unreachable!("unsupported statement kinds should be handled before statement lowering")
        }
    }
}

fn local_plan_for_lowered_var<'a>(locals: &'a [LocalPlan], name: &str) -> Option<&'a LocalPlan> {
    locals.iter().find(|local| local.name == name)
}

fn conservative_lambda_local_plan(name: &str, ty: &JavaType) -> LocalPlan {
    LocalPlan {
        name: name.to_string(),
        java_ty: ty.clone(),
        shape: fallback_local_shape_from_declared_type(ty),
        binding_mutable: true,
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

fn lower_simple_return_switch(
    plan: &SimpleReturnSwitchPlan<'_>,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> String {
    let selector_ty = crate::types::infer_expr_type(plan.selector, env.type_env());
    let selector = lower_switch_match_selector(plan.selector, env, ctx, &selector_ty);
    let mut cases = Vec::new();
    let mut default_statements = None;
    let mut pending_labels = Vec::new();
    for case in plan.cases {
        let Some(case_plan) = plan_simple_return_switch_case(case) else {
            if case.statements.is_empty() {
                pending_labels.extend(plan_switch_case(case).labels);
            }
            continue;
        };
        let statements = vec![lower_return_stmt(
            Some(case_plan.return_expr),
            env,
            wrap_returns_in_ok,
            ctx,
        )];
        pending_labels.extend(case_plan.labels);
        if pending_labels
            .iter()
            .any(|label| matches!(label, SwitchCaseLabelPlan::Default))
        {
            default_statements = Some(statements);
            pending_labels.clear();
            continue;
        }
        let patterns = pending_labels
            .iter()
            .filter_map(|label| switch_match_pattern(label, env, ctx, &selector_ty))
            .collect::<Vec<_>>();
        pending_labels.clear();
        cases.push(SwitchMatchRenderCase {
            patterns,
            statements,
        });
    }
    render_simple_return_match_switch(selector, cases, default_statements)
}

fn lower_switch_match_selector(
    selector: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    selector_ty: &JavaType,
) -> RustExpr {
    let lowered = lower_expr(selector, env, ctx);
    if matches!(selector_ty, JavaType::String) {
        RustExpr::MethodCall {
            receiver: Box::new(lowered),
            method: "as_str".to_string(),
            args: Vec::new(),
        }
    } else {
        lowered
    }
}

fn switch_match_pattern(
    label: &SwitchCaseLabelPlan,
    env: &ShapeEnv,
    ctx: LowerCtx,
    selector_ty: &JavaType,
) -> Option<String> {
    let SwitchCaseLabelPlan::Expr(expr) = label else {
        return None;
    };
    if let (JavaType::Named { name, .. }, Expr::Var(label_name)) = (selector_ty, expr)
        && let Some(variant) = ctx
            .representations
            .java_enum_variant_for_label(name, label_name)
    {
        return Some(variant);
    }
    match (selector_ty, expr) {
        (JavaType::String, Expr::Literal(Literal::String(value))) => Some(format!("{value:?}")),
        (_, Expr::Literal(Literal::String(value))) => Some(switch_match_pattern_expr(
            literal_expr(&Literal::String(value.clone())),
        )),
        (_, Expr::Literal(literal)) => Some(switch_match_pattern_expr(literal_expr(literal))),
        _ => Some(switch_match_pattern_expr(lower_expr(expr, env, ctx))),
    }
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
            exception_expr_from_new_with_cause_hint(
                &class_name,
                args,
                exception_constructor_second_arg_is_cause(expr, env, ctx),
            )
        }
        ThrowExprPlan::CauseOrSelfIfInstance {
            wrapper,
            class_names,
        } => exception_cause_or_self_if_instance_expr(lower_expr(wrapper, env, ctx), &class_names),
        ThrowExprPlan::ExistingException(value) => lower_expr(value, env, ctx),
        ThrowExprPlan::FallbackValue(value) => fallback_exception_expr(lower_expr(value, env, ctx)),
    }
}

fn exception_constructor_second_arg_is_cause(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Expr::New { args, .. } = expr else {
        return false;
    };
    if args.len() < 2 {
        return false;
    }
    let shape = infer_expr_shape_with_context(
        &args[1],
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    java_type_is_exception_like(&shape.java_ty)
}

fn java_type_is_exception_like(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let simple = name.rsplit('.').next().unwrap_or(name);
            simple == "JavaException"
                || simple == "Throwable"
                || simple == "Exception"
                || simple == "Error"
                || simple.ends_with("Exception")
                || simple.ends_with("Error")
        }
        _ => false,
    }
}

fn lower_try_stmt(
    resources: &[Stmt],
    body: &Block,
    catches: &[CatchClause],
    finally_block: Option<&Block>,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> RustStmt {
    let plan = plan_try_stmt(resources, body, catches, finally_block);
    let ok_type = exception_try_ok_type(ctx.current_return_shape, ctx.facts);
    let (mut try_body, planned_catches, completion) = match plan {
        TryStmtPlan::CatchOnly {
            body: planned_body,
            catches: planned_catches,
            completion,
        } => (
            lower_block(planned_body, &mut env.clone(), true, ctx),
            planned_catches,
            completion,
        ),
        TryStmtPlan::WithResources {
            resources,
            body: planned_body,
            catches: planned_catches,
            completion,
        } => {
            let mut resource_env = env.clone();
            let mut resource_initializers = Vec::new();
            for resource in resources {
                lower_stmt(
                    resource,
                    &mut resource_env,
                    true,
                    ctx,
                    &mut resource_initializers,
                );
                learn_stmt_shapes(
                    resource,
                    &mut resource_env,
                    ctx.current_local_shapes,
                    ctx.signatures,
                );
            }
            let body = lower_block(planned_body, &mut resource_env, true, ctx);
            (
                vec![try_with_resources_scope_stmt(
                    ok_type.clone(),
                    resource_initializers,
                    body,
                    try_resource_names(resources),
                    matches!(completion, TryBlockCompletion::Terminates),
                )],
                planned_catches,
                completion,
            )
        }
        TryStmtPlan::FinallyOnly {
            body: planned_body,
            finally_block,
        } => {
            let try_body = lower_block(planned_body, &mut env.clone(), true, ctx);
            let finally_body =
                lower_block(finally_block, &mut env.clone(), wrap_returns_in_ok, ctx);
            return try_finally_scope_stmt(try_body, finally_body, wrap_returns_in_ok);
        }
        TryStmtPlan::Unsupported => return unsupported_try_catch_finally_stmt(),
    };
    if matches!(completion, TryBlockCompletion::FallsThroughUnit) && ok_type != RustType::Unit {
        return unsupported_try_catch_finally_stmt();
    }
    if matches!(completion, TryBlockCompletion::FallsThroughUnit) {
        try_body.push(return_stmt(None, true));
    }
    let catch_arms = planned_catches
        .iter()
        .enumerate()
        .map(|(clause_index, catch)| {
            let mut catch_env = env.clone();
            catch_env.insert_local(
                catch.name.clone(),
                JavaType::Named {
                    name: "JavaException".to_string(),
                    args: Vec::new(),
                },
            );
            CatchRenderArm {
                variable_name: catch.name.clone(),
                class_names: catch_match_class_names(
                    ctx.current_type,
                    ctx.current_method,
                    clause_index,
                    catch,
                    ctx.facts,
                ),
                body: lower_block(&catch.body, &mut catch_env, wrap_returns_in_ok, ctx),
            }
        })
        .collect();
    try_catch_stmt(
        ok_type,
        try_body,
        catch_arms,
        wrap_returns_in_ok,
        matches!(completion, TryBlockCompletion::FallsThroughUnit),
    )
}

fn lower_native_collection_extend_expr_stmt(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustStmt> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if args.len() != 1 {
        return None;
    }
    let plan = plan_call_expr(
        Some(receiver),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    let CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
        method: crate::call_plan::NativeCollectionMethod::Extend,
        ..
    }) = plan
    else {
        return None;
    };

    let receiver_ty = types::infer_expr_type(receiver, env.type_env());
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_shape = infer_expr_shape_with_context(&args[0], env, shape_ctx);
    let arg = match plan_native_collection_extend_arg(&receiver_ty, &arg_shape)? {
        NativeCollectionExtendArgPlan::BorrowedSequenceSource { element_shape } => {
            borrowed_sequence_extend_arg(lower_expr(&args[0], env, ctx), &element_shape, ctx.facts)
        }
        NativeCollectionExtendArgPlan::OwnedSource { slot_shape } => {
            lower_expr_for_slot(&args[0], env, argument_slot(slot_shape), ctx)
        }
    };

    Some(expr_stmt(apply_native_collection_extend(
        adapt_native_collection_receiver(
            receiver,
            lower_expr(receiver, env, ctx),
            crate::call_plan::NativeCollectionMethod::Extend,
            env,
            ctx,
        ),
        arg,
    )))
}

fn lower_planned_call_expr_stmt(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustStmt> {
    match plan_call_statement(expr, ctx.facts)? {
        CallStatementPlan::TrivialAccessorSetter {
            receiver,
            field_name,
            value,
        } => Some(assignment_stmt(
            lower_accessor_field(receiver, &field_name, env, ctx),
            lower_expr(value, env, ctx),
        )),
        CallStatementPlan::IterableForEach {
            receiver,
            param_names,
            body,
        } => lower_iterable_for_each_stmt(receiver, &param_names, body, env, ctx),
    }
}

fn lower_throwing_call_expr_stmt(
    expr: &Expr,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> Option<RustStmt> {
    if !wrap_returns_in_ok {
        return None;
    }
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if !matches!(
        plan_call_exception_propagation(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            ctx.facts,
            env.type_env(),
            ctx.current_type,
        ),
        CallExceptionPropagationPlan::PropagateResult
    ) {
        return None;
    }
    Some(expr_stmt(propagate_result_expr(lower_expr(expr, env, ctx))))
}

fn lower_iterable_for_each_stmt(
    receiver: &Expr,
    param_names: &[String],
    body: &LambdaBody,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustStmt> {
    let element_shape =
        infer_stream_source_element_shape_with_facts(receiver, env.type_env(), ctx.facts)
            .unwrap_or_else(|| {
                fallback_local_shape_from_declared_type(&JavaType::Unknown(
                    "forEach item".to_string(),
                ))
            });
    let mut loop_env = env.clone();
    for param_name in param_names {
        loop_env.insert_local_shape(param_name.clone(), element_shape.clone());
    }
    let loop_body = match body {
        LambdaBody::Expr(expr) if param_names.len() == 1 => vec![expr_stmt(
            lower_for_each_lambda_body_expr(expr, &loop_env, ctx),
        )],
        LambdaBody::Expr(expr) => vec![expr_stmt(lower_expr(expr, &loop_env, ctx))],
        LambdaBody::Block(block) => {
            let lambda_ctx = LowerCtx {
                current_return_shape: None,
                ..ctx
            };
            lower_lambda_block(block, &mut loop_env, lambda_ctx)
        }
    };
    let pattern = if param_names.len() == 1 {
        param_names[0].clone()
    } else {
        format!("({})", param_names.join(", "))
    };
    Some(render_enhanced_for(
        &pattern,
        &lower_enhanced_for_iterable_expr(receiver, env, ctx),
        &loop_body,
    ))
}

fn lower_enhanced_for_stmt(
    var_name: &str,
    iterable: &Expr,
    body: &Block,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
) -> RustStmt {
    let mut loop_env = env.clone();
    if let Some(element_shape) =
        infer_stream_source_element_shape_with_facts(iterable, env.type_env(), ctx.facts)
    {
        loop_env.insert_local_shape(var_name.to_string(), element_shape);
    }
    render_enhanced_for(
        var_name,
        &lower_enhanced_for_iterable_expr(iterable, env, ctx),
        &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
    )
}

fn lower_enhanced_for_iterable_expr(iterable: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    let lowered = lower_expr(iterable, env, ctx);
    if enhanced_for_iterable_needs_optional_unwrap(iterable, env, ctx) {
        RustExpr::Raw(format!(
            "{}.as_ref().expect(\"Java null receiver\")",
            emit::emit_expr(&lowered)
        ))
    } else {
        lowered
    }
}

fn enhanced_for_iterable_needs_optional_unwrap(
    iterable: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let shape = infer_expr_shape_with_context(
        iterable,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    crate::types::optional_element_type(&shape.java_ty)
        .and_then(|element_ty| crate::types::sequence_element_type(&element_ty))
        .is_some()
}

fn lower_for_each_lambda_body_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    let Expr::Call {
        receiver: None,
        name,
        args,
        ..
    } = expr
    else {
        return lower_expr(expr, env, ctx);
    };
    let rust_name = crate::parser::to_snake_case(name);
    let rust_args = lower_same_object_call_args(&rust_name, args, env, ctx);
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var("self".to_string())),
        method: rust_name,
        args: rust_args,
    }
}

fn lower_same_object_call_args(
    rust_name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    let Some(current_type) = ctx.current_type else {
        return args.iter().map(|arg| lower_expr(arg, env, ctx)).collect();
    };
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            ctx.signatures
                .method_arg_shape_by_rust_name(current_type, rust_name, index)
                .map(|shape| lower_expr_for_slot(arg, env, argument_slot(shape), ctx))
                .unwrap_or_else(|| lower_expr(arg, env, ctx))
        })
        .collect()
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
    lower_expr_with_refinements(expr, env, ctx, &ExprRefinements::default())
}

#[derive(Debug, Default)]
struct ExprRefinements {
    non_null_locals: HashSet<String>,
}

impl ExprRefinements {
    fn with_non_null_local(&self, name: &str) -> Self {
        let mut refinements = Self {
            non_null_locals: self.non_null_locals.clone(),
        };
        refinements.non_null_locals.insert(name.to_string());
        refinements
    }
}

fn lower_expr_with_refinements(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        return stream_expr.expr;
    }
    if let Some(format_expr) = lower_string_concat(expr, env, ctx) {
        return format_expr;
    }
    if let Some(map_entry_expr) =
        lower_map_entry_accessor_expr_for_slot(expr, env, &Slot::Value, ctx, refinements)
    {
        return map_entry_expr;
    }
    if let Some(array_plan) = plan_array_expr(expr) {
        let children = array_plan
            .child_exprs()
            .iter()
            .map(|child| lower_expr_with_refinements(child, env, ctx, refinements))
            .collect();
        return render_array_expr(&array_plan, children);
    }
    if let Expr::Lambda { params, body } = expr {
        return lower_callback_lambda_expr(params, body, env, ctx);
    }
    if let Expr::MethodReference { target, method } = expr {
        return render_method_reference(&plan_method_reference(target, method));
    }
    if let Some(kind) = plan_unsupported_expression(expr) {
        return unsupported_expression_expr(kind);
    }
    match expr {
        Expr::Literal(lit) => literal_expr(lit),
        Expr::Var(name) if refinements.non_null_locals.contains(name) => {
            lower_refined_non_null_var(name, env)
        }
        Expr::Var(name) => lower_bare_name(name, env),
        Expr::This => this_expr(),
        Expr::Super => super_expr(),
        Expr::FieldAccess(receiver, field) => lower_field_access(
            receiver,
            field,
            env.type_env(),
            lower_expr_with_refinements(receiver, env, ctx, refinements),
        ),
        Expr::ArrayAccess(array, index) => indexed_expr(
            lower_expr_with_refinements(array, env, ctx, refinements),
            lower_expr_for_slot(index, env, index_slot(), ctx),
        ),
        Expr::ArrayCreation { .. } | Expr::ArrayInitializer(_) => {
            unreachable!("array expressions should be planned before expression lowering")
        }
        Expr::Binary { op, left, right } => {
            lower_binary_expr(*op, left, right, env, ctx, refinements)
        }
        Expr::Unary { op, expr } => lower_unary_expr(
            *op,
            lower_expr_with_refinements(expr, env, ctx, refinements),
        ),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => lower_ternary_expr(condition, then_expr, else_expr, env, ctx, refinements),
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            if name == "removeIf"
                && args.len() == 1
                && let Some(receiver) = receiver.as_deref()
                && let Some(expr) = apply_collection_remove_if(
                    lower_expr_with_refinements(receiver, env, ctx, refinements),
                    &args[0],
                )
            {
                return expr;
            }
            if let Some(expr) = lower_java_equality_call_expr(
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                env,
                ctx,
                refinements,
            ) {
                return expr;
            }
            let plan = plan_call_expr(
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                ctx.facts,
                env.type_env(),
            );
            let fallback_signature =
                fallback_receiver_method_signature(receiver.as_deref(), name, args.len(), env, ctx);
            let plan = apply_fallback_receiver_rust_name(
                plan,
                fallback_signature
                    .as_ref()
                    .map(|(rust_name, _)| rust_name.clone()),
            );
            let target = match plan {
                CallExprPlan::TrivialAccessorGetter {
                    receiver,
                    field_name,
                } => return lower_accessor_field(receiver, &field_name, env, ctx),
                CallExprPlan::Target(target) => target,
            };
            let arg_plan = plan_call_arguments_with_facts_and_args(
                receiver.as_deref(),
                name,
                resolved.as_ref(),
                args,
                ctx.signatures,
                ctx.facts,
                env.type_env(),
            );
            let arg_plan =
                apply_fallback_signature_arg_shapes(arg_plan, fallback_signature.as_ref());
            let rust_args = lower_args_for_call_target(&target, args, env, arg_plan, ctx);
            let lowered_receiver = target.receiver_expr().map(|receiver| {
                let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
                adapt_call_target_receiver_with_signature(
                    &target,
                    receiver,
                    lowered,
                    env,
                    ctx,
                    fallback_signature.as_ref().map(|(_, signature)| *signature),
                )
            });
            apply_call_target(&target, lowered_receiver, rust_args)
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            let target = plan_new_target_for_args_with_facts(
                ty,
                resolved.as_ref(),
                ctx.representations,
                ctx.facts,
                args,
                env.type_env(),
            );
            let constructor_arg_plan = if matches!(
                target.native_collection,
                Some(NativeCollectionNewTarget::WithCapacity { .. })
            ) {
                ArgumentLoweringPlan::Unshaped {
                    arg_count: args.len(),
                }
            } else {
                plan_constructor_arguments(resolved.as_ref(), args.len(), ctx.signatures)
            };
            let rust_args = lower_constructor_args_with_plan(args, env, constructor_arg_plan, ctx);
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
            apply_cast_conversion(
                lower_expr_with_refinements(expr, env, ctx, refinements),
                &plan,
            )
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
            apply_instanceof(
                lower_expr_with_refinements(expr, env, ctx, refinements),
                &plan,
            )
        }
        Expr::ClassLiteral(ty) => apply_class_literal_token(ty, ctx.facts),
        Expr::Lambda { .. } | Expr::MethodReference { .. } => {
            unreachable!(
                "unsupported expression kinds should be handled before expression lowering"
            )
        }
        Expr::Raw(raw) => raw_expr(raw),
    }
}

fn lower_callback_lambda_expr(
    params: &[LambdaParam],
    body: &LambdaBody,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let mut lambda_env = env.clone();
    for param in params {
        let ty = param
            .ty
            .clone()
            .unwrap_or_else(|| JavaType::Unknown(format!("lambda_param:{}", param.name)));
        lambda_env.insert_local_shape(
            param.name.clone(),
            fallback_local_shape_from_declared_type(&ty),
        );
    }
    let params = params
        .iter()
        .map(|param| param.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    match body {
        LambdaBody::Expr(expr) => RustExpr::Raw(format!(
            "|{params}| {}",
            emit::emit_expr(&lower_expr(expr, &lambda_env, ctx))
        )),
        LambdaBody::Block(block) => {
            let statements = lower_lambda_block(block, &mut lambda_env, ctx);
            RustExpr::Raw(format!(
                "|{params}| {{\n{}\n}}",
                emit::emit_statements(&statements, 1)
            ))
        }
    }
}

fn lower_refined_non_null_var(name: &str, env: &ShapeEnv) -> RustExpr {
    let lowered = lower_bare_name(name, env);
    if env
        .local_shape(name)
        .and_then(|shape| shape.optional_element_type())
        .is_some()
    {
        RustExpr::MethodCall {
            receiver: Box::new(lowered),
            method: "unwrap".to_string(),
            args: Vec::new(),
        }
    } else {
        lowered
    }
}

fn lower_ternary_expr(
    condition: &Expr,
    then_expr: &Expr,
    else_expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    let (then_refinements, else_refinements) = branch_refinements(condition, refinements);
    ternary_expr(
        lower_expr_for_slot(condition, env, condition_slot(), ctx),
        lower_expr_with_refinements(then_expr, env, ctx, &then_refinements),
        lower_expr_with_refinements(else_expr, env, ctx, &else_refinements),
    )
}

fn branch_refinements(
    condition: &Expr,
    refinements: &ExprRefinements,
) -> (ExprRefinements, ExprRefinements) {
    match plan_ternary_null_refinement(condition) {
        TernaryNullRefinementPlan::LocalNullTest {
            local_name,
            then_non_null,
            else_non_null,
        } => {
            let then_refinements = if then_non_null {
                refinements.with_non_null_local(local_name)
            } else {
                ExprRefinements {
                    non_null_locals: refinements.non_null_locals.clone(),
                }
            };
            let else_refinements = if else_non_null {
                refinements.with_non_null_local(local_name)
            } else {
                ExprRefinements {
                    non_null_locals: refinements.non_null_locals.clone(),
                }
            };
            (then_refinements, else_refinements)
        }
        TernaryNullRefinementPlan::None => (
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
            },
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
            },
        ),
    }
}

fn lower_binary_expr(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if is_null_literal(right)
        && let Some(expr) = lower_null_comparison_expr(
            op,
            lower_expr_with_refinements(left, env, ctx, refinements),
            false,
        )
    {
        return expr;
    }
    if is_null_literal(left)
        && let Some(expr) = lower_null_comparison_expr(
            op,
            lower_expr_with_refinements(right, env, ctx, refinements),
            true,
        )
    {
        return expr;
    }
    let left_ty = crate::types::infer_expr_type(left, env.type_env());
    let right_ty = crate::types::infer_expr_type(right, env.type_env());
    if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        let shape_ctx = ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        };
        let left_shape = crate::expr_shape::infer_expr_shape_with_context(left, env, shape_ctx);
        let right_shape = crate::expr_shape::infer_expr_shape_with_context(right, env, shape_ctx);
        return lower_equality_comparison_expr_for_op(
            op,
            plan_equality_comparison(&left_shape, &right_shape),
            lower_expr_with_refinements(left, env, ctx, refinements),
            lower_expr_with_refinements(right, env, ctx, refinements),
        );
    }
    if let Some(plan) = plan_numeric_binary_promotion(op, &left_ty, &right_ty) {
        return lower_numeric_binary_expr(
            op,
            lower_expr_with_refinements(left, env, ctx, refinements),
            lower_expr_with_refinements(right, env, ctx, refinements),
            plan,
        );
    }
    if is_ordered_comparison(op) {
        return binary_expr(
            op,
            lower_ordered_comparison_operand(left, &right_ty, env, ctx, refinements),
            lower_ordered_comparison_operand(right, &left_ty, env, ctx, refinements),
        );
    }
    binary_expr(
        op,
        lower_expr_with_refinements(left, env, ctx, refinements),
        lower_expr_with_refinements(right, env, ctx, refinements),
    )
}

fn is_ordered_comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    )
}

fn lower_ordered_comparison_operand(
    expr: &Expr,
    other_ty: &JavaType,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    if matches!(other_ty, JavaType::Primitive(PrimitiveType::Int))
        && is_native_collection_len_expr(expr, env, ctx)
    {
        return RustExpr::Raw(format!("{} as i32", emit::emit_expr(&lowered)));
    }
    lowered
}

fn is_native_collection_len_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return false;
    };
    let plan = plan_call_expr(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    matches!(
        plan,
        CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::Len,
            ..
        })
    )
}

fn is_null_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Null))
}

fn apply_fallback_signature_arg_shapes(
    plan: ArgumentLoweringPlan,
    fallback_signature: Option<&(String, &crate::signature::MethodSignaturePlan)>,
) -> ArgumentLoweringPlan {
    match (plan, fallback_signature) {
        (ArgumentLoweringPlan::Unshaped { arg_count }, Some((_, signature)))
            if signature.params.len() == arg_count =>
        {
            ArgumentLoweringPlan::Shaped(
                signature
                    .params
                    .iter()
                    .map(|param| param.shape.clone())
                    .collect(),
            )
        }
        (plan, _) => plan,
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
            .map(|(arg, shape)| {
                if matches!(arg, Expr::Lambda { .. } | Expr::MethodReference { .. }) {
                    lower_expr(arg, env, ctx)
                } else {
                    let lowered = lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx);
                    let actual_shape = infer_expr_shape_with_context(
                        arg,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    );
                    adapt_reused_owned_argument(lowered, arg, &shape, Some(&actual_shape), ctx)
                }
            })
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => args
            .iter()
            .map(|arg| lower_unshaped_argument(arg, env, ctx))
            .collect(),
    }
}

fn lower_constructor_args_with_plan(
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    match plan {
        ArgumentLoweringPlan::Shaped(shapes) => args
            .iter()
            .zip(shapes)
            .map(|(arg, shape)| {
                let lowered = if matches!(arg, Expr::Lambda { .. } | Expr::MethodReference { .. }) {
                    lower_expr(arg, env, ctx)
                } else {
                    lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx)
                };
                adapt_reused_owned_argument(lowered, arg, &shape, None, ctx)
            })
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => {
            let shape_ctx = ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            };
            args.iter()
                .map(|arg| {
                    let lowered = lower_unshaped_argument(arg, env, ctx);
                    let shape = infer_expr_shape_with_context(arg, env, shape_ctx);
                    adapt_reused_owned_argument(lowered, arg, &shape, None, ctx)
                })
                .collect()
        }
    }
}

fn lower_unshaped_argument(arg: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    let lowered = lower_expr(arg, env, ctx);
    let shape = infer_expr_shape_with_context(
        arg,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    if borrowed_self_receiver_can_read_fields(ctx)
        && shared_self_field_read_name(arg, env).is_some()
        && !matches!(shape.java_ty, JavaType::Primitive(_))
        && known_owned_argument_clone_recovery_with_facts(&shape, ctx.facts)
    {
        return RustExpr::Raw(format!(
            "{}.clone()/* TODO(translation): validate added clone */",
            emit::emit_expr(&lowered)
        ));
    }
    if reused_owned_argument_needs_clone(arg, &shape, Some(&shape), ctx) {
        return RustExpr::Raw(format!(
            "{}.clone()/* TODO(translation): validate added clone */",
            emit::emit_expr(&lowered)
        ));
    }
    lowered
}

fn lower_args_for_call_target(
    target: &CallTargetPlan<'_>,
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    match target {
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::StringFormat,
        } => lower_string_format_args(args, env, ctx),
        CallTargetPlan::NativeJavaInstance {
            method:
                crate::call_plan::NativeJavaInstanceMethod::StringContains
                | crate::call_plan::NativeJavaInstanceMethod::StringStartsWith
                | crate::call_plan::NativeJavaInstanceMethod::StringEndsWith,
            ..
        } => lower_string_pattern_args(args, env, ctx),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapInsert,
            ..
        } => lower_map_insert_args(args, env, plan, ctx),
        _ => lower_args_with_plan(args, env, plan, ctx),
    }
}

fn lower_map_insert_args(
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    match plan {
        ArgumentLoweringPlan::Shaped(shapes) => args
            .iter()
            .zip(shapes)
            .map(|(source, shape)| {
                let lowered = lower_expr_for_slot(source, env, argument_slot(shape.clone()), ctx);
                let actual_shape = infer_expr_shape_with_context(
                    source,
                    env,
                    ExprShapeContext {
                        signatures: Some(ctx.signatures),
                        representations: Some(ctx.representations),
                        facts: Some(ctx.facts),
                    },
                );
                adapt_reused_owned_argument(lowered, source, &shape, Some(&actual_shape), ctx)
            })
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => lower_args_with_plan(args, env, plan, ctx),
    }
}

fn adapt_reused_owned_argument(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    ctx: LowerCtx,
) -> RustExpr {
    if reused_owned_argument_needs_clone(source, target_shape, actual_shape, ctx) {
        RustExpr::Raw(format!(
            "{}.clone()/* TODO(translation): validate added clone */",
            emit::emit_expr(&lowered)
        ))
    } else {
        lowered
    }
}

fn reused_owned_argument_needs_clone(
    arg: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    ctx: LowerCtx,
) -> bool {
    if target_shape.ownership != OwnershipShape::Owned {
        return false;
    }
    if matches!(target_shape.dispatch, DispatchShape::ErasedObject)
        || matches!(target_shape.view, ViewShape::ErasedObjectView)
    {
        return false;
    }
    if !known_owned_argument_clone_recovery_with_facts(target_shape, ctx.facts)
        && !actual_shape.is_some_and(|actual_shape| {
            known_owned_argument_clone_recovery_with_facts(actual_shape, ctx.facts)
        })
    {
        return false;
    }
    let Expr::Var(name) = arg else {
        return false;
    };
    current_callable_binding_use_count(name, ctx).is_some_and(|use_count| use_count > 1)
}

fn known_owned_argument_clone_recovery(target_shape: &RustSlotShape) -> bool {
    matches!(target_shape.java_ty, JavaType::String)
}

fn known_owned_argument_clone_recovery_with_facts(
    target_shape: &RustSlotShape,
    facts: &FactSet,
) -> bool {
    match crate::effective_facts::EffectiveFacts::new(facts)
        .owned_recovery_capability_for(&target_shape.java_ty)
    {
        OwnedRecoveryCapability::Copy
        | OwnedRecoveryCapability::ExplicitClone { .. }
        | OwnedRecoveryCapability::ConservativeClone
        | OwnedRecoveryCapability::ImmutableSharedHandle { .. }
        | OwnedRecoveryCapability::CloneBoxAdapter { .. }
        | OwnedRecoveryCapability::FactoryCopy { .. } => true,
        OwnedRecoveryCapability::NonCloneable
        | OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. }
        | OwnedRecoveryCapability::FfiOwnedHandle { .. } => false,
    }
}

fn known_named_clone_recovery(target_shape: &RustSlotShape) -> bool {
    matches!(target_shape.java_ty, JavaType::Named { .. })
}

fn current_callable_binding_use_count(name: &str, ctx: LowerCtx) -> Option<usize> {
    let current_type = ctx.current_type?;
    if let Some(current_method) = ctx.current_method {
        let exact_key = UsageMethodKey {
            type_name: current_type.to_string(),
            method_name: current_method.to_string(),
        };
        let exact_count = ctx
            .usage
            .methods
            .get(&exact_key)
            .and_then(|method| method.bindings.get(name))
            .map(|binding| binding.uses.len());
        let rust_base = crate::parser::to_snake_case(current_method);
        let overload_prefix = format!("{rust_base}_");
        let overloaded_count = ctx
            .usage
            .methods_by_rust_name
            .iter()
            .filter(|(key, _)| {
                key.type_name == current_type
                    && (key.method_name == rust_base
                        || key.method_name.starts_with(&overload_prefix))
            })
            .filter_map(|(_, method)| method.bindings.get(name))
            .map(|binding| binding.uses.len())
            .max();
        return exact_count.into_iter().chain(overloaded_count).max();
    }
    if let Some(current_constructor) = ctx.current_constructor {
        return ctx
            .usage
            .constructors
            .get(&UsageConstructorKey {
                type_name: current_type.to_string(),
                rust_name: current_constructor.to_string(),
            })
            .and_then(|constructor| constructor.bindings.get(name))
            .map(|binding| binding.uses.len());
    }
    None
}

fn borrowed_str_slot() -> Slot {
    let mut shape = RustSlotShape::conservative(JavaType::String);
    shape.ownership = OwnershipShape::SharedBorrow;
    shape.view = ViewShape::Str;
    argument_slot(shape)
}

fn lower_string_pattern_args(args: &[Expr], env: &ShapeEnv, ctx: LowerCtx) -> Vec<RustExpr> {
    args.iter()
        .map(|arg| lower_expr_for_slot(arg, env, borrowed_str_slot(), ctx))
        .collect()
}

fn lower_string_format_args(args: &[Expr], env: &ShapeEnv, ctx: LowerCtx) -> Vec<RustExpr> {
    let Some((format_arg, display_args)) = args.split_first() else {
        return Vec::new();
    };
    let mut lowered = Vec::with_capacity(args.len());
    lowered.push(lower_expr_for_slot(
        format_arg,
        env,
        argument_slot(RustSlotShape::conservative(JavaType::String)),
        ctx,
    ));
    lowered.extend(
        display_args
            .iter()
            .map(|arg| lower_expr_for_slot(arg, env, Slot::DisplaySegment, ctx)),
    );
    lowered
}

fn adapt_call_target_receiver(
    target: &CallTargetPlan<'_>,
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    match target {
        CallTargetPlan::NativeCollectionMethod { method, .. } => {
            adapt_native_collection_receiver(receiver, lowered, *method, env, ctx)
        }
        _ if target.adapts_optional_instance_receiver() => {
            if lowered_receiver_is_optional_borrow(receiver, env, ctx) {
                return RustExpr::Raw(format!(
                    "{}.expect(\"Java null receiver\")",
                    emit::emit_expr(&lowered)
                ));
            }
            adapt_optional_instance_receiver(receiver, lowered, env, ctx)
        }
        _ => lowered,
    }
}

fn adapt_call_target_receiver_with_signature(
    target: &CallTargetPlan<'_>,
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
) -> RustExpr {
    if target.adapts_optional_instance_receiver()
        && fallback_signature.is_some_and(|signature| {
            matches!(signature.receiver, crate::signature::ReceiverPlan::Mutable)
        })
    {
        return adapt_optional_instance_receiver_with_access(receiver, lowered, env, ctx, "as_mut");
    }
    adapt_call_target_receiver(target, receiver, lowered, env, ctx)
}

fn lowered_receiver_is_optional_borrow(receiver: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Expr::Call {
        receiver: call_receiver,
        name,
        args,
        resolved,
    } = receiver
    else {
        return false;
    };
    let plan = plan_call_expr(
        call_receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    let CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod { receiver, method }) = plan
    else {
        return false;
    };
    matches!(
        native_collection_lowered_type(receiver, method, env.type_env()),
        Some(crate::conversion_plan::LoweredType::OptionalElementBorrow { .. })
    )
}

fn adapt_optional_instance_receiver(
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    adapt_optional_instance_receiver_with_access(receiver, lowered, env, ctx, "as_ref")
}

fn adapt_optional_instance_receiver_with_access(
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    access: &str,
) -> RustExpr {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if receiver_shape.optional_element_type().is_none() {
        return lowered;
    }
    RustExpr::Raw(format!("{}.{access}().unwrap()", emit::emit_expr(&lowered)))
}

fn adapt_native_collection_receiver(
    receiver: &Expr,
    lowered: RustExpr,
    method: NativeCollectionMethod,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let Some(element_ty) = receiver_shape.optional_element_type() else {
        return lowered;
    };
    if crate::stdlib::java_collection_kind_for_type(&element_ty).is_none() {
        return lowered;
    }
    let receiver = emit::emit_expr(&lowered);
    let access = match method {
        NativeCollectionMethod::ListPush
        | NativeCollectionMethod::SetInsert
        | NativeCollectionMethod::Extend
        | NativeCollectionMethod::MapInsert => "as_mut",
        NativeCollectionMethod::Len
        | NativeCollectionMethod::IsEmpty
        | NativeCollectionMethod::ListGet
        | NativeCollectionMethod::MapGet
        | NativeCollectionMethod::MapContainsKey
        | NativeCollectionMethod::MapKeys
        | NativeCollectionMethod::MapIter => "as_ref",
    };
    RustExpr::Raw(format!("{receiver}.{access}().unwrap()"))
}

fn lower_java_equality_call_expr(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if resolved.is_some() {
        return None;
    }
    let target = match plan_call_expr(receiver, name, args, None, ctx.facts, env.type_env()) {
        CallExprPlan::Target(target) => target,
        CallExprPlan::TrivialAccessorGetter { .. } => return None,
    };
    let (left, right, use_fact_aware_plan) = match target {
        CallTargetPlan::NativeJavaInstance {
            receiver,
            method: crate::call_plan::NativeJavaInstanceMethod::Equals,
        } if args.len() == 1 => (receiver, &args[0], false),
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::ObjectsEquals,
        } if args.len() == 2 => (&args[0], &args[1], true),
        _ => return None,
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let left_shape = crate::expr_shape::infer_expr_shape_with_context(left, env, shape_ctx);
    let right_shape = crate::expr_shape::infer_expr_shape_with_context(right, env, shape_ctx);
    let plan = if use_fact_aware_plan {
        plan_equality_comparison_with_facts(&left_shape, &right_shape, ctx.facts)
    } else {
        plan_equality_comparison(&left_shape, &right_shape)
    };
    Some(lower_equality_comparison_expr(
        plan,
        lower_expr_with_refinements(left, env, ctx, refinements),
        lower_expr_with_refinements(right, env, ctx, refinements),
    ))
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
    render_stream_plan_with_context(
        plan,
        lower_expr(plan.source.expr, env, ctx),
        StreamRenderContext::new_with_facts(env, ctx.facts),
    )
}

fn lower_expr_for_slot(expr: &Expr, env: &ShapeEnv, slot: Slot, ctx: LowerCtx) -> RustExpr {
    lower_expr_for_slot_with_refinements(expr, env, slot, ctx, &ExprRefinements::default())
}

fn lower_expr_for_slot_with_refinements(
    expr: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if matches!(expr, Expr::Literal(Literal::Null)) && slot_expects_optional(&slot) {
        return RustExpr::Var("None".to_string());
    }
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        let plan = plan_lowered_slot_conversion(&stream_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(stream_expr, &plan);
    }
    if let Some(sequence_get_expr) =
        lower_sequence_get_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return sequence_get_expr;
    }
    if let Some(map_get_expr) = lower_map_get_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return map_get_expr;
    }
    if let Some(map_entry_expr) =
        lower_map_entry_accessor_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return map_entry_expr;
    }
    if let Some(collection_expr) =
        lower_native_collection_call_expr_for_slot(expr, env, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&collection_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(collection_expr, &plan);
    }
    if let Some(external_expr) = lower_external_call_expr_for_slot(expr, env, ctx, refinements) {
        let plan = plan_lowered_slot_conversion(&external_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(external_expr, &plan);
    }
    if let Expr::Ternary {
        condition,
        then_expr,
        else_expr,
    } = expr
    {
        return lower_ternary_expr_for_slot(
            condition,
            then_expr,
            else_expr,
            env,
            slot,
            ctx,
            refinements,
        );
    }
    let actual = actual_shape_for_slot_expr_with_slot(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
        &slot,
    );
    if let Some(recovered_field) =
        lower_shared_self_field_owned_recovery_for_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return recovered_field;
    }
    if let Some(reused_optional) =
        lower_reused_optional_unwrap_for_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return reused_optional;
    }
    if let Some(reused_wrap) =
        lower_reused_optional_wrap_for_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return reused_wrap;
    }
    let mut lowered_expr = if slot_expects_optional(&slot)
        && matches!(expr, Expr::Var(name) if refinements.non_null_locals.contains(name))
    {
        lower_expr_with_refinements(expr, env, ctx, &ExprRefinements::default())
    } else {
        lower_expr_with_refinements(expr, env, ctx, refinements)
    };
    if slot_expr_needs_result_propagation(expr, env, ctx) {
        lowered_expr = propagate_result_expr(lowered_expr);
    }
    let plan = plan_slot_conversion(actual, slot, ctx.facts);
    apply_slot_expr_conversion(lowered_expr, &plan)
}

fn lower_shared_self_field_owned_recovery_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !borrowed_self_receiver_can_read_fields(ctx) {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if let Some(expr) =
        lower_optional_self_field_unwrap_owned_recovery(expr, env, target, ctx, refinements)
    {
        return Some(expr);
    }
    if let Some(expr) =
        lower_optional_self_field_read_owned_recovery(expr, env, target, actual, ctx, refinements)
    {
        return Some(expr);
    }
    if target.ownership != OwnershipShape::Owned || target.java_ty != actual.java_ty {
        if let Some(element_ty) = target.optional_element_type()
            && shared_self_field_read_name(expr, env).is_some()
            && !matches!(element_ty, JavaType::Primitive(_))
        {
            let element_shape = RustSlotShape::conservative(element_ty);
            if !known_owned_argument_clone_recovery_with_facts(&element_shape, ctx.facts)
                && !known_named_clone_recovery(&element_shape)
            {
                return None;
            }
            let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
            let recovered = RustExpr::Raw(format!(
                "{}.clone()/* TODO(translation): validate added clone */",
                emit::emit_expr(&lowered)
            ));
            return Some(crate::coercion::apply_expr_to_slot_with_facts(
                recovered,
                element_shape,
                slot,
                ctx.facts,
            ));
        }
        return None;
    }
    if matches!(actual.java_ty, JavaType::Primitive(_)) {
        return None;
    }
    shared_self_field_read_name(expr, env)?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let recovered = RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&lowered)));
    Some(crate::coercion::apply_expr_to_slot_with_facts(
        recovered,
        target.clone(),
        slot,
        ctx.facts,
    ))
}

fn lower_optional_self_field_read_owned_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    target: &RustSlotShape,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    shared_self_field_read_name(expr, env)?;
    let element_ty = actual.optional_element_type()?;
    if target.ownership != OwnershipShape::Owned || target.java_ty != element_ty {
        return None;
    }
    if matches!(target.java_ty, JavaType::Primitive(_))
        || (!known_owned_argument_clone_recovery_with_facts(target, ctx.facts)
            && !known_named_clone_recovery(target))
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "{}.as_ref().expect(\"Java null receiver\").clone()/* TODO(translation): validate added clone */",
        emit::emit_expr(&lowered)
    )))
}

fn lower_optional_self_field_unwrap_owned_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    target: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name != "unwrap" || !args.is_empty() {
        return None;
    }
    shared_self_field_read_name(receiver, env)?;
    let receiver_shape = infer_expr_shape_with_context(
        receiver,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let element_ty = crate::types::optional_element_type(&receiver_shape.java_ty)?;
    if target.ownership != OwnershipShape::Owned || target.java_ty != element_ty {
        return None;
    }
    if matches!(target.java_ty, JavaType::Primitive(_))
        || (!known_owned_argument_clone_recovery_with_facts(target, ctx.facts)
            && !known_named_clone_recovery(target))
    {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "{}.as_ref().expect(\"Java null receiver\").clone()/* TODO(translation): validate added clone */",
        emit::emit_expr(&lowered_receiver)
    )))
}

fn borrowed_self_receiver_can_read_fields(ctx: LowerCtx) -> bool {
    matches!(
        ctx.current_receiver,
        Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
    )
}

fn shared_self_field_read_name<'a>(expr: &'a Expr, env: &ShapeEnv) -> Option<&'a str> {
    match expr {
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field.as_str())
        }
        Expr::Var(name)
            if env.local_shape(name).is_none()
                && env.field_shape(name).is_some()
                && !env.is_static_field(name) =>
        {
            Some(name.as_str())
        }
        _ => None,
    }
}

fn slot_expr_needs_result_propagation(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return false;
    };
    matches!(
        plan_call_exception_propagation(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            ctx.facts,
            env.type_env(),
            ctx.current_type,
        ),
        CallExceptionPropagationPlan::PropagateResult
    )
}

fn slot_expects_optional(slot: &Slot) -> bool {
    let shape = match slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => shape,
        Slot::Value
        | Slot::Condition
        | Slot::Index
        | Slot::StringConcatSegment
        | Slot::DisplaySegment => {
            return false;
        }
    };
    crate::types::optional_element_type(&shape.java_ty).is_some()
}

fn lower_reused_optional_unwrap_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Var(name) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some() || target.ownership != OwnershipShape::Owned {
        return None;
    }
    if matches!(target.dispatch, DispatchShape::ErasedObject)
        || matches!(target.view, ViewShape::ErasedObjectView)
    {
        return None;
    }
    let element_ty = actual.optional_element_type()?;
    if element_ty != target.java_ty {
        return None;
    }
    if !known_owned_argument_clone_recovery(target) && !known_named_clone_recovery(target) {
        return None;
    }
    if !current_callable_binding_use_count(name, ctx).is_some_and(|use_count| use_count > 1) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "{}.as_ref().unwrap().clone()/* TODO(translation): validate added clone */",
        emit::emit_expr(&lowered)
    )))
}

fn lower_reused_optional_wrap_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Var(name) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let element_ty = target.optional_element_type()?;
    if actual.optional_element_type().is_some() || actual.ownership != OwnershipShape::Owned {
        return None;
    }
    if element_ty != actual.java_ty {
        return None;
    }
    if matches!(target.dispatch, DispatchShape::ErasedObject)
        || matches!(target.view, ViewShape::ErasedObjectView)
    {
        return None;
    }
    if !known_owned_argument_clone_recovery(actual) && !known_named_clone_recovery(actual) {
        return None;
    }
    if !current_callable_binding_use_count(name, ctx).is_some_and(|use_count| use_count > 1) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "Some({}.clone()/* TODO(translation): validate added clone */)",
        emit::emit_expr(&lowered)
    )))
}

fn lower_map_entry_accessor_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved: _,
    } = expr
    else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let (next_expr, key_ty, value_ty) = lower_map_entry_next_expr(receiver, env, ctx, refinements)?;
    let (field, actual_ty) = match name.as_str() {
        "getKey" | "get_key" => (
            "0",
            map_entry_accessor_type_or_slot_target(&key_ty, slot).unwrap_or(key_ty),
        ),
        "getValue" | "get_value" => ("1", value_ty),
        _ => return None,
    };
    let entry_access = RustExpr::Raw(format!("{}.{}", emit::emit_expr(&next_expr), field));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(actual_ty);
    Some(crate::coercion::apply_expr_to_slot_with_facts(
        entry_access,
        actual,
        slot,
        ctx.facts,
    ))
}

fn map_entry_accessor_type_or_slot_target(actual_ty: &JavaType, slot: &Slot) -> Option<JavaType> {
    if !matches!(actual_ty, JavaType::Unknown(_)) {
        return Some(actual_ty.clone());
    }
    match slot {
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            Some(target.java_ty.clone())
        }
        Slot::Value
        | Slot::Condition
        | Slot::Index
        | Slot::StringConcatSegment
        | Slot::DisplaySegment => None,
    }
}

fn lower_map_entry_next_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<(RustExpr, JavaType, JavaType)> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved: _,
    } = expr
    else {
        return None;
    };
    if name != "next" || !args.is_empty() {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let (iter_expr, key_ty, value_ty) =
        lower_map_entry_iterator_expr(receiver, env, ctx, refinements)?;
    Some((
        RustExpr::Raw(format!(
            "{}.next().expect(\"Java iterator exhausted\")",
            emit::emit_expr(&iter_expr)
        )),
        key_ty,
        value_ty,
    ))
}

fn lower_map_entry_iterator_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<(RustExpr, JavaType, JavaType)> {
    if let Expr::Call {
        receiver,
        name,
        args,
        resolved: _,
    } = expr
        && name == "iterator"
        && args.is_empty()
    {
        return lower_map_entry_set_expr(receiver.as_deref()?, env, ctx, refinements);
    }
    lower_map_entry_set_expr(expr, env, ctx, refinements)
}

fn lower_map_entry_set_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<(RustExpr, JavaType, JavaType)> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved: _,
    } = expr
    else {
        return None;
    };
    if !matches!(name.as_str(), "entrySet" | "entry_set") || !args.is_empty() {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let (key_ty, value_ty, receiver_is_optional_map) =
        map_type_args_from_expr_shape(receiver, env, ctx).unwrap_or_else(|| {
            (
                JavaType::Unknown("map_entry_key".to_string()),
                JavaType::Unknown("map_entry_value".to_string()),
                false,
            )
        });
    let mut receiver_expr = lower_expr_with_refinements(receiver, env, ctx, refinements);
    if receiver_is_optional_map {
        receiver_expr = RustExpr::Raw(format!(
            "{}.as_ref().expect(\"Java null receiver\")",
            emit::emit_expr(&receiver_expr)
        ));
    }
    Some((
        RustExpr::Raw(format!("{}.iter()", emit::emit_expr(&receiver_expr))),
        key_ty,
        value_ty,
    ))
}

fn map_type_args_from_expr_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<(JavaType, JavaType, bool)> {
    if let Some(shape) = method_call_return_shape_from_receiver(expr, env, ctx)
        && let Some(map_args) = map_type_args_from_java_type(&shape.java_ty)
    {
        return Some(map_args);
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    map_type_args_from_java_type(&shape.java_ty)
}

fn method_call_return_shape_from_receiver(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        resolved,
        ..
    } = expr
    else {
        return None;
    };
    if let Some(resolved) = resolved
        && let Some(shape) = ctx
            .signatures
            .method_return_shape_by_rust_name(&resolved.declaring_type, &resolved.rust_name)
    {
        return Some(shape);
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)
        .unwrap_or(receiver_shape.java_ty);
    let receiver_name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let snake_name = crate::parser::to_snake_case(name);
    ctx.signatures
        .method_return_shape_by_rust_name(&receiver_name, name)
        .or_else(|| {
            ctx.signatures
                .method_return_shape_by_rust_name(&receiver_name, &snake_name)
        })
}

fn fallback_receiver_method_signature<'a>(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    env: &ShapeEnv,
    ctx: LowerCtx<'a>,
) -> Option<(String, &'a crate::signature::MethodSignaturePlan)> {
    let receiver = receiver?;
    let receiver_name = receiver_type_name_from_shape(receiver, env, ctx)?;
    ctx.signatures
        .method_signature_by_base_name_and_arity(&receiver_name, name, arg_count)
}

fn receiver_type_name_from_shape(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<String> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)
        .unwrap_or(receiver_shape.java_ty);
    match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn map_type_args_from_java_type(ty: &JavaType) -> Option<(JavaType, JavaType, bool)> {
    let optional_map_ty = crate::types::optional_element_type(ty);
    let map_ty = optional_map_ty.clone().unwrap_or_else(|| ty.clone());
    let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&map_ty)?;
    Some((key_ty.clone(), value_ty.clone(), optional_map_ty.is_some()))
}

fn lower_sequence_get_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if resolved.is_some() || name != "get" || args.len() != 1 {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let element_ty = receiver_shape.sequence_element_type()?;
    let receiver_expr = lower_expr_with_refinements(receiver, env, ctx, refinements);
    let index_expr = lower_expr_for_slot(&args[0], env, index_slot(), ctx);
    let get_expr = RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java index out of bounds\")",
        emit::emit_expr(&receiver_expr),
        emit::emit_expr(&index_expr)
    ));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(element_ty);
    Some(crate::coercion::apply_expr_to_slot_with_facts(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn lower_map_get_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target_shape = slot_target_shape(slot)?;
    if target_shape.optional_element_type().is_some() {
        return None;
    }
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if resolved.is_some() || name != "get" || args.len() != 1 {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let receiver_ty = types::infer_expr_type(receiver, env.type_env());
    let (_, value_ty, _) = map_type_args_from_java_type(&receiver_ty)?;
    if target_shape.java_ty != value_ty {
        return None;
    }
    let plan = plan_call_expr(
        Some(receiver),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    let CallExprPlan::Target(
        target @ CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapGet,
            ..
        },
    ) = plan
    else {
        return None;
    };
    let rust_args = lower_args_for_call_target(
        &target,
        args,
        env,
        plan_call_arguments_with_facts_and_args(
            Some(receiver),
            name,
            resolved.as_ref(),
            args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        ctx,
    );
    let key = rust_args.into_iter().next()?;
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx)
    })?;
    let get_expr = RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java null receiver\")",
        emit::emit_expr(&lowered_receiver),
        emit::emit_expr(&key)
    ));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(value_ty);
    Some(crate::coercion::apply_expr_to_slot_with_facts(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn slot_target_shape(slot: &Slot) -> Option<&RustSlotShape> {
    match slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => Some(shape),
        _ => None,
    }
}

fn lower_native_collection_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<crate::coercion::LoweredExpr> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    let plan = plan_call_expr(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    let CallExprPlan::Target(target @ CallTargetPlan::NativeCollectionMethod { receiver, method }) =
        plan
    else {
        return None;
    };
    let actual = native_collection_lowered_type(receiver, method, env.type_env())?;
    let rust_args = lower_args_for_call_target(
        &target,
        args,
        env,
        plan_call_arguments_with_facts_and_args(
            Some(receiver),
            name,
            resolved.as_ref(),
            args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        ctx,
    );
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx)
    });
    Some(crate::coercion::LoweredExpr {
        expr: apply_call_target(&target, lowered_receiver, rust_args),
        actual,
    })
}

fn lower_external_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<crate::coercion::LoweredExpr> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if resolved.is_some() {
        return None;
    }
    let actual = external_call_lowered_type(
        receiver.as_deref(),
        name,
        args.len(),
        ctx.facts,
        env.type_env(),
        ctx.current_type,
    )?;
    let plan = plan_call_expr(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx.facts,
        env.type_env(),
    );
    let CallExprPlan::Target(target) = plan else {
        return None;
    };
    let rust_args = lower_args_with_plan(
        args,
        env,
        plan_call_arguments_with_facts_and_args(
            receiver.as_deref(),
            name,
            resolved.as_ref(),
            args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        ctx,
    );
    let lowered_receiver = target
        .receiver_expr()
        .map(|receiver| lower_expr_with_refinements(receiver, env, ctx, refinements));
    Some(crate::coercion::LoweredExpr {
        expr: apply_call_target(&target, lowered_receiver, rust_args),
        actual,
    })
}

fn lower_ternary_expr_for_slot(
    condition: &Expr,
    then_expr: &Expr,
    else_expr: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    let (then_refinements, else_refinements) = branch_refinements(condition, refinements);
    ternary_expr(
        lower_expr_for_slot(condition, env, condition_slot(), ctx),
        lower_expr_for_slot_with_refinements(then_expr, env, slot.clone(), ctx, &then_refinements),
        lower_expr_for_slot_with_refinements(else_expr, env, slot, ctx, &else_refinements),
    )
}

fn lower_string_concat(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    let plan = build_string_concat_plan(expr, env.type_env())?;
    let args = plan
        .args
        .iter()
        .map(|part| lower_expr_for_slot(part, env, Slot::StringConcatSegment, ctx))
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
