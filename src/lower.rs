use std::collections::{HashMap, HashSet};

use crate::access_lowering::{lower_bare_name, lower_field_access_with_facts};
use crate::array_lowering::{borrowed_array2_temporary_for_slot, render_array_expr};
use crate::array_plan::plan_array_expr;
use crate::call_lowering::{
    apply_call_target, apply_class_literal_token, apply_collection_remove_if,
    apply_native_collection_extend, apply_new_target_with_facts, borrowed_sequence_extend_arg,
    propagate_result_expr,
};
use crate::call_plan::{
    ArgumentLoweringPlan, CallExceptionPropagationPlan, CallExprPlan, CallStatementPlan,
    CallTargetPlan, NativeCollectionExtendArgPlan, NativeCollectionMethod,
    NativeCollectionNewTarget, NativeJavaInstanceMethod, apply_fallback_receiver_rust_name,
    apply_fallback_self_rust_name, external_call_lowered_type_with_external_index,
    external_call_returns_rust_result_with_external_index, external_constructor_arg_shapes,
    external_constructor_returns_rust_result, native_collection_lowered_type, plan_call_arguments,
    plan_call_arguments_with_facts_and_args, plan_call_exception_propagation_with_external_index,
    plan_call_expr_with_external_index, plan_call_statement, plan_constructor_arguments,
    plan_native_collection_extend_arg, plan_new_target_for_args_with_facts,
};
use crate::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, DelegatedConstructorTrailingStmt,
    plan_constructor_delegation_for_type, plan_constructor_field_values_with_inherited_context,
};
use crate::control_flow_lowering::{TailRecursiveLoopRender, render_tail_recursive_loop_body};
use crate::conversion_lowering::{
    apply_cast_conversion, apply_instanceof, apply_lowered_slot_expr_conversion,
    apply_slot_expr_conversion,
};
use crate::conversion_plan::{
    DispatchCoercionPolicy, LoweredType, ReturnValuePlan, Slot, SlotConversionPlan,
    actual_shape_for_slot_expr, actual_shape_for_slot_expr_with_slot, argument_slot,
    assignment_statement_plan_for_target, assignment_value_plan_for_shape,
    assignment_value_plan_for_target, condition_slot, index_slot, plan_cast_conversion_from_actual,
    plan_instanceof, plan_lowered_slot_conversion, plan_slot_conversion, return_value_plan,
};
use crate::diagnostics::DiagnosticBag;
use crate::emit;
use crate::exception_lowering::{
    CatchRenderArm, exception_cause_or_self_if_instance_expr,
    exception_expr_from_new_with_cause_hint, exception_result_type, exception_try_ok_type,
    fallback_exception_expr, java_exception_support_item_with_facts, return_stmt,
    throw_stmt_with_result_policy, try_catch_finally_scope_stmt, try_catch_stmt,
    try_finally_scope_stmt, try_with_resources_scope_stmt,
};
use crate::exception_plan::{
    ProgramExceptionPlan, ThrowExprPlan, TryBlockCompletion, TryStmtPlan, catch_match_class_names,
    plan_block_exception_completion, plan_program_exceptions, plan_throw_expr, plan_try_stmt,
    try_resource_names,
};
use crate::expr_render::{
    binary_expr, indexed_expr, literal_expr, named_constructor_expr, raw_expr, super_expr,
    ternary_expr, this_expr,
};
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, fallback_local_shape_from_declared_type,
    infer_expr_shape_with_context, learn_stmt_shapes, shape_env_for_constructor,
    shape_env_for_method, shape_env_for_type,
};
use crate::facts::{ExternalFactLookupIndex, FactSet, NullabilityState, OwnedRecoveryCapability};
use crate::item_assembly::{
    constructor_default_field_value_with_facts, constructor_function_shell_with_facts,
    constructor_return_body, delegating_constructor_return_body, impl_item_for_type,
    method_function_shell_with_facts_and_exception_plan, raw_item,
    struct_item_for_type_with_facts_and_support_context,
};
use crate::java_ir::Member as Jm;
use crate::java_ir::*;
use crate::javalibs::{broad_ioutils_support_item, program_uses_broad_ioutils_facade};
use crate::loop_lowering::{assemble_for_loop, assignment_condition_loop, continue_with_updates};
use crate::loop_plan::{ForBodyStmtPlan, WhileStmtPlan, plan_for_body_stmt, plan_while_stmt};
use crate::lowering_diagnostics::{LoweringDiagnosticPlan, collect_lowering_diagnostic_plans};
use crate::method_reference_lowering::render_method_reference;
use crate::method_reference_plan::{
    MethodReferencePlan, plan_current_receiver_method_reference, plan_method_reference,
};
use crate::nullability_plan::{TernaryNullRefinementPlan, plan_ternary_null_refinement};
use crate::operator_lowering::lower_equality_comparison_expr_for_op;
use crate::operator_lowering::{
    lower_equality_comparison_expr, lower_numeric_binary_expr, plan_equality_comparison,
    plan_equality_comparison_with_facts, plan_numeric_binary_promotion,
};
use crate::operator_lowering::{lower_null_comparison_expr, lower_unary_expr};
use crate::rewrite::{MethodControlFlowPlan, method_has_tail_recursive_loop_rewrite};
use crate::rust_ir::*;
use crate::shape_projection::{
    borrowed_string_content_shape, shape_is_concrete_native, shape_is_erased_object_boundary,
    shape_is_owned, shape_is_owned_non_erased, storage_shape, value_shape,
};
use crate::signature::{
    LocalPlan, ReceiverPlan, SignatureConstraint, SignaturePlan, SolvedSlotShape,
    apply_signature_constraints,
};
use crate::slot_render::{rust_type_for_java_type_with_facts, rust_type_for_slot_with_facts};
use crate::slot_shape::{DispatchShape, ViewShape};
use crate::slot_shape::{OwnershipShape, RustSlotShape};
use crate::statement_render::{
    assignment_stmt, break_stmt, continue_stmt, expr_stmt, if_stmt,
    local_let_stmt_with_facts_and_type_annotation, raw_stmt, render_block_stmt,
    render_enhanced_for, render_enhanced_for_iterator, while_stmt,
};
use crate::stdlib::{
    external_api_facade_support_items_with_local_type_program,
    external_static_string_constant_support_items_with_context_programs_and_declared_owners,
    java_class_support_item, java_platform_facade_support_item, java_string_format_support_item,
    java_string_split_support_item, program_uses_java_class_tokens,
    program_uses_java_platform_facades, program_uses_java_string_format,
    program_uses_java_string_runtime_helpers, support_item_declared_type_names,
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
    usage::{BindingUse, BindingUseFacts, UsageConstructorKey, UsageFacts, UsageMethodKey},
};

#[derive(Debug, Clone)]
pub struct LoweredProgram {
    pub file: RustFile,
    pub diagnostics: DiagnosticBag,
}

#[derive(Debug, Clone, Copy)]
struct LowerCtx<'a> {
    program: &'a Program,
    facts: &'a FactSet,
    external_index: &'a ExternalFactLookupIndex<'a>,
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
    source_summary_type_program: &'a Program,
    context_sources_are_emitted: bool,
    catch_future_use_names: Option<&'a std::collections::HashSet<String>>,
    later_read_names: Option<&'a std::collections::HashSet<String>>,
    current_stmt_read_counts: Option<&'a std::collections::HashMap<String, usize>>,
    exclusive_terminal_branch_read_names: Option<&'a std::collections::HashSet<String>>,
    preinitialized_try_capture_names: Option<&'a std::collections::HashSet<String>>,
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
        &Program::default(),
        false,
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
    source_summary_type_program: &Program,
    context_sources_are_emitted: bool,
) -> LoweredProgram {
    let external_index = facts.external_lookup_index();
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
    let exceptions = plan_program_exceptions(support_local_type_program, facts);
    let ctx = LowerCtx {
        program,
        facts,
        external_index: &external_index,
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
        source_summary_type_program,
        context_sources_are_emitted,
        catch_future_use_names: None,
        later_read_names: None,
        current_stmt_read_counts: None,
        exclusive_terminal_branch_read_names: None,
        preinitialized_try_capture_names: None,
    };
    let mut file = RustFile::default();
    if exceptions.support.emit_support_item {
        file.items
            .push(java_exception_support_item_with_facts(ctx.facts));
    }
    if program_uses_java_class_tokens(program) {
        file.items.push(java_class_support_item());
    }
    let uses_broad_ioutils = program_uses_broad_ioutils_facade(program);
    if program_uses_java_platform_facades(program) || uses_broad_ioutils {
        file.items.push(java_platform_facade_support_item());
    }
    if uses_broad_ioutils {
        file.items.push(broad_ioutils_support_item());
    }
    if program_uses_java_string_runtime_helpers(program) {
        file.items.push(java_string_split_support_item());
    }
    if program_uses_java_string_format(program) {
        file.items.push(java_string_format_support_item());
    }
    let facade_support_items = external_api_facade_support_items_with_local_type_program(
        program,
        facts,
        support_local_type_program,
        source_summary_type_program,
    );
    let declared_support_owners = support_item_declared_type_names(&facade_support_items);
    file.items.extend(facade_support_items);
    file.items.extend(
        external_static_string_constant_support_items_with_context_programs_and_declared_owners(
            program,
            facts,
            support_local_type_program,
            source_summary_type_program,
            &declared_support_owners,
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
            for (alias, type_name) in
                crate::javalibs::wildcard_import_static_type_aliases(&import.path)
            {
                env.insert_static_type_alias((*alias).to_string(), (*type_name).to_string());
                env.insert_static_type_alias(
                    crate::parser::to_snake_case(alias),
                    (*type_name).to_string(),
                );
            }
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
                let shape = storage_shape(field.ty.clone());
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

fn seed_external_inherited_instance_fields(
    env: &mut ShapeEnv,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) {
    for inherited in
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
    {
        let shape = signatures.required_field_shape(&ty.name, &inherited.rust_name);
        env.insert_type_field_shape(&ty.name, inherited.rust_name.clone(), shape.clone());
        env.insert_field_shape(inherited.rust_name, shape);
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
        ctx.support_local_type_program,
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
    if type_has_implicit_default_constructor(ty, ctx.support_local_type_program) {
        functions.push(lower_implicit_default_constructor(program, ty, ctx));
    }
    file.items.push(impl_item_for_type(ty, functions));
    for item in trait_impl_emissions_for_type_with_facts_and_exceptions(
        ctx.representations,
        ctx.support_local_type_program,
        ty,
        ctx.signatures,
        ctx.facts,
        ctx.exceptions,
    ) {
        file.items.push(raw_item(item));
    }
}

fn type_has_implicit_default_constructor(ty: &TypeDecl, support_program: &Program) -> bool {
    matches!(ty.kind, TypeKind::Class | TypeKind::Record)
        && !ty
            .members
            .iter()
            .any(|member| matches!(member, Member::Constructor(_)))
        && program_uses_zero_arg_constructor_for_type(support_program, &ty.name)
}

fn program_uses_zero_arg_constructor_for_type(program: &Program, type_name: &str) -> bool {
    program
        .types
        .iter()
        .any(|ty| type_uses_zero_arg_constructor_for_type(ty, type_name))
}

fn type_uses_zero_arg_constructor_for_type(ty: &TypeDecl, type_name: &str) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name)),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|body| block_uses_zero_arg_constructor_for_type(body, type_name)),
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(|body| block_uses_zero_arg_constructor_for_type(body, type_name)),
        Member::Type(nested) => type_uses_zero_arg_constructor_for_type(nested, type_name),
        Jm::Unsupported { .. } => false,
    })
}

fn block_uses_zero_arg_constructor_for_type(block: &Block, type_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_uses_zero_arg_constructor_for_type(stmt, type_name))
}

fn stmt_uses_zero_arg_constructor_for_type(stmt: &Stmt, type_name: &str) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => initializer
            .as_ref()
            .is_some_and(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name)),
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            expr_uses_zero_arg_constructor_for_type(expr, type_name)
        }
        Stmt::Assign { target, value } => {
            expr_uses_zero_arg_constructor_for_type(target, type_name)
                || expr_uses_zero_arg_constructor_for_type(value, type_name)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_zero_arg_constructor_for_type(condition, type_name)
                || block_uses_zero_arg_constructor_for_type(then_block, type_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_uses_zero_arg_constructor_for_type(block, type_name))
        }
        Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
            expr_uses_zero_arg_constructor_for_type(condition, type_name)
                || block_uses_zero_arg_constructor_for_type(body, type_name)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_uses_zero_arg_constructor_for_type(stmt, type_name))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name))
                || update
                    .iter()
                    .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name))
                || block_uses_zero_arg_constructor_for_type(body, type_name)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_zero_arg_constructor_for_type(iterable, type_name)
                || block_uses_zero_arg_constructor_for_type(body, type_name)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_zero_arg_constructor_for_type(selector, type_name)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(|stmt| stmt_uses_zero_arg_constructor_for_type(stmt, type_name))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|stmt| stmt_uses_zero_arg_constructor_for_type(stmt, type_name))
                || block_uses_zero_arg_constructor_for_type(body, type_name)
                || catches
                    .iter()
                    .any(|catch| block_uses_zero_arg_constructor_for_type(&catch.body, type_name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_uses_zero_arg_constructor_for_type(block, type_name))
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name)),
        Stmt::LocalType(ty) => type_uses_zero_arg_constructor_for_type(ty, type_name),
        Stmt::Block(block) => block_uses_zero_arg_constructor_for_type(block, type_name),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_zero_arg_constructor_for_type(expr: &Expr, type_name: &str) -> bool {
    match expr {
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => {
            body.is_none()
                && args.is_empty()
                && constructor_expr_targets_type(ty, resolved.as_ref(), type_name)
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr_uses_zero_arg_constructor_for_type(receiver, type_name)
        }
        Expr::ArrayAccess(array, index) => {
            expr_uses_zero_arg_constructor_for_type(array, type_name)
                || expr_uses_zero_arg_constructor_for_type(index, type_name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name))
                || initializer
                    .iter()
                    .flatten()
                    .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name))
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name)),
        Expr::Binary { left, right, .. } => {
            expr_uses_zero_arg_constructor_for_type(left, type_name)
                || expr_uses_zero_arg_constructor_for_type(right, type_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_zero_arg_constructor_for_type(condition, type_name)
                || expr_uses_zero_arg_constructor_for_type(then_expr, type_name)
                || expr_uses_zero_arg_constructor_for_type(else_expr, type_name)
        }
        Expr::Call { receiver, args, .. } => {
            receiver.as_ref().is_some_and(|receiver| {
                expr_uses_zero_arg_constructor_for_type(receiver, type_name)
            }) || args
                .iter()
                .any(|expr| expr_uses_zero_arg_constructor_for_type(expr, type_name))
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_uses_zero_arg_constructor_for_type(expr, type_name)
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => expr_uses_zero_arg_constructor_for_type(expr, type_name),
            LambdaBody::Block(block) => block_uses_zero_arg_constructor_for_type(block, type_name),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => false,
    }
}

fn constructor_expr_targets_type(
    ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    type_name: &str,
) -> bool {
    resolved.is_some_and(|resolved| {
        resolved.params.is_empty()
            && resolved.rust_name == "new"
            && crate::stdlib::canonical_external_receiver_type_key(&resolved.declaring_type)
                == crate::stdlib::canonical_external_receiver_type_key(type_name)
    }) || java_type_matches_name(ty, type_name)
}

fn java_type_matches_name(ty: &JavaType, type_name: &str) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::canonical_external_receiver_type_key(name)
                == crate::stdlib::canonical_external_receiver_type_key(type_name)
        }
        _ => false,
    }
}

fn lower_implicit_default_constructor(
    program: &Program,
    ty: &TypeDecl,
    ctx: LowerCtx,
) -> RustFunction {
    let constructor_ctx = LowerCtx {
        current_type: Some(&ty.name),
        current_method: None,
        current_constructor: Some("new"),
        current_receiver: None,
        current_local_shapes: &[],
        ..ctx
    };
    let mut env = shape_env_for_type(ty, ctx.signatures);
    seed_imported_static_type_aliases(&mut env, program);
    seed_support_type_fields(&mut env, ctx.support_local_type_program);
    seed_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.support_local_type_program);
    seed_external_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.facts);
    let fields = plan_constructor_field_values_with_inherited_context(
        ty,
        None,
        ctx.signatures,
        ctx.support_local_type_program,
        ctx.facts,
    )
    .into_iter()
    .map(|plan| lower_constructor_field_value_plan(plan, &env, constructor_ctx))
    .collect();
    RustFunction {
        name: "new".to_string(),
        params: Vec::new(),
        return_type: Some(RustType::Named(
            crate::slot_render::rust_name_for_java_named_type(&ty.name),
        )),
        body: constructor_return_body(ty, fields),
        is_pub: true,
        receiver: None,
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
    seed_external_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.facts);
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

    let fields = plan_constructor_field_values_with_inherited_context(
        ty,
        ctor.body.as_ref(),
        ctx.signatures,
        ctx.support_local_type_program,
        ctx.facts,
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

    let (body, returns_result) = match plan {
        ConstructorDelegationPlan::SameClassCall { args, resolved } => (
            delegating_constructor_return_body(
                &resolved.rust_name,
                lower_args_with_plan(
                    args,
                    env,
                    plan_constructor_arguments(Some(resolved), args.len(), ctx.signatures),
                    ctx,
                ),
            ),
            false,
        ),
        ConstructorDelegationPlan::SameClassCallWithTrailing {
            args,
            resolved,
            trailing,
        } => {
            let returns_result = delegated_constructor_trailing_can_throw(&trailing, env, ctx);
            (
                delegated_constructor_with_trailing_body(
                    ty,
                    resolved,
                    args,
                    &trailing,
                    env,
                    ctx,
                    returns_result,
                ),
                returns_result,
            )
        }
        ConstructorDelegationPlan::SuperNoArgWithTrailing { trailing } => {
            let returns_result = delegated_constructor_trailing_can_throw(&trailing, env, ctx);
            let fields = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            )
            .into_iter()
            .map(|plan| lower_constructor_field_value_plan(plan, env, ctx))
            .collect();
            (
                super_no_arg_constructor_with_trailing_body(
                    ty,
                    fields,
                    &trailing,
                    env,
                    ctx,
                    returns_result,
                ),
                returns_result,
            )
        }
        ConstructorDelegationPlan::ModeledExternalSuperclassWithTrailing { trailing, .. } => {
            let returns_result = delegated_constructor_trailing_can_throw(&trailing, env, ctx);
            let fields = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            )
            .into_iter()
            .map(|plan| lower_constructor_field_value_plan(plan, env, ctx))
            .collect();
            let trailing_instance_calls = trailing
                .iter()
                .filter(|stmt| {
                    matches!(stmt, DelegatedConstructorTrailingStmt::InstanceCall { .. })
                })
                .cloned()
                .collect::<Vec<_>>();
            if trailing_instance_calls.is_empty() {
                (constructor_return_body(ty, fields), false)
            } else {
                (
                    super_no_arg_constructor_with_trailing_body(
                        ty,
                        fields,
                        &trailing_instance_calls,
                        env,
                        ctx,
                        returns_result,
                    ),
                    returns_result,
                )
            }
        }
        ConstructorDelegationPlan::ExternalExceptionSuperclass { .. } => {
            let fields = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            )
            .into_iter()
            .map(|plan| lower_constructor_field_value_plan(plan, env, ctx))
            .collect();
            (constructor_return_body(ty, fields), false)
        }
        ConstructorDelegationPlan::Unsupported(kind) => {
            (vec![unsupported_constructor_stmt(kind)], false)
        }
    };

    let mut function =
        constructor_function_shell_with_facts(ty, ctor, ctx.signatures, body, ctx.facts);
    if returns_result {
        function.return_type = Some(exception_result_type(RustType::Named(ty.name.clone())));
    }
    Some(function)
}

fn delegated_constructor_with_trailing_body(
    ty: &TypeDecl,
    resolved: &ResolvedConstructor,
    args: &[Expr],
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
    returns_result: bool,
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
            ty,
            stmt,
            env,
            ctx,
            returns_result,
        ));
    }
    let returned = RustExpr::Var("__java_self".to_string());
    body.push(RustStmt::Return(Some(if returns_result {
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("Ok".to_string())),
            args: vec![returned],
        }
    } else {
        returned
    })));
    body
}

fn super_no_arg_constructor_with_trailing_body(
    ty: &TypeDecl,
    fields: Vec<(String, RustExpr)>,
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
    returns_result: bool,
) -> Vec<RustStmt> {
    let mut body = vec![RustStmt::Let {
        mutable: true,
        name: "__java_self".to_string(),
        ty: None,
        value: Some(RustExpr::StructLit {
            name: crate::slot_render::rust_name_for_java_named_type(&ty.name),
            fields,
        }),
    }];
    for stmt in trailing {
        body.push(lower_delegated_constructor_trailing_stmt(
            ty,
            stmt,
            env,
            ctx,
            returns_result,
        ));
    }
    let returned = RustExpr::Var("__java_self".to_string());
    body.push(RustStmt::Return(Some(if returns_result {
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("Ok".to_string())),
            args: vec![returned],
        }
    } else {
        returned
    })));
    body
}

fn delegated_constructor_trailing_can_throw(
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    trailing.iter().any(|stmt| match stmt {
        DelegatedConstructorTrailingStmt::InstanceCall { resolved, args } => matches!(
            plan_call_exception_propagation_with_lower_context(
                Some(&Expr::This),
                &resolved.rust_name,
                args,
                Some(resolved),
                ctx,
                env,
                ctx.current_type,
            ),
            CallExceptionPropagationPlan::PropagateResult
        ),
        DelegatedConstructorTrailingStmt::FieldAssignment { .. } => false,
    })
}

fn lower_delegated_constructor_trailing_stmt(
    ty: &TypeDecl,
    stmt: &DelegatedConstructorTrailingStmt<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    returns_result: bool,
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
            let receiver = Expr::This;
            let mut lowered = RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("__java_self".to_string())),
                method: resolved.rust_name.clone(),
                args: lower_args_with_plan(
                    args,
                    env,
                    plan_call_arguments(Some(resolved), args.len(), ctx.signatures),
                    ctx,
                ),
            };
            if returns_result
                && matches!(
                    plan_call_exception_propagation_with_lower_context(
                        Some(&receiver),
                        &resolved.rust_name,
                        args,
                        Some(resolved),
                        ctx,
                        env,
                        ctx.current_type,
                    ),
                    CallExceptionPropagationPlan::PropagateResult
                )
            {
                lowered = propagate_result_expr(lowered);
            }
            expr_stmt(lowered)
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
    seed_external_inherited_instance_fields(&mut env, ty, method_ctx.signatures, method_ctx.facts);
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
    lower_block_with_refinements(
        block,
        env,
        wrap_returns_in_ok,
        ctx,
        &ExprRefinements::default(),
    )
}

fn lower_block_with_refinements(
    block: &Block,
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Vec<RustStmt> {
    let preinitialized_try_capture_names =
        preinitialized_try_capture_names_for_block(&block.statements);
    let block_ctx = LowerCtx {
        preinitialized_try_capture_names: Some(&preinitialized_try_capture_names),
        ..ctx
    };
    let mut body = Vec::new();
    let mut block_refinements = ExprRefinements {
        non_null_locals: refinements.non_null_locals.clone(),
        exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
    };
    for (index, stmt) in block.statements.iter().enumerate() {
        let mut later_read_names = statement_suffix_read_names(&block.statements[index + 1..]);
        if let Some(inherited_later_read_names) = block_ctx.later_read_names {
            later_read_names.extend(inherited_later_read_names.iter().cloned());
        }
        let current_stmt_read_counts = statement_read_counts(stmt);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            current_stmt_read_counts: Some(&current_stmt_read_counts),
            ..block_ctx
        };
        lower_stmt(
            stmt,
            env,
            wrap_returns_in_ok,
            stmt_ctx,
            &block_refinements,
            &mut body,
        );
        if !matches!(stmt, Stmt::VarDecl { .. }) {
            learn_stmt_shapes(
                stmt,
                env,
                stmt_ctx.current_local_shapes,
                stmt_ctx.signatures,
            );
        }
        update_expr_refinements_after_stmt(stmt, stmt_ctx, &mut block_refinements);
    }
    body
}

fn lower_lambda_block_with_result_carrier(
    block: &Block,
    env: &mut ShapeEnv,
    ctx: LowerCtx,
    wrap_returns_in_ok: bool,
) -> Vec<RustStmt> {
    let preinitialized_try_capture_names =
        preinitialized_try_capture_names_for_block(&block.statements);
    let block_ctx = LowerCtx {
        preinitialized_try_capture_names: Some(&preinitialized_try_capture_names),
        ..ctx
    };
    let mut body = Vec::new();
    for (index, stmt) in block.statements.iter().enumerate() {
        let later_read_names = statement_suffix_read_names(&block.statements[index + 1..]);
        let current_stmt_read_counts = statement_read_counts(stmt);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            current_stmt_read_counts: Some(&current_stmt_read_counts),
            ..block_ctx
        };
        lower_stmt(
            stmt,
            env,
            wrap_returns_in_ok,
            stmt_ctx,
            &ExprRefinements::default(),
            &mut body,
        );
        if !matches!(stmt, Stmt::VarDecl { .. }) {
            learn_stmt_shapes(
                stmt,
                env,
                stmt_ctx.current_local_shapes,
                stmt_ctx.signatures,
            );
        }
    }
    body
}

fn statement_suffix_read_names(statements: &[Stmt]) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_stmt_read_names(statements, &mut names);
    names
}

fn statement_read_counts(stmt: &Stmt) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    collect_stmt_read_counts(stmt, &mut counts);
    counts
}

fn lower_stmt(
    stmt: &Stmt,
    env: &mut ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
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
            let mut local_plan = if let Some(local_plan) =
                env.next_local_plan_for_declared_type(ctx.current_local_shapes, name, ty)
            {
                local_plan.clone()
            } else {
                conservative_lambda_local_plan(name, ty, initializer.as_ref(), env, ctx)
            };
            local_plan =
                refine_local_plan_from_initializer(local_plan, initializer.as_ref(), env, ctx);
            local_plan =
                own_uninitialized_optional_borrowed_local(local_plan, name, initializer, ctx);
            if let Some(shape) =
                exact_erased_object_cast_initializer_local_shape(initializer.as_ref(), refinements)
            {
                local_plan.shape = shape;
            }
            let mut lowered_initializer = initializer
                .as_ref()
                .map(|expr| {
                    let initializer_slot = if raw_cast_initializer_erases_declared_local_type(
                        expr,
                        ty,
                        &local_plan.shape,
                    ) {
                        assignment_value_plan_for_shape(&declared_generic_local_slot_shape(
                            ty,
                            &local_plan.shape,
                        ))
                        .slot
                    } else {
                        assignment_value_plan_for_shape(&local_plan.shape).slot
                    };
                    lower_expr_for_slot_with_refinements(
                        expr,
                        env,
                        initializer_slot,
                        ctx,
                        refinements,
                    )
                })
                .or_else(|| {
                    ctx.preinitialized_try_capture_names.and_then(|names| {
                        names
                            .contains(name)
                            .then(|| RustExpr::Raw("Default::default()".to_string()))
                    })
                });
            if let Some(initializer) = lowered_initializer.as_mut() {
                hoist_borrowed_string_temporaries_in_initializer(name, initializer, out);
            }
            out.push(local_let_stmt_with_facts_and_type_annotation(
                &local_plan,
                lowered_initializer,
                ctx.facts,
                local_decl_should_annotate_type(
                    &local_plan.java_ty,
                    initializer.as_ref(),
                    env,
                    ctx,
                ),
            ));
            let env_shape =
                local_env_shape_from_initializer(&local_plan.shape, initializer.as_ref(), env, ctx)
                    .unwrap_or_else(|| local_plan.shape.clone());
            env.insert_local_shape(name.clone(), env_shape);
        }
        Stmt::Assign { target, value } => {
            let shape_ctx = ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            };
            if let Some(stmt) = lower_array2_row_replacement_stmt(target, value, env, ctx) {
                out.push(stmt);
                return;
            }
            let assignment_slot =
                assignment_slot_for_target_with_current_type(target, env, ctx, shape_ctx);
            if assignment_statement_plan_for_target_with_current_type(
                target,
                value,
                env,
                shape_ctx,
                &assignment_slot,
            ) == crate::conversion_plan::AssignmentStatementPlan::ClearOptionalTarget
            {
                out.push(expr_stmt(RustExpr::MethodCall {
                    receiver: Box::new(lower_expr(target, env, ctx)),
                    method: "take".to_string(),
                    args: Vec::new(),
                }));
            } else {
                let lowered_value = lower_expr_for_slot_with_refinements(
                    value,
                    env,
                    assignment_slot.clone(),
                    ctx,
                    refinements,
                );
                out.push(assignment_stmt(
                    lower_expr(target, env, ctx),
                    wrap_optional_assignment_value_if_needed(
                        value,
                        env,
                        assignment_slot,
                        lowered_value,
                        ctx,
                    ),
                ));
                refine_assigned_local_shape(target, value, env, ctx);
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
                out.push(expr_stmt(lower_expr_with_refinements(
                    expr,
                    env,
                    ctx,
                    refinements,
                )));
            }
        }
        Stmt::Return(expr) => out.push(lower_return_stmt_with_refinements(
            expr.as_ref(),
            env,
            wrap_returns_in_ok,
            ctx,
            refinements,
        )),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            let (then_refinements, else_refinements) = branch_refinements(condition, refinements);
            let terminal_continuation_reads = HashSet::new();
            let branches_terminate = else_block.as_ref().is_some_and(|else_block| {
                block_returns_or_throws(then_block) && block_returns_or_throws(else_block)
            });
            let then_exclusive_reads = branches_terminate
                .then(|| {
                    else_block
                        .as_ref()
                        .map(|block| statement_suffix_read_names(&block.statements))
                })
                .flatten();
            let else_exclusive_reads =
                branches_terminate.then(|| statement_suffix_read_names(&then_block.statements));
            let then_ctx = LowerCtx {
                later_read_names: if block_returns_or_throws(then_block) {
                    Some(&terminal_continuation_reads)
                } else {
                    ctx.later_read_names
                },
                exclusive_terminal_branch_read_names: then_exclusive_reads.as_ref(),
                ..ctx
            };
            let else_block_returns = else_block.as_ref().is_some_and(block_returns_or_throws);
            let else_ctx = LowerCtx {
                later_read_names: if else_block_returns {
                    Some(&terminal_continuation_reads)
                } else {
                    ctx.later_read_names
                },
                exclusive_terminal_branch_read_names: else_exclusive_reads.as_ref(),
                ..ctx
            };
            out.push(if_stmt(
                lower_expr_for_slot(condition, env, condition_slot(), ctx),
                lower_block_with_refinements(
                    then_block,
                    &mut env.clone(),
                    wrap_returns_in_ok,
                    then_ctx,
                    &then_refinements,
                ),
                else_block
                    .as_ref()
                    .map(|block| {
                        lower_block_with_refinements(
                            block,
                            &mut env.clone(),
                            wrap_returns_in_ok,
                            else_ctx,
                            &else_refinements,
                        )
                    })
                    .unwrap_or_default(),
            ));
            refine_local_shapes_from_branch_assignments(then_block, else_block.as_ref(), env, ctx);
        }
        Stmt::While { condition, body } => match plan_while_stmt(condition, body) {
            WhileStmtPlan::AssignmentInCondition {
                local_name,
                value,
                null_test,
                body,
            } => {
                let local = Expr::Var(local_name.to_string());
                let assigned_value = lower_nullable_assignment_condition_value(
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
                    refinements,
                );
                let mut body_env = loop_assignment_condition_body_env(env, local_name, null_test);
                out.push(assignment_condition_loop(
                    local_name,
                    assigned_value,
                    null_test,
                    lower_block(body, &mut body_env, wrap_returns_in_ok, ctx),
                ));
            }
            WhileStmtPlan::Plain { condition, body } => out.push(while_stmt(
                lower_expr_for_slot(condition, env, condition_slot(), ctx),
                lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
            )),
        },
        Stmt::DoWhile { body, condition } => {
            let mut loop_body = lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx);
            let lowered_condition = lower_expr_for_slot(condition, env, condition_slot(), ctx);
            loop_body.push(if_stmt(
                RustExpr::Raw(format!("!({})", emit::emit_expr(&lowered_condition))),
                vec![break_stmt(None)],
                Vec::new(),
            ));
            out.push(RustStmt::Loop {
                label: None,
                body: loop_body,
            });
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            for init_stmt in init {
                lower_stmt(init_stmt, env, wrap_returns_in_ok, ctx, refinements, out);
                if !matches!(init_stmt, Stmt::VarDecl { .. }) {
                    learn_stmt_shapes(init_stmt, env, ctx.current_local_shapes, ctx.signatures);
                }
            }
            let mut loop_body =
                lower_block_for_body(body, update, &mut env.clone(), wrap_returns_in_ok, ctx);
            for expr in update {
                loop_body.push(expr_stmt(lower_expr_with_refinements(
                    expr,
                    env,
                    ctx,
                    refinements,
                )));
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
        Stmt::Throw(expr) => out.push(throw_stmt_with_result_policy(
            lower_throw_expr_with_refinements(expr, env, ctx, refinements),
            wrap_returns_in_ok,
        )),
        Stmt::Break(label) => out.push(break_stmt(label.clone())),
        Stmt::Continue(label) => out.push(continue_stmt(label.clone())),
        Stmt::Block(block) => out.push(render_block_stmt(&lower_block_with_refinements(
            block,
            &mut env.clone(),
            wrap_returns_in_ok,
            ctx,
            refinements,
        ))),
        Stmt::LocalType(_) | Stmt::ConstructorInvocation { .. } | Stmt::Unsupported(_) => {
            unreachable!("unsupported statement kinds should be handled before statement lowering")
        }
    }
}

fn own_uninitialized_optional_borrowed_local(
    mut local_plan: LocalPlan,
    name: &str,
    initializer: &Option<Expr>,
    ctx: LowerCtx,
) -> LocalPlan {
    if initializer.is_none()
        && local_plan.shape.optional_element_type().is_some()
        && local_plan.shape.ownership == OwnershipShape::SharedBorrow
        && !current_callable_binding_is_returned(name, ctx)
    {
        local_plan.shape.ownership = OwnershipShape::Owned;
    }
    local_plan
}

fn assignment_slot_for_target_with_current_type(
    target: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    shape_ctx: ExprShapeContext<'_>,
) -> Slot {
    current_instance_field_shape_with_env(target, env, ctx)
        .map(Slot::Assignment)
        .unwrap_or_else(|| assignment_value_plan_for_target(target, env, shape_ctx).slot)
}

fn assignment_statement_plan_for_target_with_current_type(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    shape_ctx: ExprShapeContext<'_>,
    assignment_slot: &Slot,
) -> crate::conversion_plan::AssignmentStatementPlan {
    if matches!(value, Expr::Literal(Literal::Null))
        && slot_target_shape(assignment_slot)
            .is_some_and(|shape| shape.optional_element_type().is_some())
    {
        return crate::conversion_plan::AssignmentStatementPlan::ClearOptionalTarget;
    }
    assignment_statement_plan_for_target(target, value, env, shape_ctx)
}

fn wrap_optional_assignment_value_if_needed(
    value: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    lowered_value: RustExpr,
    ctx: LowerCtx,
) -> RustExpr {
    let Some(target) = slot_target_shape(&slot) else {
        return lowered_value;
    };
    if target.optional_element_type().is_none() || matches!(value, Expr::Literal(Literal::Null)) {
        return lowered_value;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let inferred_actual = actual_shape_for_slot_expr(value, env, shape_ctx);
    let planned_actual = planned_method_call_return_shape(value, env, ctx)
        .or_else(|| external_call_expr_result_shape(value, env, ctx));
    let actual = planned_actual.unwrap_or(inferred_actual);
    if actual.optional_element_type().is_some() {
        return lowered_value;
    }
    let rendered = emit::emit_expr(&lowered_value);
    if rendered == "None" {
        return lowered_value;
    }
    if let Some(inner) = rendered
        .strip_prefix("Some(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        let mut target_element = target.clone();
        target_element.java_ty = actual.java_ty.clone();
        let element_plan = plan_slot_conversion(actual, argument_slot(target_element), ctx.facts);
        let converted = apply_slot_expr_conversion(RustExpr::Raw(inner.to_string()), &element_plan);
        return RustExpr::Raw(format!("Some({})", emit::emit_expr(&converted)));
    }
    let plan = plan_slot_conversion(actual, slot, ctx.facts);
    apply_slot_expr_conversion(lowered_value, &plan)
}

fn current_instance_field_shape(expr: &Expr, ctx: LowerCtx) -> Option<RustSlotShape> {
    let field = current_instance_field_name(expr)?;
    let type_name = ctx.current_type?;
    signature_field_shape_for_type_aliases(ctx.signatures, type_name, field).cloned()
}

fn current_instance_field_shape_with_env(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    current_instance_field_shape(expr, ctx).or_else(|| {
        let Expr::Var(name) = expr else {
            return None;
        };
        if env.local_shape(name).is_some()
            || (env.is_static_field(name) && !current_env_name_is_instance_field(env, name))
        {
            return None;
        }
        let type_name = ctx.current_type?;
        signature_field_shape_for_type_aliases(ctx.signatures, type_name, name).cloned()
    })
}

fn current_env_name_is_instance_field(env: &ShapeEnv, name: &str) -> bool {
    env.is_instance_field(name) || env.is_instance_field(&crate::parser::to_snake_case(name))
}

fn signature_field_shape_for_type_aliases<'a>(
    signatures: &'a SignaturePlan,
    type_name: &str,
    field_name: &str,
) -> Option<&'a RustSlotShape> {
    [type_name, type_name.rsplit('.').next().unwrap_or(type_name)]
        .into_iter()
        .find_map(|owner| {
            signatures.field_shape(owner, field_name).or_else(|| {
                signatures.field_shape(owner, &crate::parser::to_snake_case(field_name))
            })
        })
}

fn current_instance_field_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => name.strip_prefix("this."),
        Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => Some(field),
        _ => None,
    }
}

fn is_this_receiver_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::This) || matches!(expr, Expr::Var(name) if name == "this")
}

fn loop_assignment_condition_body_env(
    env: &ShapeEnv,
    local_name: &str,
    null_test: crate::rewrite::NullTestKind,
) -> ShapeEnv {
    let mut body_env = env.clone();
    if null_test == crate::rewrite::NullTestKind::NotNull
        && let Some(element_ty) = env
            .local_shape(local_name)
            .and_then(RustSlotShape::optional_element_type)
    {
        let mut shape = RustSlotShape::for_storage(element_ty);
        shape.ownership = OwnershipShape::SharedBorrow;
        body_env.insert_local_shape(local_name.to_string(), shape);
    }
    body_env
}

fn lower_nullable_assignment_condition_value(
    value: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if slot_expects_optional(&slot) {
        let mut lowered = lower_expr_with_refinements(value, env, ctx, refinements);
        if slot_expr_needs_result_propagation(value, env, ctx) {
            lowered = propagate_result_expr(lowered);
        }
        lowered
    } else {
        lower_expr_for_slot_with_refinements(value, env, slot, ctx, refinements)
    }
}

fn update_expr_refinements_after_stmt(
    stmt: &Stmt,
    ctx: LowerCtx,
    refinements: &mut ExprRefinements,
) {
    let mut assigned = std::collections::HashSet::new();
    collect_stmt_assigned_names(std::slice::from_ref(stmt), &mut assigned);
    for name in assigned {
        refinements.non_null_locals.remove(&name);
        refinements.exact_erased_object_locals.remove(&name);
    }
    if let Some((local_name, exact_ty)) = exact_type_refinement_from_early_return_guard(stmt, ctx) {
        refinements
            .exact_erased_object_locals
            .insert(local_name, exact_ty);
    }
    if let Stmt::While { condition, body } = stmt {
        let plan = plan_while_stmt(condition, body);
        if let crate::loop_plan::PostLoopRefinementPlan::LocalNonNull { local_name } =
            crate::loop_plan::post_loop_refinement_for_while(&plan)
        {
            refinements.non_null_locals.insert(local_name.to_string());
        }
    }
}

fn exact_type_refinement_from_early_return_guard(
    stmt: &Stmt,
    ctx: LowerCtx,
) -> Option<(String, JavaType)> {
    let Stmt::If {
        condition,
        then_block,
        else_block,
    } = stmt
    else {
        return None;
    };
    if else_block.is_some() || !block_returns_boolean_literal(then_block, false) {
        return None;
    }
    exact_type_refinement_from_rejected_class_condition(condition, ctx)
}

fn block_returns_boolean_literal(block: &Block, value: bool) -> bool {
    matches!(
        block.statements.as_slice(),
        [Stmt::Return(Some(Expr::Literal(Literal::Bool(actual))))] if *actual == value
    )
}

fn exact_type_refinement_from_rejected_class_condition(
    condition: &Expr,
    ctx: LowerCtx,
) -> Option<(String, JavaType)> {
    let Expr::Binary { op, left, right } = condition else {
        return None;
    };
    if *op != BinaryOp::Ne {
        return None;
    }
    exact_type_refinement_from_get_class_pair(left, right, ctx)
        .or_else(|| exact_type_refinement_from_get_class_pair(right, left, ctx))
}

fn exact_type_refinement_from_get_class_pair(
    current_side: &Expr,
    object_side: &Expr,
    ctx: LowerCtx,
) -> Option<(String, JavaType)> {
    let current_type = ctx.current_type?;
    if !expr_is_current_get_class_call(current_side, ctx) {
        return None;
    }
    let receiver = get_class_call_receiver(object_side)?;
    let Expr::Var(local_name) = receiver else {
        return None;
    };
    let Expr::Call { .. } = object_side else {
        return None;
    };
    if !expr_is_get_class_call(object_side) {
        return None;
    }
    Some((
        local_name.clone(),
        JavaType::Unknown(current_type.to_string()),
    ))
}

fn hoist_borrowed_string_temporaries_in_initializer(
    local_name: &str,
    expr: &mut RustExpr,
    out: &mut Vec<RustStmt>,
) {
    let mut counter = 0usize;
    hoist_borrowed_string_temporaries_in_expr(local_name, expr, out, &mut counter);
}

fn hoist_borrowed_string_temporaries_in_expr(
    local_name: &str,
    expr: &mut RustExpr,
    out: &mut Vec<RustStmt>,
    counter: &mut usize,
) {
    if let RustExpr::Raw(raw) = expr
        && let Some((replacement, value)) =
            borrowed_owned_string_literal_temporary(raw, local_name, *counter)
    {
        *counter += 1;
        out.push(RustStmt::Let {
            mutable: false,
            name: replacement.name.clone(),
            ty: None,
            value: Some(RustExpr::Raw(value)),
        });
        *raw = replacement.expr;
        return;
    }
    match expr {
        RustExpr::Field(receiver, _)
        | RustExpr::Index(receiver, _)
        | RustExpr::Unary { expr: receiver, .. }
        | RustExpr::Cast { expr: receiver, .. } => {
            hoist_borrowed_string_temporaries_in_expr(local_name, receiver, out, counter);
        }
        RustExpr::Binary { left, right, .. } => {
            hoist_borrowed_string_temporaries_in_expr(local_name, left, out, counter);
            hoist_borrowed_string_temporaries_in_expr(local_name, right, out, counter);
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            hoist_borrowed_string_temporaries_in_expr(local_name, condition, out, counter);
            hoist_borrowed_string_temporaries_in_expr(local_name, then_expr, out, counter);
            hoist_borrowed_string_temporaries_in_expr(local_name, else_expr, out, counter);
        }
        RustExpr::Call { function, args } => {
            hoist_borrowed_string_temporaries_in_expr(local_name, function, out, counter);
            for arg in args {
                hoist_borrowed_string_temporaries_in_expr(local_name, arg, out, counter);
            }
        }
        RustExpr::MethodCall { receiver, args, .. } => {
            hoist_borrowed_string_temporaries_in_expr(local_name, receiver, out, counter);
            for arg in args {
                hoist_borrowed_string_temporaries_in_expr(local_name, arg, out, counter);
            }
        }
        RustExpr::StructLit { fields, .. } => {
            for (_, value) in fields {
                hoist_borrowed_string_temporaries_in_expr(local_name, value, out, counter);
            }
        }
        RustExpr::Literal(_) | RustExpr::Var(_) | RustExpr::Raw(_) => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BorrowedStringTemporary {
    name: String,
    expr: String,
}

fn borrowed_owned_string_literal_temporary(
    raw: &str,
    local_name: &str,
    counter: usize,
) -> Option<(BorrowedStringTemporary, String)> {
    if let Some(inner) = raw
        .strip_prefix("Some(&")
        .and_then(|rest| rest.strip_suffix(')'))
        && string_literal_to_string_expr(inner)
    {
        let name = borrowed_string_temporary_name(local_name, counter);
        return Some((
            BorrowedStringTemporary {
                name: name.clone(),
                expr: format!("Some(&{name})"),
            },
            inner.to_string(),
        ));
    }
    if let Some(inner) = raw.strip_prefix('&')
        && string_literal_to_string_expr(inner)
    {
        let name = borrowed_string_temporary_name(local_name, counter);
        return Some((
            BorrowedStringTemporary {
                name: name.clone(),
                expr: format!("&{name}"),
            },
            inner.to_string(),
        ));
    }
    None
}

fn string_literal_to_string_expr(raw: &str) -> bool {
    raw.starts_with('"') && raw.ends_with(".to_string()")
}

fn borrowed_string_temporary_name(local_name: &str, counter: usize) -> String {
    format!(
        "_java_borrowed_{}_{}",
        crate::parser::to_snake_case(local_name),
        counter
    )
}

fn conservative_lambda_local_plan(
    name: &str,
    ty: &JavaType,
    initializer: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> LocalPlan {
    LocalPlan {
        name: name.to_string(),
        java_ty: ty.clone(),
        shape: fallback_local_shape_from_declared_type_and_initializer(ty, initializer, env, ctx),
        binding_mutable: true,
    }
}

fn refine_local_plan_from_initializer(
    mut local_plan: LocalPlan,
    initializer: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> LocalPlan {
    let Some(initializer) = initializer else {
        return local_plan;
    };
    if local_plan.shape.view == ViewShape::Slice
        && matches!(local_plan.shape.java_ty, JavaType::Array(_))
        && matches!(
            initializer,
            Expr::ArrayCreation { .. } | Expr::ArrayInitializer(_)
        )
    {
        local_plan.shape = RustSlotShape::for_storage(local_plan.shape.java_ty.clone());
        return local_plan;
    }
    if let Expr::Cast { ty, .. } = initializer
        && cast_target_erases_slot_generic_detail(ty, &local_plan.shape.java_ty)
    {
        return local_plan;
    }
    let initializer_shape =
        planned_method_call_return_shape(initializer, env, ctx).unwrap_or_else(|| {
            infer_expr_shape_with_context(
                initializer,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                    facts: Some(ctx.facts),
                },
            )
        });
    if local_borrowed_optional_should_keep_owned_temporary(
        &local_plan.shape,
        &initializer_shape,
        initializer,
    ) {
        local_plan.shape = initializer_shape;
        return local_plan;
    }
    if local_shape_can_inherit_optional_initializer_shape(&local_plan.shape, &initializer_shape) {
        local_plan.shape = initializer_shape;
        return local_plan;
    }
    if initializer_shape_has_more_generic_information_for_local(
        &initializer_shape,
        &local_plan.shape,
    ) {
        local_plan.shape = initializer_shape;
        return local_plan;
    }
    if !local_shape_should_inherit_initializer_shape(&local_plan.shape) {
        return local_plan;
    }
    if !initializer_shape_is_more_specific_for_local(&initializer_shape, &local_plan.shape) {
        return local_plan;
    }
    local_plan.shape = initializer_shape;
    local_plan
}

fn local_borrowed_optional_should_keep_owned_temporary(
    local_shape: &RustSlotShape,
    initializer_shape: &RustSlotShape,
    initializer: &Expr,
) -> bool {
    let Some(local_element_ty) = local_shape.optional_element_type() else {
        return false;
    };
    initializer_shape
        .optional_element_type()
        .is_some_and(|initializer_element_ty| initializer_element_ty == local_element_ty)
        && local_shape.ownership == OwnershipShape::SharedBorrow
        && initializer_shape.ownership == OwnershipShape::Owned
        && initializer_creates_temporary_value(initializer)
}

fn initializer_creates_temporary_value(initializer: &Expr) -> bool {
    matches!(
        initializer,
        Expr::Call { .. }
            | Expr::New { .. }
            | Expr::ArrayCreation { .. }
            | Expr::ArrayInitializer(_)
            | Expr::Ternary { .. }
            | Expr::Cast { .. }
    )
}

fn raw_cast_initializer_erases_declared_local_type(
    initializer: &Expr,
    declared_ty: &JavaType,
    local_shape: &RustSlotShape,
) -> bool {
    let Expr::Cast { ty, .. } = initializer else {
        return false;
    };
    cast_target_erases_slot_generic_detail(ty, declared_ty)
        && slot_has_generic_or_sequence_detail(declared_ty)
        && !slot_has_generic_or_sequence_detail(&local_shape.java_ty)
}

fn declared_generic_local_slot_shape(
    declared_ty: &JavaType,
    local_shape: &RustSlotShape,
) -> RustSlotShape {
    let mut shape = local_shape.clone();
    shape.java_ty = declared_ty.clone();
    shape.type_identity = crate::slot_shape::TypeIdentity::from_java_type(declared_ty.clone());
    shape
}

fn local_env_shape_from_initializer(
    local_shape: &RustSlotShape,
    initializer: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let initializer = initializer?;
    let initializer_shape = planned_method_call_return_shape(initializer, env, ctx)?;
    (java_types_have_same_erased_identity(&local_shape.java_ty, &initializer_shape.java_ty)
        && shape_has_more_generic_information(&initializer_shape, local_shape))
    .then_some(initializer_shape)
}

fn local_decl_should_annotate_type(
    declared_ty: &JavaType,
    initializer: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    !initializer
        .and_then(|initializer| planned_external_facade_call_java_type(initializer, env, ctx))
        .as_ref()
        .is_some_and(|java_ty| {
            java_types_have_same_erased_identity(declared_ty, java_ty)
                && java_type_is_rust_crate_mapped_external(java_ty, ctx.facts)
        })
}

fn planned_external_facade_call_java_type(
    initializer: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = initializer
    else {
        return None;
    };
    if resolved.is_some() {
        return None;
    }
    external_call_lowered_type_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    )
    .and_then(|lowered| lowered.java_ty().cloned())
}

fn java_type_is_rust_crate_mapped_external(java_ty: &JavaType, facts: &FactSet) -> bool {
    let java_ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
    let JavaType::Named { name, .. } = java_ty else {
        return false;
    };
    facts
        .external_library_pack_for_type(&name)
        .is_some_and(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
}

fn local_shape_should_inherit_initializer_shape(shape: &RustSlotShape) -> bool {
    shape_is_erased_object_boundary(shape)
}

fn local_shape_can_inherit_optional_initializer_shape(
    local: &RustSlotShape,
    initializer: &RustSlotShape,
) -> bool {
    local.optional_element_type().is_none()
        && initializer
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == local.java_ty)
}

fn initializer_shape_is_more_specific_for_local(
    initializer: &RustSlotShape,
    local: &RustSlotShape,
) -> bool {
    java_types_have_same_erased_identity(&initializer.java_ty, &local.java_ty)
        && shape_is_concrete_native(initializer)
}

fn initializer_shape_has_more_generic_information_for_local(
    initializer: &RustSlotShape,
    local: &RustSlotShape,
) -> bool {
    java_types_have_same_erased_identity(&initializer.java_ty, &local.java_ty)
        && shape_has_more_generic_information(initializer, local)
}

fn java_types_have_same_erased_identity(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (JavaType::Named { name: left, .. }, JavaType::Named { name: right, .. }) => left == right,
        _ => left == right,
    }
}

fn shape_has_more_generic_information(candidate: &RustSlotShape, current: &RustSlotShape) -> bool {
    match (&candidate.java_ty, &current.java_ty) {
        (
            JavaType::Named {
                args: candidate_args,
                ..
            },
            JavaType::Named {
                args: current_args, ..
            },
        ) => candidate_args.len() > current_args.len(),
        _ => false,
    }
}

fn fallback_local_shape_from_declared_type_and_initializer(
    ty: &JavaType,
    initializer: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustSlotShape {
    let declared = fallback_local_shape_from_declared_type(ty);
    if declared.optional_element_type().is_some() {
        return declared;
    }
    let Some(initializer) = initializer else {
        return declared;
    };
    let initializer_shape =
        planned_method_call_return_shape(initializer, env, ctx).unwrap_or_else(|| {
            infer_expr_shape_with_context(
                initializer,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                    facts: Some(ctx.facts),
                },
            )
        });
    if initializer_shape
        .optional_element_type()
        .is_some_and(|element_ty| element_ty == declared.java_ty)
    {
        initializer_shape
    } else {
        declared
    }
}

fn planned_method_call_return_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    stream_like_call_return_shape(expr, env, ctx)
        .or_else(|| receiverless_current_type_call_return_shape(expr, ctx))
        .or_else(|| method_call_return_shape_from_receiver(expr, env, ctx))
}

fn refine_assigned_local_shape(target: &Expr, value: &Expr, env: &mut ShapeEnv, ctx: LowerCtx) {
    let Expr::Var(name) = target else {
        return;
    };
    let Some(current) = env.local_shape(name).cloned() else {
        return;
    };
    let Some(value_shape) = planned_method_call_return_shape(value, env, ctx).or_else(|| {
        Some(infer_expr_shape_with_context(
            value,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
        ))
    }) else {
        return;
    };
    if !java_types_have_same_erased_identity(&current.java_ty, &value_shape.java_ty)
        || !shape_has_more_generic_information(&value_shape, &current)
    {
        return;
    }
    env.insert_local_shape(name.clone(), value_shape);
}

fn refine_local_shapes_from_branch_assignments(
    then_block: &Block,
    else_block: Option<&Block>,
    env: &mut ShapeEnv,
    ctx: LowerCtx,
) {
    let mut assignments = Vec::new();
    collect_branch_assignment_shapes(then_block, env, ctx, &mut assignments);
    if let Some(else_block) = else_block {
        collect_branch_assignment_shapes(else_block, env, ctx, &mut assignments);
    }
    for (name, shape) in assignments {
        let Some(current) = env.local_shape(&name).cloned() else {
            continue;
        };
        if !java_types_have_same_erased_identity(&current.java_ty, &shape.java_ty)
            || !shape_has_more_generic_information(&shape, &current)
        {
            continue;
        }
        env.insert_local_shape(name, shape);
    }
}

fn collect_branch_assignment_shapes(
    block: &Block,
    env: &ShapeEnv,
    ctx: LowerCtx,
    out: &mut Vec<(String, RustSlotShape)>,
) {
    for stmt in &block.statements {
        match stmt {
            Stmt::Assign {
                target: Expr::Var(name),
                value,
            } => {
                if let Some(shape) =
                    planned_method_call_return_shape(value, env, ctx).or_else(|| {
                        Some(infer_expr_shape_with_context(
                            value,
                            env,
                            ExprShapeContext {
                                signatures: Some(ctx.signatures),
                                representations: Some(ctx.representations),
                                facts: Some(ctx.facts),
                            },
                        ))
                    })
                {
                    out.push((name.clone(), shape));
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_branch_assignment_shapes(then_block, env, ctx, out);
                if let Some(else_block) = else_block {
                    collect_branch_assignment_shapes(else_block, env, ctx, out);
                }
            }
            Stmt::Block(block) => collect_branch_assignment_shapes(block, env, ctx, out),
            _ => {}
        }
    }
}

fn receiverless_current_type_call_return_shape(
    expr: &Expr,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: None,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    let current_type = ctx.current_type?;
    let snake_name = crate::parser::to_snake_case(name);
    if let Some(shape) = source_summary_declared_receiverless_method_return_shape(
        current_type,
        name,
        args.len(),
        ctx,
    ) {
        return Some(shape);
    }
    ctx.signatures
        .method_return_shape_by_rust_name(current_type, name)
        .or_else(|| {
            ctx.signatures
                .method_return_shape_by_rust_name(current_type, &snake_name)
        })
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(current_type, name, args.len())
                .and_then(|(_, signature)| signature.return_shape.clone())
        })
}

fn source_summary_declared_receiverless_method_return_shape(
    current_type: &str,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let ty = find_context_type_by_key(ctx.program, current_type)?;
    source_summary_declared_inherited_method(ty, method_name, arg_count, ctx)
        .and_then(|method| source_summary_declared_return_shape_for_method(ty, method, ctx))
}

fn source_summary_declared_inherited_method<'a>(
    ty: &'a TypeDecl,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx<'a>,
) -> Option<&'a MethodDecl> {
    let mut current = ty
        .extends
        .as_deref()
        .map(crate::stdlib::canonical_external_receiver_type_key);
    let mut visited = HashSet::new();
    while let Some(type_key) = current {
        if !visited.insert(type_key.clone()) {
            break;
        }
        if let Some(summary_ty) =
            find_context_type_by_key(ctx.source_summary_type_program, &type_key)
                .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &type_key))
        {
            if let Some(method) =
                find_declared_method_by_name_and_arity(summary_ty, method_name, arg_count)
            {
                return Some(method);
            }
            current = summary_ty
                .extends
                .as_deref()
                .map(crate::stdlib::canonical_external_receiver_type_key);
            continue;
        }
        if let Some(local_ty) = find_context_type_by_key(ctx.program, &type_key) {
            current = local_ty
                .extends
                .as_deref()
                .map(crate::stdlib::canonical_external_receiver_type_key);
        } else {
            break;
        }
    }
    None
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
    render_switch(
        plan,
        lower_expr(plan.selector, env, ctx),
        cases,
        fallback_switch_all_paths_return_or_throw(plan.cases),
    )
}

fn fallback_switch_all_paths_return_or_throw(cases: &[SwitchCase]) -> bool {
    if cases.is_empty()
        || !cases.iter().any(|case| {
            case.labels
                .iter()
                .any(|label| matches!(label, SwitchLabel::Default))
        })
    {
        return false;
    }
    (0..cases.len()).all(|start| switch_path_returns_or_throws(cases, start))
}

fn switch_path_returns_or_throws(cases: &[SwitchCase], start: usize) -> bool {
    cases[start..]
        .iter()
        .flat_map(|case| &case.statements)
        .find_map(|stmt| match stmt {
            Stmt::Return(_) | Stmt::Throw(_) => Some(true),
            Stmt::Break(_) => Some(false),
            Stmt::If {
                then_block,
                else_block: Some(else_block),
                ..
            } if block_returns_or_throws(then_block) && block_returns_or_throws(else_block) => {
                Some(true)
            }
            _ => None,
        })
        .unwrap_or(false)
}

fn block_returns_or_throws(block: &Block) -> bool {
    block.statements.last().is_some_and(stmt_returns_or_throws)
}

fn stmt_returns_or_throws(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Throw(_) => true,
        Stmt::Block(block) => block_returns_or_throws(block),
        Stmt::If {
            then_block,
            else_block: Some(else_block),
            ..
        } => block_returns_or_throws(then_block) && block_returns_or_throws(else_block),
        _ => false,
    }
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
    if let (JavaType::Named { name, .. }, Expr::Var(label_name)) = (selector_ty, expr)
        && external_enum_constant_label_matches(ctx.facts, name, label_name)
    {
        let owner = crate::slot_render::rust_name_for_java_named_type(
            &crate::stdlib::canonical_external_receiver_type_key(name),
        );
        let field = crate::parser::to_snake_case(label_name);
        return Some(format!("{owner}::{field}"));
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
    let selector_ty = crate::types::infer_expr_type(plan.selector, env.type_env());
    let label_exprs = case_plan
        .labels
        .iter()
        .filter_map(|label| match label {
            SwitchCaseLabelPlan::Expr(expr) => {
                Some(lower_switch_case_label_expr(expr, env, ctx, &selector_ty))
            }
            SwitchCaseLabelPlan::Default => None,
        })
        .collect::<Vec<_>>();
    switch_case_condition(plan, &case_plan, label_exprs)
}

fn lower_switch_case_label_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    selector_ty: &JavaType,
) -> RustExpr {
    if let (JavaType::Named { name, .. }, Expr::Var(label_name)) = (selector_ty, expr) {
        if let Some(variant) = ctx
            .representations
            .java_enum_variant_for_label(name, label_name)
        {
            return RustExpr::Raw(variant);
        }
        if external_enum_constant_label_matches(ctx.facts, name, label_name) {
            let owner = crate::slot_render::rust_name_for_java_named_type(
                &crate::stdlib::canonical_external_receiver_type_key(name),
            );
            let field = crate::parser::to_snake_case(label_name);
            return RustExpr::Raw(format!("{owner}::{field}"));
        }
    }
    lower_expr(expr, env, ctx)
}

fn external_enum_constant_label_matches(facts: &FactSet, owner: &str, label: &str) -> bool {
    facts
        .external_field_for(owner, label)
        .and_then(|fact| fact.field_type.as_ref())
        .is_some_and(|field_type| external_enum_constant_field_type_matches(owner, field_type))
        || crate::external_packs::external_field_for(owner, label)
            .and_then(|fact| fact.field_type)
            .is_some_and(|field_type| external_enum_constant_field_type_matches(owner, &field_type))
}

fn external_enum_constant_field_type_matches(owner: &str, field_type: &JavaType) -> bool {
    matches!(
        field_type,
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name)
                    == crate::stdlib::canonical_external_receiver_type_key(owner)
    )
}

fn lower_java_enum_static_field_access(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    java_enum_static_field_owner_candidates(receiver_name, env, ctx)
        .into_iter()
        .find_map(|owner| {
            ctx.representations
                .java_enum_variant_for_label(&owner, field)
                .map(RustExpr::Raw)
        })
}

fn lower_java_enum_static_field_access_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let DispatchShape::ClosedEnum { enum_name } = &target.dispatch else {
        return None;
    };
    let lowered = lower_java_enum_static_field_access(receiver, field, env, ctx)?;
    let rendered = emit::emit_expr(&lowered);
    rendered
        .starts_with(&format!("{enum_name}::"))
        .then_some(lowered)
}

fn java_enum_static_field_owner_candidates(
    receiver_name: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
        push_unique_receiver_type_name(&mut candidates, alias.to_string());
    }
    push_unique_receiver_type_name(&mut candidates, receiver_name.to_string());
    if let Some(current_type) = ctx.current_type
        && let Some(current) = find_context_type_by_key(ctx.program, current_type)
    {
        collect_nested_enum_owner_candidates(current, receiver_name, &mut candidates);
    }
    candidates
}

fn collect_nested_enum_owner_candidates(ty: &TypeDecl, receiver_name: &str, out: &mut Vec<String>) {
    for member in &ty.members {
        let Member::Type(nested) = member else {
            continue;
        };
        if nested.kind == TypeKind::Enum
            && type_simple_name_matches_receiver_alias(&nested.name, receiver_name)
        {
            push_unique_receiver_type_name(out, nested.name.clone());
        }
        collect_nested_enum_owner_candidates(nested, receiver_name, out);
    }
}

fn type_simple_name_matches_receiver_alias(type_name: &str, receiver_name: &str) -> bool {
    let simple = type_name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(type_name);
    simple == receiver_name || crate::parser::to_snake_case(simple) == receiver_name
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
        lower_stmt(
            &stmt,
            &mut case_env,
            wrap_returns_in_ok,
            ctx,
            &ExprRefinements::default(),
            &mut out,
        );
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
    lower_return_stmt_with_refinements(
        expr,
        env,
        wrap_returns_in_ok,
        ctx,
        &ExprRefinements::default(),
    )
}

fn lower_return_stmt_with_refinements(
    expr: Option<&Expr>,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustStmt {
    let value = expr.map(|expr| match return_value_plan(ctx.current_return_shape) {
        ReturnValuePlan::Slot(slot) => {
            lower_expr_for_slot_with_refinements(expr, env, slot, ctx, refinements)
        }
        ReturnValuePlan::Raw => lower_expr_with_refinements(expr, env, ctx, refinements),
    });
    return_stmt(value, wrap_returns_in_ok)
}

fn lower_throw_expr_with_refinements(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    match plan_throw_expr(expr) {
        ThrowExprPlan::NewException { class_name, args } => {
            let args = args
                .iter()
                .map(|arg| lower_expr_with_refinements(arg, env, ctx, refinements))
                .collect();
            exception_expr_from_new_with_cause_hint(
                &class_name,
                args,
                exception_constructor_second_arg_is_cause(expr, env, ctx),
            )
        }
        ThrowExprPlan::CauseOrSelfIfInstance {
            wrapper,
            class_names,
        } => exception_cause_or_self_if_instance_expr(
            lower_expr_with_refinements(wrapper, env, ctx, refinements),
            &class_names,
        ),
        ThrowExprPlan::ExistingException(value) => {
            lower_expr_with_refinements(value, env, ctx, refinements)
        }
        ThrowExprPlan::FallbackValue(value) => {
            fallback_exception_expr(lower_expr_with_refinements(value, env, ctx, refinements))
        }
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
        } => {
            let catch_future_use_names = catch_body_read_names(planned_catches);
            let try_ctx = LowerCtx {
                catch_future_use_names: Some(&catch_future_use_names),
                ..ctx
            };
            (
                lower_block(planned_body, &mut env.clone(), true, try_ctx),
                planned_catches,
                completion,
            )
        }
        TryStmtPlan::WithResources {
            resources,
            body: planned_body,
            catches: planned_catches,
            completion,
        } => {
            let catch_future_use_names = catch_body_read_names(planned_catches);
            let try_ctx = LowerCtx {
                catch_future_use_names: Some(&catch_future_use_names),
                ..ctx
            };
            let mut resource_env = env.clone();
            let mut resource_initializers = Vec::new();
            for resource in resources {
                lower_stmt(
                    resource,
                    &mut resource_env,
                    true,
                    try_ctx,
                    &ExprRefinements::default(),
                    &mut resource_initializers,
                );
                learn_stmt_shapes(
                    resource,
                    &mut resource_env,
                    ctx.current_local_shapes,
                    try_ctx.signatures,
                );
            }
            let body = lower_block(planned_body, &mut resource_env, true, try_ctx);
            let resource_cleanup_stmts =
                try_resource_cleanup_stmts(resources, &resource_env, try_ctx);
            (
                vec![try_with_resources_scope_stmt(
                    ok_type.clone(),
                    resource_initializers,
                    body,
                    resource_cleanup_stmts,
                    matches!(completion, TryBlockCompletion::Terminates),
                )],
                planned_catches,
                completion,
            )
        }
        TryStmtPlan::FinallyOnly {
            body: planned_body,
            finally_block,
            completion,
        } => {
            let try_body = lower_block(planned_body, &mut env.clone(), true, ctx);
            let finally_body =
                lower_block(finally_block, &mut env.clone(), wrap_returns_in_ok, ctx);
            return try_finally_scope_stmt(
                ok_type,
                try_body,
                finally_body,
                matches!(completion, TryBlockCompletion::Terminates),
                wrap_returns_in_ok,
            );
        }
        TryStmtPlan::CatchFinally {
            body: planned_body,
            catches: planned_catches,
            finally_block,
            completion,
        } => {
            let catch_future_use_names = catch_body_read_names(planned_catches);
            let try_ctx = LowerCtx {
                catch_future_use_names: Some(&catch_future_use_names),
                ..ctx
            };
            let try_body = lower_block(planned_body, &mut env.clone(), true, try_ctx);
            let try_ok_type = if matches!(completion, TryBlockCompletion::FallsThroughUnit) {
                RustType::Unit
            } else {
                ok_type.clone()
            };
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
            let finally_body =
                lower_block(finally_block, &mut env.clone(), wrap_returns_in_ok, ctx);
            return try_catch_finally_scope_stmt(
                try_ok_type,
                try_body,
                catch_arms,
                finally_body,
                matches!(completion, TryBlockCompletion::Terminates),
                wrap_returns_in_ok,
                matches!(completion, TryBlockCompletion::FallsThroughUnit),
            );
        }
        TryStmtPlan::Unsupported => return unsupported_try_catch_finally_stmt(),
    };
    let try_ok_type = if matches!(completion, TryBlockCompletion::FallsThroughUnit) {
        RustType::Unit
    } else {
        ok_type
    };
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
        try_ok_type,
        try_body,
        catch_arms,
        wrap_returns_in_ok,
        matches!(completion, TryBlockCompletion::FallsThroughUnit),
    )
}

fn try_resource_cleanup_stmts(resources: &[Stmt], env: &ShapeEnv, ctx: LowerCtx) -> Vec<RustStmt> {
    try_resource_names(resources)
        .into_iter()
        .map(|name| {
            let receiver = Expr::Var(name.clone());
            let close_call = RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var(name.clone())),
                method: "close".to_string(),
                args: Vec::new(),
            };
            if slot_expr_needs_result_propagation_from_shape_env(
                Some(&receiver),
                "close",
                0,
                env,
                ctx,
            ) || resource_close_declares_result(resources, &name, ctx)
                || matches!(
                    plan_call_exception_propagation_with_lower_context(
                        Some(&receiver),
                        "close",
                        &[],
                        None,
                        ctx,
                        env,
                        ctx.current_type,
                    ),
                    CallExceptionPropagationPlan::PropagateResult
                )
            {
                expr_stmt(propagate_result_expr(close_call))
            } else {
                expr_stmt(close_call)
            }
        })
        .collect()
}

fn resource_close_declares_result(resources: &[Stmt], resource_name: &str, ctx: LowerCtx) -> bool {
    let Some(resource_ty) = resources.iter().find_map(|resource| match resource {
        Stmt::VarDecl { ty, name, .. } if name == resource_name => Some(ty),
        _ => None,
    }) else {
        return false;
    };
    resource_type_names_for_close_lookup(resource_ty)
        .into_iter()
        .any(|receiver_type| {
            ctx.facts
                .external_api_effect_for_receiver_with_arity(Some(receiver_type), "close", Some(0))
                .is_some_and(|effect| !effect.thrown.is_empty())
                || crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(receiver_type),
                    "close",
                    Some(0),
                )
                .is_some_and(|effect| !effect.thrown.is_empty())
        })
}

fn resource_type_names_for_close_lookup(ty: &JavaType) -> Vec<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            vec![name.as_str(), name.rsplit('.').next().unwrap_or(name)]
        }
        _ => Vec::new(),
    }
}

fn catch_body_read_names(catches: &[CatchClause]) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for catch in catches {
        collect_stmt_read_names(&catch.body.statements, &mut names);
        names.remove(&catch.name);
    }
    names
}

fn preinitialized_try_capture_names_for_block(
    statements: &[Stmt],
) -> std::collections::HashSet<String> {
    let mut declared_uninitialized = std::collections::HashSet::new();
    let mut preinitialize = std::collections::HashSet::new();
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                name,
                initializer: None,
                ..
            } => {
                declared_uninitialized.insert(name.clone());
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut assigned = std::collections::HashSet::new();
                collect_stmt_assigned_names(resources, &mut assigned);
                collect_stmt_assigned_names(&body.statements, &mut assigned);
                for catch in catches {
                    collect_stmt_assigned_names(&catch.body.statements, &mut assigned);
                    assigned.remove(&catch.name);
                }
                if let Some(finally_block) = finally_block {
                    collect_stmt_assigned_names(&finally_block.statements, &mut assigned);
                }
                for name in assigned {
                    if declared_uninitialized.contains(&name) {
                        preinitialize.insert(name);
                    }
                }
            }
            Stmt::VarDecl {
                name,
                initializer: Some(_),
                ..
            } => {
                declared_uninitialized.remove(name);
            }
            _ => {}
        }
    }
    preinitialize
}

fn collect_stmt_assigned_names(statements: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } => {
                if let Expr::Var(name) = target {
                    names.insert(name.clone());
                }
                collect_expr_assigned_names(target, names);
                collect_expr_assigned_names(value, names);
            }
            Stmt::VarDecl {
                initializer, name, ..
            } => {
                if let Some(initializer) = initializer {
                    collect_expr_assigned_names(initializer, names);
                }
                names.remove(name);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_expr_assigned_names(condition, names);
                collect_stmt_assigned_names(&then_block.statements, names);
                if let Some(else_block) = else_block {
                    collect_stmt_assigned_names(&else_block.statements, names);
                }
            }
            Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
                collect_expr_assigned_names(condition, names);
                collect_stmt_assigned_names(&body.statements, names);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_stmt_assigned_names(init, names);
                if let Some(condition) = condition {
                    collect_expr_assigned_names(condition, names);
                }
                for expr in update {
                    collect_expr_assigned_names(expr, names);
                }
                collect_stmt_assigned_names(&body.statements, names);
            }
            Stmt::EnhancedFor {
                var_name,
                iterable,
                body,
                ..
            } => {
                collect_expr_assigned_names(iterable, names);
                collect_stmt_assigned_names(&body.statements, names);
                names.remove(var_name);
            }
            Stmt::Switch { selector, cases } => {
                collect_expr_assigned_names(selector, names);
                for case in cases {
                    collect_stmt_assigned_names(&case.statements, names);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_stmt_assigned_names(resources, names);
                collect_stmt_assigned_names(&body.statements, names);
                for catch in catches {
                    collect_stmt_assigned_names(&catch.body.statements, names);
                    names.remove(&catch.name);
                }
                if let Some(finally_block) = finally_block {
                    collect_stmt_assigned_names(&finally_block.statements, names);
                }
            }
            Stmt::Block(block) => collect_stmt_assigned_names(&block.statements, names),
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_expr_assigned_names(expr, names);
            }
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_expr_assigned_names(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_expr_assigned_names(receiver, names);
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_assigned_names(left, names);
            collect_expr_assigned_names(right, names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_assigned_names(condition, names);
            collect_expr_assigned_names(then_expr, names);
            collect_expr_assigned_names(else_expr, names);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_assigned_names(receiver, names);
            }
            for arg in args {
                collect_expr_assigned_names(arg, names);
            }
        }
        Expr::New { args, .. } | Expr::ArrayInitializer(args) => {
            for arg in args {
                collect_expr_assigned_names(arg, names);
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_expr_assigned_names(array, names);
            collect_expr_assigned_names(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_assigned_names(dimension, names);
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    collect_expr_assigned_names(element, names);
                }
            }
        }
        Expr::Cast { expr, .. } => collect_expr_assigned_names(expr, names),
        Expr::InstanceOf { expr, .. } => collect_expr_assigned_names(expr, names),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_expr_assigned_names(expr, names),
            LambdaBody::Block(block) => collect_stmt_assigned_names(&block.statements, names),
        },
        Expr::MethodReference { .. }
        | Expr::Var(_)
        | Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_stmt_read_counts(stmt: &Stmt, counts: &mut HashMap<String, usize>) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_expr_read_counts(initializer, counts);
            }
        }
        Stmt::Assign { target, value } => {
            collect_expr_read_counts(target, counts);
            collect_expr_read_counts(value, counts);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_read_counts(expr, counts);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_read_counts(condition, counts);
            for stmt in &then_block.statements {
                collect_stmt_read_counts(stmt, counts);
            }
            if let Some(else_block) = else_block {
                for stmt in &else_block.statements {
                    collect_stmt_read_counts(stmt, counts);
                }
            }
        }
        Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
            collect_expr_read_counts(condition, counts);
            for stmt in &body.statements {
                collect_stmt_read_counts(stmt, counts);
            }
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stmt_read_counts(stmt, counts);
            }
            if let Some(condition) = condition {
                collect_expr_read_counts(condition, counts);
            }
            for expr in update {
                collect_expr_read_counts(expr, counts);
            }
            for stmt in &body.statements {
                collect_stmt_read_counts(stmt, counts);
            }
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_expr_read_counts(iterable, counts);
            for stmt in &body.statements {
                collect_stmt_read_counts(stmt, counts);
            }
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_read_counts(selector, counts);
            for case in cases {
                for stmt in &case.statements {
                    collect_stmt_read_counts(stmt, counts);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                collect_stmt_read_counts(stmt, counts);
            }
            for stmt in &body.statements {
                collect_stmt_read_counts(stmt, counts);
            }
            for catch in catches {
                for stmt in &catch.body.statements {
                    collect_stmt_read_counts(stmt, counts);
                }
            }
            if let Some(finally_block) = finally_block {
                for stmt in &finally_block.statements {
                    collect_stmt_read_counts(stmt, counts);
                }
            }
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_read_counts(arg, counts);
            }
        }
        Stmt::Block(block) => {
            for stmt in &block.statements {
                collect_stmt_read_counts(stmt, counts);
            }
        }
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_expr_read_counts(expr: &Expr, counts: &mut HashMap<String, usize>) {
    match expr {
        Expr::Var(name) => {
            *counts.entry(name.clone()).or_insert(0) += 1;
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_expr_read_counts(receiver, counts);
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_read_counts(left, counts);
            collect_expr_read_counts(right, counts);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_read_counts(condition, counts);
            collect_expr_read_counts(then_expr, counts);
            collect_expr_read_counts(else_expr, counts);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_read_counts(receiver, counts);
            }
            for arg in args {
                collect_expr_read_counts(arg, counts);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_read_counts(arg, counts);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        for stmt in &body.statements {
                            collect_stmt_read_counts(stmt, counts);
                        }
                    }
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_read_counts(expr, counts);
        }
        Expr::ArrayAccess(array, index) => {
            collect_expr_read_counts(array, counts);
            collect_expr_read_counts(index, counts);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_expr_read_counts(expr, counts);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_expr_read_counts(expr, counts);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_expr_read_counts(item, counts);
            }
        }
        Expr::Lambda { params, body } => {
            let mut nested = HashMap::new();
            match body {
                LambdaBody::Expr(expr) => collect_expr_read_counts(expr, &mut nested),
                LambdaBody::Block(block) => {
                    for stmt in &block.statements {
                        collect_stmt_read_counts(stmt, &mut nested);
                    }
                }
            }
            for param in params {
                nested.remove(&param.name);
            }
            for (name, count) in nested {
                *counts.entry(name).or_insert(0) += count;
            }
        }
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_stmt_read_names(statements: &[Stmt], names: &mut std::collections::HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                initializer, name, ..
            } => {
                if let Some(initializer) = initializer {
                    collect_expr_read_names(initializer, names);
                }
                names.remove(name);
            }
            Stmt::Assign { target, value } => {
                collect_expr_read_names(target, names);
                collect_expr_read_names(value, names);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_expr_read_names(expr, names);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_expr_read_names(condition, names);
                collect_stmt_read_names(&then_block.statements, names);
                if let Some(else_block) = else_block {
                    collect_stmt_read_names(&else_block.statements, names);
                }
            }
            Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
                collect_expr_read_names(condition, names);
                collect_stmt_read_names(&body.statements, names);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_stmt_read_names(init, names);
                if let Some(condition) = condition {
                    collect_expr_read_names(condition, names);
                }
                for expr in update {
                    collect_expr_read_names(expr, names);
                }
                collect_stmt_read_names(&body.statements, names);
            }
            Stmt::EnhancedFor {
                var_name,
                iterable,
                body,
                ..
            } => {
                collect_expr_read_names(iterable, names);
                collect_stmt_read_names(&body.statements, names);
                names.remove(var_name);
            }
            Stmt::Switch { selector, cases } => {
                collect_expr_read_names(selector, names);
                for case in cases {
                    collect_stmt_read_names(&case.statements, names);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_stmt_read_names(resources, names);
                collect_stmt_read_names(&body.statements, names);
                for catch in catches {
                    collect_stmt_read_names(&catch.body.statements, names);
                    names.remove(&catch.name);
                }
                if let Some(finally_block) = finally_block {
                    collect_stmt_read_names(&finally_block.statements, names);
                }
            }
            Stmt::Block(block) => collect_stmt_read_names(&block.statements, names),
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_expr_read_names(expr: &Expr, names: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            names.insert(name.clone());
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_expr_read_names(receiver, names);
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_read_names(left, names);
            collect_expr_read_names(right, names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_read_names(condition, names);
            collect_expr_read_names(then_expr, names);
            collect_expr_read_names(else_expr, names);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_read_names(receiver, names);
            }
            for arg in args {
                collect_expr_read_names(arg, names);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_read_names(arg, names);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        collect_stmt_read_names(&body.statements, names);
                    }
                }
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_read_names(expr, names);
        }
        Expr::ArrayAccess(array, index) => {
            collect_expr_read_names(array, names);
            collect_expr_read_names(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_expr_read_names(expr, names);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_expr_read_names(expr, names);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_expr_read_names(item, names);
            }
        }
        Expr::Lambda { params, body } => {
            match body {
                LambdaBody::Expr(expr) => collect_expr_read_names(expr, names),
                LambdaBody::Block(block) => collect_stmt_read_names(&block.statements, names),
            }
            for param in params {
                names.remove(&param.name);
            }
        }
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
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
    let plan =
        plan_call_expr_with_lower_context(Some(receiver), name, args, resolved.as_ref(), ctx, env);
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

    let lowered_receiver = adapt_native_collection_receiver(
        receiver,
        lower_expr(receiver, env, ctx),
        crate::call_plan::NativeCollectionMethod::Extend,
        env,
        ctx,
    );
    if native_collection_extend_needs_self_read_temp(receiver, &args[0], env, ctx) {
        let temp_name = "__java2rust_add_all_source";
        return Some(raw_stmt(format!(
            "let {temp_name} = {};\n{}.extend({temp_name});\n",
            emit::emit_expr(&arg),
            emit::emit_expr(&lowered_receiver)
        )));
    }

    Some(expr_stmt(apply_native_collection_extend(
        lowered_receiver,
        arg,
    )))
}

fn plan_call_expr_with_lower_context<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    args: &'a [Expr],
    resolved: Option<&'a ResolvedCall>,
    ctx: LowerCtx<'_>,
    env: &ShapeEnv,
) -> CallExprPlan<'a> {
    plan_call_expr_with_external_index(
        receiver,
        name,
        args,
        resolved,
        ctx.facts,
        ctx.external_index,
        env.type_env(),
    )
}

fn native_collection_extend_needs_self_read_temp(
    receiver: &Expr,
    arg: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    native_collection_extend_receiver_is_self_field(receiver, env) && expr_reads_self(arg, env, ctx)
}

fn native_collection_extend_receiver_is_self_field(receiver: &Expr, env: &ShapeEnv) -> bool {
    match receiver {
        Expr::FieldAccess(base, _) if matches!(base.as_ref(), Expr::This) => true,
        Expr::Var(name) => env.field_type(name).is_some(),
        _ => false,
    }
}

fn expr_reads_self(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    match expr {
        Expr::This | Expr::Super => true,
        Expr::Var(name) => env.field_type(name).is_some(),
        Expr::FieldAccess(base, _) => {
            matches!(base.as_ref(), Expr::This) || expr_reads_self(base, env, ctx)
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            receiverless_call_reads_self(receiver.as_deref(), name, args.len(), ctx)
                || receiver
                    .as_deref()
                    .is_some_and(|receiver| matches!(receiver, Expr::This | Expr::Super))
                || receiver
                    .as_deref()
                    .is_some_and(|receiver| expr_reads_self(receiver, env, ctx))
                || args.iter().any(|arg| expr_reads_self(arg, env, ctx))
        }
        Expr::New { args, .. } => args.iter().any(|arg| expr_reads_self(arg, env, ctx)),
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::ArrayAccess(expr, _) => expr_reads_self(expr, env, ctx),
        Expr::Binary { left, right, .. } => {
            expr_reads_self(left, env, ctx) || expr_reads_self(right, env, ctx)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_reads_self(condition, env, ctx)
                || expr_reads_self(then_expr, env, ctx)
                || expr_reads_self(else_expr, env, ctx)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_reads_self(expr, env, ctx))
                || initializer.as_ref().is_some_and(|values| {
                    values.iter().any(|value| expr_reads_self(value, env, ctx))
                })
        }
        Expr::ArrayInitializer(values) => {
            values.iter().any(|value| expr_reads_self(value, env, ctx))
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => expr_reads_self(expr, env, ctx),
            crate::java_ir::LambdaBody::Block(block) => block
                .statements
                .iter()
                .any(|stmt| stmt_reads_self(stmt, env, ctx)),
        },
        Expr::Literal(_) | Expr::ClassLiteral(_) | Expr::MethodReference { .. } | Expr::Raw(_) => {
            false
        }
    }
}

fn receiverless_call_reads_self(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    ctx: LowerCtx,
) -> bool {
    if receiver.is_some() {
        return false;
    }
    let Some(current_type) = ctx.current_type else {
        return false;
    };
    if source_summary_declared_receiverless_method_return_shape(current_type, name, arg_count, ctx)
        .is_some()
    {
        return true;
    }
    ctx.signatures
        .method_signature_by_base_name_and_arity(current_type, name, arg_count)
        .is_some_and(|(_, signature)| !matches!(signature.receiver, ReceiverPlan::None))
}

fn stmt_reads_self(stmt: &Stmt, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    match stmt {
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_reads_self(expr, env, ctx)
        }
        Stmt::Assign { target, value } => {
            expr_reads_self(target, env, ctx) || expr_reads_self(value, env, ctx)
        }
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_reads_self(expr, env, ctx)),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_reads_self(condition, env, ctx)
                || then_block
                    .statements
                    .iter()
                    .any(|stmt| stmt_reads_self(stmt, env, ctx))
                || else_block.as_ref().is_some_and(|branch| {
                    branch
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_self(stmt, env, ctx))
                })
        }
        Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
            expr_reads_self(condition, env, ctx)
                || body
                    .statements
                    .iter()
                    .any(|stmt| stmt_reads_self(stmt, env, ctx))
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| stmt_reads_self(stmt, env, ctx))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_reads_self(expr, env, ctx))
                || update.iter().any(|expr| expr_reads_self(expr, env, ctx))
                || body
                    .statements
                    .iter()
                    .any(|stmt| stmt_reads_self(stmt, env, ctx))
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_reads_self(iterable, env, ctx)
                || body
                    .statements
                    .iter()
                    .any(|stmt| stmt_reads_self(stmt, env, ctx))
        }
        Stmt::Switch { selector, cases } => {
            expr_reads_self(selector, env, ctx)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => expr_reads_self(expr, env, ctx),
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_self(stmt, env, ctx))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|resource| stmt_reads_self(resource, env, ctx))
                || body
                    .statements
                    .iter()
                    .any(|stmt| stmt_reads_self(stmt, env, ctx))
                || catches.iter().any(|catch| {
                    catch
                        .body
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_self(stmt, env, ctx))
                })
                || finally_block.as_ref().is_some_and(|block| {
                    block
                        .statements
                        .iter()
                        .any(|stmt| stmt_reads_self(stmt, env, ctx))
                })
        }
        Stmt::Block(block) => block
            .statements
            .iter()
            .any(|stmt| stmt_reads_self(stmt, env, ctx)),
        Stmt::ConstructorInvocation { args, .. } => {
            args.iter().any(|arg| expr_reads_self(arg, env, ctx))
        }
        Stmt::LocalType(_) | Stmt::Unsupported(_) => false,
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) => false,
    }
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
        CallStatementPlan::NativeArraysSort { target } => Some(RustStmt::Expr(RustExpr::Raw(
            format!("{}.sort()", emit::emit_expr(&lower_expr(target, env, ctx))),
        ))),
        CallStatementPlan::NativeArraysFill { target, value } => {
            Some(RustStmt::Expr(RustExpr::Raw(format!(
                "{}.fill({})",
                emit::emit_expr(&lower_expr(target, env, ctx)),
                emit::emit_expr(&lower_expr(value, env, ctx))
            ))))
        }
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
    if !matches!(expr, Expr::Call { .. }) {
        return None;
    }
    if !slot_expr_needs_result_propagation(expr, env, ctx) {
        return None;
    }
    Some(expr_stmt(lower_expr(expr, env, ctx)))
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
            lower_lambda_block_with_result_carrier(
                block,
                &mut loop_env,
                lambda_ctx,
                ctx.current_return_shape.is_some(),
            )
        }
    };
    let pattern = if param_names.len() == 1 {
        param_names[0].clone()
    } else {
        format!("({})", param_names.join(", "))
    };
    if let Some(iterator) = lower_int_stream_range_expr(receiver, env, ctx) {
        return Some(render_enhanced_for_iterator(
            &pattern, &iterator, &loop_body,
        ));
    }
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
    if static_string_sequence_field_iterable(iterable, env, ctx) {
        loop_env.insert_local_shape(var_name.to_string(), borrowed_string_content_shape());
        let iterator = RustExpr::Raw(format!(
            "{}.iter().copied()",
            emit::emit_expr(&lower_enhanced_for_iterable_expr(iterable, env, ctx))
        ));
        return render_enhanced_for_iterator(
            var_name,
            &iterator,
            &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
        );
    }
    if let Some(element_shape) =
        infer_stream_source_element_shape_with_facts(iterable, env.type_env(), ctx.facts)
    {
        loop_env.insert_local_shape(var_name.to_string(), element_shape);
    }
    if let Some(iterator) = lower_int_stream_range_expr(iterable, env, ctx) {
        return render_enhanced_for_iterator(
            var_name,
            &iterator,
            &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
        );
    }
    render_enhanced_for(
        var_name,
        &lower_enhanced_for_iterable_expr(iterable, env, ctx),
        &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
    )
}

fn static_string_sequence_field_iterable(iterable: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    if !matches!(iterable, Expr::FieldAccess(_, _)) {
        return false;
    }
    let shape = infer_expr_shape_with_context(
        iterable,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    java_type_is_string_sequence(&shape.java_ty)
}

fn java_type_is_string_sequence(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Array(inner) => matches!(inner.as_ref(), JavaType::String),
        JavaType::Named { name, args } if name.rsplit('.').next().unwrap_or(name) == "List" => {
            matches!(args.as_slice(), [JavaType::String])
        }
        _ => false,
    }
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
            lower_field_access_with_facts(
                receiver,
                field_name,
                env.type_env(),
                lower_expr(receiver, env, ctx),
                ctx.facts,
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
    exact_erased_object_locals: HashMap<String, JavaType>,
}

impl ExprRefinements {
    fn with_non_null_local(&self, name: &str) -> Self {
        let mut refinements = Self {
            non_null_locals: self.non_null_locals.clone(),
            exact_erased_object_locals: self.exact_erased_object_locals.clone(),
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
    if let Some(format_expr) = lower_string_concat_with_refinements(expr, env, ctx, refinements) {
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
        Expr::FieldAccess(receiver, field) => {
            if let Some(enum_variant) =
                lower_java_enum_static_field_access(receiver, field, env, ctx)
            {
                return enum_variant;
            }
            lower_field_access_with_facts(
                receiver,
                field,
                env.type_env(),
                lower_expr_with_refinements(receiver, env, ctx, refinements),
                ctx.facts,
            )
        }
        Expr::ArrayAccess(array, index) => {
            lower_array_access_expr(array, index, env, ctx, refinements)
        }
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
            if let Some(expr) = lower_sequence_callback_call_expr(
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
            if let Some(expr) =
                lower_functional_interface_local_call(receiver.as_deref(), name, args, env, ctx)
            {
                return expr;
            }
            let plan = plan_call_expr_with_lower_context(
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                ctx,
                env,
            );
            let fallback_signature =
                fallback_receiver_method_signature(receiver.as_deref(), name, args, env, ctx);
            let context_fallback_signature = source_context_receiver_method_fallback_signature(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
            )
            .or_else(|| {
                source_context_static_method_fallback_signature(
                    receiver.as_deref(),
                    name,
                    args,
                    env,
                    ctx,
                )
            });
            let fallback_rust_name = context_fallback_signature
                .as_ref()
                .map(|signature| signature.rust_name.clone())
                .or_else(|| {
                    fallback_signature
                        .as_ref()
                        .map(|(rust_name, _)| rust_name.clone())
                });
            let plan = apply_fallback_receiver_rust_name(plan, fallback_rust_name.clone());
            let plan = apply_fallback_self_rust_name(plan, fallback_rust_name);
            let plan = retarget_native_java_string_call_from_shape(
                plan,
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
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
            let arg_plan = if static_owner_receiver_is_non_program_emitted_context(
                receiver.as_deref(),
                env,
                ctx,
            ) {
                arg_plan
            } else {
                apply_fallback_signature_arg_shapes(arg_plan, fallback_signature.as_ref())
            };
            let arg_plan = apply_context_fallback_signature_arg_shapes(
                arg_plan,
                context_fallback_signature.as_ref(),
            );
            let arg_plan = apply_static_fallback_reference_arg_shapes(
                arg_plan,
                receiver.as_deref(),
                env,
                ctx,
                fallback_signature.as_ref(),
            );
            let rust_args = lower_args_for_call_target(
                &target,
                receiver.as_deref(),
                args,
                env,
                arg_plan,
                resolved.as_ref(),
                ctx,
                fallback_signature.as_ref().map(|(_, signature)| *signature),
            );
            let lowered_receiver = target.receiver_expr().map(|receiver| {
                let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
                adapt_call_target_receiver_with_signature(
                    &target,
                    receiver,
                    lowered,
                    env,
                    ctx,
                    refinements,
                    fallback_signature.as_ref().map(|(_, signature)| *signature),
                )
            });
            let mut lowered =
                apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx);
            if slot_expr_needs_result_propagation(expr, env, ctx) {
                lowered = propagate_result_expr(lowered);
            }
            lowered
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            if concrete_java_type_name(ty)
                .map(crate::stdlib::canonical_external_receiver_type_key)
                .as_deref()
                == Some("URI")
                && args.len() == 1
            {
                let lowered = RustExpr::Call {
                    function: Box::new(RustExpr::Raw("URI::parse".to_string())),
                    args: lower_args_with_plan(
                        args,
                        env,
                        ArgumentLoweringPlan::Unshaped {
                            arg_count: args.len(),
                        },
                        ctx,
                    ),
                };
                return propagate_result_expr(lowered);
            }
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
                plan_constructor_arguments_with_signature_fallback(
                    resolved.as_ref(),
                    args.len(),
                    ctx.signatures,
                    ty,
                    &target.rust_name,
                    ctx.facts,
                )
            };
            let rust_args = lower_constructor_args_with_plan(args, env, constructor_arg_plan, ctx);
            let mut lowered = apply_new_target_with_facts(&target, rust_args, ctx.facts);
            if external_constructor_returns_rust_result(ty, args.len(), ctx.facts) {
                lowered = propagate_result_expr(lowered);
            }
            lowered
        }
        Expr::Cast { ty, expr } => {
            if matches!(expr.as_ref(), Expr::Literal(Literal::Null)) {
                return typed_none_for_java_type(ty, ctx.facts);
            }
            if cast_inner_is_current_self(expr, ty, ctx) {
                return this_expr();
            }
            let inner_shape =
                planned_method_call_return_shape(expr, env, ctx).unwrap_or_else(|| {
                    infer_expr_shape_with_context(
                        expr,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    )
                });
            if cast_target_erases_slot_generic_detail(ty, &inner_shape.java_ty) {
                return lower_expr_with_refinements(expr, env, ctx, refinements);
            }
            let plan = plan_cast_conversion_from_actual(
                inner_shape,
                ty,
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
    lower_callback_lambda_expr_with_return_shape(params, body, env, ctx, None, false)
}

fn lower_callback_lambda_expr_with_return_shape(
    params: &[LambdaParam],
    body: &LambdaBody,
    env: &ShapeEnv,
    ctx: LowerCtx,
    current_return_shape: Option<&SolvedSlotShape>,
    wrap_returns_in_ok: bool,
) -> RustExpr {
    lower_callback_lambda_expr_with_shapes(
        params,
        body,
        env,
        ctx,
        current_return_shape,
        wrap_returns_in_ok,
        None,
    )
}

fn lower_callback_lambda_expr_with_shapes(
    params: &[LambdaParam],
    body: &LambdaBody,
    env: &ShapeEnv,
    ctx: LowerCtx,
    current_return_shape: Option<&SolvedSlotShape>,
    wrap_returns_in_ok: bool,
    param_shapes: Option<&[RustSlotShape]>,
) -> RustExpr {
    let mut lambda_env = env.clone();
    for (index, param) in params.iter().enumerate() {
        let shape = param_shapes
            .and_then(|shapes| shapes.get(index))
            .cloned()
            .unwrap_or_else(|| {
                let ty = param
                    .ty
                    .clone()
                    .unwrap_or_else(|| JavaType::Unknown(format!("lambda_param:{}", param.name)));
                fallback_local_shape_from_declared_type(&ty)
            });
        lambda_env.insert_local_shape(param.name.clone(), shape);
    }
    let params = params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            param_shapes
                .and_then(|shapes| shapes.get(index))
                .map(|shape| format!("{}: {}", param.name, render_callback_param_type(shape, ctx)))
                .unwrap_or_else(|| param.name.clone())
        })
        .collect::<Vec<_>>()
        .join(", ");
    match body {
        LambdaBody::Expr(expr) => RustExpr::Raw(format!(
            "|{params}| {}",
            emit::emit_expr(&lower_expr(expr, &lambda_env, ctx))
        )),
        LambdaBody::Block(block) => {
            let lambda_ctx = LowerCtx {
                current_return_shape,
                ..ctx
            };
            let statements = lower_lambda_block_with_result_carrier(
                block,
                &mut lambda_env,
                lambda_ctx,
                wrap_returns_in_ok,
            );
            RustExpr::Raw(format!(
                "|{params}| {{\n{}\n}}",
                emit::emit_statements(&statements, 1)
            ))
        }
    }
}

fn lower_sequence_callback_call_expr(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !matches!(
        name,
        "map" | "filter" | "flatMap" | "mapToPair" | "mapPartitions" | "mapPartitionsWithIndex"
    ) || args.len() != 1
    {
        return None;
    }
    let receiver_expr = receiver?;
    let callback = match &args[0] {
        Expr::Lambda { .. } => {
            lower_unshaped_argument_for_receiver(&args[0], receiver_expr, env, ctx)?
        }
        Expr::MethodReference { target, method } => {
            let element_shape = stream_like_receiver_element_shape(receiver_expr, env, ctx)?;
            render_typed_method_reference_callback(target, method, &element_shape, ctx)
        }
        _ => return None,
    };
    let plan = plan_call_expr_with_lower_context(receiver, name, args, resolved, ctx, env);
    let CallExprPlan::Target(target) = plan else {
        return None;
    };
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    });
    Some(apply_call_target_with_lower_context(
        &target,
        lowered_receiver,
        vec![callback],
        ctx,
    ))
}

fn render_typed_method_reference_callback(
    target: &str,
    method: &str,
    element_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> RustExpr {
    let param_type = render_callback_param_type(element_shape, ctx);
    match plan_method_reference(target, method) {
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            RustExpr::Raw(format!("|value: {param_type}| {owner}::{method}(value)"))
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            RustExpr::Raw(format!("|value: {param_type}| value.{method}()"))
        }
        MethodReferencePlan::CurrentReceiverMethod { method, arity } => {
            let arg_count = arity.unwrap_or(1);
            let args = (0..arg_count)
                .map(|index| {
                    if index == 0 {
                        "value".to_string()
                    } else {
                        format!("arg{index}")
                    }
                })
                .collect::<Vec<_>>();
            RustExpr::Raw(format!(
                "|value: {param_type}| self.{method}({})",
                args.join(", ")
            ))
        }
        MethodReferencePlan::Constructor { type_name } => {
            RustExpr::Raw(format!("|value: {param_type}| {type_name}::new(value)"))
        }
    }
}

fn render_callback_param_type(shape: &RustSlotShape, ctx: LowerCtx) -> String {
    if let Some(ty) = render_tuple_like_callback_param_type(&shape.java_ty, ctx.facts) {
        return ty;
    }
    if let Some(ty) = render_iterator_callback_param_type(&shape.java_ty, ctx.facts) {
        return ty;
    }
    emit::emit_type(&rust_type_for_slot_with_facts(shape, ctx.facts))
}

fn render_iterator_callback_param_type(java_ty: &JavaType, facts: &FactSet) -> Option<String> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    if simple != "Iterator" {
        return None;
    }
    let element_ty = args.first().map_or_else(
        || "Box<dyn std::any::Any>".to_string(),
        |arg| emit::emit_type(&rust_type_for_java_type_with_facts(arg, facts)),
    );
    Some(format!("std::vec::IntoIter<{element_ty}>"))
}

fn render_tuple_like_callback_param_type(java_ty: &JavaType, facts: &FactSet) -> Option<String> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    if !simple.starts_with("Tuple") || args.is_empty() {
        return None;
    }
    let rendered_args = args
        .iter()
        .map(|arg| emit::emit_type(&rust_type_for_java_type_with_facts(arg, facts)))
        .collect::<Vec<_>>();
    Some(format!(
        "{}<{}>",
        crate::slot_render::rust_name_for_java_named_type(simple),
        rendered_args.join(", ")
    ))
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
                    exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
                }
            };
            let else_refinements = if else_non_null {
                refinements.with_non_null_local(local_name)
            } else {
                ExprRefinements {
                    non_null_locals: refinements.non_null_locals.clone(),
                    exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
                }
            };
            (then_refinements, else_refinements)
        }
        TernaryNullRefinementPlan::None => (
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
                exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
            },
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
                exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
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
        && let Some(expr) =
            lower_null_comparison_expr_for_shape(op, left, false, env, ctx, refinements)
    {
        return expr;
    }
    if is_null_literal(left)
        && let Some(expr) =
            lower_null_comparison_expr_for_shape(op, right, true, env, ctx, refinements)
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
        if let Some(expr) =
            lower_current_type_get_class_comparison(op, left, right, env, ctx, refinements)
        {
            return expr;
        }
        if let Some(expr) = lower_current_self_erased_object_identity_comparison(
            op,
            left,
            &left_shape,
            right,
            &right_shape,
            env,
            ctx,
            refinements,
        ) {
            return expr;
        }
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

fn lower_current_type_get_class_comparison(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    ctx.current_type?;
    let object_expr = if expr_is_current_get_class_call(left, ctx) && expr_is_get_class_call(right)
    {
        get_class_call_receiver(right)?
    } else if expr_is_current_get_class_call(right, ctx) && expr_is_get_class_call(left) {
        get_class_call_receiver(left)?
    } else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let object_shape =
        crate::expr_shape::infer_expr_shape_with_context(object_expr, env, shape_ctx);
    if !shape_is_erased_object_boundary(&object_shape) {
        return None;
    }
    let object_type_predicate =
        erased_object_is_current_type_predicate(object_expr, &object_shape, env, ctx, refinements)?;
    Some(match op {
        BinaryOp::Eq => RustExpr::Raw(object_type_predicate),
        BinaryOp::Ne => RustExpr::Raw(format!("!({object_type_predicate})")),
        _ => return None,
    })
}

fn expr_is_current_get_class_call(expr: &Expr, ctx: LowerCtx) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    if !is_java_get_class_name(name) || !args.is_empty() {
        return false;
    }
    receiver
        .as_deref()
        .is_none_or(|receiver| is_this_receiver_expr(receiver) && ctx.current_type.is_some())
}

fn expr_is_get_class_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            name,
            args,
            ..
        } if is_java_get_class_name(name) && args.is_empty()
    )
}

fn is_java_get_class_name(name: &str) -> bool {
    matches!(name, "getClass" | "get_class")
}

fn get_class_call_receiver(expr: &Expr) -> Option<&Expr> {
    let Expr::Call { receiver, .. } = expr else {
        return None;
    };
    receiver.as_deref()
}

fn erased_object_is_current_type_predicate(
    object_expr: &Expr,
    object_shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<String> {
    let current_type = ctx.current_type?;
    let rust_type = crate::slot_render::rust_name_for_java_named_type(current_type);
    let lowered_object = if object_shape.optional_element_type().is_some() {
        lower_expr_with_refinements(object_expr, env, ctx, &ExprRefinements::default())
    } else {
        lower_expr_with_refinements(object_expr, env, ctx, refinements)
    };
    let object = emit::emit_expr(&lowered_object);
    if object_shape.optional_element_type().is_some() {
        Some(format!(
            "{object}.as_ref().is_some_and(|value| value.is::<{rust_type}>())"
        ))
    } else {
        Some(format!("{object}.is::<{rust_type}>()"))
    }
}

fn lower_current_self_erased_object_identity_comparison(
    op: BinaryOp,
    left: &Expr,
    left_shape: &RustSlotShape,
    right: &Expr,
    right_shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let current_type = ctx.current_type?;
    let (object_expr, object_shape) =
        if is_this_receiver_expr(left) && shape_is_erased_object_boundary(right_shape) {
            (right, right_shape)
        } else if is_this_receiver_expr(right) && shape_is_erased_object_boundary(left_shape) {
            (left, left_shape)
        } else {
            return None;
        };
    let lowered_object = lower_expr_with_refinements(object_expr, env, ctx, refinements);
    let rust_type = crate::slot_render::rust_name_for_java_named_type(current_type);
    let object = emit::emit_expr(&lowered_object);
    let identity_expr = if object_shape.optional_element_type().is_some() {
        format!(
            "{object}.as_ref().and_then(|value| value.downcast_ref::<{rust_type}>()).is_some_and(|other| std::ptr::eq(self, other))"
        )
    } else {
        format!(
            "{object}.downcast_ref::<{rust_type}>().is_some_and(|other| std::ptr::eq(self, other))"
        )
    };
    Some(match op {
        BinaryOp::Eq => RustExpr::Raw(identity_expr),
        BinaryOp::Ne => RustExpr::Raw(format!("!({identity_expr})")),
        _ => return None,
    })
}

fn lower_null_comparison_expr_for_shape(
    op: BinaryOp,
    value: &Expr,
    null_on_left: bool,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let shape = infer_expr_shape_with_context(
        value,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let shape = current_instance_field_shape_with_env(value, env, ctx).unwrap_or(shape);
    if shape.optional_element_type().is_none() {
        return match op {
            BinaryOp::Eq => Some(RustExpr::Literal("false".to_string())),
            BinaryOp::Ne => Some(RustExpr::Literal("true".to_string())),
            _ => None,
        };
    }
    lower_null_comparison_expr(
        op,
        lower_expr_with_refinements(value, env, ctx, refinements),
        null_on_left,
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
        return RustExpr::Cast {
            expr: Box::new(lowered),
            ty: "i32".to_string(),
        };
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
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
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
    let arg_count = match &plan {
        ArgumentLoweringPlan::Unshaped { arg_count } => *arg_count,
        ArgumentLoweringPlan::Shaped(shapes) => shapes.len(),
    };
    if let Some((_, signature)) = fallback_signature
        && signature.params.len() == arg_count
    {
        return ArgumentLoweringPlan::Shaped(
            signature
                .params
                .iter()
                .map(|param| param.shape.clone())
                .collect(),
        );
    }
    plan
}

fn static_owner_receiver_is_emitted_context(
    receiver: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    receiver.is_some_and(|receiver| {
        fallback_receiver_is_static_owner_expr(receiver, env, ctx)
            && static_owner_expr_candidates(receiver, env)
                .iter()
                .any(|owner| static_context_owner_is_emitted(owner, ctx))
    })
}

fn static_owner_receiver_is_non_program_emitted_context(
    receiver: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    receiver.is_some_and(|receiver| {
        fallback_receiver_is_static_owner_expr(receiver, env, ctx)
            && static_owner_expr_candidates(receiver, env)
                .iter()
                .any(|owner| {
                    static_context_owner_is_emitted(owner, ctx)
                        && find_context_type_by_key(ctx.program, owner).is_none()
                })
    })
}

fn static_owner_expr_candidates(receiver: &Expr, env: &ShapeEnv) -> Vec<String> {
    let mut owners = Vec::new();
    let Expr::Var(receiver_name) = receiver else {
        return owners;
    };
    if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
        push_unique_receiver_type_name(&mut owners, alias.to_string());
    }
    push_unique_receiver_type_name(&mut owners, receiver_name.clone());
    owners
}

#[derive(Debug, Clone)]
struct ContextFallbackMethodSignature {
    rust_name: String,
    arg_shapes: Vec<RustSlotShape>,
}

fn source_context_param_shapes(method: &MethodDecl, ctx: LowerCtx) -> Vec<RustSlotShape> {
    method
        .params
        .iter()
        .map(|param| source_context_param_shape(param, ctx))
        .collect()
}

fn source_context_param_shape(param: &Param, ctx: LowerCtx) -> RustSlotShape {
    crate::shape_projection::param_shape_for_java_type(
        ctx.representations,
        param.ty.clone(),
        param.is_varargs,
    )
}

fn source_context_trait_receiver_param_shape(
    param: &Param,
    ctx: LowerCtx,
    receiver_uses_dyn_trait_slots: bool,
) -> RustSlotShape {
    let shape = source_context_param_shape(param, ctx);
    if receiver_uses_dyn_trait_slots
        && source_context_reference_param_should_borrow(&param.ty, param.is_varargs)
    {
        apply_signature_constraints(
            shape,
            &[SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            )],
        )
    } else {
        shape
    }
}

fn source_context_reference_param_should_borrow(java_ty: &JavaType, is_varargs: bool) -> bool {
    !is_varargs
        && matches!(
            java_ty,
            JavaType::Named { .. } | JavaType::Unknown(_) | JavaType::Wildcard(_)
        )
}

fn apply_context_fallback_signature_arg_shapes(
    plan: ArgumentLoweringPlan,
    fallback_signature: Option<&ContextFallbackMethodSignature>,
) -> ArgumentLoweringPlan {
    let arg_count = match &plan {
        ArgumentLoweringPlan::Unshaped { arg_count } => *arg_count,
        ArgumentLoweringPlan::Shaped(shapes) => shapes.len(),
    };
    if let Some(signature) = fallback_signature
        && signature.arg_shapes.len() == arg_count
    {
        return ArgumentLoweringPlan::Shaped(signature.arg_shapes.clone());
    }
    plan
}

fn apply_static_fallback_reference_arg_shapes(
    plan: ArgumentLoweringPlan,
    receiver: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    fallback_signature: Option<&(String, &crate::signature::MethodSignaturePlan)>,
) -> ArgumentLoweringPlan {
    let Some((_, signature)) = fallback_signature else {
        return plan;
    };
    let Some(Expr::Var(receiver_name)) = receiver else {
        return plan;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return plan;
    }
    if static_owner_receiver_is_emitted_context(receiver, env, ctx)
        || static_context_owner_is_emitted(receiver_name, ctx)
        || static_owner_receiver_is_emitted_program_type(receiver, env, ctx)
    {
        return plan;
    }
    let arg_count = match &plan {
        ArgumentLoweringPlan::Unshaped { arg_count } => *arg_count,
        ArgumentLoweringPlan::Shaped(shapes) => shapes.len(),
    };
    if signature.params.len() != arg_count {
        return plan;
    }
    ArgumentLoweringPlan::Shaped(
        signature
            .params
            .iter()
            .map(|param| {
                let mut shape = param.shape.clone();
                if matches!(shape.ownership, OwnershipShape::Owned)
                    && source_context_reference_param_should_borrow(
                        &param.java_ty,
                        param.is_varargs,
                    )
                {
                    shape.ownership = OwnershipShape::SharedBorrow;
                }
                shape
            })
            .collect(),
    )
}

fn static_owner_receiver_is_emitted_program_type(
    receiver: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Some(receiver) = receiver else {
        return false;
    };
    static_owner_expr_candidates(receiver, env)
        .iter()
        .any(|owner| ctx.program.types.iter().any(|ty| ty.name == *owner))
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
                let lowered = lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx);
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

fn plan_constructor_arguments_with_signature_fallback(
    resolved: Option<&ResolvedConstructor>,
    arg_count: usize,
    signatures: &crate::signature::SignaturePlan,
    constructed_ty: &JavaType,
    rust_name: &str,
    facts: &FactSet,
) -> ArgumentLoweringPlan {
    let plan = plan_constructor_arguments(resolved, arg_count, signatures);
    let ArgumentLoweringPlan::Unshaped { arg_count } = plan else {
        return plan;
    };
    let Some(type_name) = concrete_java_type_name(constructed_ty) else {
        return ArgumentLoweringPlan::Unshaped { arg_count };
    };
    if let Some(shapes) =
        external_constructor_arg_shapes(constructed_ty, rust_name, arg_count, facts)
    {
        return ArgumentLoweringPlan::Shaped(shapes);
    }
    let Some(signature) = signatures.constructor_signature_by_rust_name(type_name, rust_name)
    else {
        return ArgumentLoweringPlan::Unshaped { arg_count };
    };
    if signature.params.len() != arg_count {
        return ArgumentLoweringPlan::Unshaped { arg_count };
    }
    ArgumentLoweringPlan::Shaped(
        signature
            .params
            .iter()
            .map(|param| param.shape.clone())
            .collect(),
    )
}

fn lower_unshaped_argument(arg: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    if let Some(callback) = lower_functional_cast_callback_unshaped(arg, env, ctx) {
        return callback;
    }
    if let Some(current_self) = lower_current_self_cast_unshaped_argument(arg, ctx) {
        return current_self;
    }
    let mut lowered = lower_expr(arg, env, ctx);
    if slot_expr_needs_result_propagation(arg, env, ctx) {
        lowered = propagate_result_expr(lowered);
    }
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
        return RustExpr::Raw(render_audited_clone_recovery(&emit::emit_expr(&lowered)));
    }
    if reused_owned_argument_needs_clone(arg, &shape, Some(&shape), ctx) {
        return RustExpr::Raw(render_audited_clone_recovery(&emit::emit_expr(&lowered)));
    }
    lowered
}

fn lower_current_self_cast_unshaped_argument(arg: &Expr, ctx: LowerCtx) -> Option<RustExpr> {
    let Expr::Cast { ty, expr } = arg else {
        return None;
    };
    if !cast_inner_is_current_self(expr, ty, ctx) {
        return None;
    }
    if matches!(
        ctx.current_receiver,
        Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
    ) {
        return Some(RustExpr::Raw(
            "self.clone()/* TODO(translation): validate added clone */".to_string(),
        ));
    }
    Some(this_expr())
}

fn lower_current_self_cast_for_slot(expr: &Expr, slot: &Slot, ctx: LowerCtx) -> Option<RustExpr> {
    let current_self_ty;
    let is_bare_this = matches!(expr, Expr::This);
    let ty = match expr {
        Expr::This => {
            let current_type = ctx.current_type?;
            current_self_ty = JavaType::Named {
                name: current_type.to_string(),
                args: Vec::new(),
            };
            &current_self_ty
        }
        Expr::Cast { ty, expr: inner } if cast_inner_is_current_self(inner, ty, ctx) => ty,
        _ => return None,
    };
    let target = slot_target_shape(slot)?;
    let target_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    let same_type = same_concrete_java_type(&target_ty, ty);
    let implemented_target = is_bare_this && current_type_implements_target(&target_ty, ctx);
    if !same_type && !implemented_target {
        return None;
    }
    if target.ownership == OwnershipShape::Owned
        && matches!(
            ctx.current_receiver,
            Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
        )
    {
        if implemented_target && matches!(target.dispatch, DispatchShape::ErasedObject) {
            return Some(RustExpr::Raw(
                "Box::new(self.clone()/* TODO(translation): validate added clone */) as Box<dyn std::any::Any>"
                    .to_string(),
            ));
        }
        if implemented_target && let DispatchShape::BoxedDynTrait { trait_name } = &target.dispatch
        {
            return Some(RustExpr::Raw(format!(
                "Box::new(self.clone()/* TODO(translation): validate added clone */) as Box<dyn {trait_name}>"
            )));
        }
        return Some(RustExpr::Raw(
            "self.clone()/* TODO(translation): validate added clone */".to_string(),
        ));
    }
    Some(this_expr())
}

fn current_type_implements_target(target_ty: &JavaType, ctx: LowerCtx) -> bool {
    let Some(current_type) = ctx.current_type else {
        return false;
    };
    find_type_decl_by_name(ctx.support_local_type_program, current_type).is_some_and(|ty| {
        ty.implements.iter().any(|implemented| {
            same_concrete_java_type_name(target_ty, implemented)
                || same_concrete_java_type(
                    target_ty,
                    &crate::parser::parse_hierarchy_type(implemented),
                )
        })
    })
}

fn find_type_decl_by_name<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_by_name_in_type(ty, type_name))
}

fn find_type_decl_by_name_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if ty.name == type_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_by_name_in_type(nested, type_name),
        _ => None,
    })
}

fn exact_erased_object_cast_initializer_local_shape(
    initializer: Option<&Expr>,
    refinements: &ExprRefinements,
) -> Option<RustSlotShape> {
    let (cast_ty, local_name) = exact_erased_object_cast_parts(initializer?)?;
    let exact_ty = refinements.exact_erased_object_locals.get(local_name)?;
    if !same_concrete_java_type(cast_ty, exact_ty) {
        return None;
    }
    let mut shape = RustSlotShape::conservative(cast_ty.clone());
    shape.ownership = OwnershipShape::SharedBorrow;
    Some(shape)
}

fn lower_exact_erased_object_cast_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let (cast_ty, local_name) = exact_erased_object_cast_parts(expr)?;
    let exact_ty = refinements.exact_erased_object_locals.get(local_name)?;
    if !same_concrete_java_type(cast_ty, exact_ty) {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if target.ownership != OwnershipShape::SharedBorrow
        || !same_concrete_java_type(cast_ty, &target.java_ty)
    {
        return None;
    }
    let rust_type =
        crate::slot_render::rust_name_for_java_named_type(concrete_java_type_name(cast_ty)?);
    let object_expr = if exact_erased_object_local_is_optional(local_name, env, ctx) {
        format!(
            "{local_name}.as_ref().and_then(|value| value.downcast_ref::<{rust_type}>()).expect(\"invalid Java cast\")"
        )
    } else {
        format!("{local_name}.downcast_ref::<{rust_type}>().expect(\"invalid Java cast\")")
    };
    Some(RustExpr::Raw(object_expr))
}

fn exact_erased_object_cast_parts(expr: &Expr) -> Option<(&JavaType, &str)> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let Expr::Var(local_name) = inner.as_ref() else {
        return None;
    };
    Some((ty, local_name))
}

fn exact_erased_object_local_is_optional(local_name: &str, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    env.local_shape(local_name)
        .or_else(|| {
            ctx.current_local_shapes
                .iter()
                .find(|local| local.name == local_name)
                .map(|local| &local.shape)
        })
        .is_some_and(|shape| shape.optional_element_type().is_some())
}

fn lower_functional_cast_callback_unshaped(
    arg: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = arg else {
        return None;
    };
    if !java_type_is_known_functional_interface(ty) {
        return None;
    }
    match inner.as_ref() {
        Expr::Lambda { params, body } => Some(lower_callback_lambda_expr(params, body, env, ctx)),
        Expr::MethodReference { target, method } => {
            let plan = plan_method_reference(target, method);
            Some(render_method_reference(&plan))
        }
        _ => None,
    }
}

fn lower_args_for_call_target(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
) -> Vec<RustExpr> {
    if args.len() == 1
        && matches!(args.first(), Some(Expr::Lambda { .. }))
        && let Some(lowered) = lower_unshaped_argument_for_call_target(&args[0], target, env, ctx)
            .or_else(|| {
                call_receiver.and_then(|receiver| {
                    lower_unshaped_argument_for_receiver(&args[0], receiver, env, ctx)
                })
            })
    {
        return vec![lowered];
    }
    if let Some(packed) = fallback_signature
        .and_then(|signature| lower_varargs_call_args_with_signature(args, env, signature, ctx))
    {
        return packed;
    }
    if let Some(static_signature_plan) = static_call_target_signature_arg_plan(target, args, ctx) {
        return lower_args_with_plan(args, env, static_signature_plan, ctx);
    }
    if let Some(packed) = lower_resolved_varargs_call_args(args, env, resolved, ctx) {
        return packed;
    }
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
        _ if external_generic_facade_target_keeps_argument_shapes(target, env, ctx) => {
            lower_external_generic_facade_args_with_actual_shapes(args, env, plan, ctx)
        }
        _ if matches!(plan, ArgumentLoweringPlan::Unshaped { .. })
            && external_facade_preserving_call_clones_leading_arg(target) =>
        {
            lower_unshaped_external_facade_preserving_args(target, call_receiver, args, env, ctx)
        }
        _ if matches!(plan, ArgumentLoweringPlan::Unshaped { .. }) => args
            .iter()
            .map(|arg| {
                lower_unshaped_argument_for_call_target(arg, target, env, ctx)
                    .or_else(|| {
                        call_receiver.and_then(|receiver| {
                            lower_unshaped_argument_for_receiver(arg, receiver, env, ctx)
                        })
                    })
                    .unwrap_or_else(|| lower_unshaped_argument(arg, env, ctx))
            })
            .collect(),
        _ => lower_args_with_plan(args, env, plan, ctx),
    }
}

fn static_call_target_signature_arg_plan(
    target: &CallTargetPlan<'_>,
    args: &[Expr],
    ctx: LowerCtx,
) -> Option<ArgumentLoweringPlan> {
    let CallTargetPlan::StaticOwner {
        owner, rust_name, ..
    } = target
    else {
        return None;
    };
    if find_context_type_by_key(ctx.program, owner).is_none()
        && !(ctx.context_sources_are_emitted
            && find_context_type_by_key(ctx.support_local_type_program, owner).is_some())
    {
        return None;
    }
    let signature = ctx
        .signatures
        .method_signature_by_rust_name(owner, rust_name)?;
    if signature.params.len() != args.len() {
        return None;
    }
    Some(ArgumentLoweringPlan::Shaped(
        signature
            .params
            .iter()
            .map(|param| param.shape.clone())
            .collect(),
    ))
}

fn external_facade_preserving_call_clones_leading_arg(target: &CallTargetPlan<'_>) -> bool {
    let rust_name = match target {
        CallTargetPlan::InstanceReceiver { rust_name, .. }
        | CallTargetPlan::InstanceSelf { rust_name }
        | CallTargetPlan::StaticSelf { rust_name } => *rust_name,
        CallTargetPlan::FallbackReceiver { rust_name, .. }
        | CallTargetPlan::FallbackSelf { rust_name }
        | CallTargetPlan::FallbackFree { rust_name }
        | CallTargetPlan::StaticOwner { rust_name, .. } => rust_name,
        CallTargetPlan::ExternalFacadeReceiver { rust_path, .. } => {
            rust_path.rsplit("::").next().unwrap_or(rust_path)
        }
        _ => return false,
    };
    rust_name == "sort_by_key"
        || rust_name
            .strip_prefix("sort_by_key_")
            .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit()))
}

fn lower_unshaped_external_facade_preserving_args(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    args.iter()
        .enumerate()
        .map(|(index, arg)| {
            let lowered = lower_unshaped_argument_for_call_target(arg, target, env, ctx)
                .or_else(|| {
                    call_receiver.and_then(|receiver| {
                        lower_unshaped_argument_for_receiver(arg, receiver, env, ctx)
                    })
                })
                .unwrap_or_else(|| lower_unshaped_argument(arg, env, ctx));
            if index == 0 && external_facade_preserving_arg_needs_clone(arg, env, ctx) {
                let rendered = emit::emit_expr(&lowered);
                if rendered.ends_with(".clone()")
                    || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
                {
                    lowered
                } else {
                    RustExpr::Raw(render_audited_clone_recovery(&rendered))
                }
            } else {
                lowered
            }
        })
        .collect()
}

fn external_facade_preserving_arg_needs_clone(arg: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    if !matches!(arg, Expr::Var(_)) {
        return false;
    }
    let shape = infer_expr_shape_with_context(
        arg,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    !matches!(shape.java_ty, JavaType::Primitive(_))
        && known_owned_argument_clone_recovery_with_facts(&shape, ctx.facts)
}

fn lower_varargs_call_args_with_signature(
    args: &[Expr],
    env: &ShapeEnv,
    signature: &crate::signature::MethodSignaturePlan,
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let varargs_param = signature.params.last()?;
    if !varargs_param.is_varargs {
        return None;
    }
    let fixed_count = signature.params.len().saturating_sub(1);
    if args.len() < fixed_count {
        return None;
    }
    if args.len() == signature.params.len() && last_arg_is_explicit_varargs_array(args, env, ctx) {
        return None;
    }
    lower_varargs_call_args_from_params(args, env, &signature.params, ctx)
}

fn lower_resolved_varargs_call_args(
    args: &[Expr],
    env: &ShapeEnv,
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let resolved = resolved?;
    let signature = ctx
        .signatures
        .method_signature_by_rust_name(&resolved.declaring_type, &resolved.rust_name)?;
    let varargs_param = signature.params.last()?;
    if !varargs_param.is_varargs {
        return None;
    }
    let fixed_count = signature.params.len().saturating_sub(1);
    if args.len() < fixed_count {
        return None;
    }
    if args.len() == signature.params.len() && last_arg_is_explicit_varargs_array(args, env, ctx) {
        return None;
    }
    lower_varargs_call_args_from_params(args, env, &signature.params, ctx)
}

fn lower_varargs_call_args_from_params(
    args: &[Expr],
    env: &ShapeEnv,
    params: &[crate::signature::ParamPlan],
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let varargs_param = params.last()?;
    let element_ty = crate::types::sequence_element_type(&varargs_param.java_ty)
        .or_else(|| varargs_param.shape.sequence_element_type())?;
    let element_shape = value_shape(element_ty);
    let fixed_count = params.len().saturating_sub(1);
    let mut lowered = Vec::with_capacity(params.len());
    for (arg, param) in args.iter().take(fixed_count).zip(params) {
        let lowered_arg = lower_expr_for_slot(arg, env, argument_slot(param.shape.clone()), ctx);
        let actual_shape = infer_expr_shape_with_context(
            arg,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
        );
        lowered.push(adapt_reused_owned_argument(
            lowered_arg,
            arg,
            &param.shape,
            Some(&actual_shape),
            ctx,
        ));
    }
    let varargs = args
        .iter()
        .skip(fixed_count)
        .map(|arg| emit::emit_expr(&lower_varargs_element_expr(arg, env, &element_shape, ctx)))
        .collect::<Vec<_>>();
    let packed_vec = if varargs.is_empty() {
        "Vec::new()".to_string()
    } else {
        format!("vec![{}]", varargs.join(", "))
    };
    let packed = if varargs_param.shape.ownership == OwnershipShape::SharedBorrow {
        format!("&{packed_vec}")
    } else {
        packed_vec
    };
    lowered.push(RustExpr::Raw(packed));
    Some(lowered)
}

fn lower_varargs_element_expr(
    arg: &Expr,
    env: &ShapeEnv,
    element_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> RustExpr {
    let slot = Slot::Argument(element_shape.clone());
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let actual_shape = actual_shape_for_slot_expr_with_slot(arg, env, shape_ctx, &slot);
    let plan = plan_slot_conversion(actual_shape.clone(), slot, ctx.facts);
    let lowered = lower_expr(arg, env, ctx);
    if actual_shape.ownership == OwnershipShape::SharedBorrow
        && shape_is_owned_non_erased(element_shape)
        && known_owned_argument_clone_recovery_with_facts(&actual_shape, ctx.facts)
        && slot_conversion_plan_contains_external_into(&plan.conversion)
    {
        let cloned = RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&lowered)));
        return apply_slot_expr_conversion(cloned, &plan);
    }
    apply_slot_expr_conversion(lowered, &plan)
}

fn last_arg_is_explicit_varargs_array(args: &[Expr], env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Some(last) = args.last() else {
        return false;
    };
    let shape = infer_expr_shape_with_context(
        last,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    shape.sequence_element_type().is_some()
}

fn lower_unshaped_argument_for_call_target(
    arg: &Expr,
    target: &CallTargetPlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if let Some(element_shape) = functional_cast_single_input_shape(arg) {
        return lower_single_input_callback_arg(arg, element_shape, env, ctx);
    }
    let element_shape = call_target_single_callback_input_shape(target, env, ctx)?;
    lower_single_input_callback_arg(arg, element_shape, env, ctx)
}

fn functional_cast_single_input_shape(expr: &Expr) -> Option<RustSlotShape> {
    let Expr::Cast { ty, .. } = expr else {
        return None;
    };
    if !java_type_is_known_functional_interface(ty) {
        return None;
    }
    functional_interface_single_input_shape(ty)
}

fn call_target_single_callback_input_shape(
    target: &CallTargetPlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let receiver = target.receiver_expr()?;
    let element_shape = stream_like_receiver_element_shape(receiver, env, ctx)?;
    if call_target_is_map_partitions(target) {
        return Some(fallback_local_shape_from_declared_type(&JavaType::Named {
            name: "Iterator".to_string(),
            args: vec![element_shape.java_ty],
        }));
    }
    call_target_is_element_callback(target).then_some(element_shape)
}

fn call_target_is_element_callback(target: &CallTargetPlan<'_>) -> bool {
    call_target_rust_method_name(target).is_some_and(|rust_name| {
        matches!(
            rust_name,
            "map" | "map_to_pair" | "filter" | "flat_map" | "map_partitions"
        )
    })
}

fn call_target_is_map_partitions(target: &CallTargetPlan<'_>) -> bool {
    call_target_rust_method_name(target).is_some_and(|rust_name| {
        rust_name == "map_partitions"
            || rust_name == "map_partitions_to_pair"
            || rust_name
                .strip_prefix("map_partitions_")
                .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit()))
            || rust_name
                .strip_prefix("map_partitions_to_pair_")
                .is_some_and(|suffix| suffix.chars().all(|ch| ch.is_ascii_digit()))
    })
}

fn lower_unshaped_argument_for_receiver(
    arg: &Expr,
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let element_shape = stream_like_receiver_element_shape(receiver, env, ctx)?;
    lower_single_input_callback_arg(arg, element_shape, env, ctx)
}

fn lower_single_input_callback_arg(
    arg: &Expr,
    element_shape: RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    match arg {
        Expr::Lambda { params, body } if params.len() == 1 => {
            Some(lower_callback_lambda_expr_with_shapes(
                params,
                body,
                env,
                ctx,
                None,
                false,
                Some(&[element_shape]),
            ))
        }
        Expr::MethodReference { target, method } => {
            lower_typed_single_input_method_reference(target, method, &element_shape, ctx)
        }
        Expr::Cast { ty, expr } if java_type_is_known_functional_interface(ty) => {
            let cast_shape = functional_interface_single_input_shape(ty).unwrap_or(element_shape);
            lower_single_input_callback_arg(expr, cast_shape, env, ctx)
        }
        _ => None,
    }
}

fn functional_interface_single_input_shape(ty: &JavaType) -> Option<RustSlotShape> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    let simple = crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .unwrap_or(name.as_str())
        .to_string();
    if simple == "FlatMapFunction"
        && let (
            Some(JavaType::Named {
                name: first,
                args: first_args,
            }),
            Some(return_ty),
        ) = (args.first(), args.get(1))
        && first_args.is_empty()
        && crate::stdlib::canonical_external_receiver_type_key(first)
            .rsplit('.')
            .next()
            .is_some_and(|name| name == "Iterator")
    {
        return Some(fallback_local_shape_from_declared_type(&JavaType::Named {
            name: first.clone(),
            args: vec![return_ty.clone()],
        }));
    }
    args.first().map(fallback_local_shape_from_declared_type)
}

fn lower_functional_interface_local_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(callback_name) = receiver? else {
        return None;
    };
    let shape = env.local_shape(callback_name)?;
    let input_shapes =
        functional_interface_input_shapes_for_method(&shape.java_ty, name, args.len())?;
    let lowered_args = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let lowered = input_shapes
                .get(index)
                .cloned()
                .map(|shape| lower_expr_for_slot(arg, env, argument_slot(shape), ctx))
                .unwrap_or_else(|| lower_expr(arg, env, ctx));
            preserve_local_callback_argument(arg, lowered, env, ctx)
        })
        .collect();
    Some(RustExpr::Call {
        function: Box::new(RustExpr::Var(callback_name.clone())),
        args: lowered_args,
    })
}

fn preserve_local_callback_argument(
    arg: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let Expr::Var(name) = arg else {
        return lowered;
    };
    let Some(shape) = env.local_shape(name) else {
        return lowered;
    };
    if ctx.facts.is_copy_type(&shape.java_ty) {
        return lowered;
    }
    let rendered = emit::emit_expr(&lowered);
    if rendered.ends_with(".clone()")
        || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
    {
        lowered
    } else {
        RustExpr::Raw(render_audited_clone_recovery(&rendered))
    }
}

fn functional_interface_input_shapes_for_method(
    ty: &JavaType,
    method: &str,
    arity: usize,
) -> Option<Vec<RustSlotShape>> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    let simple = crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .unwrap_or(name.as_str())
        .to_string();
    let input_count = match (simple.as_str(), method) {
        ("Supplier", "get") | ("Runnable", "run") | ("Callable", "call") => 0,
        (
            "Consumer" | "Function" | "Predicate" | "UnaryOperator" | "DoubleConsumer"
            | "IntConsumer" | "LongConsumer",
            "accept" | "apply" | "test" | "applyAsDouble" | "applyAsInt" | "applyAsLong",
        ) => 1,
        ("BiConsumer" | "BiFunction" | "BinaryOperator", "accept" | "apply") => 2,
        _ => return None,
    };
    if input_count != arity || args.len() < input_count {
        return None;
    }
    Some(
        args.iter()
            .take(input_count)
            .map(fallback_local_shape_from_declared_type)
            .collect(),
    )
}

fn lower_typed_single_input_method_reference(
    target: &str,
    method: &str,
    element_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let plan = plan_method_reference(target, method);
    let param_ty = render_callback_param_type(element_shape, ctx);
    match plan {
        MethodReferencePlan::AssociatedFunction { owner, method }
            if owner == "Objects" && method == "non_null" =>
        {
            Some(RustExpr::Raw(format!(
                "|value: {param_ty}| Objects::non_null(value)"
            )))
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            let param_ty = render_unbound_accessor_callback_param_type(element_shape, ctx);
            Some(RustExpr::Raw(format!(
                "|value: {param_ty}| value.{method}()"
            )))
        }
        _ => None,
    }
}

fn render_unbound_accessor_callback_param_type(shape: &RustSlotShape, ctx: LowerCtx) -> String {
    if let JavaType::Named { name, .. } = &shape.java_ty {
        let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
        if canonical
            .rsplit('.')
            .next()
            .is_some_and(|simple| matches!(simple, "Tuple2" | "Tuple3" | "Tuple4"))
        {
            return canonical
                .rsplit('.')
                .next()
                .unwrap_or(name.as_str())
                .to_string();
        }
    }
    render_callback_param_type(shape, ctx)
}

fn call_target_rust_method_name<'a>(target: &'a CallTargetPlan<'a>) -> Option<&'a str> {
    match target {
        CallTargetPlan::InstanceReceiver { rust_name, .. }
        | CallTargetPlan::InstanceSelf { rust_name }
        | CallTargetPlan::StaticSelf { rust_name } => Some(*rust_name),
        CallTargetPlan::FallbackReceiver { rust_name, .. }
        | CallTargetPlan::FallbackSelf { rust_name }
        | CallTargetPlan::FallbackFree { rust_name }
        | CallTargetPlan::StaticOwner { rust_name, .. } => Some(rust_name.as_str()),
        CallTargetPlan::ExternalFacadeReceiver { rust_path, .. } => {
            Some(rust_path.rsplit("::").next().unwrap_or(rust_path))
        }
        _ => None,
    }
}

fn stream_like_receiver_element_shape(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    if matches!(receiver, Expr::Call { name, .. } if name == "filter")
        && let Some(receiver_shape) = stream_like_call_return_shape(receiver, env, ctx)
        && let Some(element_ty) = receiver_shape.sequence_element_type()
    {
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if let Some(element_ty) = receiver_shape.sequence_element_type() {
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    if let Some(mapped_shape) = stream_like_map_result_element_shape(receiver, env, ctx) {
        return Some(mapped_shape);
    }
    let receiver_java_ty = crate::types::infer_expr_type(receiver, env.type_env());
    if let Some(element_ty) = crate::types::sequence_element_type(&receiver_java_ty) {
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    if let Some(return_ty) = external_receiver_call_return_type(receiver, ctx.facts)
        && let Some(element_ty) = crate::types::sequence_element_type(&return_ty)
    {
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    if let Expr::Call {
        receiver: call_receiver,
        name,
        args,
        ..
    } = receiver
        && let Some(lowered) = external_call_lowered_type_with_lower_context(
            call_receiver.as_deref(),
            name,
            args.len(),
            ctx,
            env,
            ctx.current_type,
        )
        && let Some(java_ty) = lowered.java_ty()
        && let Some(element_ty) = crate::types::sequence_element_type(java_ty)
    {
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    if let Expr::Call {
        receiver: Some(source),
        name,
        args,
        ..
    } = receiver
        && matches!(
            name.as_str(),
            "stream" | "parallelStream" | "parallel_stream"
        )
        && args.is_empty()
    {
        let source_shape = infer_expr_shape_with_context(source, env, shape_ctx);
        let element_ty = source_shape.sequence_element_type().or_else(|| {
            source_shape
                .optional_element_type()
                .as_ref()
                .and_then(crate::types::sequence_element_type)
        })?;
        return Some(fallback_local_shape_from_declared_type(&element_ty));
    }
    None
}

fn stream_like_call_return_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if args.len() != 1 {
        return None;
    }
    if name == "filter" {
        return Some(infer_expr_shape_with_context(
            receiver,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
        ));
    }
    let element_shape = stream_like_map_result_element_shape(expr, env, ctx)?;
    match name.as_str() {
        "map" => Some(RustSlotShape::for_storage(JavaType::Named {
            name: "JavaRDD".to_string(),
            args: vec![element_shape.java_ty],
        })),
        "mapToPair" => tuple2_type_args(&element_shape.java_ty).map(|args| {
            RustSlotShape::for_storage(JavaType::Named {
                name: "JavaPairRDD".to_string(),
                args,
            })
        }),
        _ => None,
    }
}

fn stream_like_map_result_element_shape(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: Some(map_receiver),
        name,
        args,
        ..
    } = receiver
    else {
        return None;
    };
    if !matches!(name.as_str(), "map" | "mapToPair") || args.len() != 1 {
        return None;
    }
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };
    if params.len() != 1 {
        return None;
    }
    let input_shape = stream_like_receiver_element_shape(map_receiver, env, ctx)?;
    let mut lambda_env = env.clone();
    lambda_env.insert_local_shape(params[0].name.clone(), input_shape);
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    match body {
        LambdaBody::Expr(expr) => tuple2_constructor_result_shape(expr, &lambda_env, ctx)
            .or_else(|| external_call_expr_result_shape(expr, &lambda_env, ctx))
            .or_else(|| Some(infer_expr_shape_with_context(expr, &lambda_env, shape_ctx))),
        LambdaBody::Block(_) => None,
    }
}

fn tuple2_constructor_result_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let args = match expr {
        Expr::New { ty, args, .. } => {
            let tuple_name = match ty {
                JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
                _ => return None,
            };
            if !java_type_name_is_tuple2(tuple_name) {
                return None;
            }
            args
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if matches!(name.as_str(), "new" | "of" | "apply") && expr_is_tuple2_owner(receiver) => {
            args
        }
        _ => return None,
    };
    if args.len() != 2 {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let tuple_args = args
        .iter()
        .map(|arg| {
            if matches!(arg, Expr::Literal(Literal::Null)) {
                JavaType::Void
            } else {
                infer_expr_shape_with_context(arg, env, shape_ctx).java_ty
            }
        })
        .collect::<Vec<_>>();
    Some(RustSlotShape::for_storage(JavaType::Named {
        name: "Tuple2".to_string(),
        args: tuple_args,
    }))
}

fn expr_is_tuple2_owner(expr: &Expr) -> bool {
    match expr {
        Expr::Var(name) => java_type_name_is_tuple2(name),
        Expr::FieldAccess(receiver, field) => expr_is_tuple2_owner(receiver) || field == "Tuple2",
        _ => false,
    }
}

fn tuple2_type_args(java_ty: &JavaType) -> Option<Vec<JavaType>> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    if java_type_name_is_tuple2(name) && args.len() == 2 {
        Some(args.clone())
    } else {
        None
    }
}

fn java_type_name_is_tuple2(name: &str) -> bool {
    crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        == Some("Tuple2")
}

fn external_call_expr_result_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    let lowered = external_call_lowered_type_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    )
    .or_else(|| {
        external_call_lowered_type_from_planned_static_owner(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            env,
            ctx,
        )
    })?;
    lowered
        .java_ty()
        .map(fallback_local_shape_from_declared_type)
}

fn external_call_lowered_type_with_lower_context(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    ctx: LowerCtx<'_>,
    env: &ShapeEnv,
    current_type: Option<&str>,
) -> Option<LoweredType> {
    external_call_lowered_type_with_external_index(
        receiver,
        name,
        arg_count,
        ctx.facts,
        ctx.external_index,
        env.type_env(),
        current_type,
    )
}

fn external_call_returns_rust_result_with_lower_context(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    ctx: LowerCtx<'_>,
    env: &ShapeEnv,
    current_type: Option<&str>,
) -> bool {
    external_call_returns_rust_result_with_external_index(
        receiver,
        name,
        arg_count,
        ctx.facts,
        ctx.external_index,
        env.type_env(),
        current_type,
    )
}

fn plan_call_exception_propagation_with_lower_context(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx<'_>,
    env: &ShapeEnv,
    current_type: Option<&str>,
) -> CallExceptionPropagationPlan {
    plan_call_exception_propagation_with_external_index(
        receiver,
        name,
        args,
        resolved,
        ctx.facts,
        ctx.external_index,
        env.type_env(),
        current_type,
    )
}

fn external_call_lowered_type_from_planned_static_owner(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<LoweredType> {
    let plan = plan_call_expr_with_lower_context(receiver, name, args, resolved, ctx, env);
    let CallExprPlan::Target(CallTargetPlan::StaticOwner { owner, .. }) = plan else {
        return None;
    };
    static_owner_result_type_candidates(receiver, &owner, env)
        .into_iter()
        .find_map(|owner| {
            ctx.facts
                .external_api_effect_for_receiver_with_arity(Some(&owner), name, Some(args.len()))
                .cloned()
                .or_else(|| {
                    crate::external_packs::external_api_effect_for_receiver_with_arity(
                        Some(&owner),
                        name,
                        Some(args.len()),
                    )
                })
                .and_then(|effect| external_effect_lowered_type(effect))
                .or_else(|| {
                    crate::stdlib::known_external_method_return_type(Some(&owner), name, args.len())
                        .map(LoweredType::from_java)
                })
        })
}

fn external_effect_lowered_type(
    effect: crate::facts::ExternalApiEffectFact,
) -> Option<LoweredType> {
    let return_ty = effect.return_type?;
    match effect.return_nullability {
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
            if !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            Some(LoweredType::from_java(crate::types::optional_java_type(
                return_ty,
            )))
        }
        _ => Some(LoweredType::from_java(return_ty)),
    }
}

fn static_owner_result_type_candidates(
    receiver: Option<&Expr>,
    planned_owner: &str,
    env: &ShapeEnv,
) -> Vec<String> {
    let mut owners = Vec::new();
    if let Some(Expr::Var(receiver_name)) = receiver {
        if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
            owners.push(alias.to_string());
            owners.push(crate::stdlib::canonical_external_receiver_type_key(alias));
        }
        owners.push(receiver_name.clone());
        owners.push(crate::stdlib::canonical_external_receiver_type_key(
            receiver_name,
        ));
    }
    owners.push(planned_owner.to_string());
    owners.push(crate::stdlib::canonical_external_receiver_type_key(
        planned_owner,
    ));
    owners.dedup();
    owners
}

fn external_receiver_call_return_type(receiver: &Expr, facts: &FactSet) -> Option<JavaType> {
    let Expr::Call {
        name,
        args,
        resolved,
        ..
    } = receiver
    else {
        return None;
    };
    let resolved = resolved.as_ref()?;
    let arity = Some(args.len());
    for method_name in [name.as_str(), resolved.rust_name.as_str()] {
        if let Some(effect) = facts
            .external_api_effect_for_receiver_with_arity(
                Some(&resolved.declaring_type),
                method_name,
                arity,
            )
            .cloned()
            .or_else(|| {
                crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(&resolved.declaring_type),
                    method_name,
                    arity,
                )
            })
            && let Some(return_type) = effect.return_type
        {
            return Some(return_type);
        }
    }
    None
}

fn apply_call_target_with_lower_context(
    target: &CallTargetPlan<'_>,
    lowered_receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
    ctx: LowerCtx,
) -> RustExpr {
    if ctx.current_method.is_some()
        && matches!(ctx.current_receiver, Some(ReceiverPlan::None))
        && let CallTargetPlan::FallbackSelf { rust_name } = target
    {
        return RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("Self::{rust_name}"))),
            args,
        };
    }
    apply_call_target(target, lowered_receiver, args)
}

fn retarget_native_java_string_call_from_shape<'a>(
    plan: CallExprPlan<'a>,
    receiver: Option<&'a Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> CallExprPlan<'a> {
    if matches!(
        plan,
        CallExprPlan::Target(CallTargetPlan::NativeJavaInstance { .. })
    ) {
        return plan;
    }
    let Some(receiver) = receiver else {
        return plan;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let receiver_shapes = current_instance_field_shape_with_env(receiver, env, ctx)
        .into_iter()
        .chain(std::iter::once(receiver_shape))
        .collect::<Vec<_>>();
    let receiver_is_string_like = receiver_shapes.iter().any(|shape| {
        let receiver_ty = crate::types::optional_element_type(&shape.java_ty)
            .unwrap_or_else(|| shape.java_ty.clone());
        matches!(receiver_ty, JavaType::String) || slot_shape_renders_as_string_like(shape, ctx)
    });
    if !receiver_is_string_like {
        return plan;
    }
    let method = match (name, args.len()) {
        ("length", 0) => NativeJavaInstanceMethod::StringLen,
        ("isEmpty", 0) => NativeJavaInstanceMethod::StringIsEmpty,
        ("trim", 0) => NativeJavaInstanceMethod::StringTrim,
        ("toLowerCase", 0) => NativeJavaInstanceMethod::StringToLowercase,
        ("toString", 0) => NativeJavaInstanceMethod::StringToString,
        ("contains", 1) => NativeJavaInstanceMethod::StringContains,
        ("startsWith", 1) => NativeJavaInstanceMethod::StringStartsWith,
        ("endsWith", 1) => NativeJavaInstanceMethod::StringEndsWith,
        ("split", 1 | 2) => NativeJavaInstanceMethod::StringSplit,
        ("substring", 1 | 2) => NativeJavaInstanceMethod::StringSubstring,
        ("hashCode" | "hash_code", 0) => NativeJavaInstanceMethod::StringHashCode,
        _ => return plan,
    };
    CallExprPlan::Target(CallTargetPlan::NativeJavaInstance { receiver, method })
}

fn slot_shape_renders_as_string_like(shape: &RustSlotShape, ctx: LowerCtx) -> bool {
    rust_type_is_string_like(&rust_type_for_slot_with_facts(shape, ctx.facts))
}

fn rust_type_is_string_like(ty: &RustType) -> bool {
    match ty {
        RustType::String => true,
        RustType::Option(inner) | RustType::Ref { inner, .. } => rust_type_is_string_like(inner),
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::F32
        | RustType::F64
        | RustType::Char
        | RustType::Named(_)
        | RustType::Vec(_)
        | RustType::Array2(_)
        | RustType::Slice(_)
        | RustType::Raw(_) => false,
    }
}

fn external_generic_facade_target_keeps_argument_shapes(
    target: &CallTargetPlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Some(receiver) = target.receiver_expr() else {
        return false;
    };
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    let effective_facts = crate::effective_facts::EffectiveFacts::new(ctx.facts);
    effective_facts.is_final_type(&receiver_ty)
        && crate::stdlib::java_collection_kind_for_type(&receiver_ty).is_some()
}

fn lower_external_generic_facade_args_with_actual_shapes(
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
                let original_shape = shape.clone();
                if external_generic_facade_arg_shape_is_unspecified_object(&shape) {
                    let actual_shape = infer_expr_shape_with_context(
                        arg,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    );
                    let lowered =
                        lower_expr_for_slot(arg, env, argument_slot(actual_shape.clone()), ctx);
                    adapt_reused_owned_argument(
                        lowered,
                        arg,
                        &actual_shape,
                        Some(&actual_shape),
                        ctx,
                    )
                } else {
                    let actual_shape = infer_expr_shape_with_context(
                        arg,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    );
                    let lowered = lower_expr_for_slot(arg, env, argument_slot(shape), ctx);
                    adapt_reused_owned_argument(
                        lowered,
                        arg,
                        &original_shape,
                        Some(&actual_shape),
                        ctx,
                    )
                }
            })
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => args
            .iter()
            .map(|arg| {
                let actual_shape = infer_expr_shape_with_context(
                    arg,
                    env,
                    ExprShapeContext {
                        signatures: Some(ctx.signatures),
                        representations: Some(ctx.representations),
                        facts: Some(ctx.facts),
                    },
                );
                lower_expr_for_slot(arg, env, argument_slot(actual_shape), ctx)
            })
            .collect(),
    }
}

fn external_generic_facade_arg_shape_is_unspecified_object(shape: &RustSlotShape) -> bool {
    matches!(&shape.java_ty, JavaType::Unknown(_))
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
    if shared_borrow_argument_needs_owned_clone(target_shape, actual_shape, ctx) {
        let rendered = emit::emit_expr(&lowered);
        if rendered.ends_with(".clone()")
            || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
        {
            return lowered;
        }
        return RustExpr::Raw(render_audited_clone_recovery(&rendered));
    }
    if reused_owned_argument_needs_clone(source, target_shape, actual_shape, ctx) {
        let rendered = emit::emit_expr(&lowered);
        if rendered_has_audited_clone_recovery(&rendered) {
            return lowered;
        }
        RustExpr::Raw(render_audited_clone_recovery(&rendered))
    } else {
        lowered
    }
}

fn rendered_has_audited_clone_recovery(rendered: &str) -> bool {
    rendered.ends_with(".clone()")
        || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
        || rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")
}

fn render_audited_clone_recovery(rendered: &str) -> String {
    let receiver = if rust_method_receiver_needs_parentheses(rendered) {
        format!("({rendered})")
    } else {
        rendered.to_string()
    };
    format!("{receiver}.clone()/* TODO(translation): validate added clone */")
}

fn rust_method_receiver_needs_parentheses(rendered: &str) -> bool {
    rendered.contains(" as ")
}

fn shared_borrow_argument_needs_owned_clone(
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    ctx: LowerCtx,
) -> bool {
    let Some(actual_shape) = actual_shape else {
        return false;
    };
    actual_shape.ownership == OwnershipShape::SharedBorrow
        && shape_is_owned_non_erased(target_shape)
        && target_shape.java_ty != JavaType::String
        && (known_owned_argument_clone_recovery_with_facts(target_shape, ctx.facts)
            || known_owned_argument_clone_recovery_with_facts(actual_shape, ctx.facts)
            || known_named_clone_recovery(target_shape))
}

fn reused_owned_argument_needs_clone(
    arg: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    ctx: LowerCtx,
) -> bool {
    if target_shape.optional_element_type().is_some()
        && target_shape.ownership == OwnershipShape::Owned
        && actual_shape.is_some_and(|actual_shape| actual_shape == target_shape)
    {
        return optional_owned_argument_clone_recovery(target_shape, ctx.facts)
            && matches!(arg, Expr::Var(_))
            && current_callable_binding_has_later_read(
                match arg {
                    Expr::Var(name) => name,
                    _ => unreachable!("checked above"),
                },
                ctx,
            );
    }
    if actual_shape
        .is_some_and(|actual_shape| actual_shape.ownership == OwnershipShape::SharedBorrow)
    {
        return false;
    }
    if !shape_is_owned_non_erased(target_shape) {
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
    if ctx
        .catch_future_use_names
        .is_some_and(|future_uses| future_uses.contains(name))
    {
        return true;
    }
    if ctx
        .exclusive_terminal_branch_read_names
        .is_some_and(|exclusive_reads| exclusive_reads.contains(name))
        && !current_callable_binding_has_later_read(name, ctx)
    {
        return false;
    }
    if let Some(later_reads) = ctx.later_read_names
        && let Some(current_counts) = ctx.current_stmt_read_counts
    {
        return later_reads.contains(name) || current_counts.get(name).copied().unwrap_or(0) > 1;
    }
    current_callable_binding_use_count(name, ctx).is_some_and(|use_count| use_count > 1)
}

fn known_owned_argument_clone_recovery(target_shape: &RustSlotShape) -> bool {
    matches!(target_shape.java_ty, JavaType::String)
}

fn optional_owned_argument_clone_recovery(target_shape: &RustSlotShape, facts: &FactSet) -> bool {
    let Some(element_ty) = target_shape.optional_element_type() else {
        return false;
    };
    let mut element_shape = target_shape.clone();
    element_shape.java_ty = element_ty;
    element_shape.ownership = OwnershipShape::Owned;
    known_owned_argument_clone_recovery(&element_shape)
        || known_named_clone_recovery(&element_shape)
        || known_owned_argument_clone_recovery_with_facts(&element_shape, facts)
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

fn current_callable_binding_has_later_read(name: &str, ctx: LowerCtx) -> bool {
    ctx.later_read_names
        .is_some_and(|later_reads| later_reads.contains(name))
}

fn current_callable_binding_is_returned(name: &str, ctx: LowerCtx) -> bool {
    current_callable_binding_facts(name, ctx).is_some_and(|binding| {
        binding
            .uses
            .iter()
            .any(|use_site| matches!(use_site, BindingUse::Returned))
    })
}

fn current_callable_binding_needs_clone_for_move(name: &str, ctx: LowerCtx) -> bool {
    current_callable_binding_facts(name, ctx).is_some_and(|binding| {
        binding
            .uses
            .iter()
            .any(|use_site| !matches!(use_site, BindingUse::ContentRead | BindingUse::Returned))
    })
}

fn current_callable_binding_facts<'a>(
    name: &str,
    ctx: LowerCtx<'a>,
) -> Option<&'a BindingUseFacts> {
    let current_type = ctx.current_type?;
    if let Some(current_method) = ctx.current_method {
        let exact_key = UsageMethodKey {
            type_name: current_type.to_string(),
            method_name: current_method.to_string(),
        };
        if let Some(binding) = ctx
            .usage
            .methods
            .get(&exact_key)
            .and_then(|method| method.bindings.get(name))
        {
            return Some(binding);
        }
        let rust_base = crate::parser::to_snake_case(current_method);
        let overload_prefix = format!("{rust_base}_");
        return ctx
            .usage
            .methods_by_rust_name
            .iter()
            .find(|(key, method)| {
                key.type_name == current_type
                    && (key.method_name == rust_base
                        || key.method_name.starts_with(&overload_prefix))
                    && method.bindings.contains_key(name)
            })
            .and_then(|(_, method)| method.bindings.get(name));
    }
    if let Some(current_constructor) = ctx.current_constructor {
        return ctx
            .usage
            .constructors
            .get(&UsageConstructorKey {
                type_name: current_type.to_string(),
                rust_name: current_constructor.to_string(),
            })
            .and_then(|constructor| constructor.bindings.get(name));
    }
    None
}

fn borrowed_str_slot() -> Slot {
    argument_slot(borrowed_string_content_shape())
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
        argument_slot(value_shape(JavaType::String)),
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
    refinements: &ExprRefinements,
) -> RustExpr {
    match target {
        CallTargetPlan::NativeCollectionMethod { method, .. } => {
            adapt_native_collection_receiver(receiver, lowered, *method, env, ctx)
        }
        _ if refined_non_null_receiver(receiver, refinements)
            && target.adapts_optional_instance_receiver() =>
        {
            let lowered =
                lower_expr_with_refinements(receiver, env, ctx, &ExprRefinements::default());
            adapt_optional_instance_receiver(receiver, lowered, env, ctx)
        }
        _ if refined_non_null_receiver(receiver, refinements) => lowered,
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
    refinements: &ExprRefinements,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
) -> RustExpr {
    if refined_non_null_receiver(receiver, refinements) {
        if target.adapts_optional_instance_receiver() {
            let access = if fallback_signature.is_some_and(|signature| {
                matches!(signature.receiver, crate::signature::ReceiverPlan::Mutable)
            }) || target.mutates_receiver()
            {
                "as_mut"
            } else {
                "as_ref"
            };
            let lowered =
                lower_expr_with_refinements(receiver, env, ctx, &ExprRefinements::default());
            return adapt_optional_instance_receiver_with_access(
                receiver, lowered, env, ctx, access,
            );
        }
        return lowered;
    }
    if target.adapts_optional_instance_receiver()
        && fallback_signature.is_some_and(|signature| {
            matches!(signature.receiver, crate::signature::ReceiverPlan::Mutable)
        })
    {
        return adapt_optional_instance_receiver_with_access(receiver, lowered, env, ctx, "as_mut");
    }
    if target.adapts_optional_instance_receiver() && target.mutates_receiver() {
        return adapt_optional_instance_receiver_with_access(receiver, lowered, env, ctx, "as_mut");
    }
    adapt_call_target_receiver(target, receiver, lowered, env, ctx, refinements)
}

fn refined_non_null_receiver(receiver: &Expr, refinements: &ExprRefinements) -> bool {
    matches!(receiver, Expr::Var(name) if refinements.non_null_locals.contains(name))
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
    let plan = plan_call_expr_with_lower_context(
        call_receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
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
    let inferred_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let receiver_is_optional = std::iter::once(inferred_shape)
        .chain(current_instance_field_shape_with_env(receiver, env, ctx))
        .any(|shape| shape.optional_element_type().is_some());
    if !receiver_is_optional {
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
    let target = match plan_call_expr_with_lower_context(receiver, name, args, None, ctx, env) {
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
        lower_stream_source_expr(plan.source.expr, env, ctx),
        StreamRenderContext::new_with_facts_and_self_receiver(
            env,
            ctx.facts,
            ctx.signatures,
            ctx.external_index,
            ctx.current_type,
            matches!(
                ctx.current_receiver,
                Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
            ),
        ),
    )
}

fn lower_stream_source_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    if let Some(range) = lower_int_stream_range_expr(expr, env, ctx) {
        return range;
    }
    lower_expr(expr, env, ctx)
}

fn lower_int_stream_range_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
        && args.len() == 2
        && matches!(receiver.as_ref(), Expr::Var(owner)
            if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream")
        && matches!(name.as_str(), "range" | "range_closed")
    {
        let start = emit::emit_expr(&lower_expr(&args[0], env, ctx));
        let end = emit::emit_expr(&lower_expr(&args[1], env, ctx));
        let range = if name == "range_closed" {
            format!("({start}..={end})")
        } else {
            format!("({start}..{end})")
        };
        return Some(RustExpr::Raw(range));
    }
    None
}

fn typed_none_for_java_type(ty: &JavaType, facts: &FactSet) -> RustExpr {
    let rust_ty = emit::emit_type(&rust_type_for_java_type_with_facts(ty, facts));
    RustExpr::Raw(format!("None::<{rust_ty}>"))
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
    if let Some(callback_expr) =
        lower_functional_cast_callback_for_slot(expr, env, &slot, ctx, refinements)
    {
        return callback_expr;
    }
    if let Some(current_self) = lower_current_self_cast_for_slot(expr, &slot, ctx) {
        return current_self;
    }
    if let Some(exact_erased_object) =
        lower_exact_erased_object_cast_for_slot(expr, env, &slot, ctx, refinements)
    {
        return exact_erased_object;
    }
    if matches!(expr, Expr::Literal(Literal::Null)) && slot_expects_optional(&slot) {
        return typed_none_for_slot(&slot).unwrap_or_else(|| RustExpr::Var("None".to_string()));
    }
    if let Some(expr) = lower_java_enum_static_field_access_for_slot(expr, env, &slot, ctx) {
        return expr;
    }
    if let Some(expr) =
        lower_typed_collection_value_cast_source_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) =
        lower_target_typed_external_call_cast_source_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) = lower_concrete_cast_source_for_slot(expr, env, &slot, ctx, refinements) {
        return expr;
    }
    if let Some(expr) = lower_optional_cast_source_for_slot(expr, env, &slot, ctx, refinements) {
        return expr;
    }
    if let Some(expr) = lower_raw_erasure_cast_source_for_slot(expr, env, &slot, ctx, refinements) {
        return expr;
    }
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        let plan = plan_lowered_slot_conversion(&stream_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(stream_expr, &plan);
    }
    if let Some(array_access_expr) =
        lower_array_access_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return array_access_expr;
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
    if let Some(array_expr) = lower_array_expr_for_slot(expr, env, &slot, ctx, refinements) {
        return array_expr;
    }
    if let Some(collection_expr) =
        lower_native_collection_call_expr_for_slot(expr, env, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&collection_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(collection_expr, &plan);
    }
    if let Some(callback_expr) =
        lower_callback_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&callback_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(callback_expr, &plan);
    }
    if let Some(accessor_expr) =
        lower_current_trivial_accessor_getter_for_slot(expr, env, &slot, ctx, refinements)
    {
        return accessor_expr;
    }
    if let Some(external_expr) =
        lower_exact_generic_external_call_for_slot(expr, env, &slot, ctx, refinements)
    {
        return external_expr;
    }
    if let Some(external_expr) =
        lower_external_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&external_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(external_expr, &plan);
    }
    if let Some(constructor_expr) =
        lower_external_constructor_expr_for_concrete_slot(expr, env, &slot, ctx, refinements)
    {
        return constructor_expr;
    }
    if let Expr::Lambda { params, body } = expr
        && let Some(return_shape) = callback_result_shape_from_functional_slot(&slot, ctx)
    {
        return lower_callback_lambda_expr_with_return_shape(
            params,
            body,
            env,
            ctx,
            Some(&return_shape),
            false,
        );
    }
    if let Expr::MethodReference { target, method } = expr
        && matches!(target.as_str(), "this" | "self")
    {
        let plan = plan_current_receiver_method_reference(
            method,
            current_receiver_method_reference_arity(method, ctx),
        );
        let lowered = render_method_reference(&plan);
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
        let plan = plan_slot_conversion(actual, slot, ctx.facts);
        return apply_slot_expr_conversion(lowered, &plan);
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
    if let Some(external_expr) =
        lower_exact_generic_external_call_for_slot(expr, env, &slot, ctx, refinements)
    {
        return external_expr;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let mut actual = planned_method_call_return_shape(expr, env, ctx)
        .or_else(|| external_call_expr_result_shape(expr, env, ctx))
        .or_else(|| optional_slot_non_optional_expr_shape(expr, env, &slot, shape_ctx))
        .unwrap_or_else(|| actual_shape_for_slot_expr_with_slot(expr, env, shape_ctx, &slot));
    if let Some(refined_actual) =
        refined_non_null_optional_element_shape_for_slot(expr, &actual, &slot, refinements)
    {
        actual = refined_actual;
    }
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
    if let Some(reused_optional_display) =
        lower_reused_optional_display_for_slot(expr, env, &slot, &actual, ctx)
    {
        return reused_optional_display;
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

fn lower_functional_cast_callback_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    if !java_type_is_known_functional_interface(ty) {
        return None;
    }
    if !matches!(
        inner.as_ref(),
        Expr::Lambda { .. } | Expr::MethodReference { .. }
    ) {
        return None;
    }
    if let Some(input_shape) = functional_interface_single_input_shape(ty)
        && let Some(lowered) = lower_single_input_callback_arg(inner, input_shape, env, ctx)
    {
        return Some(lowered);
    }
    let lowered = lower_expr_for_slot_with_refinements(inner, env, slot.clone(), ctx, refinements);
    Some(lowered)
}

fn lower_current_trivial_accessor_getter_for_slot(
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
    let CallExprPlan::TrivialAccessorGetter {
        receiver,
        field_name,
    } = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    )
    else {
        return None;
    };
    if !receiver
        .as_ref()
        .is_none_or(|receiver| is_this_receiver_expr(receiver))
    {
        return None;
    }
    let field_expr = Expr::Var(field_name);
    Some(lower_expr_for_slot_with_refinements(
        &field_expr,
        env,
        slot.clone(),
        ctx,
        refinements,
    ))
}

fn java_type_is_known_functional_interface(ty: &JavaType) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
            let simple = canonical.rsplit('.').next().unwrap_or(canonical.as_str());
            crate::stdlib::is_known_functional_interface(simple)
        }
        _ => false,
    }
}

fn optional_slot_non_optional_expr_shape(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let target = slot_target_shape(slot)?;
    let element_ty = target.optional_element_type()?;
    let expr_ty = crate::types::infer_expr_type(expr, env.type_env());
    if !same_concrete_java_type(&expr_ty, &element_ty) {
        return None;
    }
    let mut actual = actual_shape_for_slot_expr(expr, env, ctx);
    if actual.optional_element_type().is_some() {
        return None;
    }
    if matches!(actual.java_ty, JavaType::Unknown(_)) {
        actual.java_ty = element_ty;
    }
    Some(actual)
}

fn refined_non_null_optional_element_shape_for_slot(
    expr: &Expr,
    actual: &RustSlotShape,
    slot: &Slot,
    refinements: &ExprRefinements,
) -> Option<RustSlotShape> {
    if !non_null_refinements_make_optional_expr_an_element(expr, refinements) {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some() {
        return None;
    }
    let element_ty = actual.optional_element_type()?;
    if !matches!(target.java_ty, JavaType::Unknown(_))
        && !same_concrete_java_type(&element_ty, &target.java_ty)
    {
        return None;
    }
    let mut element_shape = actual.clone();
    element_shape.java_ty = element_ty;
    Some(element_shape)
}

fn typed_none_for_slot(slot: &Slot) -> Option<RustExpr> {
    let shape = match slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => shape,
        Slot::Value
        | Slot::Condition
        | Slot::Index
        | Slot::StringConcatSegment
        | Slot::DisplaySegment => return None,
    };
    let element_ty = crate::types::optional_element_type(&shape.java_ty)?;
    let element_shape = value_shape(element_ty);
    if !shape_is_erased_object_boundary(&element_shape)
        && !matches!(
            element_shape.java_ty,
            JavaType::Named { ref name, .. } if name == "Object" || name == "java.lang.Object"
        )
    {
        return None;
    }
    Some(RustExpr::Raw("None::<Box<dyn std::any::Any>>".to_string()))
}

fn lower_array_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let array_plan = plan_array_expr(expr)?;
    let target = slot_target_shape(slot)?;
    let element_ty = target.sequence_element_type()?;
    if matches!(
        array_plan,
        crate::array_plan::ArrayExprPlan::Creation { .. }
    ) {
        return None;
    }
    let element_shape = value_shape(element_ty);
    let children = array_plan
        .child_exprs()
        .iter()
        .map(|child| {
            if let Some(recovered) =
                lower_reused_external_into_array_child(child, env, &element_shape, ctx, refinements)
            {
                return recovered;
            }
            lower_expr_for_slot_with_refinements(
                child,
                env,
                argument_slot(element_shape.clone()),
                ctx,
                refinements,
            )
        })
        .collect();
    let lowered = render_array_expr(&array_plan, children);
    if let Some(temporary) = borrowed_array2_temporary_for_slot(&lowered, target) {
        return Some(temporary);
    }
    let actual = LoweredType::from_java(target.java_ty.clone());
    let plan = plan_lowered_slot_conversion(&actual, slot.clone(), ctx.facts);
    Some(apply_lowered_slot_expr_conversion(
        crate::coercion::LoweredExpr {
            expr: lowered,
            actual,
        },
        &plan,
    ))
}

fn lower_array_access_expr(
    array: &Expr,
    index: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Expr::ArrayAccess(outer_array, outer_index) = array
        && crate::types::two_dimensional_array_element_type(&crate::types::infer_expr_type(
            outer_array,
            env.type_env(),
        ))
        .is_some()
    {
        return RustExpr::Raw(format!(
            "{}[[{}, {}]]",
            emit::emit_expr(&lower_expr_with_refinements(
                outer_array,
                env,
                ctx,
                refinements
            )),
            emit::emit_expr(&lower_expr_for_slot(outer_index, env, index_slot(), ctx)),
            emit::emit_expr(&lower_expr_for_slot(index, env, index_slot(), ctx)),
        ));
    }

    if crate::types::two_dimensional_array_element_type(&crate::types::infer_expr_type(
        array,
        env.type_env(),
    ))
    .is_some()
    {
        return RustExpr::Raw(format!(
            "{}.row({}).to_vec()",
            emit::emit_expr(&lower_expr_with_refinements(array, env, ctx, refinements)),
            emit::emit_expr(&lower_expr_for_slot(index, env, index_slot(), ctx)),
        ));
    }

    indexed_expr(
        lower_expr_with_refinements(array, env, ctx, refinements),
        lower_expr_for_slot(index, env, index_slot(), ctx),
    )
}

fn lower_array2_row_replacement_stmt(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustStmt> {
    let Expr::ArrayAccess(array, index) = target else {
        return None;
    };
    crate::types::two_dimensional_array_element_type(&crate::types::infer_expr_type(
        array,
        env.type_env(),
    ))?;
    let array = emit::emit_expr(&lower_expr(array, env, ctx));
    let index = emit::emit_expr(&lower_expr_for_slot(index, env, index_slot(), ctx));
    let value = emit::emit_expr(&lower_expr(value, env, ctx));
    Some(raw_stmt(format!(
        "{array}.row_mut({index}).assign(&ndarray::ArrayView1::from({value}));"
    )))
}

fn lower_reused_external_into_array_child(
    child: &Expr,
    env: &ShapeEnv,
    element_shape: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Var(name) = child else {
        return None;
    };
    if !ctx
        .later_read_names
        .is_some_and(|later_reads| later_reads.contains(name))
    {
        return None;
    }
    let slot = argument_slot(element_shape.clone());
    let actual = actual_shape_for_slot_expr_with_slot(
        child,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
        &slot,
    );
    if !known_owned_argument_clone_recovery_with_facts(&actual, ctx.facts) {
        return None;
    }
    let plan = plan_slot_conversion(actual, slot, ctx.facts);
    if !slot_conversion_plan_contains_external_into(&plan.conversion) {
        return None;
    }
    let lowered = lower_expr_with_refinements(child, env, ctx, refinements);
    let cloned = RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&lowered)));
    Some(apply_slot_expr_conversion(cloned, &plan))
}

fn slot_conversion_plan_contains_external_into(plan: &SlotConversionPlan) -> bool {
    matches!(
        plan,
        SlotConversionPlan::Shape(shape)
            if matches!(shape.dispatch, DispatchCoercionPolicy::ExternalInto { .. })
    )
}

fn current_receiver_method_reference_arity(method: &str, ctx: LowerCtx) -> Option<usize> {
    let current_type = ctx.current_type?;
    let rust_name = crate::parser::to_snake_case(method);
    ctx.signatures
        .method_signature_by_rust_name(current_type, &rust_name)
        .map(|signature| signature.params.len())
}

fn lower_optional_cast_source_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let target_element_ty = target.optional_element_type()?;
    if target_element_ty != *ty {
        return None;
    }
    if optional_cast_inner_java_type(inner, env, slot, ctx) != Some(target.java_ty.clone()) {
        return None;
    }
    Some(lower_expr_for_slot_with_refinements(
        inner,
        env,
        slot.clone(),
        ctx,
        refinements,
    ))
}

fn optional_cast_inner_java_type(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<JavaType> {
    if let Expr::Call {
        receiver,
        name,
        args,
        resolved: None,
    } = expr
        && let Some(actual) = external_call_lowered_type_with_lower_context(
            receiver.as_deref(),
            name,
            args.len(),
            ctx,
            env,
            ctx.current_type,
        )
        && let Some(java_ty) = actual.java_ty()
    {
        return Some(java_ty.clone());
    }
    Some(
        actual_shape_for_slot_expr_with_slot(
            expr,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
            slot,
        )
        .java_ty,
    )
}

fn lower_typed_collection_value_cast_source_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let target_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    if !same_concrete_java_type(&target_ty, ty) {
        return None;
    }
    if !typed_collection_value_expr_matches_cast(inner, ty, env) {
        return None;
    }
    Some(lower_expr_for_slot_with_refinements(
        inner,
        env,
        slot.clone(),
        ctx,
        refinements,
    ))
}

fn typed_collection_value_expr_matches_cast(
    expr: &Expr,
    cast_ty: &JavaType,
    env: &ShapeEnv,
) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    match (receiver.as_deref(), name.as_str(), args.as_slice()) {
        (Some(receiver), "get", [_]) => {
            let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
            let collection_value_ty = map_type_args_from_java_type(&receiver_ty)
                .map(|(_, value_ty, _)| value_ty)
                .or_else(|| crate::types::sequence_element_type(&receiver_ty));
            collection_value_ty.is_some_and(|value_ty| same_concrete_java_type(&value_ty, cast_ty))
        }
        _ => false,
    }
}

fn lower_concrete_cast_source_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let target_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    if !same_concrete_java_type(&target_ty, ty) {
        return None;
    }
    if !cast_inner_can_flow_through_concrete_slot(inner, ty, env, ctx) {
        return None;
    }
    Some(lower_expr_for_slot_with_refinements(
        inner,
        env,
        slot.clone(),
        ctx,
        refinements,
    ))
}

fn lower_target_typed_external_call_cast_source_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let target_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    if !same_concrete_java_type(&target_ty, ty) {
        return None;
    }
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = inner.as_ref()
    else {
        return None;
    };
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    );
    let CallExprPlan::Target(target_plan) = plan else {
        return None;
    };
    if !matches!(
        target_plan,
        CallTargetPlan::FallbackReceiver { .. }
            | CallTargetPlan::ExternalCrateReceiver { .. }
            | CallTargetPlan::ExternalFacadeReceiver { .. }
    ) {
        return None;
    }
    let rust_args = lower_args_for_call_target(
        &target_plan,
        receiver.as_deref(),
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
        resolved.as_ref(),
        ctx,
        None,
    );
    let lowered_receiver = target_plan.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target_plan, receiver, lowered, env, ctx, refinements)
    });
    let mut lowered_expr =
        apply_call_target_with_lower_context(&target_plan, lowered_receiver, rust_args, ctx);
    if slot_expr_needs_result_propagation(inner, env, ctx) {
        lowered_expr = propagate_result_expr(lowered_expr);
    }
    let actual = target_typed_call_cast_actual_shape(inner, env, ctx, &target_ty, target);
    Some(apply_expr_to_slot_via_plan(
        lowered_expr,
        actual,
        slot,
        ctx.facts,
    ))
}

fn target_typed_call_cast_actual_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    target_ty: &JavaType,
    slot_target: &RustSlotShape,
) -> RustSlotShape {
    if let Some(call_shape) = external_call_expr_result_shape(expr, env, ctx)
        && external_call_shape_already_matches_cast_slot(&call_shape, target_ty, slot_target)
    {
        return call_shape;
    }
    fallback_local_shape_from_declared_type(target_ty)
}

fn external_call_shape_already_matches_cast_slot(
    call_shape: &RustSlotShape,
    target_ty: &JavaType,
    slot_target: &RustSlotShape,
) -> bool {
    if same_concrete_java_type(&call_shape.java_ty, &slot_target.java_ty) {
        return true;
    }
    call_shape
        .optional_element_type()
        .is_some_and(|element_ty| same_concrete_java_type(&element_ty, target_ty))
}

fn cast_inner_can_flow_through_concrete_slot(
    expr: &Expr,
    cast_ty: &JavaType,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    if cast_inner_is_current_self(expr, cast_ty, ctx) {
        return true;
    }
    let actual_ty = crate::types::infer_expr_type(expr, env.type_env());
    if same_concrete_java_type(&actual_ty, cast_ty) {
        return true;
    }
    match actual_ty {
        JavaType::Unknown(_) => matches!(
            expr,
            Expr::Call { .. }
                | Expr::New { .. }
                | Expr::Ternary { .. }
                | Expr::Var(_)
                | Expr::FieldAccess(_, _)
        ),
        JavaType::Named { .. } => external_call_expr_result_shape(expr, env, ctx)
            .is_some_and(|shape| same_concrete_java_type(&shape.java_ty, cast_ty)),
        _ => false,
    }
}

fn cast_inner_is_current_self(expr: &Expr, cast_ty: &JavaType, ctx: LowerCtx) -> bool {
    let current_self = match expr {
        Expr::This => true,
        Expr::Var(name) => matches!(name.as_str(), "this" | "self"),
        _ => false,
    };
    current_self
        && ctx
            .current_type
            .is_some_and(|current_type| same_concrete_java_type_name(cast_ty, current_type))
}

fn lower_raw_erasure_cast_source_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    let target_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    if !cast_target_erases_slot_generic_detail(ty, &target_ty) {
        return None;
    }
    if raw_erasure_cast_inner_already_matches_slot(inner, &target_ty, env, ctx) {
        return Some(lower_expr_with_refinements(inner, env, ctx, refinements));
    }
    if target.optional_element_type().is_some() {
        let mut element_shape = target.clone();
        element_shape.java_ty = target_ty;
        return Some(lower_expr_for_slot_with_refinements(
            inner,
            env,
            Slot::Assignment(element_shape),
            ctx,
            refinements,
        ));
    }
    Some(lower_expr_for_slot_with_refinements(
        inner,
        env,
        slot.clone(),
        ctx,
        refinements,
    ))
}

fn raw_erasure_cast_inner_already_matches_slot(
    inner: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Some(inner_shape) = planned_method_call_return_shape(inner, env, ctx)
        .or_else(|| external_call_expr_result_shape(inner, env, ctx))
    else {
        return false;
    };
    java_types_have_same_erased_identity(&inner_shape.java_ty, target_ty)
        && slot_has_generic_or_sequence_detail(&inner_shape.java_ty)
        && slot_has_generic_or_sequence_detail(target_ty)
}

fn cast_target_erases_slot_generic_detail(cast_ty: &JavaType, slot_ty: &JavaType) -> bool {
    let Some(cast_name) = concrete_java_type_name(cast_ty) else {
        return false;
    };
    if !same_concrete_java_type_name(slot_ty, cast_name)
        && !raw_collection_cast_targets_typed_sequence_slot(cast_name, slot_ty)
    {
        return false;
    }
    if !slot_has_generic_or_sequence_detail(slot_ty) {
        return false;
    }
    match cast_ty {
        JavaType::Unknown(_) => true,
        JavaType::Named { args, .. } => {
            args.is_empty() || args.iter().all(|arg| matches!(arg, JavaType::Wildcard(_)))
        }
        _ => false,
    }
}

fn raw_collection_cast_targets_typed_sequence_slot(cast_name: &str, slot_ty: &JavaType) -> bool {
    crate::stdlib::java_collection_kind_for_name(cast_name)
        .is_some_and(|kind| kind != crate::stdlib::JavaCollectionKind::Map)
        && crate::types::sequence_element_type(slot_ty).is_some()
}

fn slot_has_generic_or_sequence_detail(slot_ty: &JavaType) -> bool {
    match slot_ty {
        JavaType::Named { args, .. } if !args.is_empty() => true,
        _ => crate::types::sequence_element_type(slot_ty).is_some(),
    }
}

fn lower_callback_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    if !call_result_slot_shapes_callback_argument(name, args, slot) {
        return None;
    }
    let target_shape = slot_target_shape(slot)?;
    let callback_return_shape = callback_result_shape_from_future_slot(slot, ctx)?;
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    );
    let target = match plan {
        CallExprPlan::Target(target) => target,
        _ => return None,
    };
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver_with_signature(
            &target,
            receiver,
            lowered,
            env,
            ctx,
            refinements,
            None,
        )
    });
    let rust_args = args
        .iter()
        .map(|arg| match arg {
            Expr::Lambda { params, body } => lower_callback_lambda_expr_with_return_shape(
                params,
                body,
                env,
                ctx,
                Some(&callback_return_shape),
                true,
            ),
            _ => lower_unshaped_argument(arg, env, ctx),
        })
        .collect();
    Some(crate::coercion::LoweredExpr {
        expr: apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx),
        actual: crate::conversion_plan::LoweredType::Slot(Box::new(target_shape.clone())),
    })
}

fn call_result_slot_shapes_callback_argument(name: &str, args: &[Expr], slot: &Slot) -> bool {
    name == "submit"
        && args.iter().any(|arg| matches!(arg, Expr::Lambda { .. }))
        && slot_target_shape(slot)
            .and_then(|shape| callback_result_type_from_future_java_type(&shape.java_ty))
            .is_some()
}

fn callback_result_shape_from_functional_slot(
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<SolvedSlotShape> {
    callback_result_shape_from_future_slot(slot, ctx)
}

fn callback_result_shape_from_future_slot(slot: &Slot, ctx: LowerCtx) -> Option<SolvedSlotShape> {
    let target = slot_target_shape(slot)?;
    let result_ty = callback_result_type_from_future_java_type(&target.java_ty)?;
    crate::shape_projection::return_shape_for_java_type(ctx.representations, &result_ty)
}

fn callback_result_type_from_future_java_type(java_ty: &JavaType) -> Option<JavaType> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(canonical.as_str());
    if simple == "Future" && args.len() == 1 {
        Some(args[0].clone())
    } else {
        None
    }
}

fn lower_exact_generic_external_call_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some() {
        return None;
    }
    let external_shape = external_call_expr_result_shape(expr, env, ctx)?;
    if external_shape.java_ty == target.java_ty
        && slot_has_generic_or_sequence_detail(&external_shape.java_ty)
        && slot_has_generic_or_sequence_detail(&target.java_ty)
    {
        return Some(lower_expr_with_refinements(expr, env, ctx, refinements));
    }
    None
}

fn lower_external_constructor_expr_for_concrete_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    _refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::New {
        ty,
        args,
        body,
        resolved,
    } = expr
    else {
        return None;
    };
    let target_shape = slot_target_shape(slot)?;
    let constructed_ty = resolved
        .as_ref()
        .map(|resolved| &resolved.constructed_type)
        .unwrap_or(ty);
    let type_name = concrete_java_type_name(constructed_ty)?;
    if type_name == "Object" || type_name == "java.lang.Object" {
        return None;
    }
    if !same_concrete_java_type_name(&target_shape.java_ty, type_name) {
        return None;
    }
    if body.is_some() && shape_is_concrete_native(target_shape) {
        return Some(RustExpr::Raw("Default::default()".to_string()));
    }
    let arg_types = vec![JavaType::Unknown("_".to_string()); args.len()];
    if ctx
        .facts
        .external_constructor_for(type_name, &arg_types)
        .is_none()
        && crate::external_packs::external_constructor_for(type_name, &arg_types).is_none()
    {
        return None;
    }
    let target = plan_new_target_for_args_with_facts(
        ty,
        resolved.as_ref(),
        ctx.representations,
        ctx.facts,
        args,
        env.type_env(),
    );
    if target.native_collection.is_some() || target.external_crate.is_some() {
        return None;
    }
    let constructor_arg_plan = plan_constructor_arguments_with_signature_fallback(
        resolved.as_ref(),
        args.len(),
        ctx.signatures,
        ty,
        &target.rust_name,
        ctx.facts,
    );
    let rust_args = lower_constructor_args_with_plan(args, env, constructor_arg_plan, ctx);
    let lowered = named_constructor_expr(type_name, &target.rust_name, rust_args);
    let actual = RustSlotShape::for_storage(JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    });
    let plan = plan_slot_conversion(actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn concrete_java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn same_concrete_java_type_name(java_ty: &JavaType, expected: &str) -> bool {
    let Some(actual) = concrete_java_type_name(java_ty) else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(actual)
        == crate::stdlib::canonical_external_receiver_type_key(expected)
}

fn same_concrete_java_type(left: &JavaType, right: &JavaType) -> bool {
    let Some(right_name) = concrete_java_type_name(right) else {
        return left == right;
    };
    same_concrete_java_type_name(left, right_name)
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
        lower_optional_self_field_owned_option_recovery(expr, env, target, actual, ctx, refinements)
    {
        return Some(expr);
    }
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
    if !shape_is_owned(target) || target.java_ty != actual.java_ty {
        if matches!(
            target.ownership,
            OwnershipShape::Owned | OwnershipShape::MutableBorrow
        ) && shared_self_field_read_name_with_ctx(expr, env, ctx).is_some()
            && !matches!(actual.java_ty, JavaType::Primitive(_))
            && (known_owned_argument_clone_recovery_with_facts(actual, ctx.facts)
                || known_named_clone_recovery(actual))
        {
            let plan = plan_slot_conversion(actual.clone(), slot.clone(), ctx.facts);
            if slot_conversion_plan_contains_external_into(&plan.conversion) {
                let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
                let recovered =
                    RustExpr::Raw(render_audited_clone_recovery(&emit::emit_expr(&lowered)));
                return Some(apply_slot_expr_conversion(recovered, &plan));
            }
        }
        if let Some(element_ty) = target.optional_element_type()
            && shared_self_field_read_name_with_ctx(expr, env, ctx).is_some()
            && !matches!(element_ty, JavaType::Primitive(_))
        {
            let element_shape = value_shape(element_ty);
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
            return Some(apply_expr_to_slot_via_plan(
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
    shared_self_field_read_name_with_ctx(expr, env, ctx)?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let recovered = RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&lowered)));
    Some(apply_expr_to_slot_via_plan(
        recovered,
        target.clone(),
        slot,
        ctx.facts,
    ))
}

fn lower_optional_self_field_owned_option_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    target: &RustSlotShape,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    shared_self_field_read_name_with_ctx(expr, env, ctx)?;
    let field_actual = if actual.optional_element_type().is_some() {
        actual.clone()
    } else {
        current_instance_field_shape_with_env(expr, env, ctx)?
    };
    if !shape_is_owned(target)
        || target.java_ty != field_actual.java_ty
        || target.optional_element_type().is_none()
        || field_actual.optional_element_type().is_none()
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "{}.clone()",
        emit::emit_expr(&lowered)
    )))
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
    if !shape_is_owned(target) || target.java_ty != element_ty {
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
    shared_self_field_read_name_with_ctx(receiver, env, ctx)?;
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
    if !shape_is_owned(target) || target.java_ty != element_ty {
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

fn shared_self_field_read_name_with_ctx<'a>(
    expr: &'a Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<&'a str> {
    shared_self_field_read_name(expr, env).or_else(|| {
        let Expr::Var(name) = expr else {
            return None;
        };
        if env.local_shape(name).is_some()
            || (env.is_static_field(name) && !current_env_name_is_instance_field(env, name))
        {
            return None;
        }
        current_instance_field_shape_with_env(expr, env, ctx)?;
        Some(name.as_str())
    })
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
    if external_call_lowered_type_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    )
    .is_some()
    {
        return external_call_returns_rust_result_with_lower_context(
            receiver.as_deref(),
            name,
            args.len(),
            ctx,
            env,
            ctx.current_type,
        );
    }
    matches!(
        plan_call_exception_propagation_with_lower_context(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            ctx,
            env,
            ctx.current_type,
        ),
        CallExceptionPropagationPlan::PropagateResult
    ) || slot_expr_needs_result_propagation_from_shape_env(
        receiver.as_deref(),
        name,
        args.len(),
        env,
        ctx,
    )
}

fn slot_expr_needs_result_propagation_from_shape_env(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Some(receiver_ty) = receiver.and_then(|receiver| receiver_shape_java_type(receiver, env))
    else {
        return false;
    };
    let Some(receiver_name) = external_receiver_name_from_shape_type(receiver_ty) else {
        return false;
    };
    crate::call_plan::external_call_has_thrown_effect_for_receiver(
        ctx.facts,
        receiver_name,
        name,
        arg_count,
    )
}

fn receiver_shape_java_type<'a>(receiver: &Expr, env: &'a ShapeEnv) -> Option<&'a JavaType> {
    match receiver {
        Expr::Var(name) => env
            .local_shape(name)
            .or_else(|| env.field_shape(name))
            .map(|shape| &shape.java_ty),
        Expr::FieldAccess(_, field) => env.field_shape(field).map(|shape| &shape.java_ty),
        _ => None,
    }
}

fn external_receiver_name_from_shape_type(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(
            name.rsplit_once("::")
                .map(|(_, simple)| simple)
                .unwrap_or(name),
        ),
        _ => None,
    }
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
    let Expr::Var(_) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some() || !shape_is_owned(target) {
        return None;
    }
    if shape_is_erased_object_boundary(target) {
        return None;
    }
    let element_ty = actual.optional_element_type()?;
    if element_ty != target.java_ty {
        return None;
    }
    if !known_owned_argument_clone_recovery(target) && !known_named_clone_recovery(target) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let rendered = emit::emit_expr(&lowered);
    if actual.ownership == OwnershipShape::SharedBorrow {
        return Some(RustExpr::Raw(format!(
            "{}.as_ref().map(|value| (**value).clone()).unwrap()/* TODO(translation): validate added clone */",
            rendered
        )));
    }
    Some(RustExpr::Raw(format!(
        "{rendered}.as_ref().unwrap().clone()/* TODO(translation): validate added clone */"
    )))
}

fn lower_reused_optional_display_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(name) = expr else {
        return None;
    };
    if !matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment) {
        return None;
    }
    actual.optional_element_type()?;
    if !current_callable_binding_needs_clone_for_move(name, ctx) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, &ExprRefinements::default());
    Some(RustExpr::Raw(format!(
        "{}.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
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
    if actual.optional_element_type().is_some() || !shape_is_owned(actual) {
        return None;
    }
    if element_ty != actual.java_ty {
        return None;
    }
    if shape_is_erased_object_boundary(target) {
        return None;
    }
    if target.ownership == OwnershipShape::SharedBorrow {
        let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
        return Some(apply_expr_to_slot_via_plan(
            lowered,
            actual.clone(),
            slot,
            ctx.facts,
        ));
    }
    if !known_owned_argument_clone_recovery(actual) && !known_named_clone_recovery(actual) {
        return None;
    }
    if !current_callable_binding_needs_clone_for_move(name, ctx) {
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
    const MAP_ENTRY_VALUE_ACCESSOR: &str = concat!("get", "Value");
    let (field, actual_ty) = match name.as_str() {
        "getKey" | "get_key" => (
            "0",
            map_entry_accessor_type_or_slot_target(&key_ty, slot).unwrap_or(key_ty),
        ),
        accessor if accessor == MAP_ENTRY_VALUE_ACCESSOR || accessor == "get_value" => {
            ("1", value_ty)
        }
        _ => return None,
    };
    let entry_access = RustExpr::Raw(format!("{}.{}", emit::emit_expr(&next_expr), field));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(actual_ty);
    Some(apply_expr_to_slot_via_plan(
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
        && let Some(map_args) = map_type_args_from_shape_java_type(&shape.java_ty)
    {
        return Some(map_args);
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    map_type_args_from_shape_java_type(&shape.java_ty)
}

fn map_type_args_from_shape_java_type(ty: &JavaType) -> Option<(JavaType, JavaType, bool)> {
    if let Some(map_args) = map_type_args_from_java_type(ty) {
        return Some(map_args);
    }
    let element_ty = crate::types::optional_element_type(ty)?;
    let (key_ty, value_ty, _) = map_type_args_from_java_type(&element_ty)?;
    Some((key_ty, value_ty, true))
}

fn method_call_return_shape_from_receiver(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if stream_like_call_preserves_receiver_shape(name, args.len(), &receiver_shape) {
        return Some(receiver_shape);
    }
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)
        .unwrap_or(receiver_shape.java_ty);
    let receiver_name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let snake_name = crate::parser::to_snake_case(name);
    if let Some(resolved) = resolved
        && let Some(shape) = source_summary_declared_method_return_shape(
            &resolved.declaring_type,
            name,
            args.len(),
            ctx,
        )
    {
        return Some(shape);
    }
    if let Some(shape) =
        source_summary_declared_method_return_shape(&receiver_name, name, args.len(), ctx)
    {
        return Some(shape);
    }
    if let Some(resolved) = resolved
        && let Some(shape) = ctx
            .signatures
            .method_return_shape_by_rust_name(&resolved.declaring_type, &resolved.rust_name)
    {
        return Some(shape);
    }
    if let Some(shape) = ctx
        .signatures
        .method_return_shape_by_rust_name(&receiver_name, name)
        .or_else(|| {
            ctx.signatures
                .method_return_shape_by_rust_name(&receiver_name, &snake_name)
        })
    {
        return Some(shape);
    }
    ctx.signatures
        .method_signature_by_base_name_and_arg_types(
            &receiver_name,
            name,
            &args
                .iter()
                .map(|arg| crate::types::infer_expr_type(arg, env.type_env()))
                .collect::<Vec<_>>(),
        )
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(&receiver_name, name, args.len())
        })
        .and_then(|(_, signature)| signature.return_shape.clone())
}

fn source_summary_declared_method_return_shape(
    receiver_name: &str,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    if find_context_type_by_key(ctx.program, receiver_name).is_some() {
        return None;
    }
    let ty = find_context_type_by_key(ctx.support_local_type_program, receiver_name)
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, receiver_name))?;
    let method = find_declared_method_by_name_and_arity(ty, method_name, arg_count)?;
    source_summary_declared_return_shape_for_method(ty, method, ctx)
}

fn find_declared_method_by_name_and_arity<'a>(
    ty: &'a TypeDecl,
    method_name: &str,
    arg_count: usize,
) -> Option<&'a MethodDecl> {
    ty.members.iter().find_map(|member| {
        let Member::Method(method) = member else {
            return None;
        };
        let rust_name = method
            .rust_name
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
        (method.params.len() == arg_count
            && (method.name == method_name || rust_name == method_name))
            .then_some(method)
    })
}

fn source_summary_declared_return_shape_for_method(
    ty: &TypeDecl,
    method: &MethodDecl,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let shape = crate::shape_projection::return_shape_for_java_type(
        ctx.representations,
        &method.return_type,
    )?;
    if let Some(effect) = ctx.facts.method_effect_for(&ty.name, &method.name)
        && effect.return_nullability != NullabilityState::Unknown
    {
        return Some(apply_signature_constraints(
            shape,
            &[SignatureConstraint::RequiredStorageNullability(
                effect.return_nullability,
            )],
        ));
    }
    let body_constraints = crate::signature::source_method_return_nullability_constraints(method);
    if !body_constraints.is_empty() {
        return Some(apply_signature_constraints(shape, &body_constraints));
    }
    if !matches!(
        crate::stdlib::source_summary_method_return_nullability(&method.return_type),
        NullabilityState::DefinitelyNonNull
    ) {
        return Some(shape);
    }
    Some(apply_signature_constraints(
        shape,
        &[SignatureConstraint::RequiredStorageNullability(
            NullabilityState::DefinitelyNonNull,
        )],
    ))
}

fn stream_like_call_preserves_receiver_shape(
    name: &str,
    arg_count: usize,
    receiver_shape: &RustSlotShape,
) -> bool {
    if !matches!(
        name,
        "filter"
            | "sortByKey"
            | "sort_by_key"
            | "reduceByKey"
            | "reduce_by_key"
            | "partitionBy"
            | "partition_by"
    ) {
        return false;
    }
    if matches!(name, "filter") && arg_count != 1 {
        return false;
    }
    let JavaType::Named { name, args } = &receiver_shape.java_ty else {
        return false;
    };
    !args.is_empty()
        && matches!(
            crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next(),
            Some("JavaRDD" | "JavaPairRDD")
        )
}

fn fallback_receiver_method_signature<'a>(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx<'a>,
) -> Option<(String, &'a crate::signature::MethodSignaturePlan)> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    for receiver_name in
        fallback_receiver_type_name_candidates(receiver, name, args.len(), env, ctx)
    {
        if receiver
            .is_some_and(|receiver| fallback_receiver_is_static_owner_expr(receiver, env, ctx))
            && static_context_owner_is_emitted(&receiver_name, ctx)
            && find_context_type_by_key(ctx.program, &receiver_name).is_none()
        {
            continue;
        }
        if let Some(signature) = ctx
            .signatures
            .method_signature_by_base_name_and_arg_types_with_facts(
                &receiver_name,
                name,
                &arg_types,
                ctx.facts,
            )
            .or_else(|| {
                ctx.signatures.method_signature_by_base_name_and_arg_types(
                    &receiver_name,
                    name,
                    &arg_types,
                )
            })
        {
            return Some(signature);
        }
        if let Some(signature) = ctx
            .signatures
            .method_signature_by_base_name_and_arity(&receiver_name, name, args.len())
            .or_else(|| {
                ctx.signatures
                    .method_signature_by_base_name_accepting_arity(&receiver_name, name, args.len())
            })
        {
            return Some(signature);
        }
    }
    None
}

fn fallback_receiver_is_static_owner_expr(receiver: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let Expr::Var(receiver_name) = receiver else {
        return false;
    };
    if env.field_type(receiver_name).is_some() {
        return false;
    }
    if env.type_env().static_type_alias(receiver_name).is_some()
        || find_context_type_by_key(ctx.program, receiver_name).is_some()
        || find_context_type_by_key(ctx.support_local_type_program, receiver_name).is_some()
        || find_context_type_by_key(ctx.source_summary_type_program, receiver_name).is_some()
    {
        return true;
    }
    env.local_type(receiver_name).is_none()
}

fn source_context_static_method_fallback_signature(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ContextFallbackMethodSignature> {
    let Expr::Var(receiver_name) = receiver? else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    let mut owner_candidates = Vec::new();
    if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
        push_unique_receiver_type_name(&mut owner_candidates, alias.to_string());
    }
    push_unique_receiver_type_name(&mut owner_candidates, receiver_name.clone());
    for owner in owner_candidates {
        let Some(ty) = find_context_type_by_key(ctx.support_local_type_program, &owner)
            .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))
        else {
            continue;
        };
        let matching_methods = ty
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Method(method)
                    if (static_context_owner_is_emitted(&owner, ctx)
                        || method.modifiers.contains(&Modifier::Static))
                        && method.name == name
                        && method.params.len() == args.len() =>
                {
                    Some(method)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if matching_methods.len() != 1 {
            continue;
        }
        if static_context_owner_is_emitted(&owner, ctx) {
            let method = matching_methods[0];
            let isolated_program = Program {
                package: ctx.support_local_type_program.package.clone(),
                imports: ctx.support_local_type_program.imports.clone(),
                types: vec![ty.clone()],
            };
            let usage = crate::usage::analyze_usage(&isolated_program);
            let signatures = crate::signature::solve_signatures_with_facts_and_support_context(
                &isolated_program,
                ctx.representations,
                &usage,
                ctx.facts,
                ctx.support_local_type_program,
            );
            if let Some((rust_name, signature)) =
                signatures.method_signature_by_base_name_and_arity(&ty.name, name, args.len())
            {
                return Some(ContextFallbackMethodSignature {
                    rust_name,
                    arg_shapes: signature
                        .params
                        .iter()
                        .map(|param| param.shape.clone())
                        .collect(),
                });
            }
            return Some(ContextFallbackMethodSignature {
                rust_name: method
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
                arg_shapes: source_context_param_shapes(method, ctx),
            });
        }
        let isolated_program = Program {
            package: ctx.support_local_type_program.package.clone(),
            imports: ctx.support_local_type_program.imports.clone(),
            types: vec![ty.clone()],
        };
        let usage = crate::usage::analyze_usage(&isolated_program);
        let signatures = crate::signature::solve_signatures_with_facts_and_support_context(
            &isolated_program,
            ctx.representations,
            &usage,
            ctx.facts,
            ctx.support_local_type_program,
        );
        if let Some((rust_name, signature)) =
            signatures.method_signature_by_base_name_and_arity(&ty.name, name, args.len())
        {
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: signature
                    .params
                    .iter()
                    .map(|param| param.shape.clone())
                    .collect(),
            });
        }
        let rust_name = matching_methods[0]
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&matching_methods[0].name));
        return Some(ContextFallbackMethodSignature {
            rust_name,
            arg_shapes: source_context_param_shapes(matching_methods[0], ctx),
        });
    }
    None
}

fn static_context_owner_is_emitted(owner: &str, ctx: LowerCtx) -> bool {
    find_context_type_by_key(ctx.program, owner).is_some()
        || (ctx.context_sources_are_emitted
            && find_context_type_by_key(ctx.support_local_type_program, owner).is_some())
}

fn source_context_receiver_method_fallback_signature(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ContextFallbackMethodSignature> {
    let receiver = receiver?;
    if fallback_receiver_is_static_owner_expr(receiver, env, ctx) {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_uses_dyn_trait_slots = receiver_expr_uses_dyn_trait_slots(receiver, env, ctx);
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    let snake_name = crate::parser::to_snake_case(name);
    for owner in fallback_receiver_type_name_candidates(Some(receiver), name, args.len(), env, ctx)
    {
        let Some(ty) = find_context_type_by_key(ctx.support_local_type_program, &owner)
            .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))
        else {
            continue;
        };
        let matching_methods = ty
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Method(method)
                    if !method.modifiers.contains(&Modifier::Static)
                        && (method.name == name
                            || crate::parser::to_snake_case(&method.name) == snake_name)
                        && method.params.len() == args.len() =>
                {
                    Some(method)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if matching_methods.is_empty() {
            continue;
        }
        let isolated_program = Program {
            package: ctx.support_local_type_program.package.clone(),
            imports: ctx.support_local_type_program.imports.clone(),
            types: vec![ty.clone()],
        };
        let usage = crate::usage::analyze_usage(&isolated_program);
        let signatures = crate::signature::solve_signatures_with_facts_and_support_context(
            &isolated_program,
            ctx.representations,
            &usage,
            ctx.facts,
            ctx.support_local_type_program,
        );
        if let Some((rust_name, signature)) = signatures
            .method_signature_by_base_name_and_arg_types(&ty.name, name, &arg_types)
            .or_else(|| {
                signatures.method_signature_by_base_name_and_arity(&ty.name, name, args.len())
            })
        {
            let method = matching_methods[0];
            let effective_signature = ctx
                .signatures
                .method_signature_by_rust_name(&ty.name, &rust_name)
                .unwrap_or(signature);
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: source_context_param_shapes(method, ctx)
                    .into_iter()
                    .zip(effective_signature.params.iter())
                    .map(|(declared, solved)| {
                        if source_context_reference_param_should_borrow(
                            &solved.java_ty,
                            solved.is_varargs,
                        ) {
                            solved.shape.clone()
                        } else {
                            declared
                        }
                    })
                    .collect(),
            });
        }
        if matching_methods.len() == 1 {
            let method = matching_methods[0];
            let rust_name = method
                .rust_name
                .clone()
                .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: method
                    .params
                    .iter()
                    .map(|param| {
                        source_context_trait_receiver_param_shape(
                            param,
                            ctx,
                            receiver_uses_dyn_trait_slots,
                        )
                    })
                    .collect(),
            });
        }
    }
    None
}

fn receiver_expr_uses_dyn_trait_slots(receiver: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    matches!(
        receiver_shape.dispatch,
        DispatchShape::DynTrait { .. } | DispatchShape::BoxedDynTrait { .. }
    )
}

fn find_context_type_by_key<'a>(program: &'a Program, type_key: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_context_type_by_key_in_type(ty, type_key))
}

fn find_context_type_by_key_in_type<'a>(ty: &'a TypeDecl, type_key: &str) -> Option<&'a TypeDecl> {
    if context_type_key_matches(&ty.name, type_key) {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_context_type_by_key_in_type(nested, type_key),
        _ => None,
    })
}

fn context_type_key_matches(candidate: &str, query: &str) -> bool {
    if crate::stdlib::canonical_external_receiver_type_key(candidate)
        == crate::stdlib::canonical_external_receiver_type_key(query)
    {
        return true;
    }
    let simple = candidate
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(candidate);
    crate::stdlib::canonical_external_receiver_type_key(simple)
        == crate::stdlib::canonical_external_receiver_type_key(query)
}

fn fallback_receiver_type_name_candidates(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(receiver) = receiver {
        if let Some(receiver_name) = receiver_type_name_from_shape(receiver, env, ctx) {
            push_unique_receiver_type_name(&mut candidates, receiver_name);
        }
        if let Expr::Var(receiver_name) = receiver
            && env.local_type(receiver_name).is_none()
            && env.field_type(receiver_name).is_none()
        {
            if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
                push_unique_receiver_type_name(&mut candidates, alias.to_string());
            }
            push_unique_receiver_type_name(&mut candidates, receiver_name.clone());
        }
    } else if let Some(current_type) = ctx.current_type {
        push_unique_receiver_type_name(&mut candidates, current_type.to_string());
    }
    candidates.retain(|candidate| {
        ctx.signatures
            .has_method_signature_by_base_name_accepting_arity(candidate, name, arg_count)
            || find_context_type_by_key(ctx.support_local_type_program, candidate).is_some()
            || find_context_type_by_key(ctx.source_summary_type_program, candidate).is_some()
    });
    candidates
}

fn push_unique_receiver_type_name(candidates: &mut Vec<String>, receiver_name: String) {
    if !candidates
        .iter()
        .any(|candidate| candidate == &receiver_name)
    {
        candidates.push(receiver_name);
    }
}

fn receiver_type_name_from_shape(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<String> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    match &receiver_shape.dispatch {
        DispatchShape::DynTrait { trait_name } | DispatchShape::BoxedDynTrait { trait_name } => {
            return Some(trait_name.clone());
        }
        DispatchShape::Concrete
        | DispatchShape::ClosedEnum { .. }
        | DispatchShape::ErasedObject => {}
    }
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
        resolved: _,
    } = expr
    else {
        return None;
    };
    if name != "get" || args.len() != 1 {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let optional_sequence_ty = receiver_shape.optional_element_type();
    let element_ty = receiver_shape.sequence_element_type().or_else(|| {
        optional_sequence_ty
            .as_ref()
            .and_then(crate::types::sequence_element_type)
    })?;
    let mut receiver_expr = lower_expr_with_refinements(receiver, env, ctx, refinements);
    if optional_sequence_ty.is_some() {
        receiver_expr = RustExpr::Raw(format!(
            "{}.as_ref().expect(\"Java null receiver\")",
            emit::emit_expr(&receiver_expr)
        ));
    }
    let index_expr = lower_expr_for_slot(&args[0], env, index_slot(), ctx);
    let get_expr = RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java index out of bounds\")",
        emit::emit_expr(&receiver_expr),
        emit::emit_expr(&index_expr)
    ));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(element_ty);
    if let Some(adapted) =
        lower_borrowed_sequence_get_subtype_to_concrete_slot(&get_expr, &actual, slot, ctx.facts)
    {
        return Some(adapted);
    }
    Some(apply_expr_to_slot_via_plan(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn lower_borrowed_sequence_get_subtype_to_concrete_slot(
    get_expr: &RustExpr,
    actual: &RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    if actual.java_ty == target.java_ty || !shape_is_owned(&target) {
        return None;
    }
    let reaches = facts
        .hierarchy_subtype_policy(&actual.java_ty, &target.java_ty)
        .reaches
        || javalibs_known_java_subtype(&actual.java_ty, &target.java_ty);
    if !reaches {
        return None;
    }
    let target_ty = emit::emit_type(&rust_type_for_slot_with_facts(target, facts));
    Some(RustExpr::Raw(format!(
        "{target_ty}::new_1({}.clone())",
        emit::emit_expr(get_expr)
    )))
}

fn javalibs_known_java_subtype(actual: &JavaType, target: &JavaType) -> bool {
    match (actual, target) {
        (JavaType::Named { name: actual, .. }, JavaType::Named { name: target, .. })
        | (JavaType::Unknown(actual), JavaType::Named { name: target, .. })
        | (JavaType::Named { name: actual, .. }, JavaType::Unknown(target))
        | (JavaType::Unknown(actual), JavaType::Unknown(target)) => {
            crate::javalibs::is_known_java_subtype(actual, target)
        }
        _ => false,
    }
}

fn lower_array_access_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::ArrayAccess(array, index) = expr else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let array_shape = infer_expr_shape_with_context(array, env, shape_ctx);
    let element_ty = array_shape.sequence_element_type()?;
    let access_expr = lower_array_access_expr(array, index, env, ctx, refinements);
    let actual = crate::shape_projection::borrowed_sequence_element_shape(element_ty);
    if let Some(target) = slot_target_shape(slot)
        && target
            .optional_element_type()
            .is_some_and(|target_element_ty| target_element_ty == actual.java_ty)
        && matches!(target.ownership, OwnershipShape::SharedBorrow)
        && matches!(target.view, ViewShape::Native)
    {
        return Some(RustExpr::Raw(format!(
            "Some(&{})",
            emit::emit_expr(&access_expr)
        )));
    }
    Some(apply_expr_to_slot_via_plan(
        access_expr,
        actual,
        slot,
        ctx.facts,
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
    let plan =
        plan_call_expr_with_lower_context(Some(receiver), name, args, resolved.as_ref(), ctx, env);
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
        Some(receiver),
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
        resolved.as_ref(),
        ctx,
        None,
    );
    let key = rust_args.into_iter().next()?;
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    })?;
    let get_expr = RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java null receiver\")",
        emit::emit_expr(&lowered_receiver),
        emit::emit_expr(&key)
    ));
    let actual = crate::shape_projection::borrowed_sequence_element_shape(value_ty);
    Some(apply_expr_to_slot_via_plan(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn apply_expr_to_slot_via_plan(
    expr: RustExpr,
    actual: RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> RustExpr {
    let plan = plan_slot_conversion(actual, slot.clone(), facts);
    apply_slot_expr_conversion(expr, &plan)
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
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    );
    let CallExprPlan::Target(target @ CallTargetPlan::NativeCollectionMethod { receiver, method }) =
        plan
    else {
        return None;
    };
    let actual = native_collection_lowered_type(receiver, method, env.type_env())?;
    let rust_args = lower_args_for_call_target(
        &target,
        Some(receiver),
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
        resolved.as_ref(),
        ctx,
        None,
    );
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    });
    let mut lowered_expr =
        apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx);
    if slot_expr_needs_result_propagation(expr, env, ctx) {
        lowered_expr = propagate_result_expr(lowered_expr);
    }
    Some(crate::coercion::LoweredExpr {
        expr: lowered_expr,
        actual,
    })
}

fn lower_external_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    let mut actual = external_call_lowered_type_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    )
    .or_else(|| {
        external_call_lowered_type_from_planned_static_owner(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            env,
            ctx,
        )
    })?;
    let inferred_actual = infer_expr_shape_with_context(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    if inferred_actual.view != ViewShape::Native
        && matches!(actual, LoweredType::Slot(_))
        && !slot_targets_erased_object(slot)
        && !lowered_type_has_concrete_java_type_when_inferred_is_unknown(&actual, &inferred_actual)
        && !lowered_type_slot_has_more_generic_information(&actual, &inferred_actual)
    {
        actual = LoweredType::Slot(Box::new(inferred_actual));
    }
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    );
    let CallExprPlan::Target(target) = plan else {
        return None;
    };
    let rust_args = lower_args_for_call_target(
        &target,
        receiver.as_deref(),
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
        resolved.as_ref(),
        ctx,
        None,
    );
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    });
    let mut lowered_expr =
        apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx);
    if external_call_returns_rust_result_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    ) || slot_expr_needs_result_propagation(expr, env, ctx)
    {
        lowered_expr = propagate_result_expr(lowered_expr);
    }
    actual = external_call_actual_for_matching_generic_slot(actual, slot);
    Some(crate::coercion::LoweredExpr {
        expr: lowered_expr,
        actual,
    })
}

fn lowered_type_has_concrete_java_type_when_inferred_is_unknown(
    actual: &LoweredType,
    inferred: &RustSlotShape,
) -> bool {
    let Some(actual_ty) = actual.java_ty() else {
        return false;
    };
    !matches!(actual_ty, JavaType::Unknown(_)) && matches!(inferred.java_ty, JavaType::Unknown(_))
}

fn external_call_actual_for_matching_generic_slot(actual: LoweredType, slot: &Slot) -> LoweredType {
    let LoweredType::Slot(actual_shape) = &actual else {
        return actual;
    };
    let Some(target) = slot_target_shape(slot) else {
        return actual;
    };
    if actual_shape.java_ty == target.java_ty
        && slot_has_generic_or_sequence_detail(&actual_shape.java_ty)
        && slot_has_generic_or_sequence_detail(&target.java_ty)
    {
        return LoweredType::Slot(Box::new(target.clone()));
    }
    actual
}

fn slot_targets_erased_object(slot: &Slot) -> bool {
    let Some(target) = slot_target_shape(slot) else {
        return false;
    };
    if matches!(target.dispatch, DispatchShape::ErasedObject)
        || matches!(target.view, ViewShape::ErasedObjectView)
    {
        return true;
    }
    target.optional_element_type().is_some_and(|element_ty| {
        let element_shape = crate::conversion_plan::optional_element_slot_shape(target, element_ty);
        matches!(element_shape.dispatch, DispatchShape::ErasedObject)
            || matches!(element_shape.view, ViewShape::ErasedObjectView)
    })
}

fn lowered_type_slot_has_more_generic_information(
    actual: &LoweredType,
    inferred: &RustSlotShape,
) -> bool {
    let LoweredType::Slot(actual) = actual else {
        return false;
    };
    java_types_have_same_erased_identity(&actual.java_ty, &inferred.java_ty)
        && shape_has_more_generic_information(actual, inferred)
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
    if matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment)
        && let Some((tested_expr, then_branch_non_null)) =
            ternary_condition_non_null_branch_expr(condition)
        && let Some(non_null_expr) = null_guarded_ternary_non_null_value_expr(
            tested_expr,
            then_expr,
            else_expr,
            then_branch_non_null,
            env,
        )
        && (optional_expr_element_shape(non_null_expr, env, ctx).is_some()
            || optional_expr_element_shape(tested_expr, env, ctx).is_some())
    {
        let lower_non_null_branch = |branch_refinements: &ExprRefinements| {
            lower_non_null_optional_display_branch(
                non_null_expr,
                env,
                slot.clone(),
                ctx,
                branch_refinements,
            )
            .or_else(|| {
                lower_non_null_optional_display_branch(
                    tested_expr,
                    env,
                    slot.clone(),
                    ctx,
                    branch_refinements,
                )
            })
            .unwrap_or_else(|| {
                lower_expr_for_slot_with_refinements(
                    non_null_expr,
                    env,
                    slot.clone(),
                    ctx,
                    branch_refinements,
                )
            })
        };
        return ternary_expr(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            if then_branch_non_null {
                lower_non_null_branch(&then_refinements)
            } else {
                lower_expr_for_slot_with_refinements(
                    then_expr,
                    env,
                    slot.clone(),
                    ctx,
                    &then_refinements,
                )
            },
            if then_branch_non_null {
                lower_expr_for_slot_with_refinements(
                    else_expr,
                    env,
                    slot.clone(),
                    ctx,
                    &else_refinements,
                )
            } else {
                lower_non_null_branch(&else_refinements)
            },
        );
    }
    if let Some(target) = slot_target_shape(&slot)
        && let Some(element_ty) = target.optional_element_type()
        && matches!(
            (then_expr, else_expr),
            (Expr::Literal(Literal::Null), _) | (_, Expr::Literal(Literal::Null))
        )
    {
        let mut element_shape = target.clone();
        element_shape.java_ty = element_ty;
        let lower_optional_branch =
            |branch: &Expr, branch_refinements: &ExprRefinements| -> RustExpr {
                if matches!(branch, Expr::Literal(Literal::Null)) {
                    RustExpr::Var("None".to_string())
                } else {
                    RustExpr::Call {
                        function: Box::new(RustExpr::Var("Some".to_string())),
                        args: vec![lower_expr_for_slot_with_refinements(
                            branch,
                            env,
                            Slot::Assignment(element_shape.clone()),
                            ctx,
                            branch_refinements,
                        )],
                    }
                }
            };
        return ternary_expr(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_optional_branch(then_expr, &then_refinements),
            lower_optional_branch(else_expr, &else_refinements),
        );
    }
    if !matches!(
        (then_expr, else_expr),
        (Expr::Literal(Literal::Null), _) | (_, Expr::Literal(Literal::Null))
    ) && let Some(target) = slot_target_shape(&slot)
        && let Some(element_ty) = target.optional_element_type()
        && !expr_type_is_optional(then_expr, env)
        && !expr_type_is_optional(else_expr, env)
    {
        let mut element_shape = target.clone();
        element_shape.java_ty = element_ty;
        let lowered = ternary_expr(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_expr_for_slot_with_refinements(
                then_expr,
                env,
                Slot::Assignment(element_shape.clone()),
                ctx,
                &then_refinements,
            ),
            lower_expr_for_slot_with_refinements(
                else_expr,
                env,
                Slot::Assignment(element_shape),
                ctx,
                &else_refinements,
            ),
        );
        return RustExpr::Call {
            function: Box::new(RustExpr::Var("Some".to_string())),
            args: vec![lowered],
        };
    }
    ternary_expr(
        lower_expr_for_slot(condition, env, condition_slot(), ctx),
        lower_expr_for_slot_with_refinements(then_expr, env, slot.clone(), ctx, &then_refinements),
        lower_expr_for_slot_with_refinements(else_expr, env, slot, ctx, &else_refinements),
    )
}

fn ternary_condition_non_null_branch_expr(condition: &Expr) -> Option<(&Expr, bool)> {
    let Expr::Binary { op, left, right } = condition else {
        return None;
    };
    if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        return None;
    }
    let tested_expr = if matches!(right.as_ref(), Expr::Literal(Literal::Null)) {
        left.as_ref()
    } else if matches!(left.as_ref(), Expr::Literal(Literal::Null)) {
        right.as_ref()
    } else {
        return None;
    };
    Some((tested_expr, matches!(op, BinaryOp::Ne)))
}

fn null_guarded_ternary_non_null_value_expr<'a>(
    tested_expr: &'a Expr,
    then_expr: &'a Expr,
    else_expr: &'a Expr,
    then_branch_non_null: bool,
    env: &ShapeEnv,
) -> Option<&'a Expr> {
    let candidate = if then_branch_non_null {
        then_expr
    } else {
        else_expr
    };
    if same_null_guarded_value_expr(candidate, tested_expr, env) {
        Some(candidate)
    } else {
        None
    }
}

fn same_null_guarded_value_expr(left: &Expr, right: &Expr, env: &ShapeEnv) -> bool {
    if left == right {
        return true;
    }
    match (
        shared_self_field_read_name(left, env),
        shared_self_field_read_name(right, env),
    ) {
        (Some(left), Some(right)) => field_names_match(left, right),
        _ => false,
    }
}

fn field_names_match(left: &str, right: &str) -> bool {
    left == right
        || crate::parser::to_snake_case(left) == right
        || left == crate::parser::to_snake_case(right)
}

fn optional_expr_element_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let actual = actual_shape_for_slot_expr_with_slot(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
        &Slot::Value,
    );
    let actual = if actual.optional_element_type().is_some() {
        actual
    } else {
        current_instance_field_shape_with_env(expr, env, ctx).unwrap_or(actual)
    };
    let element_ty = actual.optional_element_type()?;
    let mut element_shape = actual;
    element_shape.java_ty = element_ty;
    element_shape.ownership = OwnershipShape::SharedBorrow;
    Some(element_shape)
}

fn lower_non_null_optional_display_branch(
    expr: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let element_shape = optional_expr_element_shape(expr, env, ctx)?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let unwrapped = if non_null_refinements_make_optional_expr_an_element(expr, refinements) {
        lowered
    } else {
        RustExpr::Raw(format!("{}.as_ref().unwrap()", emit::emit_expr(&lowered)))
    };
    let plan = plan_slot_conversion(element_shape, slot, ctx.facts);
    Some(apply_slot_expr_conversion(unwrapped, &plan))
}

fn non_null_refinements_make_optional_expr_an_element(
    expr: &Expr,
    refinements: &ExprRefinements,
) -> bool {
    matches!(expr, Expr::Var(name) if refinements.non_null_locals.contains(name))
}

fn expr_type_is_optional(expr: &Expr, env: &ShapeEnv) -> bool {
    crate::types::optional_element_type(&crate::types::infer_expr_type(expr, env.type_env()))
        .is_some()
}

fn lower_string_concat_with_refinements(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let plan = build_string_concat_plan(expr, env.type_env())?;
    let args = plan
        .args
        .iter()
        .map(|part| {
            lower_expr_for_slot_with_refinements(
                part,
                env,
                Slot::StringConcatSegment,
                ctx,
                refinements,
            )
        })
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
    for (index, stmt) in block.statements.iter().enumerate() {
        let later_read_names = statement_suffix_read_names(&block.statements[index + 1..]);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            ..ctx
        };
        lower_stmt_for_body(stmt, update, env, wrap_returns_in_ok, stmt_ctx, &mut out);
        if !matches!(stmt, Stmt::VarDecl { .. }) {
            learn_stmt_shapes(
                stmt,
                env,
                stmt_ctx.current_local_shapes,
                stmt_ctx.signatures,
            );
        }
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
        ForBodyStmtPlan::Default { stmt } => lower_stmt(
            stmt,
            env,
            wrap_returns_in_ok,
            ctx,
            &ExprRefinements::default(),
            out,
        ),
    }
}
