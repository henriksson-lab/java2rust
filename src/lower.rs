use std::collections::{BTreeSet, HashMap, HashSet};

use crate::access_lowering::{lower_bare_name, lower_field_access_with_facts};
use crate::array_lowering::{
    array2_from_rows_projection, array2_index_projection, array2_row_mut_assign_stmt,
    borrowed_array2_temporary_for_slot, ragged_array_outer_creation_projection, render_array_expr,
};
use crate::array_plan::plan_array_expr;
use crate::call_lowering::{
    apply_call_target, apply_class_literal_token, apply_collection_remove_if,
    apply_native_collection_extend, apply_new_target_with_facts,
};
use crate::call_plan::{
    ArgumentLoweringPlan, CallExceptionPropagationPlan, CallExprPlan, CallStatementPlan,
    CallTargetPlan, NativeCollectionExtendArgPlan, NativeCollectionMethod,
    NativeCollectionNewTarget, NativeJavaInstanceMethod, NativePrintStreamMethod,
    NativeSystemStream, apply_fallback_receiver_rust_name, apply_fallback_self_rust_name,
    apply_fallback_static_owner_rust_name,
    external_call_lowered_type_from_static_owner_plan_with_external_index,
    external_call_lowered_type_with_external_index,
    external_call_returns_rust_result_with_external_index,
    external_constructor_arg_shapes_with_args, external_constructor_returns_rust_result,
    external_receiver_call_return_type_with_external_index,
    facade_abi_argument_plan_for_rust_method_name, facade_abi_argument_plan_for_rust_path,
    facade_receiver_slot_plan, known_external_call_lowered_type, native_collection_lowered_type,
    non_self_trivial_accessor_target, optional_borrowed_receiver_slot_plan,
    optional_receiver_slot_plan, plan_call_arguments, plan_call_arguments_with_facts_and_args,
    plan_call_exception_propagation_with_external_index, plan_call_expr_with_external_index,
    plan_call_statement, plan_constructor_arguments, plan_native_collection_extend_arg,
    plan_new_target_for_args_with_facts, resource_close_declares_result_with_external_index,
    retarget_static_call_to_receiver_if,
};
use crate::coercion::{
    BorrowProjectionUse, Coercion, JavaDisplayPlan, OptionalSequenceReceiverProjection,
    SequenceIterationProjection, abstract_interface_method_stub_projection,
    abstract_method_stub_projection, anonymous_dyn_trait_projection, apply_audited_clone_recovery,
    apply_clone_projection, apply_coercions, apply_java_display_plan, apply_literal_slot_plan,
    array_fill_projection, array2_rows_i32_projection, borrow_as_dyn_trait,
    borrowed_sequence_extend_arg, borrowed_string_literal_temporary_plan,
    boxed_dyn_trait_projection, char_sequence_collect_string_projection,
    clone_erased_object_ref_once, collection_sort_by_comparator_projection,
    collection_sort_projection, constructor_self_field_projection,
    constructor_self_field_replacement, deref_borrowed_projection, ensure_borrow_projection,
    erased_object_borrowed_downcast_ref_projection, erased_object_box_projection,
    exception_self_as_supertype_facade_boundary_projection,
    explicit_java_facade_cast_placeholder_projection, force_shared_borrow_projection,
    function_call_projection, into_projection, iterator_next_expect_projection,
    java_integral_literal_char, java_update_expression_projection, len_i32_projection,
    literal_slot_plan, logical_not_projection, map_get_expect_projection,
    map_get_mut_expect_projection, map_get_projection, normalize_map_key_borrow_projection,
    normalize_reused_clone_for_borrowed_argument, optional_copied_expect_projection,
    outer_this_field_projection, owned_map_keys_set_projection, owner_associated_path_projection,
    parenthesize_borrowed_method_receiver, print_macro_projection,
    project_optional_element_borrow_carrier, project_optional_receiver,
    project_optional_sequence_receiver, propagate_result_projection,
    qualified_static_storage_accessor_projection, qualified_static_storage_setter_path_projection,
    reborrow_optional_mut_projection, receiver_method_call_projection_exact,
    render_sequence_iteration_source, rendered_expr_has_clone_recovery,
    rendered_expr_has_direct_clone_recovery, rendered_expr_is_mutable_alias,
    rewrite_downcast_expect_message, rust_expr_is_constructed_optional_value,
    rust_expr_mentions_constructor_self_temporary, self_associated_path_projection,
    sequence_get_expect_projection, sequence_iter_cloned_projection,
    sequence_iter_copied_projection, snapshot_rendered_shared_borrow_argument,
    static_storage_accessor_projection, static_storage_getter_body_projection,
    static_storage_setter_body_projection, static_storage_setter_path_projection,
    subtype_owned_constructor_from_borrowed_projection, to_owned_sequence_projection,
    tuple_field_projection, typed_none_projection, unsupported_constructor_call_handoff_projection,
    unsupported_deserialization_cast_handoff_projection,
    unsupported_outer_capture_constructor_projection, unwrap_java_null_receiver_projection,
    unwrap_optional_shared_value, unwrap_result_or_panic_uncaught_java_exception,
    varargs_packed_vec_projection, varargs_spread_extend_projection,
};
use crate::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, DelegatedConstructorTrailingStmt,
    plan_constructor_delegation_for_type, plan_constructor_field_values_with_inherited_context,
};
use crate::control_flow_lowering::{TailRecursiveLoopRender, render_tail_recursive_loop_body};
use crate::conversion_lowering::{
    adapt_optional_assignment_value_for_target as render_adapt_optional_assignment_value_for_target,
    apply_cast_conversion, apply_instanceof, apply_lowered_slot_expr_conversion,
    apply_slot_expr_conversion, construct_optional_value_for_target,
};
use crate::conversion_plan::{
    CloneRecoveryPolicy, DispatchCoercionPolicy, LoweredType, OptionalSlotConstructionPlan,
    ReturnValuePlan, Slot, SlotConversionPlan, actual_shape_for_slot_expr,
    actual_shape_for_slot_expr_with_slot, argument_slot, assignment_statement_plan_for_target,
    assignment_value_plan_for_shape, assignment_value_plan_for_target,
    borrowed_element_target_matches_element, condition_slot, index_slot,
    optional_element_slot_shape_with_facts, optional_slot_construction_plan_for_actual,
    plan_cast_conversion_from_actual, plan_instanceof, plan_lowered_slot_conversion,
    plan_slot_conversion, receiver_method_autoref_satisfies_facade_slot,
    receiver_method_autoref_satisfies_optional_slot, return_value_plan,
    shape_coercion_plan_with_facts,
};
use crate::diagnostics::DiagnosticBag;
use crate::emit;
use crate::exception_lowering::{
    CatchRenderArm, exception_cause_or_self_if_instance_expr,
    exception_expr_from_new_with_cause_hint, exception_result_type, exception_try_ok_type,
    fallback_exception_expr, java_exception_support_item_with_facts, return_stmt,
    throw_stmt_with_result_policy, try_catch_finally_scope_stmt, try_catch_stmt,
    try_catch_value_fallthrough_stmt, try_finally_scope_stmt, try_with_resources_scope_stmt,
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
use crate::facts::{
    CallableKey, ExternalFactLookupIndex, FactSet, HierarchyStrategy, NullabilityState,
    OwnedRecoveryCapability, ResultAbi, Staticness,
};
use crate::item_assembly::{
    constructor_default_field_value_with_facts, constructor_function_shell_with_facts,
    constructor_return_body, delegating_constructor_return_body, impl_item_for_type,
    is_static_final_field, method_function_shell_with_facts_and_exception_plan, raw_item,
    static_storage_initial_fallback_value, struct_item_for_type_with_facts_and_support_context,
};
use crate::java_ir::Member as Jm;
use crate::java_ir::*;
use crate::javalibs::{broad_ioutils_support_item, program_uses_broad_ioutils_facade};
use crate::loop_lowering::{assemble_for_loop, assignment_condition_loop, continue_with_updates};
use crate::loop_plan::{ForBodyStmtPlan, WhileStmtPlan, plan_for_body_stmt, plan_while_stmt};
use crate::lowering_diagnostics::{LoweringDiagnosticPlan, collect_lowering_diagnostic_plans};
use crate::method_reference_lowering::{
    render_method_reference, render_typed_single_input_method_reference_callback_with_facts,
};
use crate::method_reference_plan::{
    MethodReferencePlan, plan_current_receiver_method_reference, plan_method_reference,
};
use crate::nullability_plan::{TernaryNullRefinementPlan, plan_ternary_null_refinement};
use crate::operator_lowering::lower_equality_comparison_expr_for_op;
use crate::operator_lowering::{lower_null_comparison_expr, lower_unary_expr};
use crate::operator_lowering::{
    lower_numeric_binary_expr, plan_equality_comparison, plan_equality_comparison_with_facts,
    plan_numeric_binary_promotion,
};
use crate::rewrite::{MethodControlFlowPlan, method_has_tail_recursive_loop_rewrite};
use crate::rust_ir::*;
use crate::shape_projection::{
    borrowed_sequence_element_shape, borrowed_string_content_shape,
    indexed_read_shape_for_access_with_facts, project_java_value_shape, shape_is_concrete_native,
    shape_is_erased_object_boundary, shape_is_owned, shape_is_owned_non_erased, storage_shape,
    value_shape,
};
use crate::signature::{
    LocalPlan, ReceiverPlan, SignatureConstraint, SignaturePlan, SolvedSlotShape,
    apply_signature_constraints,
};
use crate::slot_policy::borrowed_content_type_policy;
use crate::slot_render::{
    erased_object_rust_type, rust_type_for_java_type_with_facts, rust_type_for_slot_with_facts,
};
use crate::slot_shape::{DispatchShape, MutabilityShape, ViewShape};
use crate::slot_shape::{OwnershipShape, RustSlotShape};
use crate::statement_render::{
    assignment_stmt, break_stmt, continue_stmt, expr_stmt, if_stmt,
    local_let_stmt_with_facts_and_type_annotation, raw_stmt, render_block_stmt,
    render_enhanced_for, render_enhanced_for_iterator, while_stmt,
};
use crate::stdlib::{
    external_api_facade_support_items_with_local_type_program,
    external_static_string_constant_support_items_with_context_programs_and_declared_owners,
    java_class_support_item_with_classloader_shim, java_erased_object_runtime_support_item,
    java_platform_facade_support_item, java_string_format_support_item,
    java_string_split_support_item, program_uses_java_class_tokens,
    program_uses_java_erased_object_runtime_support, program_uses_java_platform_facades,
    program_uses_java_string_format, program_uses_java_string_runtime_helpers,
    support_item_declared_type_names,
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
    unsupported_constructor_body, unsupported_constructor_message_policy,
    unsupported_expression_expr, unsupported_member_diagnostic_policy, unsupported_statement_stmt,
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
    later_mutated_names: Option<&'a std::collections::HashSet<String>>,
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
        later_mutated_names: None,
        current_stmt_read_counts: None,
        exclusive_terminal_branch_read_names: None,
        preinitialized_try_capture_names: None,
    };
    let mut file = RustFile::default();
    if exceptions.support.emit_support_item {
        file.items
            .push(java_exception_support_item_with_facts(ctx.facts));
    }
    let uses_broad_ioutils = program_uses_broad_ioutils_facade(program);
    let uses_java_platform_facades =
        program_uses_java_platform_facades(program) || uses_broad_ioutils;
    if program_uses_java_class_tokens(program)
        || program_has_source_object_get_class_fallback_items(program)
    {
        file.items
            .push(java_class_support_item_with_classloader_shim(
                !uses_java_platform_facades,
            ));
    }
    if program_uses_java_erased_object_runtime_support(program)
        || program_has_source_object_to_string_fallback_items(program)
    {
        file.items.push(java_erased_object_runtime_support_item());
    }
    if uses_java_platform_facades {
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
    let occupied_inherent_methods = occupied_inherent_methods_for_items(&file.items);
    file.items.extend(source_object_to_string_fallback_items(
        program,
        &occupied_inherent_methods,
    ));
    LoweredProgram { file, diagnostics }
}

fn source_object_to_string_fallback_items(
    program: &Program,
    occupied_inherent_methods: &BTreeSet<(String, String)>,
) -> Vec<RustItem> {
    let mut items = Vec::new();
    for ty in &program.types {
        collect_source_object_to_string_fallback_items(ty, occupied_inherent_methods, &mut items);
    }
    items
}

fn program_has_source_object_to_string_fallback_items(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(type_has_source_object_to_string_fallback_item)
}

fn program_has_source_object_get_class_fallback_items(program: &Program) -> bool {
    program
        .types
        .iter()
        .any(type_has_source_object_get_class_fallback_item)
}

fn type_has_source_object_to_string_fallback_item(ty: &TypeDecl) -> bool {
    (matches!(ty.kind, TypeKind::Class | TypeKind::Record)
        && !type_declares_zero_arg_rust_method(ty, "to_string"))
        || ty.members.iter().any(|member| match member {
            Member::Type(nested) => type_has_source_object_to_string_fallback_item(nested),
            _ => false,
        })
}

fn type_has_source_object_get_class_fallback_item(ty: &TypeDecl) -> bool {
    (matches!(ty.kind, TypeKind::Class | TypeKind::Record)
        && !type_declares_zero_arg_rust_method(ty, "get_class"))
        || ty.members.iter().any(|member| match member {
            Member::Type(nested) => type_has_source_object_get_class_fallback_item(nested),
            _ => false,
        })
}

fn collect_source_object_to_string_fallback_items(
    ty: &TypeDecl,
    occupied_inherent_methods: &BTreeSet<(String, String)>,
    items: &mut Vec<RustItem>,
) {
    if matches!(ty.kind, TypeKind::Class | TypeKind::Record) {
        let rust_name = crate::slot_render::rust_name_for_java_named_type(&ty.name);
        let mut methods = Vec::new();
        if !type_declares_zero_arg_rust_method(ty, "to_string")
            && !occupied_inherent_methods.contains(&(rust_name.clone(), "to_string".to_string()))
        {
            methods.push(
                "    pub fn to_string(&self) -> String { java_erased_object_to_string(self) }"
                    .to_string(),
            );
        }
        if !type_declares_zero_arg_rust_method(ty, "get_class")
            && !occupied_inherent_methods.contains(&(rust_name.clone(), "get_class".to_string()))
        {
            methods.push(format!(
                "    pub fn get_class(&self) -> JavaClass<{rust_name}> {{ JavaClass::<{rust_name}>::literal({:?}) }}",
                ty.name
            ));
        }
        if !methods.is_empty() {
            items.push(RustItem::Raw(format!(
                "impl {rust_name} {{\n{}\n}}",
                methods.join("\n")
            )));
        }
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_source_object_to_string_fallback_items(
                nested,
                occupied_inherent_methods,
                items,
            );
        }
    }
}

fn type_declares_zero_arg_rust_method(ty: &TypeDecl, rust_name: &str) -> bool {
    ty.members.iter().any(|member| {
        let Member::Method(method) = member else {
            return false;
        };
        method.params.is_empty() && crate::parser::to_snake_case(&method.name) == rust_name
    })
}

fn occupied_inherent_methods_for_items(items: &[RustItem]) -> BTreeSet<(String, String)> {
    let mut occupied = BTreeSet::new();
    for item in items {
        match item {
            RustItem::Impl(imp) => {
                for function in &imp.functions {
                    occupied.insert((imp.target.clone(), function.name.clone()));
                }
            }
            RustItem::Raw(raw) => collect_raw_inherent_methods(raw, &mut occupied),
            RustItem::Struct(_) | RustItem::Function(_) | RustItem::Comment(_) => {}
        }
    }
    occupied
}

fn collect_raw_inherent_methods(raw: &str, occupied: &mut BTreeSet<(String, String)>) {
    let mut current_owner: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if let Some(owner) = raw_inherent_impl_owner(trimmed) {
            current_owner = Some(owner);
        }
        if let Some(owner) = current_owner.as_ref()
            && let Some(method_name) = raw_impl_method_name(trimmed)
        {
            occupied.insert((owner.clone(), method_name));
        }
        if trimmed == "}" {
            current_owner = None;
        }
    }
}

fn raw_inherent_impl_owner(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("impl ")?;
    if rest.contains(" for ") {
        return None;
    }
    let rest = if rest.starts_with('<') {
        let generic_end = rest.find('>')?;
        rest[generic_end + 1..].trim_start()
    } else {
        rest
    };
    let owner = rest
        .split(|ch: char| ch == '<' || ch == '{' || ch.is_whitespace())
        .next()
        .unwrap_or_default();
    (!owner.is_empty()).then(|| owner.to_string())
}

fn raw_impl_method_name(trimmed: &str) -> Option<String> {
    let rest = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))?;
    let name = rest
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .unwrap_or_default();
    (!name.is_empty()).then(|| name.to_string())
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

fn seed_context_static_type_aliases(env: &mut ShapeEnv, ctx: LowerCtx) {
    seed_program_static_type_aliases(env, ctx.program);
    seed_program_static_type_aliases(env, ctx.support_local_type_program);
    seed_program_static_type_aliases(env, ctx.source_summary_type_program);
}

fn seed_program_static_type_aliases(env: &mut ShapeEnv, program: &Program) {
    for ty in &program.types {
        seed_type_static_type_aliases(env, ty, None);
    }
}

fn seed_type_static_type_aliases(env: &mut ShapeEnv, ty: &TypeDecl, outer_simple: Option<&str>) {
    let simple = ty
        .name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(ty.name.as_str());
    env.insert_static_type_alias(simple.to_string(), ty.name.clone());
    env.insert_static_type_alias(crate::parser::to_snake_case(simple), ty.name.clone());
    env.insert_static_type_alias(crate::parser::to_snake_case(&ty.name), ty.name.clone());
    if let Some(outer_simple) = outer_simple
        && let Some(nested_simple) = simple.strip_prefix(outer_simple)
        && !nested_simple.is_empty()
    {
        let alias = upper_camel_type_name_from_static_receiver(nested_simple);
        env.insert_static_type_alias(alias.clone(), ty.name.clone());
        env.insert_static_type_alias(crate::parser::to_snake_case(&alias), ty.name.clone());
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            seed_type_static_type_aliases(env, nested, Some(simple));
        }
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
                if crate::item_assembly::static_final_field_has_associated_const(ty, field) {
                    env.insert_type_field_shape(&ty.name, field.name.clone(), shape.clone());
                    env.insert_static_field_shape_for_owner(&ty.name, field.name.clone(), shape);
                } else if field.modifiers.contains(&Modifier::Static) {
                    env.insert_type_static_storage_field_shape(
                        &ty.name,
                        field.name.clone(),
                        shape.clone(),
                    );
                    env.insert_static_storage_field_shape(field.name.clone(), shape);
                } else {
                    env.insert_type_field_shape(&ty.name, field.name.clone(), shape);
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

fn seed_outer_this_capture_fields(env: &mut ShapeEnv, ty: &TypeDecl, ctx: LowerCtx) {
    let Some(outer_type) = ctx.facts.captures.outer_this_type_for_type(&ty.name) else {
        return;
    };
    let Some(outer_decl) = find_context_type_by_key(ctx.program, outer_type)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, outer_type))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, outer_type))
    else {
        return;
    };
    for member in &outer_decl.members {
        let Member::Field(field) = member else {
            continue;
        };
        let Some(shape) =
            signature_field_shape_for_type_aliases(ctx.signatures, outer_type, &field.name)
                .cloned()
        else {
            continue;
        };
        env.insert_field_shape(field.name.clone(), shape);
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
            if let Some(item) = nested_type_simple_alias_item(nested) {
                file.items.push(raw_item(item));
            }
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
        if matches!(ty.kind, TypeKind::Enum) {
            let functions = ty
                .members
                .iter()
                .filter_map(|member| match member {
                    Member::Method(method) => Some(lower_method(program, ty, method, ctx)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !functions.is_empty() {
                file.items.push(RustItem::Impl(RustImpl {
                    target: crate::slot_render::rust_name_for_java_named_type(&ty.name),
                    associated_consts: Vec::new(),
                    functions,
                }));
            }
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
        return;
    }

    file.items
        .push(struct_item_for_type_with_facts_and_support_context(
            ty,
            ctx.signatures,
            ctx.facts,
            ctx.support_local_type_program,
        ));
    for item in static_storage_items_for_type(ty, ctx) {
        file.items.push(raw_item(item));
    }

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
    for inherited in crate::inherited_members::inherited_instance_method_refs_for_type(
        ty,
        ctx.support_local_type_program,
    ) {
        functions.push(lower_inherited_method(program, ty, inherited, ctx));
    }
    if type_has_implicit_default_constructor(ty, ctx.support_local_type_program) {
        functions.push(lower_implicit_default_constructor(program, ty, ctx));
    }
    functions.extend(static_storage_accessor_functions_for_type(ty, ctx));
    functions.extend(abstract_interface_method_stubs(ty, ctx));
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
    for item in source_supertype_conversion_impls_for_type(ty, ctx) {
        file.items.push(raw_item(item));
    }
    for item in source_external_supertype_conversion_impls_for_type(ty, ctx) {
        file.items.push(raw_item(item));
    }
}

fn static_storage_items_for_type(ty: &TypeDecl, ctx: LowerCtx) -> Vec<String> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            if !field.modifiers.contains(&Modifier::Static)
                || crate::item_assembly::static_final_field_has_associated_const(ty, field)
            {
                return None;
            }
            let shape = ctx.signatures.required_field_shape(&ty.name, &field.name);
            let rust_ty = crate::slot_render::rust_type_for_slot_with_facts(&shape, ctx.facts);
            Some(format!(
                "std::thread_local! {{\n    static {}: std::cell::RefCell<Option<{}>> = const {{ std::cell::RefCell::new(None) }};\n}}",
                static_storage_symbol_name(ty, &field.name),
                emit::emit_type(&rust_ty)
            ))
        })
        .collect()
}

fn static_storage_accessor_functions_for_type(ty: &TypeDecl, ctx: LowerCtx) -> Vec<RustFunction> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            if !field.modifiers.contains(&Modifier::Static)
                || crate::item_assembly::static_final_field_has_associated_const(ty, field)
            {
                return None;
            }
            Some(static_storage_accessors_for_field(ty, field, ctx))
        })
        .flatten()
        .collect()
}

fn static_storage_accessors_for_field(
    ty: &TypeDecl,
    field: &FieldDecl,
    ctx: LowerCtx,
) -> Vec<RustFunction> {
    let shape = ctx.signatures.required_field_shape(&ty.name, &field.name);
    let rust_ty = crate::slot_render::rust_type_for_slot_with_facts(&shape, ctx.facts);
    let getter_name = format!("__static_{}", crate::parser::to_snake_case(&field.name));
    let setter_name = format!("__set_static_{}", crate::parser::to_snake_case(&field.name));
    let storage_name = static_storage_symbol_name(ty, &field.name);
    let initial = static_storage_initial_value_expr(ty, field, &shape, ctx);
    vec![
        RustFunction {
            name: getter_name,
            params: Vec::new(),
            return_type: Some(rust_ty.clone()),
            body: vec![RustStmt::Return(Some(
                static_storage_getter_body_projection(&storage_name, initial),
            ))],
            is_pub: !field.modifiers.contains(&Modifier::Private),
            receiver: None,
        },
        RustFunction {
            name: setter_name,
            params: vec![crate::rust_ir::RustParam {
                name: "value".to_string(),
                ty: rust_ty,
            }],
            return_type: None,
            body: vec![RustStmt::Raw(static_storage_setter_body_projection(
                &storage_name,
            ))],
            is_pub: !field.modifiers.contains(&Modifier::Private),
            receiver: None,
        },
    ]
}

fn static_storage_initial_value_expr(
    ty: &TypeDecl,
    field: &FieldDecl,
    shape: &crate::slot_shape::RustSlotShape,
    ctx: LowerCtx,
) -> RustExpr {
    let Some(initializer) = field.initializer.as_ref() else {
        return static_storage_initial_fallback_value(shape, ctx.facts);
    };
    if matches!(shape.java_ty, JavaType::String)
        && let Some(value) = static_string_storage_initial_value_expr(ty, initializer, ctx)
    {
        return value;
    }
    if matches!(shape.java_ty, JavaType::Primitive(_))
        && let Some(value) = static_primitive_storage_initial_value_expr(ty, initializer, ctx)
    {
        return value;
    }
    let value = match initializer {
        Expr::Literal(Literal::Null) if shape.optional_element_type().is_some() => {
            return typed_none_for_slot(&Slot::Return(shape.clone()))
                .unwrap_or_else(|| RustExpr::Raw("None".to_string()));
        }
        Expr::Literal(literal) => literal_expr(literal),
        Expr::Var(name) => {
            let reference = emit::emit_expr(&self_associated_path_projection(
                &static_const_reference_name_for_type(ty, name),
            ));
            if matches!(shape.java_ty, JavaType::String) {
                RustExpr::Raw(format!("{reference}.to_string()"))
            } else {
                RustExpr::Raw(reference)
            }
        }
        Expr::New { ty, args, body, .. } if args.is_empty() && body.is_none() => {
            let rust_name = crate::java_naming::rust_name_for_java_named_type(
                concrete_java_type_name(ty).unwrap_or_else(|| match ty {
                    JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
                    _ => "",
                }),
            );
            if rust_name.is_empty() {
                static_storage_initial_fallback_value(shape, ctx.facts)
            } else {
                function_call_projection(
                    owner_associated_path_projection(&rust_name, "new"),
                    Vec::new(),
                )
            }
        }
        _ => static_storage_initial_fallback_value(shape, ctx.facts),
    };
    if let Some(element_ty) = shape.optional_element_type() {
        planned_optional_value_for_target(
            value,
            RustSlotShape::conservative(element_ty),
            shape,
            ctx.facts,
        )
    } else {
        value
    }
}

fn static_primitive_storage_initial_value_expr(
    ty: &TypeDecl,
    initializer: &Expr,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    match initializer {
        Expr::Literal(literal) => Some(literal_expr(literal)),
        Expr::Var(name) => static_storage_reference_expr(ty, name, None, ctx),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            static_storage_reference_expr(ty, field, None, ctx)
        }
        Expr::FieldAccess(receiver, field) => {
            let Expr::Var(owner) = receiver.as_ref() else {
                return None;
            };
            static_storage_reference_expr(ty, field, Some(owner), ctx)
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.is_empty() && matches!(name.as_str(), "length" | "len") => {
            let receiver = static_string_storage_initial_value_expr(ty, receiver, ctx)?;
            Some(RustExpr::Raw(format!(
                "{}.len() as i32",
                emit::emit_expr(&receiver)
            )))
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => Some(RustExpr::Unary {
            op: "-".to_string(),
            expr: Box::new(static_primitive_storage_initial_value_expr(ty, expr, ctx)?),
        }),
        Expr::Unary {
            op: UnaryOp::Pos,
            expr,
        } => static_primitive_storage_initial_value_expr(ty, expr, ctx),
        Expr::Cast { expr, ty: cast_ty } if primitive_rust_cast_type(cast_ty).is_some() => {
            Some(RustExpr::Cast {
                expr: Box::new(static_primitive_storage_initial_value_expr(ty, expr, ctx)?),
                ty: primitive_rust_cast_type(cast_ty)?.to_string(),
            })
        }
        Expr::Binary { op, left, right } if static_storage_binary_op_is_allowed(*op) => {
            Some(RustExpr::Binary {
                op: crate::operator_lowering::binary_op_to_rust(*op).to_string(),
                left: Box::new(static_primitive_storage_initial_value_expr(ty, left, ctx)?),
                right: Box::new(static_primitive_storage_initial_value_expr(ty, right, ctx)?),
            })
        }
        _ => None,
    }
}

fn static_storage_reference_expr(
    current_ty: &TypeDecl,
    field_name: &str,
    owner_name: Option<&str>,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let owner_ty = owner_name
        .and_then(|owner| {
            find_type_decl_by_name(ctx.program, owner)
                .or_else(|| find_type_decl_by_name(ctx.support_local_type_program, owner))
                .or_else(|| find_type_decl_by_name(ctx.source_summary_type_program, owner))
        })
        .unwrap_or(current_ty);
    let field = static_field_decl_by_name(owner_ty, field_name)?;
    let member = static_const_reference_name_for_type(owner_ty, field_name);
    let owner = owner_name.map(crate::slot_render::rust_name_for_java_named_type);
    if crate::item_assembly::static_final_field_has_associated_const(owner_ty, field) {
        return Some(match owner {
            Some(owner) => owner_associated_path_projection(&owner, member),
            None => self_associated_path_projection(member),
        });
    }
    Some(match owner {
        Some(owner) => qualified_static_storage_accessor_projection(&owner, member),
        None => static_storage_accessor_projection(member),
    })
}

fn primitive_rust_cast_type(ty: &JavaType) -> Option<&'static str> {
    match ty {
        JavaType::Primitive(PrimitiveType::Byte) => Some("i8"),
        JavaType::Primitive(PrimitiveType::Short) => Some("i16"),
        JavaType::Primitive(PrimitiveType::Int) => Some("i32"),
        JavaType::Primitive(PrimitiveType::Long) => Some("i64"),
        JavaType::Primitive(PrimitiveType::Float) => Some("f32"),
        JavaType::Primitive(PrimitiveType::Double) => Some("f64"),
        JavaType::Primitive(PrimitiveType::Char) => Some("char"),
        JavaType::Primitive(PrimitiveType::Boolean) => Some("bool"),
        _ => None,
    }
}

fn static_storage_binary_op_is_allowed(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
    )
}

fn static_string_storage_initial_value_expr(
    ty: &TypeDecl,
    initializer: &Expr,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    match initializer {
        Expr::Literal(Literal::String(value)) => {
            Some(RustExpr::Literal(format!("{value:?}.to_string()")))
        }
        Expr::Var(name) => static_string_storage_reference_expr(ty, name, None, ctx),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            static_string_storage_reference_expr(ty, field, None, ctx)
        }
        Expr::FieldAccess(receiver, field) => {
            let Expr::Var(owner) = receiver.as_ref() else {
                return None;
            };
            static_string_storage_reference_expr(ty, field, Some(owner), ctx)
        }
        Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } => {
            let left = static_string_storage_initial_value_expr(ty, left, ctx)?;
            let right = static_string_storage_initial_value_expr(ty, right, ctx)?;
            Some(RustExpr::Raw(format!(
                "format!(\"{{}}{{}}\", {}, {})",
                emit::emit_expr(&left),
                emit::emit_expr(&right)
            )))
        }
        _ => None,
    }
}

fn static_string_storage_reference_expr(
    current_ty: &TypeDecl,
    field_name: &str,
    owner_name: Option<&str>,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let owner_ty = owner_name
        .and_then(|owner| {
            find_type_decl_by_name(ctx.program, owner)
                .or_else(|| find_type_decl_by_name(ctx.support_local_type_program, owner))
                .or_else(|| find_type_decl_by_name(ctx.source_summary_type_program, owner))
        })
        .unwrap_or(current_ty);
    let field = static_field_decl_by_name(owner_ty, field_name)?;
    if !matches!(field.ty, JavaType::String) {
        return None;
    }
    let member = static_const_reference_name_for_type(owner_ty, field_name);
    let owner = owner_name.map(crate::slot_render::rust_name_for_java_named_type);
    if crate::item_assembly::static_final_field_has_associated_const(owner_ty, field) {
        let path = if let Some(owner) = owner {
            owner_associated_path_projection(&owner, member)
        } else {
            self_associated_path_projection(member)
        };
        return Some(RustExpr::Raw(format!(
            "{}.to_string()",
            emit::emit_expr(&path)
        )));
    }
    Some(match owner {
        Some(owner) => qualified_static_storage_accessor_projection(&owner, member),
        None => static_storage_accessor_projection(member),
    })
}

fn static_field_decl_by_name<'a>(ty: &'a TypeDecl, name: &str) -> Option<&'a FieldDecl> {
    let normalized = crate::parser::to_snake_case(name);
    ty.members.iter().find_map(|member| {
        let Member::Field(field) = member else {
            return None;
        };
        (field.modifiers.contains(&Modifier::Static)
            && (field.name == name || crate::parser::to_snake_case(&field.name) == normalized))
            .then_some(field)
    })
}

fn static_const_reference_name_for_type<'a>(ty: &'a TypeDecl, name: &'a str) -> &'a str {
    ty.members
        .iter()
        .find_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            (is_static_final_field(field)
                && (field.name == name || crate::parser::to_snake_case(&field.name) == name))
                .then_some(field.name.as_str())
        })
        .unwrap_or(name)
}

fn static_storage_symbol_name(ty: &TypeDecl, field_name: &str) -> String {
    let type_name = crate::slot_render::rust_name_for_java_named_type(&ty.name);
    let raw = format!(
        "{}_{}",
        crate::parser::to_snake_case(&type_name),
        crate::parser::to_snake_case(field_name)
    );
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn source_supertype_conversion_impls_for_type(ty: &TypeDecl, ctx: LowerCtx) -> Vec<String> {
    let mut impls = Vec::new();
    let mut current = ty.extends.as_deref();
    let mut visited = HashSet::new();
    while let Some(super_name) = current {
        let Some(super_ty) = find_type_decl_by_name(ctx.support_local_type_program, super_name)
            .or_else(|| find_type_decl_by_name(ctx.source_summary_type_program, super_name))
        else {
            break;
        };
        if !visited.insert(super_ty.name.clone()) {
            break;
        }
        if source_supertype_conversion_target_is_emitted(super_ty, ctx)
            && let Some(item) = source_supertype_conversion_impl_for_pair(
                ty,
                super_ty,
                ctx.support_local_type_program,
                ctx.source_summary_type_program,
            )
        {
            impls.push(item);
        }
        current = super_ty.extends.as_deref();
    }
    impls
}

fn source_external_supertype_conversion_impls_for_type(
    ty: &TypeDecl,
    ctx: LowerCtx,
) -> Vec<String> {
    let sub_name = crate::slot_render::rust_name_for_java_named_type(&ty.name);
    crate::stdlib::resolved_external_super_names_for_method_queries(
        ctx.facts,
        ty,
        ctx.support_local_type_program,
    )
    .into_iter()
    .flat_map(|super_name| {
        ctx.facts
            .external_conversion_adapter_facts
            .iter()
            .cloned()
            .chain(crate::external_packs::external_conversion_adapters())
            .filter(move |adapter| {
                matches!(
                    adapter.kind,
                    crate::facts::ExternalConversionAdapterKind::Into
                ) && crate::facts::external_type_name_matches(&adapter.from_type, &super_name)
            })
    })
    .filter_map(|adapter| {
        let to_key = crate::stdlib::canonical_external_receiver_type_key(&adapter.to_type);
        let to_name = crate::slot_render::rust_name_for_java_named_type(&to_key);
        if matches!(to_name.as_str(), "OutputStream") {
            return None;
        }
        (sub_name != to_name).then(|| {
            format!(
                "impl From<{sub_name}> for {to_name} {{\n    fn from(_value: {sub_name}) -> Self {{ Default::default() }}\n}}"
            )
        })
    })
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect()
}

fn source_supertype_conversion_target_is_emitted(super_ty: &TypeDecl, ctx: LowerCtx) -> bool {
    find_type_decl_by_name(ctx.program, &super_ty.name).is_some()
        || (ctx.context_sources_are_emitted
            && (find_type_decl_by_name(ctx.support_local_type_program, &super_ty.name).is_some()
                || find_type_decl_by_name(ctx.source_summary_type_program, &super_ty.name)
                    .is_some()))
}

fn source_supertype_conversion_impl_for_pair(
    sub_ty: &TypeDecl,
    super_ty: &TypeDecl,
    support_program: &Program,
    source_summary_program: &Program,
) -> Option<String> {
    let field_names = source_instance_field_names_for_conversion(
        super_ty,
        support_program,
        source_summary_program,
    );
    let sub_name = crate::slot_render::rust_name_for_java_named_type(&sub_ty.name);
    let super_name = crate::slot_render::rust_name_for_java_named_type(&super_ty.name);
    if sub_name == super_name {
        return None;
    }
    let fields = field_names
        .into_iter()
        .map(|name| format!("            {name}: value.{name},"))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "impl From<{sub_name}> for {super_name} {{\n    fn from(value: {sub_name}) -> Self {{\n        {super_name} {{\n{fields}\n        }}\n    }}\n}}"
    ))
}

fn source_instance_field_names_for_conversion(
    ty: &TypeDecl,
    support_program: &Program,
    source_summary_program: &Program,
) -> Vec<String> {
    let mut fields = Vec::new();
    fields.extend(ty.members.iter().filter_map(|member| match member {
        Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
            Some(field.name.clone())
        }
        _ => None,
    }));
    fields.extend(
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
            .into_iter()
            .map(|inherited| inherited.field.name),
    );
    fields.extend(
        crate::inherited_members::inherited_instance_fields_for_type(ty, source_summary_program)
            .into_iter()
            .map(|inherited| inherited.field.name),
    );
    let mut seen = HashSet::new();
    fields.retain(|field| seen.insert(field.clone()));
    fields
}

fn type_has_implicit_default_constructor(ty: &TypeDecl, support_program: &Program) -> bool {
    matches!(ty.kind, TypeKind::Class | TypeKind::Record)
        && !ty
            .members
            .iter()
            .any(|member| matches!(member, Member::Constructor(_)))
        && program_uses_zero_arg_constructor_for_type(support_program, &ty.name)
}

fn abstract_interface_method_stubs(ty: &TypeDecl, ctx: LowerCtx) -> Vec<RustFunction> {
    if !ty.modifiers.contains(&Modifier::Abstract) {
        return Vec::new();
    }
    ty.implements
        .iter()
        .filter_map(|interface_name| {
            lower_find_type_decl(ctx.support_local_type_program, interface_name)
        })
        .flat_map(|interface| {
            interface.members.iter().filter_map(|member| {
                let Member::Method(interface_method) = member else {
                    return None;
                };
                if lower_implementing_method_candidates(ty, interface_method)
                    .into_iter()
                    .any(|candidate| candidate.body.is_some())
                {
                    return None;
                }
                Some(abstract_interface_method_stub(
                    interface,
                    interface_method,
                    ctx,
                ))
            })
        })
        .collect()
}

fn abstract_interface_method_stub(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    ctx: LowerCtx,
) -> RustFunction {
    let exception_plan = ctx.exceptions.method_plan(interface, interface_method);
    let return_expr = Some(abstract_interface_method_stub_projection(
        &interface.name,
        &interface_method.name,
    ));
    let body = vec![return_stmt(
        return_expr,
        exception_plan.wrap_returns_in_result,
    )];
    method_function_shell_with_facts_and_exception_plan(
        interface,
        interface_method,
        ctx.signatures,
        body,
        ctx.facts,
        exception_plan,
    )
}

fn lower_implementing_method_candidates<'a>(
    ty: &'a TypeDecl,
    interface_method: &MethodDecl,
) -> Vec<&'a MethodDecl> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if method.name == interface_method.name => Some(method),
            _ => None,
        })
        .collect()
}

fn lower_find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find(|ty| ty.name == type_name || ty.name.rsplit('.').next() == Some(type_name))
}

fn nested_type_simple_alias_item(nested: &TypeDecl) -> Option<String> {
    let simple = nested.name.rsplit('.').next().unwrap_or(&nested.name);
    let alias = upper_camel_type_name_from_static_receiver(simple);
    if alias == "Builder" {
        return None;
    }
    let target = crate::slot_render::rust_name_for_java_named_type(&nested.name);
    let target = if nested.kind == TypeKind::Interface {
        format!("dyn {target}")
    } else {
        target
    };
    (alias != target).then(|| format!("pub type {alias} = {target};"))
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
    seed_context_static_type_aliases(&mut env, ctx);
    seed_support_type_fields(&mut env, ctx.support_local_type_program);
    seed_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.support_local_type_program);
    seed_external_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.facts);
    seed_outer_this_capture_fields(&mut env, ty, ctx);
    let fields = with_outer_this_capture_field(
        ty,
        plan_constructor_field_values_with_inherited_context(
            ty,
            None,
            ctx.signatures,
            ctx.support_local_type_program,
            ctx.facts,
        )
        .into_iter()
        .map(|plan| lower_constructor_field_value_plan(plan, &env, constructor_ctx))
        .collect(),
        ctx,
    );
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

fn with_outer_this_capture_field(
    ty: &TypeDecl,
    mut fields: Vec<(String, RustExpr)>,
    ctx: LowerCtx,
) -> Vec<(String, RustExpr)> {
    let Some(outer_type) = ctx.facts.captures.outer_this_type_for_type(&ty.name) else {
        return fields;
    };
    if fields.iter().any(|(name, _)| name == "__outer") {
        return fields;
    }
    fields.insert(
        0,
        (
            "__outer".to_string(),
            unsupported_outer_capture_constructor_projection(&ty.name, outer_type),
        ),
    );
    fields
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
    seed_context_static_type_aliases(&mut env, ctx);
    seed_support_type_fields(&mut env, ctx.support_local_type_program);
    seed_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.support_local_type_program);
    seed_external_inherited_instance_fields(&mut env, ty, ctx.signatures, ctx.facts);
    seed_outer_this_capture_fields(&mut env, ty, ctx);
    if let Some(body) = &ctor.body
        && let Some(function) = lower_delegating_constructor(ty, ctor, body, &env, constructor_ctx)
    {
        return function;
    }
    if let Some(body) = &ctor.body
        && let Some((fields, trailing, returns_result)) =
            implicit_source_super_constructor_with_trailing_fields(ty, body, &env, constructor_ctx)
    {
        let mut function = constructor_function_shell_with_facts(
            ty,
            ctor,
            ctx.signatures,
            super_no_arg_constructor_with_trailing_body(
                ty,
                with_outer_this_capture_field(ty, fields, constructor_ctx),
                &trailing,
                &env,
                constructor_ctx,
                returns_result,
            ),
            ctx.facts,
        );
        if returns_result {
            function.return_type = Some(exception_result_type(RustType::Named(ty.name.clone())));
        }
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

    let field_plans = plan_constructor_field_values_with_inherited_context(
        ty,
        ctor.body.as_ref(),
        ctx.signatures,
        ctx.support_local_type_program,
        ctx.facts,
    );
    let body = if constructor_field_plans_need_body_locals(&field_plans, ctor.body.as_ref()) {
        constructor_return_body_with_body_local_dependent_fields(
            ty,
            &field_plans,
            ctor.body.as_ref().expect("checked above"),
            &env,
            constructor_ctx,
        )
    } else if constructor_field_plans_need_self_temporary(&field_plans, &env, constructor_ctx) {
        constructor_return_body_with_self_dependent_fields(
            ty,
            &field_plans,
            ctor.body.as_ref(),
            &env,
            constructor_ctx,
        )
    } else {
        constructor_return_body(
            ty,
            with_outer_this_capture_field(
                ty,
                field_plans
                    .into_iter()
                    .map(|plan| lower_constructor_field_value_plan(plan, &env, constructor_ctx))
                    .collect(),
                constructor_ctx,
            ),
        )
    };

    constructor_function_shell_with_facts(ty, ctor, ctx.signatures, body, ctx.facts)
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
        } => {
            let slot = assignment_value_plan_for_shape(&shape).slot;
            let lowered = lower_expr_for_slot(value, env, slot.clone(), ctx);
            let lowered =
                wrap_optional_assignment_value_if_needed(value, env, slot.clone(), lowered, ctx);
            (
                field_name,
                adapt_reused_owned_value_for_slot(lowered, value, &slot, env, ctx),
            )
        }
        ConstructorFieldValuePlan::Default { field_name, shape } => {
            constructor_default_field_value_with_facts(field_name, shape, ctx.facts)
        }
    }
}

fn lower_constructor_field_value_expr_for_shape(
    value: &Expr,
    shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let slot = assignment_value_plan_for_shape(shape).slot;
    let lowered = lower_expr_for_slot(value, env, slot.clone(), ctx);
    let lowered = wrap_optional_assignment_value_if_needed(value, env, slot.clone(), lowered, ctx);
    adapt_reused_owned_value_for_slot(lowered, value, &slot, env, ctx)
}

fn constructor_field_plans_need_self_temporary(
    plans: &[ConstructorFieldValuePlan<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    plans.iter().any(|plan| {
        constructor_field_value_expr(plan)
            .is_some_and(|expr| constructor_expr_needs_self_temporary(expr, env, ctx))
    })
}

fn constructor_field_plans_need_body_locals(
    plans: &[ConstructorFieldValuePlan<'_>],
    body: Option<&Block>,
) -> bool {
    let Some(body) = body else {
        return false;
    };
    let local_names = constructor_body_local_decl_names(body);
    !local_names.is_empty()
        && plans.iter().any(|plan| match plan {
            ConstructorFieldValuePlan::Assigned { value, .. } => {
                expr_reads_any_name(value, &local_names)
            }
            ConstructorFieldValuePlan::FieldInitializer { .. }
            | ConstructorFieldValuePlan::Default { .. } => false,
        })
}

fn constructor_body_local_decl_names(body: &Block) -> HashSet<String> {
    body.statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::VarDecl { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn expr_reads_any_name(expr: &Expr, names: &HashSet<String>) -> bool {
    let mut read_names = HashSet::new();
    collect_expr_read_names(expr, &mut read_names);
    read_names.iter().any(|name| names.contains(name))
}

fn constructor_field_value_expr<'a>(plan: &'a ConstructorFieldValuePlan<'a>) -> Option<&'a Expr> {
    match plan {
        ConstructorFieldValuePlan::Assigned { value, .. }
        | ConstructorFieldValuePlan::FieldInitializer { value, .. } => Some(value),
        ConstructorFieldValuePlan::Default { .. } => None,
    }
}

fn constructor_expr_needs_self_temporary(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    match expr {
        Expr::This | Expr::Super => true,
        Expr::Var(name) => {
            env.local_type(name).is_none()
                && current_env_name_is_instance_field(env, name)
                && !bare_name_looks_like_static_owner(name)
        }
        Expr::Literal(_) => false,
        Expr::FieldAccess(receiver, _) => {
            matches!(receiver.as_ref(), Expr::This | Expr::Super)
                || constructor_expr_needs_self_temporary(receiver, env, ctx)
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            receiverless_call_reads_self(receiver.as_deref(), name, args.len(), ctx)
                || receiver.as_deref().is_some_and(|receiver| {
                    matches!(receiver, Expr::This | Expr::Super)
                        || constructor_expr_needs_self_temporary(receiver, env, ctx)
                })
                || args
                    .iter()
                    .any(|arg| constructor_expr_needs_self_temporary(arg, env, ctx))
        }
        Expr::New { args, .. } | Expr::ArrayInitializer(args) => args
            .iter()
            .any(|arg| constructor_expr_needs_self_temporary(arg, env, ctx)),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|arg| constructor_expr_needs_self_temporary(arg, env, ctx))
                || initializer.as_ref().is_some_and(|initializer| {
                    initializer
                        .iter()
                        .any(|arg| constructor_expr_needs_self_temporary(arg, env, ctx))
                })
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            constructor_expr_needs_self_temporary(expr, env, ctx)
        }
        Expr::ArrayAccess(array, index) => {
            constructor_expr_needs_self_temporary(array, env, ctx)
                || constructor_expr_needs_self_temporary(index, env, ctx)
        }
        Expr::Binary { left, right, .. } => {
            constructor_expr_needs_self_temporary(left, env, ctx)
                || constructor_expr_needs_self_temporary(right, env, ctx)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            constructor_expr_needs_self_temporary(condition, env, ctx)
                || constructor_expr_needs_self_temporary(then_expr, env, ctx)
                || constructor_expr_needs_self_temporary(else_expr, env, ctx)
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => false,
    }
}

fn constructor_return_body_with_self_dependent_fields(
    ty: &TypeDecl,
    plans: &[ConstructorFieldValuePlan<'_>],
    source_body: Option<&Block>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    constructor_field_plans_with_trailing_body_ordered(ty, plans, source_body, &[], env, ctx, false)
}

fn constructor_return_body_with_body_local_dependent_fields(
    ty: &TypeDecl,
    plans: &[ConstructorFieldValuePlan<'_>],
    body: &Block,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let local_names = constructor_body_local_decl_names(body);
    constructor_field_plans_with_body_local_decls(ty, plans, body, &local_names, env, ctx)
}

fn constructor_field_plans_with_body_local_decls(
    ty: &TypeDecl,
    plans: &[ConstructorFieldValuePlan<'_>],
    source_body: &Block,
    local_names: &HashSet<String>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustStmt> {
    let mut fields = Vec::new();
    let mut deferred = Vec::new();
    for plan in plans {
        let local_dependent = match plan {
            ConstructorFieldValuePlan::Assigned { value, .. } => {
                expr_reads_any_name(value, local_names)
            }
            ConstructorFieldValuePlan::FieldInitializer { .. }
            | ConstructorFieldValuePlan::Default { .. } => false,
        };
        let self_dependent = constructor_field_value_expr(plan)
            .is_some_and(|expr| constructor_expr_needs_self_temporary(expr, env, ctx));
        if local_dependent || self_dependent {
            let (field_name, shape) = constructor_field_name_and_shape(plan);
            let seed = if local_dependent {
                constructor_default_field_value_with_facts(
                    field_name.clone(),
                    shape.clone(),
                    ctx.facts,
                )
            } else {
                constructor_self_temporary_seed_field_value(plan, env, ctx).unwrap_or_else(|| {
                    constructor_default_field_value_with_facts(
                        field_name.clone(),
                        shape.clone(),
                        ctx.facts,
                    )
                })
            };
            fields.push(seed);
            deferred.push(plan);
        } else {
            fields.push(lower_constructor_field_value_plan(plan.clone(), env, ctx));
        }
    }

    fields = with_outer_this_capture_field(ty, fields, ctx);
    let mut lowered_body = vec![RustStmt::Let {
        mutable: true,
        name: "__java_self".to_string(),
        ty: None,
        value: Some(RustExpr::StructLit {
            name: crate::slot_render::rust_name_for_java_named_type(&ty.name),
            fields,
        }),
    }];

    let mut local_env = env.clone();
    for stmt in &source_body.statements {
        if matches!(stmt, Stmt::VarDecl { .. }) {
            lower_stmt(
                stmt,
                &mut local_env,
                false,
                ctx,
                &ExprRefinements::default(),
                &mut lowered_body,
            );
        }
    }

    for plan in deferred {
        let (field_name, value, shape) = constructor_assigned_field_parts(plan);
        let lowered = lower_self_dependent_constructor_field_value(value, shape, &local_env, ctx);
        lowered_body.push(assignment_stmt(
            constructor_self_field_projection(&field_name),
            lowered,
        ));
    }

    lowered_body.push(RustStmt::Return(Some(RustExpr::Var(
        "__java_self".to_string(),
    ))));
    lowered_body
}

fn constructor_field_plans_with_trailing_body_ordered(
    ty: &TypeDecl,
    plans: &[ConstructorFieldValuePlan<'_>],
    source_body: Option<&Block>,
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
    env: &ShapeEnv,
    ctx: LowerCtx,
    returns_result: bool,
) -> Vec<RustStmt> {
    let mut fields = Vec::new();
    let mut deferred = Vec::new();
    let assignment_order = source_body.map(constructor_body_field_assignment_order);
    for (plan_index, plan) in plans.iter().enumerate() {
        let self_dependent = constructor_field_value_expr(plan)
            .is_some_and(|expr| constructor_expr_needs_self_temporary(expr, env, ctx));
        if self_dependent {
            let (field_name, shape) = constructor_field_name_and_shape(plan);
            fields.push(
                constructor_self_temporary_seed_field_value(plan, env, ctx).unwrap_or_else(|| {
                    constructor_default_field_value_with_facts(
                        field_name.clone(),
                        shape.clone(),
                        ctx.facts,
                    )
                }),
            );
            deferred.push((plan_index, plan));
        } else {
            fields.push(lower_constructor_field_value_plan(plan.clone(), env, ctx));
        }
    }

    fields = with_outer_this_capture_field(ty, fields, ctx);
    let mut body = vec![RustStmt::Let {
        mutable: true,
        name: "__java_self".to_string(),
        ty: None,
        value: Some(RustExpr::StructLit {
            name: crate::slot_render::rust_name_for_java_named_type(&ty.name),
            fields,
        }),
    }];
    if let Some(assignment_order) = assignment_order.as_ref() {
        deferred.sort_by_key(|(plan_index, plan)| {
            deferred_constructor_field_order_key(*plan_index, plan, assignment_order)
        });
    }
    for (_, plan) in deferred {
        let (field_name, value, shape) = constructor_assigned_field_parts(plan);
        let lowered = lower_self_dependent_constructor_field_value(value, shape, env, ctx);
        body.push(assignment_stmt(
            constructor_self_field_projection(&field_name),
            lowered,
        ));
    }
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
        function_call_projection(RustExpr::Raw("Ok".to_string()), vec![returned])
    } else {
        returned
    })));
    body
}

fn constructor_body_field_assignment_order(
    body: &Block,
) -> std::collections::HashMap<String, usize> {
    body.statements
        .iter()
        .enumerate()
        .filter_map(|(index, stmt)| match stmt {
            Stmt::Assign { target, .. } => match target {
                Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                    Some((crate::parser::to_snake_case(field), index))
                }
                Expr::Var(field) => Some((crate::parser::to_snake_case(field), index)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn deferred_constructor_field_order_key(
    plan_index: usize,
    plan: &ConstructorFieldValuePlan<'_>,
    assignment_order: &std::collections::HashMap<String, usize>,
) -> (usize, usize) {
    match plan {
        ConstructorFieldValuePlan::Assigned { field_name, .. } => (
            1,
            assignment_order
                .get(field_name)
                .copied()
                .unwrap_or(usize::MAX),
        ),
        ConstructorFieldValuePlan::FieldInitializer { .. } => (0, plan_index),
        ConstructorFieldValuePlan::Default { .. } => (2, plan_index),
    }
}

fn sort_deferred_constructor_field_assignments_by_body_order(
    assignments: &mut [DelegatedConstructorTrailingStmt<'_>],
    body: &Block,
) {
    let assignment_order = constructor_body_field_assignment_order(body);
    assignments.sort_by_key(|assignment| match assignment {
        DelegatedConstructorTrailingStmt::FieldAssignment { field_name, .. } => assignment_order
            .get(field_name)
            .copied()
            .unwrap_or(usize::MAX),
        DelegatedConstructorTrailingStmt::InstanceCall { .. }
        | DelegatedConstructorTrailingStmt::ExprStatement { .. } => usize::MAX,
    });
}

fn modeled_external_super_constructor_state_field_assignments<'a>(
    superclass: &str,
    args: &'a [Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<DelegatedConstructorTrailingStmt<'a>> {
    let Some(field_name) = crate::external_packs::modeled_external_super_constructor_state_field(
        superclass,
        args.len(),
    ) else {
        return Vec::new();
    };
    let Some(value) = args.first() else {
        return Vec::new();
    };
    let rust_name = crate::parser::to_snake_case(field_name);
    if env.local_shape(&rust_name).is_some() && !current_env_name_is_instance_field(env, &rust_name)
    {
        return Vec::new();
    }
    if ctx
        .current_type
        .is_some_and(|owner| ctx.signatures.field_shape(owner, &rust_name).is_none())
    {
        return Vec::new();
    }
    vec![DelegatedConstructorTrailingStmt::FieldAssignment {
        field_name: rust_name,
        value,
    }]
}

fn constructor_self_temporary_seed_field_value(
    plan: &ConstructorFieldValuePlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<(String, RustExpr)> {
    let (field_name, value, shape) = constructor_assigned_field_parts(plan);
    let slot = assignment_value_plan_for_shape(shape).slot;
    let lowered = lower_expr_for_slot(value, env, slot.clone(), ctx);
    let lowered = wrap_optional_assignment_value_if_needed(value, env, slot, lowered, ctx);
    let lowered = constructor_self_value_expr(lowered, value, env, ctx);
    if rust_expr_mentions_constructor_self_temporary(&lowered) {
        return None;
    }
    Some((field_name.clone(), lowered))
}

fn lower_self_dependent_constructor_field_value(
    value: &Expr,
    shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let self_ctx = constructor_self_temporary_lower_ctx(ctx);
    let slot = assignment_value_plan_for_shape(shape).slot;
    let lowered = lower_expr_for_slot(value, env, slot.clone(), self_ctx);
    let lowered =
        wrap_optional_assignment_value_if_needed(value, env, slot.clone(), lowered, self_ctx);
    let lowered = adapt_reused_owned_value_for_slot(lowered, value, &slot, env, self_ctx);
    constructor_self_value_expr(lowered, value, env, self_ctx)
}

fn constructor_self_temporary_lower_ctx(ctx: LowerCtx) -> LowerCtx {
    LowerCtx {
        current_receiver: Some(ReceiverPlan::Mutable),
        ..ctx
    }
}

fn constructor_field_name_and_shape<'a>(
    plan: &'a ConstructorFieldValuePlan<'a>,
) -> (&'a String, &'a RustSlotShape) {
    match plan {
        ConstructorFieldValuePlan::Assigned {
            field_name, shape, ..
        }
        | ConstructorFieldValuePlan::FieldInitializer {
            field_name, shape, ..
        }
        | ConstructorFieldValuePlan::Default { field_name, shape } => (field_name, shape),
    }
}

fn constructor_assigned_field_parts<'a>(
    plan: &'a ConstructorFieldValuePlan<'a>,
) -> (&'a String, &'a Expr, &'a RustSlotShape) {
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
        } => (field_name, value, shape),
        ConstructorFieldValuePlan::Default { .. } => {
            unreachable!("deferred constructor fields always have an initializer")
        }
    }
}

fn constructor_self_value_expr(
    expr: RustExpr,
    source: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let expr = replace_self_receiver(expr, "__java_self.clone()");
    if slot_expr_needs_result_propagation(source, env, ctx) {
        return unwrap_result_or_panic_uncaught_java_exception(expr);
    }
    expr
}

fn replace_self_receiver(expr: RustExpr, replacement: &str) -> RustExpr {
    match expr {
        RustExpr::Var(name) if name == "self" => self_replacement_expr(replacement),
        RustExpr::Field(receiver, field) => {
            let receiver = if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "self") {
                constructor_self_field_replacement_expr(replacement)
            } else {
                replace_self_receiver(*receiver, replacement)
            };
            RustExpr::Field(Box::new(receiver), field)
        }
        RustExpr::Index(target, index) => RustExpr::Index(
            Box::new(replace_self_receiver(*target, replacement)),
            Box::new(replace_self_receiver(*index, replacement)),
        ),
        RustExpr::Binary { op, left, right } => RustExpr::Binary {
            op,
            left: Box::new(replace_self_receiver(*left, replacement)),
            right: Box::new(replace_self_receiver(*right, replacement)),
        },
        RustExpr::Unary { op, expr } => RustExpr::Unary {
            op,
            expr: Box::new(replace_self_receiver(*expr, replacement)),
        },
        RustExpr::Cast { expr, ty } => RustExpr::Cast {
            expr: Box::new(replace_self_receiver(*expr, replacement)),
            ty,
        },
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => RustExpr::If {
            condition: Box::new(replace_self_receiver(*condition, replacement)),
            then_expr: Box::new(replace_self_receiver(*then_expr, replacement)),
            else_expr: Box::new(replace_self_receiver(*else_expr, replacement)),
        },
        RustExpr::Call { function, args } => RustExpr::Call {
            function: Box::new(replace_self_receiver(*function, replacement)),
            args: args
                .into_iter()
                .map(|arg| replace_self_receiver(arg, replacement))
                .collect(),
        },
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => RustExpr::MethodCall {
            receiver: Box::new(replace_self_receiver(*receiver, replacement)),
            method,
            args: args
                .into_iter()
                .map(|arg| replace_self_receiver(arg, replacement))
                .collect(),
        },
        RustExpr::StructLit { name, fields } => RustExpr::StructLit {
            name,
            fields: fields
                .into_iter()
                .map(|(field, value)| (field, replace_self_receiver(value, replacement)))
                .collect(),
        },
        RustExpr::Raw(text) => RustExpr::Raw(text.replace(
            "self.",
            &format!("{}.", constructor_self_field_replacement(replacement)),
        )),
        other => other,
    }
}

fn constructor_self_field_replacement_expr(replacement: &str) -> RustExpr {
    self_replacement_expr(&constructor_self_field_replacement(replacement))
}

fn self_replacement_expr(replacement: &str) -> RustExpr {
    if replacement
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        RustExpr::Var(replacement.to_string())
    } else {
        RustExpr::Raw(replacement.to_string())
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
                lower_resolved_constructor_varargs_args(args, env, resolved, ctx).unwrap_or_else(
                    || {
                        lower_args_with_plan(
                            args,
                            env,
                            plan_constructor_arguments(Some(resolved), args.len(), ctx.signatures),
                            ctx,
                        )
                    },
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
            let field_plans = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            );
            (
                constructor_field_plans_with_trailing_body_ordered(
                    ty,
                    &field_plans,
                    Some(body),
                    &trailing,
                    env,
                    ctx,
                    returns_result,
                ),
                returns_result,
            )
        }
        ConstructorDelegationPlan::ModeledExternalSuperclassWithTrailing {
            args,
            superclass,
            trailing,
        } => {
            let returns_result = delegated_constructor_trailing_can_throw(&trailing, env, ctx);
            let field_plans = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            );
            let mut trailing_instance_calls =
                modeled_external_super_constructor_state_field_assignments(
                    &superclass,
                    args,
                    env,
                    ctx,
                );
            trailing_instance_calls.extend(
                trailing
                    .iter()
                    .filter(|stmt| {
                        matches!(
                            stmt,
                            DelegatedConstructorTrailingStmt::InstanceCall { .. }
                                | DelegatedConstructorTrailingStmt::ExprStatement { .. }
                        )
                    })
                    .cloned(),
            );
            (
                constructor_field_plans_with_trailing_body_ordered(
                    ty,
                    &field_plans,
                    Some(body),
                    &trailing_instance_calls,
                    env,
                    ctx,
                    returns_result,
                ),
                returns_result,
            )
        }
        ConstructorDelegationPlan::ExternalExceptionSuperclass { .. } => {
            let field_plans = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            );
            (
                constructor_field_plans_with_trailing_body_ordered(
                    ty,
                    &field_plans,
                    Some(body),
                    &[],
                    env,
                    ctx,
                    false,
                ),
                false,
            )
        }
        ConstructorDelegationPlan::ExternalExceptionSuperclassWithTrailing { trailing, .. } => {
            let trailing_side_effects = delegated_constructor_trailing_side_effects(&trailing);
            let returns_result =
                delegated_constructor_trailing_can_throw(&trailing_side_effects, env, ctx);
            let field_plans = plan_constructor_field_values_with_inherited_context(
                ty,
                Some(body),
                ctx.signatures,
                ctx.support_local_type_program,
                ctx.facts,
            );
            (
                constructor_field_plans_with_trailing_body_ordered(
                    ty,
                    &field_plans,
                    Some(body),
                    &trailing_side_effects,
                    env,
                    ctx,
                    returns_result,
                ),
                returns_result,
            )
        }
        ConstructorDelegationPlan::Unsupported(kind) => {
            if kind == crate::unsupported::UnsupportedConstructorLowering::SuperRequiresInheritanceLowering
                && let Some((fields, trailing, returns_result)) =
                    source_super_constructor_with_trailing_fields(ty, body, env, ctx)
            {
                return Some(constructor_function_shell_with_facts(
                    ty,
                    ctor,
                    ctx.signatures,
                    super_no_arg_constructor_with_trailing_body(
                        ty,
                        fields,
                        &trailing,
                        env,
                        ctx,
                        returns_result,
                    ),
                    ctx.facts,
                ));
            }
            (unsupported_constructor_body(kind), false)
        }
    };

    let mut function =
        constructor_function_shell_with_facts(ty, ctor, ctx.signatures, body, ctx.facts);
    if returns_result {
        function.return_type = Some(exception_result_type(RustType::Named(ty.name.clone())));
    }
    Some(function)
}

fn source_super_constructor_with_trailing_fields<'a>(
    ty: &'a TypeDecl,
    body: &'a Block,
    env: &ShapeEnv,
    ctx: LowerCtx<'a>,
) -> Option<(
    Vec<(String, RustExpr)>,
    Vec<DelegatedConstructorTrailingStmt<'a>>,
    bool,
)> {
    let Stmt::ConstructorInvocation {
        kind: crate::java_ir::ConstructorInvocationKind::Super,
        args,
        ..
    } = body.statements.first()?
    else {
        return None;
    };
    let trailing = crate::constructor_plan::delegated_constructor_trailing_stmts_for_lowering(
        &body.statements[1..],
    )?;
    source_super_constructor_fields_and_trailing(ty, body, args, trailing, env, ctx)
}

fn source_super_constructor_fields_and_trailing<'a>(
    ty: &'a TypeDecl,
    body: &'a Block,
    args: &'a [Expr],
    trailing: Vec<DelegatedConstructorTrailingStmt<'a>>,
    env: &ShapeEnv,
    ctx: LowerCtx<'a>,
) -> Option<(
    Vec<(String, RustExpr)>,
    Vec<DelegatedConstructorTrailingStmt<'a>>,
    bool,
)> {
    let superclass = ty.extends.as_deref()?;
    let super_ty = find_super_type_decl_by_erased_name(ctx.support_local_type_program, superclass)?;
    let super_ctor = super_ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Constructor(ctor) if constructor_arity_matches(ctor, args.len()) => Some(ctor),
            _ => None,
        })
        .next()?;
    let (super_body, super_bindings) =
        delegated_source_constructor_body(super_ty, super_ctor, args).unwrap_or_else(|| {
            (
                super_ctor.body.as_ref().expect("checked below"),
                super_ctor
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .zip(args.iter().cloned())
                    .collect(),
            )
        });
    let super_bindings = super_bindings
        .iter()
        .map(|(name, expr)| (name.as_str(), expr))
        .collect::<Vec<_>>();

    let inherited_field_names = crate::inherited_members::inherited_instance_fields_for_type(
        ty,
        ctx.support_local_type_program,
    )
    .into_iter()
    .map(|inherited| inherited.field.name)
    .collect::<std::collections::HashSet<_>>();
    let super_plans = plan_constructor_field_values_with_inherited_context(
        super_ty,
        Some(super_body),
        ctx.signatures,
        ctx.support_local_type_program,
        ctx.facts,
    );
    let nested_implicit_super_values =
        nested_implicit_source_super_field_values(super_ty, super_body, env, ctx)
            .unwrap_or_default()
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
    let child_plans = plan_constructor_field_values_with_inherited_context(
        ty,
        Some(body),
        ctx.signatures,
        ctx.support_local_type_program,
        ctx.facts,
    );
    let trailing_read_names = delegated_constructor_trailing_read_names(&trailing);

    let mut fields = Vec::new();
    let mut super_field_values = std::collections::HashMap::new();
    let mut deferred_child_field_assignments = Vec::new();
    for plan in super_plans {
        let (field_name, shape) = constructor_field_name_and_shape(&plan);
        if !inherited_field_names.contains(field_name) {
            continue;
        }
        let value = if let Some(value) = nested_implicit_super_values.get(field_name) {
            value.clone()
        } else {
            match constructor_field_value_expr(&plan) {
                Some(value) => {
                    let substituted = substitute_expr_vars(value, &super_bindings);
                    lower_constructor_field_value_expr_for_shape(&substituted, shape, env, ctx)
                }
                None => {
                    constructor_default_field_value_with_facts(
                        field_name.clone(),
                        shape.clone(),
                        ctx.facts,
                    )
                    .1
                }
            }
        };
        super_field_values.insert(field_name.clone(), (value, shape.clone()));
    }

    for plan in child_plans {
        let (field_name, child_shape) = constructor_field_name_and_shape(&plan);
        if let Some((value, super_shape)) = super_field_values.remove(field_name) {
            let value = adapt_inherited_super_field_value_for_child_shape(
                value,
                super_shape,
                child_shape,
                ctx,
            );
            fields.push((field_name.clone(), value));
        } else if let Some(value) = match &plan {
            ConstructorFieldValuePlan::Assigned { value, .. }
            | ConstructorFieldValuePlan::FieldInitializer { value, .. }
                if constructor_expr_needs_self_temporary(value, env, ctx) =>
            {
                Some(*value)
            }
            _ => None,
        } {
            let (field_name, shape) = constructor_field_name_and_shape(&plan);
            fields.push(
                constructor_self_temporary_seed_field_value(&plan, env, ctx).unwrap_or_else(|| {
                    constructor_default_field_value_with_facts(
                        field_name.clone(),
                        shape.clone(),
                        ctx.facts,
                    )
                }),
            );
            deferred_child_field_assignments.push(
                DelegatedConstructorTrailingStmt::FieldAssignment {
                    field_name: field_name.clone(),
                    value,
                },
            );
        } else {
            fields.push(lower_constructor_field_value_plan_with_future_reads(
                plan,
                env,
                ctx,
                &trailing_read_names,
            ));
        }
    }

    sort_deferred_constructor_field_assignments_by_body_order(
        &mut deferred_child_field_assignments,
        body,
    );
    let mut trailing_instance_calls = trailing
        .into_iter()
        .filter(|stmt| {
            matches!(
                stmt,
                DelegatedConstructorTrailingStmt::InstanceCall { .. }
                    | DelegatedConstructorTrailingStmt::ExprStatement { .. }
            )
        })
        .collect::<Vec<_>>();
    trailing_instance_calls.splice(0..0, deferred_child_field_assignments);
    let returns_result =
        delegated_constructor_trailing_can_throw(&trailing_instance_calls, env, ctx);
    Some((fields, trailing_instance_calls, returns_result))
}

fn adapt_inherited_super_field_value_for_child_shape(
    value: RustExpr,
    super_shape: RustSlotShape,
    child_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> RustExpr {
    let Some(element_ty) = child_shape.optional_element_type() else {
        return value;
    };
    if !optional_element_is_source_java_enum(&element_ty, ctx) {
        return value;
    }
    let rendered = emit::emit_expr(&value);
    if rendered == "None"
        || rendered == "Default::default()"
        || rust_expr_is_constructed_optional_value(&value)
    {
        return value;
    }
    if super_shape.optional_element_type().is_none() {
        return planned_optional_assignment_value_for_target(
            value,
            super_shape,
            Slot::Assignment(child_shape.clone()),
            child_shape,
            false,
            ctx.facts,
        );
    }
    let mut element_shape = child_shape.clone();
    element_shape.java_ty = element_ty;
    let construction = OptionalSlotConstructionPlan {
        target: child_shape.clone(),
        element_shape: element_shape.clone(),
        element_plan: shape_coercion_plan_with_facts(&element_shape, &element_shape, ctx.facts),
        wrapper_ownership: OwnershipShape::Owned,
    };
    construct_optional_value_for_target(value, construction)
}

fn optional_element_is_source_java_enum(java_ty: &JavaType, ctx: LowerCtx) -> bool {
    match java_ty {
        JavaType::Named { name, .. } => matches_source_java_enum_type(name, ctx),
        _ => false,
    }
}

fn lower_constructor_field_value_plan_with_future_reads(
    plan: ConstructorFieldValuePlan<'_>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    future_reads: &std::collections::HashSet<String>,
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
        } => {
            let slot = assignment_value_plan_for_shape(&shape).slot;
            let lowered = lower_expr_for_slot(value, env, slot.clone(), ctx);
            let lowered =
                wrap_optional_assignment_value_if_needed(value, env, slot.clone(), lowered, ctx);
            let lowered = adapt_reused_owned_value_for_slot(lowered, value, &slot, env, ctx);
            let lowered = if let Expr::Var(name) = value {
                if future_reads.contains(name)
                    && shape_is_owned_non_erased(&shape)
                    && (known_named_clone_recovery(&shape)
                        || known_owned_argument_clone_recovery_with_facts(&shape, ctx.facts))
                {
                    apply_audited_clone_recovery(lowered)
                } else {
                    lowered
                }
            } else {
                lowered
            };
            (field_name, lowered)
        }
        ConstructorFieldValuePlan::Default { field_name, shape } => {
            constructor_default_field_value_with_facts(field_name, shape, ctx.facts)
        }
    }
}

fn delegated_constructor_trailing_read_names(
    trailing: &[DelegatedConstructorTrailingStmt<'_>],
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in trailing {
        match stmt {
            DelegatedConstructorTrailingStmt::FieldAssignment { value, .. } => {
                collect_expr_read_names(value, &mut names);
            }
            DelegatedConstructorTrailingStmt::InstanceCall { args, .. } => {
                for arg in *args {
                    collect_expr_read_names(arg, &mut names);
                }
            }
            DelegatedConstructorTrailingStmt::ExprStatement { expr } => {
                collect_expr_read_names(expr, &mut names);
            }
        }
    }
    names
}

fn nested_implicit_source_super_field_values(
    ty: &TypeDecl,
    body: &Block,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<Vec<(String, RustExpr)>> {
    if matches!(
        body.statements.first(),
        Some(Stmt::ConstructorInvocation { .. })
    ) {
        return None;
    }
    implicit_source_super_constructor_with_trailing_fields(ty, body, env, ctx)
        .map(|(fields, _, _)| fields)
}

fn delegated_source_constructor_body<'a>(
    ty: &'a TypeDecl,
    ctor: &'a ConstructorDecl,
    args: &[Expr],
) -> Option<(&'a Block, Vec<(String, Expr)>)> {
    let body = ctor.body.as_ref()?;
    let Stmt::ConstructorInvocation {
        kind: crate::java_ir::ConstructorInvocationKind::This,
        args: delegated_args,
        resolved,
    } = body.statements.first()?
    else {
        return Some((
            body,
            ctor.params
                .iter()
                .map(|param| param.name.clone())
                .zip(args.iter().cloned())
                .collect(),
        ));
    };
    let resolved = resolved.as_ref()?;
    let target = ty.members.iter().find_map(|member| match member {
        Member::Constructor(candidate)
            if candidate
                .rust_name
                .as_deref()
                .is_some_and(|rust_name| rust_name == resolved.rust_name) =>
        {
            Some(candidate)
        }
        _ => None,
    })?;
    let target_body = target.body.as_ref()?;
    Some((
        target_body,
        target
            .params
            .iter()
            .map(|param| param.name.clone())
            .zip(delegated_args.iter().cloned())
            .collect(),
    ))
}

fn implicit_source_super_constructor_with_trailing_fields<'a>(
    ty: &'a TypeDecl,
    body: &'a Block,
    env: &ShapeEnv,
    ctx: LowerCtx<'a>,
) -> Option<(
    Vec<(String, RustExpr)>,
    Vec<DelegatedConstructorTrailingStmt<'a>>,
    bool,
)> {
    if matches!(
        body.statements.first(),
        Some(Stmt::ConstructorInvocation { .. })
    ) {
        return None;
    }
    let trailing = crate::constructor_plan::delegated_constructor_trailing_stmts_for_lowering(
        &body.statements,
    )?;
    source_super_constructor_fields_and_trailing(ty, body, &[], trailing, env, ctx)
}

fn constructor_arity_matches(ctor: &crate::java_ir::ConstructorDecl, arg_count: usize) -> bool {
    if ctor.params.len() == arg_count {
        return true;
    }
    ctor.params
        .last()
        .is_some_and(|param| param.is_varargs && arg_count >= ctor.params.len().saturating_sub(1))
}

fn unsupported_source_constructor_call_expr(
    ty: &JavaType,
    args: &[Expr],
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let type_name = concrete_java_type_name(ty)?;
    let decl = find_context_type_by_key(ctx.program, type_name)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, type_name))?;
    decl.members.iter().find_map(|member| {
        let Member::Constructor(ctor) = member else {
            return None;
        };
        if !constructor_arity_matches(ctor, args.len()) {
            return None;
        }
        let body = ctor.body.as_ref()?;
        let plan = plan_constructor_delegation_for_type(Some(decl), body)?;
        let crate::constructor_plan::ConstructorDelegationPlan::Unsupported(kind) = plan else {
            return None;
        };
        if kind != crate::unsupported::UnsupportedConstructorLowering::SuperWithUnsupportedTrailingStatements
        {
            return None;
        }
        let policy = unsupported_constructor_message_policy(kind);
        Some(unsupported_constructor_call_handoff_projection(
            &policy.message,
        ))
    })
}

fn substitute_expr_vars(expr: &Expr, bindings: &[(&str, &Expr)]) -> Expr {
    match expr {
        Expr::Var(name) => bindings
            .iter()
            .find_map(|(binding, value)| (*binding == name).then(|| (*value).clone()))
            .unwrap_or_else(|| expr.clone()),
        Expr::FieldAccess(receiver, field) => Expr::FieldAccess(
            Box::new(substitute_expr_vars(receiver, bindings)),
            field.clone(),
        ),
        Expr::ArrayAccess(array, index) => Expr::ArrayAccess(
            Box::new(substitute_expr_vars(array, bindings)),
            Box::new(substitute_expr_vars(index, bindings)),
        ),
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => Expr::ArrayCreation {
            element_type: element_type.clone(),
            dimensions: dimensions
                .iter()
                .map(|expr| substitute_expr_vars(expr, bindings))
                .collect(),
            initializer: initializer.as_ref().map(|initializer| {
                initializer
                    .iter()
                    .map(|expr| substitute_expr_vars(expr, bindings))
                    .collect()
            }),
        },
        Expr::ArrayInitializer(values) => Expr::ArrayInitializer(
            values
                .iter()
                .map(|expr| substitute_expr_vars(expr, bindings))
                .collect(),
        ),
        Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(substitute_expr_vars(left, bindings)),
            right: Box::new(substitute_expr_vars(right, bindings)),
        },
        Expr::Unary { op, expr } => Expr::Unary {
            op: *op,
            expr: Box::new(substitute_expr_vars(expr, bindings)),
        },
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => Expr::Ternary {
            condition: Box::new(substitute_expr_vars(condition, bindings)),
            then_expr: Box::new(substitute_expr_vars(then_expr, bindings)),
            else_expr: Box::new(substitute_expr_vars(else_expr, bindings)),
        },
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => Expr::Call {
            receiver: receiver
                .as_ref()
                .map(|receiver| Box::new(substitute_expr_vars(receiver, bindings))),
            name: name.clone(),
            args: args
                .iter()
                .map(|expr| substitute_expr_vars(expr, bindings))
                .collect(),
            resolved: resolved.clone(),
        },
        Expr::New {
            ty,
            args,
            body,
            resolved,
        } => Expr::New {
            ty: ty.clone(),
            args: args
                .iter()
                .map(|expr| substitute_expr_vars(expr, bindings))
                .collect(),
            body: body.clone(),
            resolved: resolved.clone(),
        },
        Expr::Cast { ty, expr } => Expr::Cast {
            ty: ty.clone(),
            expr: Box::new(substitute_expr_vars(expr, bindings)),
        },
        Expr::InstanceOf { expr, ty } => Expr::InstanceOf {
            expr: Box::new(substitute_expr_vars(expr, bindings)),
            ty: ty.clone(),
        },
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => expr.clone(),
    }
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
        value: Some(function_call_projection(
            self_associated_path_projection(&resolved.rust_name),
            lower_args_with_plan(
                args,
                env,
                plan_constructor_arguments(Some(resolved), args.len(), ctx.signatures),
                ctx,
            ),
        )),
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
        function_call_projection(RustExpr::Raw("Ok".to_string()), vec![returned])
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
        function_call_projection(RustExpr::Raw("Ok".to_string()), vec![returned])
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
        DelegatedConstructorTrailingStmt::ExprStatement { expr } => {
            slot_expr_needs_result_propagation(expr, env, ctx)
        }
    })
}

fn delegated_constructor_trailing_side_effects<'a>(
    trailing: &'a [DelegatedConstructorTrailingStmt<'a>],
) -> Vec<DelegatedConstructorTrailingStmt<'a>> {
    trailing
        .iter()
        .filter(|stmt| {
            matches!(
                stmt,
                DelegatedConstructorTrailingStmt::InstanceCall { .. }
                    | DelegatedConstructorTrailingStmt::ExprStatement { .. }
            )
        })
        .cloned()
        .collect()
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
            let lowered = lower_self_dependent_constructor_field_value(value, &shape, env, ctx);
            assignment_stmt(constructor_self_field_projection(field_name), lowered)
        }
        DelegatedConstructorTrailingStmt::InstanceCall { resolved, args } => {
            let receiver = Expr::This;
            let mut lowered = receiver_method_call_projection_exact(
                RustExpr::Var("__java_self".to_string()),
                &resolved.rust_name,
                lower_args_with_plan(
                    args,
                    env,
                    plan_call_arguments(Some(resolved), args.len(), ctx.signatures),
                    ctx,
                ),
            );
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
                lowered = propagate_result_projection(lowered);
            }
            expr_stmt(lowered)
        }
        DelegatedConstructorTrailingStmt::ExprStatement { expr } => {
            if let Some(stmt) =
                lower_inherited_context_trailing_expr_stmt(expr, env, ctx, returns_result)
            {
                return stmt;
            }
            let mut lowered = replace_self_receiver(lower_expr(expr, env, ctx), "__java_self");
            if returns_result && slot_expr_needs_result_propagation(expr, env, ctx) {
                lowered = propagate_result_projection(lowered);
            }
            expr_stmt(lowered)
        }
    }
}

fn lower_inherited_context_trailing_expr_stmt(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    returns_result: bool,
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
    let Expr::Call {
        receiver: context_receiver,
        name: context_getter,
        args: context_args,
        ..
    } = receiver.as_ref()
    else {
        return None;
    };
    if !matches!(context_getter.as_str(), "get_context" | "getContext")
        || !context_args.is_empty()
        || !matches!(context_receiver.as_deref(), None | Some(Expr::This))
    {
        return None;
    }
    let lowered_args = resolved
        .as_ref()
        .and_then(|resolved| lower_resolved_varargs_call_args(args, env, Some(resolved), ctx))
        .unwrap_or_else(|| {
            lower_args_with_plan(
                args,
                env,
                resolved
                    .as_ref()
                    .map(|resolved| plan_call_arguments(Some(resolved), args.len(), ctx.signatures))
                    .unwrap_or(ArgumentLoweringPlan::Unshaped {
                        arg_count: args.len(),
                    }),
                ctx,
            )
        });
    let method = resolved
        .as_ref()
        .map(|resolved| resolved.rust_name.clone())
        .unwrap_or_else(|| crate::parser::to_snake_case(name));
    let mut lowered = receiver_method_call_projection_exact(
        RustExpr::Raw("__java_self.context".to_string()),
        &method,
        lowered_args,
    );
    if returns_result && slot_expr_needs_result_propagation(expr, env, ctx) {
        lowered = propagate_result_projection(lowered);
    }
    Some(expr_stmt(lowered))
}

fn lower_method(
    program: &Program,
    ty: &TypeDecl,
    method: &MethodDecl,
    ctx: LowerCtx,
) -> RustFunction {
    lower_method_with_exception_owner(program, ty, method, ty, ctx)
}

fn lower_method_with_exception_owner(
    program: &Program,
    ty: &TypeDecl,
    method: &MethodDecl,
    exception_ty: &TypeDecl,
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
    seed_imported_static_type_aliases(&mut env, method_ctx.support_local_type_program);
    seed_imported_static_type_aliases(&mut env, method_ctx.source_summary_type_program);
    seed_context_static_type_aliases(&mut env, method_ctx);
    seed_support_type_fields(&mut env, method_ctx.support_local_type_program);
    seed_inherited_instance_fields(
        &mut env,
        ty,
        method_ctx.signatures,
        method_ctx.support_local_type_program,
    );
    seed_external_inherited_instance_fields(&mut env, ty, method_ctx.signatures, method_ctx.facts);
    let exception_plan = method_ctx.exceptions.method_plan(exception_ty, method);
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
    } else if method.body.is_none() && method.modifiers.contains(&Modifier::Abstract) {
        vec![expr_stmt(abstract_method_stub_projection(
            &ty.name,
            &method.name,
        ))]
    } else if method.body.is_none() {
        bodyless_source_summary_method_body(
            method_signature.return_shape.as_ref(),
            wrap_returns_in_ok,
        )
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
    prepend_assigned_parameter_shadows(method, &mut body);
    method_function_shell_with_facts_and_exception_plan(
        ty,
        method,
        ctx.signatures,
        body,
        ctx.facts,
        exception_plan,
    )
}

fn lower_inherited_method(
    program: &Program,
    emitted_ty: &TypeDecl,
    inherited: crate::inherited_members::InheritedInstanceMethodRef<'_>,
    ctx: LowerCtx,
) -> RustFunction {
    if inherited.method.body.is_none() {
        lower_method(program, inherited.declaring_ty, inherited.method, ctx)
    } else {
        lower_method_with_exception_owner(
            program,
            emitted_ty,
            inherited.method,
            inherited.declaring_ty,
            ctx,
        )
    }
}

fn bodyless_source_summary_method_body(
    return_shape: Option<&RustSlotShape>,
    wrap_returns_in_ok: bool,
) -> Vec<RustStmt> {
    if return_shape.is_none_or(|shape| shape.java_ty == JavaType::Void) {
        return wrap_returns_in_ok
            .then(|| return_stmt(None, true))
            .into_iter()
            .collect();
    }
    vec![return_stmt(
        Some(RustExpr::Raw("Default::default()".to_string())),
        wrap_returns_in_ok,
    )]
}

fn prepend_assigned_parameter_shadows(method: &MethodDecl, body: &mut Vec<RustStmt>) {
    let Some(method_body) = &method.body else {
        return;
    };
    let mut assigned = std::collections::HashSet::new();
    collect_stmt_assigned_names(&method_body.statements, &mut assigned);
    let mut shadows = method
        .params
        .iter()
        .filter(|param| assigned.contains(&param.name))
        .map(|param| RustStmt::Let {
            mutable: true,
            name: param.name.clone(),
            ty: None,
            value: Some(RustExpr::Var(param.name.clone())),
        })
        .collect::<Vec<_>>();
    if shadows.is_empty() {
        return;
    }
    shadows.append(body);
    *body = shadows;
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
        map_entry_locals: refinements.map_entry_locals.clone(),
    };
    for (index, stmt) in block.statements.iter().enumerate() {
        let mut later_read_names = statement_suffix_read_names(&block.statements[index + 1..]);
        if let Some(inherited_later_read_names) = block_ctx.later_read_names {
            later_read_names.extend(inherited_later_read_names.iter().cloned());
        }
        let mut later_mutated_names =
            statement_suffix_mutated_names(&block.statements[index + 1..]);
        if let Some(inherited_later_mutated_names) = block_ctx.later_mutated_names {
            later_mutated_names.extend(inherited_later_mutated_names.iter().cloned());
        }
        let current_stmt_read_counts = statement_read_counts(stmt);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            later_mutated_names: Some(&later_mutated_names),
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
        let later_mutated_names = statement_suffix_mutated_names(&block.statements[index + 1..]);
        let current_stmt_read_counts = statement_read_counts(stmt);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            later_mutated_names: Some(&later_mutated_names),
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

fn statement_suffix_mutated_names(statements: &[Stmt]) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_stmt_assigned_names(statements, &mut names);
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
            if declared_graphics_create_initializer_should_anchor_local_type(
                ty,
                initializer.as_ref(),
            ) {
                local_plan.shape = declared_generic_local_slot_shape(ty, &local_plan.shape);
            }
            if ctx
                .later_mutated_names
                .is_some_and(|names| names.contains(name))
            {
                local_plan.binding_mutable = true;
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
                    let lowered = lower_expr_for_slot_with_refinements(
                        expr,
                        env,
                        initializer_slot.clone(),
                        ctx,
                        refinements,
                    );
                    adapt_reused_owned_value_for_slot(lowered, expr, &initializer_slot, env, ctx)
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
            if let Some(stmt) =
                lower_static_storage_assignment_stmt(target, value, env, ctx, refinements)
            {
                out.push(stmt);
                return;
            }
            if let Some(stmt) =
                lower_qualified_static_storage_assignment_stmt(target, value, env, ctx, refinements)
            {
                out.push(stmt);
                return;
            }
            let assignment_slot =
                target_assignment_slot_with_current_type(target, env, ctx, shape_ctx);
            if assignment_statement_plan_for_target_with_current_type(
                target,
                value,
                env,
                shape_ctx,
                &assignment_slot,
            ) == crate::conversion_plan::AssignmentStatementPlan::ClearOptionalTarget
            {
                out.push(expr_stmt(receiver_method_call_projection_exact(
                    lower_expr(target, env, ctx),
                    "take",
                    Vec::new(),
                )));
            } else {
                let lowered_value = lower_expr_for_slot_with_refinements(
                    value,
                    env,
                    assignment_slot.clone(),
                    ctx,
                    refinements,
                );
                let lowered_value = adapt_reused_owned_value_for_slot(
                    lowered_value,
                    value,
                    &assignment_slot,
                    env,
                    ctx,
                );
                let lowered_target =
                    lower_array_assignment_target_for_stmt(target, env, ctx, refinements)
                        .unwrap_or_else(|| lower_expr(target, env, ctx));
                out.push(assignment_stmt(
                    lowered_target,
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
            if source_null_assertion_call_expr_stmt_is_noop(expr, env, ctx) {
                return;
            } else if let Some(stmt) = lower_native_collection_extend_expr_stmt(expr, env, ctx) {
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
            WhileStmtPlan::Infinite { body } => out.push(RustStmt::Loop {
                label: None,
                body: lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx),
            }),
        },
        Stmt::DoWhile { body, condition } => {
            let mut loop_body = lower_block(body, &mut env.clone(), wrap_returns_in_ok, ctx);
            let lowered_condition = lower_expr_for_slot(condition, env, condition_slot(), ctx);
            loop_body.push(if_stmt(
                logical_not_projection(lowered_condition),
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
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => out.push(lower_enhanced_for_stmt(
            var_type,
            var_name,
            iterable,
            body,
            env,
            wrap_returns_in_ok,
            ctx,
            refinements,
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

fn target_assignment_slot_with_current_type(
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
    if expr_lowers_to_optional_carrier_for_slot(value, &target) {
        return lowered_value;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let inferred_actual = optional_field_access_actual_shape_for_slot(value, env, &slot, ctx)
        .unwrap_or_else(|| actual_shape_for_slot_expr(value, env, shape_ctx));
    let planned_actual = preferred_planned_or_external_call_result_shape(value, env, ctx);
    let actual = planned_actual.unwrap_or(inferred_actual);
    let already_optional_result = known_external_call_result_shape_from_syntax(value, ctx)
        .is_some_and(|shape| shape.optional_element_type().is_some());
    planned_optional_assignment_value_for_target(
        lowered_value,
        actual,
        slot.clone(),
        &target,
        already_optional_result,
        ctx.facts,
    )
}

fn expr_lowers_to_optional_carrier_for_slot(value: &Expr, target: &RustSlotShape) -> bool {
    target.optional_element_type().is_some()
        && matches!(
            value,
            Expr::Ternary {
                then_expr,
                else_expr,
                ..
            } if matches!(
                (then_expr.as_ref(), else_expr.as_ref()),
                (Expr::Literal(Literal::Null), _) | (_, Expr::Literal(Literal::Null))
            )
        )
}

fn optional_field_access_actual_shape_for_slot(
    value: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::FieldAccess(receiver, field) = value else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)
        .unwrap_or_else(|| receiver_shape.java_ty.clone());
    let receiver_owner = concrete_java_type_name(&receiver_ty).map(str::to_string);
    optional_receiver_field_actual_shape_for_slot(field, env, slot, ctx, receiver_owner.as_deref())
}

fn lower_get_resource_as_stream_expr_for_optional_slot(
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
    if !matches!(
        name.as_str(),
        "getResourceAsStream" | "get_resource_as_stream"
    ) || args.len() != 1
    {
        return None;
    }
    let target = slot_target_shape(slot)?;
    let target_element_ty = target.optional_element_type()?;
    if concrete_java_type_name(&target_element_ty)
        .map(crate::stdlib::canonical_external_receiver_type_key)
        .as_deref()
        != Some("InputStream")
    {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    let lowered_arg = lower_expr_for_slot(
        &args[0],
        env,
        argument_slot(RustSlotShape::conservative(JavaType::String)),
        ctx,
    );
    Some(receiver_method_call_projection_exact(
        lowered_receiver,
        "get_resource_as_stream",
        vec![lowered_arg],
    ))
}

fn lower_create_graphics_expr_for_graphics_slot(
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
    if !matches!(name.as_str(), "createGraphics" | "create_graphics") || !args.is_empty() {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if !matches!(
        target.java_ty,
        JavaType::Named { ref name, .. } | JavaType::Unknown(ref name)
            if matches!(name.rsplit('.').next().unwrap_or(name.as_str()), "Graphics")
    ) {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    let lowered =
        receiver_method_call_projection_exact(lowered_receiver, "create_graphics", Vec::new());
    Some(apply_expr_to_slot_via_plan(
        lowered,
        RustSlotShape::conservative(JavaType::Named {
            name: "Graphics2D".to_string(),
            args: Vec::new(),
        }),
        slot,
        ctx.facts,
    ))
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
    if bare_local_name_shadows_field(expr, env) {
        return None;
    }
    current_instance_field_shape(expr, ctx)
        .or_else(|| {
            current_instance_field_name(expr)
                .and_then(|field| env.field_shape(field))
                .cloned()
        })
        .or_else(|| outer_instance_field_shape_with_env(expr, env, ctx))
        .or_else(|| {
            let Expr::Var(name) = expr else {
                return None;
            };
            if env.is_static_field(name) && !current_env_name_is_instance_field(env, name) {
                return None;
            }
            let type_name = ctx.current_type?;
            signature_field_shape_for_type_aliases(ctx.signatures, type_name, name).cloned()
        })
}

fn bare_local_name_shadows_field(expr: &Expr, env: &ShapeEnv) -> bool {
    matches!(expr, Expr::Var(name) if !name.contains('.') && env.local_shape(name).is_some())
}

fn outer_instance_field_shape_with_env(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::Var(name) = expr else {
        return None;
    };
    if !name.contains('.') && env.local_shape(name).is_some() {
        return None;
    }
    let current_type = ctx.current_type?;
    let outer_type = ctx.facts.captures.outer_this_type_for_type(current_type)?;
    signature_field_shape_for_type_aliases(ctx.signatures, outer_type, name).cloned()
}

fn current_env_name_is_instance_field(env: &ShapeEnv, name: &str) -> bool {
    env.is_instance_field(name) || env.is_instance_field(&crate::parser::to_snake_case(name))
}

fn bare_name_looks_like_static_owner(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
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

fn source_java_enum_instance_field_accessor_name(name: &str, ctx: LowerCtx) -> Option<String> {
    let current_type = ctx.current_type?;
    let field_name = crate::parser::to_snake_case(name.strip_prefix("this.").unwrap_or(name));
    let ty = find_source_enum_type_decl_for_current_type(current_type, ctx)?;
    if ty.kind != TypeKind::Enum {
        return None;
    }
    ty.members.iter().find_map(|member| {
        let Member::Field(field) = member else {
            return None;
        };
        if field.modifiers.contains(&Modifier::Static) {
            return None;
        }
        (crate::parser::to_snake_case(&field.name) == field_name).then(|| field_name.clone())
    })
}

fn source_java_enum_field_accessor_name_for_receiver(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<String> {
    if is_this_receiver_expr(receiver) {
        return source_java_enum_instance_field_accessor_name(field, ctx);
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let ty = find_source_enum_type_decl_for_java_type(&receiver_shape.java_ty, ctx)?;
    let field_name = crate::parser::to_snake_case(field);
    source_java_enum_payload_field_shape(ty, &field_name).map(|_| field_name)
}

fn source_java_enum_instance_field_accessor_shape(
    expr: &Expr,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let field_name = match expr {
        Expr::Var(name) => crate::parser::to_snake_case(name.strip_prefix("this.").unwrap_or(name)),
        Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => {
            crate::parser::to_snake_case(field)
        }
        _ => return None,
    };
    let current_type = ctx.current_type?;
    let ty = find_source_enum_type_decl_for_current_type(current_type, ctx)?;
    if ty.kind != TypeKind::Enum {
        return None;
    }
    source_java_enum_payload_field_shape(ty, &field_name)
}

fn source_java_enum_field_accessor_shape_for_receiver(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    if is_this_receiver_expr(receiver) {
        return source_java_enum_instance_field_accessor_shape(
            &Expr::FieldAccess(Box::new(Expr::This), field.to_string()),
            ctx,
        );
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let ty = find_source_enum_type_decl_for_java_type(&receiver_shape.java_ty, ctx)?;
    let field_name = crate::parser::to_snake_case(field);
    source_java_enum_payload_field_shape(ty, &field_name)
}

fn source_java_enum_field_accessor_shape_for_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    match expr {
        Expr::FieldAccess(receiver, field) => {
            source_java_enum_field_accessor_shape_for_receiver(receiver, field, env, ctx)
        }
        _ => source_java_enum_instance_field_accessor_shape(expr, ctx),
    }
}

fn source_java_enum_payload_field_shape(ty: &TypeDecl, field_name: &str) -> Option<RustSlotShape> {
    if ty.kind != TypeKind::Enum {
        return None;
    }
    ty.members.iter().find_map(|member| {
        let Member::Field(field) = member else {
            return None;
        };
        if field.modifiers.contains(&Modifier::Static)
            || crate::parser::to_snake_case(&field.name) != field_name
        {
            return None;
        }
        Some(value_shape(field.ty.clone()))
    })
}

fn lower_source_java_enum_instance_field_accessor_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let actual = source_java_enum_instance_field_accessor_shape(expr, ctx)?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let plan = plan_slot_conversion(actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn lower_source_java_enum_field_accessor_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return lower_source_java_enum_instance_field_accessor_for_slot(
            expr,
            env,
            slot,
            ctx,
            refinements,
        );
    };
    let actual = source_java_enum_field_accessor_shape_for_receiver(receiver, field, env, ctx)?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let plan = plan_slot_conversion(actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn find_source_enum_type_decl_for_current_type<'a>(
    current_type: &str,
    ctx: LowerCtx<'a>,
) -> Option<&'a TypeDecl> {
    let simple = current_type
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(current_type);
    [
        ctx.program,
        ctx.support_local_type_program,
        ctx.source_summary_type_program,
    ]
    .into_iter()
    .find_map(|program| {
        find_type_decl_by_name(program, current_type)
            .or_else(|| find_unique_type_decl_by_simple_name(program, simple))
            .or_else(|| find_type_decl_by_rust_name(program, current_type))
            .filter(|ty| ty.kind == TypeKind::Enum)
    })
}

fn find_source_enum_type_decl_for_java_type<'a>(
    java_ty: &JavaType,
    ctx: LowerCtx<'a>,
) -> Option<&'a TypeDecl> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    let simple = name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(name);
    [
        ctx.program,
        ctx.support_local_type_program,
        ctx.source_summary_type_program,
    ]
    .into_iter()
    .find_map(|program| {
        find_type_decl_by_name(program, name)
            .or_else(|| find_unique_type_decl_by_simple_name(program, simple))
            .or_else(|| {
                find_type_decl_by_rust_name(
                    program,
                    &crate::slot_render::rust_name_for_java_named_type(name),
                )
            })
            .filter(|ty| ty.kind == TypeKind::Enum)
    })
}

fn find_type_decl_by_rust_name<'a>(program: &'a Program, rust_name: &str) -> Option<&'a TypeDecl> {
    program.types.iter().find_map(|ty| {
        type_decl_and_nested_types(ty)
            .into_iter()
            .find(|candidate| {
                crate::slot_render::rust_name_for_java_named_type(&candidate.name) == rust_name
            })
    })
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
    if slot_expects_optional(&slot) && slot_expr_needs_result_propagation(value, env, ctx) {
        return lower_expr_for_slot_with_refinements(value, env, slot, ctx, refinements);
    }
    if slot_expects_optional(&slot) {
        return lower_expr_with_refinements(value, env, ctx, refinements);
    }
    lower_expr_for_slot_with_refinements(value, env, slot, ctx, refinements)
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
        refinements.map_entry_locals.remove(&name);
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
    if let Some(refinement) = exact_type_refinement_from_rejected_instanceof_condition(condition) {
        return Some(refinement);
    }
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

fn exact_type_refinement_from_rejected_instanceof_condition(
    condition: &Expr,
) -> Option<(String, JavaType)> {
    match condition {
        Expr::Unary {
            op: UnaryOp::Not,
            expr,
        } => exact_type_refinement_from_instanceof_expr(expr),
        Expr::Binary { op, left, right } if matches!(op, BinaryOp::Eq | BinaryOp::Ne) => {
            let left_refinement = exact_type_refinement_from_instanceof_expr(left);
            let right_refinement = exact_type_refinement_from_instanceof_expr(right);
            let bool_value = bool_literal_expr(left).or_else(|| bool_literal_expr(right))?;
            match (op, bool_value, left_refinement.or(right_refinement)) {
                (BinaryOp::Eq, false, Some(refinement))
                | (BinaryOp::Ne, true, Some(refinement)) => Some(refinement),
                _ => None,
            }
        }
        _ => None,
    }
}

fn exact_type_refinement_from_instanceof_expr(expr: &Expr) -> Option<(String, JavaType)> {
    let Expr::InstanceOf { expr, ty } = expr else {
        return None;
    };
    let Expr::Var(local_name) = expr.as_ref() else {
        return None;
    };
    Some((local_name.clone(), ty.clone()))
}

fn bool_literal_expr(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn lower_optional_sequence_length_access(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let inner_ty = receiver_shape.optional_element_type()?;
    if crate::types::sequence_element_type(&inner_ty).is_none()
        && crate::types::two_dimensional_array_element_type(&inner_ty).is_none()
    {
        return None;
    }
    let plan = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        "length",
        env.type_env(),
        ctx.facts,
    )?;
    let lowered_receiver = lower_read_sequence_receiver(receiver, env, ctx, refinements);
    Some(match plan.op {
        crate::member_access_plan::MemberAccessOp::LenAsI32 => len_i32_projection(lowered_receiver),
        crate::member_access_plan::MemberAccessOp::Array2RowsAsI32
            if two_dimensional_array_expr_uses_nested_slice(
                receiver_expr_for_length_plan(receiver),
                env,
                ctx,
            ) =>
        {
            len_i32_projection(lowered_receiver)
        }
        crate::member_access_plan::MemberAccessOp::Array2RowsAsI32 => {
            array2_rows_i32_projection(lowered_receiver)
        }
        crate::member_access_plan::MemberAccessOp::Array2ColsAsI32 => {
            len_i32_projection(lowered_receiver)
        }
        _ => return None,
    })
}

fn receiver_expr_for_length_plan(receiver: &Expr) -> &Expr {
    receiver
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
    let name = borrowed_string_temporary_name(local_name, counter);
    borrowed_string_literal_temporary_plan(raw, &name).map(|plan| {
        (
            BorrowedStringTemporary {
                name,
                expr: plan.replacement_expr,
            },
            plan.initializer_expr,
        )
    })
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
    if let Some(initializer_shape) =
        typed_static_constant_initializer_shape(initializer, &local_plan, env, ctx)
    {
        local_plan.shape = initializer_shape;
        return local_plan;
    }
    local_plan = preserve_declared_erased_object_local_shape(local_plan);
    let Some(initializer) = initializer else {
        return local_plan;
    };
    if local_plan.shape.optional_element_type().is_some()
        && optional_borrowed_local_array_initializer_should_stay_owned(
            initializer,
            &local_plan.shape,
        )
    {
        let java_ty = crate::types::optional_element_type(&local_plan.shape.java_ty)
            .unwrap_or_else(|| local_plan.shape.java_ty.clone());
        local_plan.shape = RustSlotShape::for_storage(java_ty);
        return local_plan;
    }
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
    if let Some(constructor_shape) =
        external_crate_constructor_initializer_shape(initializer, env, ctx)
        && java_types_have_same_erased_identity(
            &constructor_shape.java_ty,
            &local_plan.shape.java_ty,
        )
    {
        local_plan.shape = constructor_shape;
        return local_plan;
    }
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

fn typed_static_constant_initializer_shape(
    initializer: Option<&Expr>,
    local_plan: &LocalPlan,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::FieldAccess(receiver, field) = initializer? else {
        return None;
    };
    let plan = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        env.type_env(),
        ctx.facts,
    )?;
    if !matches!(
        plan.op,
        crate::member_access_plan::MemberAccessOp::StaticConstant { .. }
    ) || !same_concrete_java_type(&plan.shape.java_ty, &local_plan.java_ty)
    {
        return None;
    }
    Some(plan.shape)
}

fn external_crate_constructor_initializer_shape(
    initializer: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::New {
        ty, args, resolved, ..
    } = initializer
    else {
        return None;
    };
    let target = plan_new_target_for_args_with_facts(
        ty,
        resolved.as_ref(),
        ctx.representations,
        ctx.facts,
        args,
        env.type_env(),
    );
    target.external_crate.as_ref()?;
    let mut shape = target.owner_shape;
    crate::shape_projection::apply_required_dispatch(&mut shape, DispatchShape::Concrete);
    Some(shape)
}

fn preserve_declared_erased_object_local_shape(mut local_plan: LocalPlan) -> LocalPlan {
    if !java_type_is_erased_object_name(&local_plan.java_ty) {
        return local_plan;
    }
    if shape_is_erased_object_boundary(&local_plan.shape)
        && local_plan.shape.java_ty == local_plan.java_ty
    {
        return local_plan;
    }
    if shape_is_erased_object_boundary(&local_plan.shape)
        && local_plan
            .shape
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == local_plan.java_ty)
    {
        return local_plan;
    }
    let semantic_ty = if local_plan.shape.optional_element_type().is_some() {
        crate::types::optional_java_type(local_plan.java_ty.clone())
    } else {
        local_plan.java_ty.clone()
    };
    local_plan.shape = RustSlotShape::for_storage(semantic_ty);
    local_plan.shape.dispatch = DispatchShape::ErasedObject;
    local_plan.shape.view = ViewShape::ErasedObjectView;
    local_plan.shape.ownership = OwnershipShape::Owned;
    local_plan
}

fn optional_borrowed_local_array_initializer_should_stay_owned(
    initializer: &Expr,
    shape: &RustSlotShape,
) -> bool {
    matches!(
        shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && crate::types::optional_element_type(&shape.java_ty)
        .is_some_and(|element_ty| matches!(element_ty, JavaType::Array(_)))
        && matches!(
            initializer,
            Expr::ArrayCreation { .. } | Expr::ArrayInitializer(_)
        )
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

fn declared_graphics_create_initializer_should_anchor_local_type(
    declared_ty: &JavaType,
    initializer: Option<&Expr>,
) -> bool {
    let name = match declared_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return false,
    };
    if !matches!(name.rsplit('.').next().unwrap_or(name.as_str()), "Graphics") {
        return false;
    }
    let Some(Expr::Call {
        receiver: Some(_),
        name,
        args,
        resolved: _,
    }) = initializer
    else {
        return false;
    };
    args.is_empty() && matches!(name.as_str(), "createGraphics" | "create_graphics")
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
    local.ownership == OwnershipShape::Owned
        && !shape_is_erased_object_boundary(local)
        && local.optional_element_type().is_none()
        && initializer
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == local.java_ty)
}

fn initializer_shape_is_more_specific_for_local(
    initializer: &RustSlotShape,
    local: &RustSlotShape,
) -> bool {
    if shape_is_erased_object_boundary(local) {
        return false;
    }
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
    if declared.optional_element_type().is_some() || shape_is_erased_object_boundary(&declared) {
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
    constructor_call_return_shape(expr, env, ctx)
        .or_else(|| stream_like_call_return_shape(expr, env, ctx))
        .or_else(|| receiverless_current_type_call_return_shape(expr, ctx))
        .or_else(|| method_call_return_shape_from_receiver(expr, env, ctx))
}

fn constructor_call_return_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::New {
        ty, args, resolved, ..
    } = expr
    else {
        return None;
    };
    let target = plan_new_target_for_args_with_facts(
        ty,
        resolved.as_ref(),
        ctx.representations,
        ctx.facts,
        args,
        env.type_env(),
    );
    if target.external_crate.is_some() {
        let mut shape = target.owner_shape;
        crate::shape_projection::apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        return Some(shape);
    }
    Some(target.owner_shape)
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
    if args.is_empty()
        && let Some(shape) =
            getter_field_return_shape_from_receiver(current_type, name, ctx.signatures)
    {
        return Some(shape);
    }
    if let Some(field_name) = source_method_returned_field_name(current_type, name, args.len(), ctx)
        && let Some(field_shape) =
            signature_field_shape_for_type_aliases(ctx.signatures, current_type, &field_name)
    {
        return Some(field_shape.clone());
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
    source_summary_declared_inherited_method(ty, method_name, arg_count, ctx).and_then(
        |(declaring_ty, method)| {
            source_summary_declared_return_shape_for_method(declaring_ty, method, ctx)
        },
    )
}

fn source_summary_declared_inherited_method<'a>(
    ty: &'a TypeDecl,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx<'a>,
) -> Option<(&'a TypeDecl, &'a MethodDecl)> {
    let mut current = ty
        .extends
        .as_deref()
        .map(crate::stdlib::canonical_external_receiver_type_key);
    let mut visited = HashSet::new();
    while let Some(type_key) = current {
        if !visited.insert(type_key.clone()) {
            break;
        }
        if let Some(local_ty) = find_context_type_by_key(ctx.program, &type_key)
            && let Some(method) =
                find_declared_method_by_name_and_arity(local_ty, method_name, arg_count)
        {
            return Some((local_ty, method));
        }
        if let Some(summary_ty) =
            find_context_type_by_key(ctx.source_summary_type_program, &type_key)
                .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &type_key))
        {
            if let Some(method) =
                find_declared_method_by_name_and_arity(summary_ty, method_name, arg_count)
            {
                return Some((summary_ty, method));
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

fn block_contains_value_return(block: &Block) -> bool {
    block.statements.iter().any(stmt_contains_value_return)
}

fn stmt_contains_value_return(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(Some(_)) => true,
        Stmt::Return(None) | Stmt::Throw(_) => false,
        Stmt::Block(block) => block_contains_value_return(block),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_contains_value_return(then_block)
                || else_block.as_ref().is_some_and(block_contains_value_return)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. } => block_contains_value_return(body),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            block_contains_value_return(body)
                || catches
                    .iter()
                    .any(|catch| block_contains_value_return(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_contains_value_return)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_contains_value_return)),
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
    let selector_shape = infer_expr_shape_with_context(
        plan.selector,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
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
            .filter_map(|label| {
                switch_match_pattern(label, env, ctx, &selector_ty, &selector_shape)
            })
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
        receiver_method_call_projection_exact(lowered, "as_str", Vec::new())
    } else {
        lowered
    }
}

fn switch_match_pattern(
    label: &SwitchCaseLabelPlan,
    env: &ShapeEnv,
    ctx: LowerCtx,
    selector_ty: &JavaType,
    selector_shape: &RustSlotShape,
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
    if let Some(element_ty) = selector_shape.optional_element_type()
        && let JavaType::Named { name, .. } = &element_ty
        && let Expr::Var(label_name) = expr
    {
        if let Some(variant) = ctx
            .representations
            .java_enum_variant_for_label(name, label_name)
        {
            return Some(format!("Some({variant})"));
        }
        if external_enum_constant_label_matches(ctx.facts, name, label_name) {
            let owner = crate::slot_render::rust_name_for_java_named_type(
                &crate::stdlib::canonical_external_receiver_type_key(name),
            );
            let field = crate::parser::to_snake_case(label_name);
            return Some(format!("Some({owner}::{field})"));
        }
    }
    if matches!(
        selector_ty,
        JavaType::Primitive(crate::java_ir::PrimitiveType::Char)
    ) && let Some(label) = char_literal_expr_from_integral_expr(expr)
    {
        return Some(switch_match_pattern_expr(label));
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
    let selector_shape = infer_expr_shape_with_context(
        plan.selector,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let label_exprs = case_plan
        .labels
        .iter()
        .filter_map(|label| match label {
            SwitchCaseLabelPlan::Expr(expr) => Some(lower_switch_case_label_expr(
                expr,
                env,
                ctx,
                &selector_ty,
                &selector_shape,
            )),
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
    selector_shape: &RustSlotShape,
) -> RustExpr {
    if matches!(
        selector_ty,
        JavaType::Primitive(crate::java_ir::PrimitiveType::Char)
    ) && let Some(label) = char_literal_expr_from_integral_expr(expr)
    {
        return label;
    }
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
            return owner_associated_path_projection(&owner, &field);
        }
    }
    if let Some(element_ty) = selector_shape.optional_element_type()
        && let JavaType::Named { name, .. } = &element_ty
        && let Expr::Var(label_name) = expr
    {
        if let Some(variant) = ctx
            .representations
            .java_enum_variant_for_label(name, label_name)
        {
            return RustExpr::Raw(format!("Some({variant})"));
        }
        if external_enum_constant_label_matches(ctx.facts, name, label_name) {
            let owner = crate::slot_render::rust_name_for_java_named_type(
                &crate::stdlib::canonical_external_receiver_type_key(name),
            );
            let field = crate::parser::to_snake_case(label_name);
            return RustExpr::Raw(format!("Some({owner}::{field})"));
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
    lower_java_enum_static_field_access_with_owner(receiver, field, env, ctx).map(|(_, expr)| expr)
}

fn lower_java_enum_static_field_access_with_owner(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<(String, RustExpr)> {
    if let Some(owner) = source_type_name_from_expr(receiver, env, ctx)
        && let Some(variant) = ctx
            .representations
            .java_enum_variant_for_label(&owner, field)
    {
        return Some((owner, RustExpr::Raw(variant)));
    }
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    java_enum_static_field_owner_candidates(receiver_name, env, ctx)
        .into_iter()
        .find_map(|owner| {
            ctx.representations
                .java_enum_variant_for_label(&owner, field)
                .map(|variant| (owner, RustExpr::Raw(variant)))
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
    let (owner, lowered) =
        lower_java_enum_static_field_access_with_owner(receiver, field, env, ctx)?;
    let actual = crate::shape_projection::value_shape_for_java_type(
        ctx.representations,
        JavaType::Named {
            name: owner.clone(),
            args: Vec::new(),
        },
    );
    let rendered = emit::emit_expr(&lowered);
    let source_interface_trait_name =
        source_enum_directly_implements_target_interface(&owner, &target.java_ty, ctx);
    match &target.dispatch {
        DispatchShape::ClosedEnum { enum_name } => rendered
            .starts_with(&format!("{enum_name}::"))
            .then_some(lowered),
        _ if target
            .optional_element_type()
            .is_some_and(|element_ty| same_concrete_java_type(&element_ty, &actual.java_ty))
            || same_concrete_java_type(&target.java_ty, &actual.java_ty) =>
        {
            Some(apply_expr_to_slot_via_plan(
                lowered, actual, slot, ctx.facts,
            ))
        }
        DispatchShape::DynTrait { trait_name }
            if ctx
                .facts
                .hierarchy_subtype_policy(
                    &JavaType::Named {
                        name: owner.clone(),
                        args: Vec::new(),
                    },
                    &target.java_ty,
                )
                .reaches
                || source_enum_directly_implements_rust_trait_name(&owner, trait_name, ctx) =>
        {
            Some(borrow_as_dyn_trait(lowered, trait_name))
        }
        _ if target.view == ViewShape::DynTraitView
            && enum_owner_reaches_interface_target(&owner, &target.java_ty, ctx) =>
        {
            let trait_name = crate::slot_render::rust_name_for_java_named_type(
                concrete_java_type_name(&target.java_ty)?,
            );
            Some(borrow_as_dyn_trait(lowered, &trait_name))
        }
        _ if target.ownership == OwnershipShape::SharedBorrow
            && concrete_java_type_name(&target.java_ty)
                .and_then(|target_name| {
                    ctx.program
                        .types
                        .iter()
                        .find(|ty| ty.name == target_name)
                        .map(|ty| (target_name, &ty.kind))
                })
                .is_some_and(|(target_name, kind)| {
                    *kind == TypeKind::Interface
                        && enum_owner_reaches_interface_target(&owner, &target.java_ty, ctx)
                        && !target_name.is_empty()
                }) =>
        {
            let trait_name = crate::slot_render::rust_name_for_java_named_type(
                concrete_java_type_name(&target.java_ty)?,
            );
            Some(borrow_as_dyn_trait(lowered, &trait_name))
        }
        _ if source_interface_trait_name.is_some() => {
            let trait_name = source_interface_trait_name
                .as_deref()
                .expect("source interface trait name was checked");
            Some(borrow_as_dyn_trait(lowered, trait_name))
        }
        _ => None,
    }
}

fn enum_owner_reaches_interface_target(owner: &str, target: &JavaType, ctx: LowerCtx) -> bool {
    if ctx
        .facts
        .hierarchy_subtype_policy(
            &JavaType::Named {
                name: owner.to_string(),
                args: Vec::new(),
            },
            target,
        )
        .reaches
    {
        return true;
    }
    source_enum_directly_implements_target_interface(owner, target, ctx).is_some()
}

fn source_enum_directly_implements_rust_trait_name(
    owner: &str,
    trait_name: &str,
    ctx: LowerCtx,
) -> bool {
    let Some(enum_ty) = find_type_decl_by_name(ctx.support_local_type_program, owner)
        .or_else(|| find_type_decl_by_name(&ctx.program, owner))
    else {
        return false;
    };
    enum_ty.kind == TypeKind::Enum
        && enum_ty.implements.iter().any(|implemented| {
            crate::slot_render::rust_name_for_java_named_type(implemented) == trait_name
        })
}

fn source_enum_directly_implements_target_interface(
    owner: &str,
    target: &JavaType,
    ctx: LowerCtx,
) -> Option<String> {
    let target_name = concrete_java_type_name(target)?;
    let target_is_interface = find_type_decl_by_name(ctx.support_local_type_program, target_name)
        .or_else(|| find_type_decl_by_name(&ctx.program, target_name))
        .is_some_and(|ty| ty.kind == TypeKind::Interface);
    if !target_is_interface {
        return None;
    }
    let enum_ty = find_type_decl_by_name(ctx.support_local_type_program, owner)
        .or_else(|| find_type_decl_by_name(&ctx.program, owner))?;
    if enum_ty.kind != TypeKind::Enum {
        return None;
    }
    enum_ty
        .implements
        .iter()
        .any(|implemented| {
            same_concrete_java_type_name(
                &crate::parser::parse_hierarchy_type(implemented),
                target_name,
            )
        })
        .then(|| crate::slot_render::rust_name_for_java_named_type(target_name))
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
    push_unique_receiver_type_name(
        &mut candidates,
        upper_camel_type_name_from_static_receiver(receiver_name),
    );
    if let Some(current_type) = ctx.current_type
        && let Some(current) = find_context_type_by_key(ctx.program, current_type)
    {
        collect_nested_enum_owner_candidates(current, receiver_name, &mut candidates);
    }
    candidates
}

fn upper_camel_type_name_from_static_receiver(receiver_name: &str) -> String {
    receiver_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
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
    let value = expr.map(|expr| {
        let lowered = match return_value_plan(ctx.current_return_shape) {
            ReturnValuePlan::Slot(slot) => {
                lower_expr_for_slot_with_refinements(expr, env, slot, ctx, refinements)
            }
            ReturnValuePlan::Raw => lower_expr_with_refinements(expr, env, ctx, refinements),
        };
        if !wrap_returns_in_ok && slot_expr_needs_result_propagation(expr, env, ctx) {
            unwrap_result_or_panic_uncaught_java_exception(lowered)
        } else {
            lowered
        }
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
            let second_arg_is_cause = exception_constructor_second_arg_is_cause(expr, env, ctx);
            let args = args
                .iter()
                .enumerate()
                .map(|(index, arg)| {
                    if second_arg_is_cause && index == 1 {
                        lower_expr_with_refinements(arg, env, ctx, refinements)
                    } else {
                        lower_expr_for_slot_with_refinements(
                            arg,
                            env,
                            Slot::DisplaySegment,
                            ctx,
                            refinements,
                        )
                    }
                })
                .collect();
            exception_expr_from_new_with_cause_hint(&class_name, args, second_arg_is_cause)
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
            let mut finally_read_names = statement_suffix_read_names(&finally_block.statements);
            if let Some(inherited_later_read_names) = ctx.later_read_names {
                finally_read_names.extend(inherited_later_read_names.iter().cloned());
            }
            let try_ctx = LowerCtx {
                later_read_names: Some(&finally_read_names),
                ..ctx
            };
            let try_body = lower_block(planned_body, &mut env.clone(), true, try_ctx);
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
            let mut finally_read_names = statement_suffix_read_names(&finally_block.statements);
            if let Some(inherited_later_read_names) = ctx.later_read_names {
                finally_read_names.extend(inherited_later_read_names.iter().cloned());
            }
            let try_ctx = LowerCtx {
                catch_future_use_names: Some(&catch_future_use_names),
                later_read_names: Some(&finally_read_names),
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
    let value_fallthrough_try = matches!(completion, TryBlockCompletion::FallsThroughUnit)
        && ctx.current_return_shape.is_some()
        && block_contains_value_return(body);
    if matches!(completion, TryBlockCompletion::FallsThroughUnit) && !value_fallthrough_try {
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
    if value_fallthrough_try {
        return try_catch_value_fallthrough_stmt(
            ctx.current_return_shape
                .map(|shape| crate::slot_render::rust_type_for_slot_with_facts(shape, ctx.facts))
                .unwrap_or(RustType::Unit),
            try_body,
            catch_arms,
            wrap_returns_in_ok,
            ctx.current_return_shape
                .is_some_and(|shape| shape.optional_element_type().is_some()),
        );
    }
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
            let close_call = receiver_method_call_projection_exact(
                RustExpr::Var(name.clone()),
                "close",
                Vec::new(),
            );
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
                expr_stmt(propagate_result_projection(close_call))
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
    resource_close_declares_result_with_external_index(resource_ty, ctx.external_index)
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
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => {
            if let Expr::Var(name) = expr.as_ref() {
                names.insert(name.clone());
            }
            collect_expr_assigned_names(expr, names);
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_expr_assigned_names(receiver, names);
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            if let Expr::Var(name) = left.as_ref() {
                names.insert(name.clone());
            }
            collect_expr_assigned_names(left, names);
            collect_expr_assigned_names(right, names);
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
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if let Some(Expr::Var(target_name)) =
                static_collection_mutator_target(receiver.as_deref(), name, args)
            {
                names.insert(target_name.clone());
            }
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

fn static_collection_mutator_target<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<&'a Expr> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if receiver_key == "Arrays" && matches!(name, "sort" | "fill") {
        return args.first();
    }
    if receiver_key == "Collections" && name == "sort" && matches!(args.len(), 1 | 2) {
        return args.first();
    }
    None
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
    let planned_extend = matches!(
        plan,
        CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
            method: crate::call_plan::NativeCollectionMethod::Extend,
            ..
        })
    );
    if !planned_extend && !optional_collection_receiver_bulk_extend_method(receiver, name, env, ctx)
    {
        return None;
    }

    let receiver_ty = native_collection_extend_receiver_type(receiver, env, ctx)?;
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
            lower_native_map_extend_owned_arg(&receiver_ty, &args[0], &arg_shape, env, ctx)
                .unwrap_or_else(|| {
                    lower_expr_for_slot(&args[0], env, argument_slot(slot_shape), ctx)
                })
        }
    };

    let lowered_receiver = lower_expr(receiver, env, ctx);
    let lowered_receiver = if crate::types::optional_element_type(&receiver_ty)
        .is_some_and(|ty| crate::stdlib::java_collection_kind_for_type(&ty).is_some())
    {
        adapt_optional_instance_receiver_with_slot_plan(
            receiver,
            lowered_receiver,
            env,
            ctx,
            true,
            true,
        )
    } else {
        adapt_native_collection_receiver(
            receiver,
            lowered_receiver,
            crate::call_plan::NativeCollectionMethod::Extend,
            env,
            ctx,
        )
    };
    if native_collection_extend_needs_self_read_temp(receiver, &args[0], env, ctx) {
        let temp_name = "__java2rust_add_all_source";
        return Some(raw_stmt(format!(
            "let {temp_name} = {};\n{};\n",
            emit::emit_expr(&arg),
            emit::emit_expr(&apply_native_collection_extend(
                lowered_receiver,
                RustExpr::Var(temp_name.to_string())
            ))
        )));
    }

    Some(expr_stmt(apply_native_collection_extend(
        lowered_receiver,
        arg,
    )))
}

fn native_collection_extend_receiver_type(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    let inferred_ty = types::infer_expr_type(receiver, env.type_env());
    if crate::stdlib::java_collection_kind_for_type(
        &crate::types::optional_element_type(&inferred_ty).unwrap_or_else(|| inferred_ty.clone()),
    )
    .is_some()
    {
        return Some(inferred_ty);
    }

    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let inferred_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if inferred_shape
        .optional_element_type()
        .is_some_and(|ty| crate::stdlib::java_collection_kind_for_type(&ty).is_some())
    {
        return Some(inferred_shape.java_ty);
    }

    let field_shape = current_instance_field_shape_with_env(receiver, env, ctx)?;
    if field_shape
        .optional_element_type()
        .is_some_and(|ty| crate::stdlib::java_collection_kind_for_type(&ty).is_some())
    {
        return Some(field_shape.java_ty);
    }
    None
}

fn optional_collection_receiver_bulk_extend_method(
    receiver: &Expr,
    method: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Some(receiver_ty) = native_collection_extend_receiver_type(receiver, env, ctx) else {
        return false;
    };
    let effective_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or_else(|| receiver_ty.clone());
    let Some(kind) = crate::stdlib::java_collection_kind_for_type(&effective_ty) else {
        return false;
    };
    matches!(
        (kind, method),
        (
            crate::stdlib::JavaCollectionKind::List | crate::stdlib::JavaCollectionKind::Set,
            "addAll" | "add_all"
        ) | (crate::stdlib::JavaCollectionKind::Map, "putAll" | "put_all")
    )
}

fn lower_native_map_extend_owned_arg(
    receiver_ty: &JavaType,
    arg: &Expr,
    arg_shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let receiver_ty =
        crate::types::optional_element_type(receiver_ty).unwrap_or_else(|| receiver_ty.clone());
    if crate::stdlib::java_collection_kind_for_type(&receiver_ty)
        != Some(crate::stdlib::JavaCollectionKind::Map)
    {
        return None;
    }
    let (target_key_ty, target_value_ty, _) = map_type_args_from_java_type(&receiver_ty)?;
    let (source_key_ty, source_value_ty, _) = map_type_args_from_java_type(&arg_shape.java_ty)?;
    if source_key_ty != target_key_ty
        || source_value_ty == target_value_ty
        || !java_type_is_erased_object_like_for_lowering(&target_value_ty)
    {
        return None;
    }

    let source_shape = RustSlotShape::for_storage(arg_shape.java_ty.clone());
    let lowered_source = lower_expr_for_slot(arg, env, argument_slot(source_shape), ctx);
    let target_value_shape = crate::shape_projection::project_java_value_shape(
        Some(ctx.representations),
        target_value_ty,
    );
    let value_expr = apply_expr_to_slot_via_plan(
        RustExpr::Var("v".to_string()),
        RustSlotShape::for_storage(source_value_ty),
        &argument_slot(target_value_shape),
        ctx.facts,
    );
    Some(RustExpr::Raw(format!(
        "{}.into_iter().map(|(k, v)| (k, {}))",
        emit::emit_expr(&lowered_source),
        emit::emit_expr(&value_expr)
    )))
}

fn java_type_is_erased_object_like_for_lowering(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Unknown(name) | JavaType::Named { name, .. }
        if name.eq_ignore_ascii_case("Object") || name.eq_ignore_ascii_case("java.lang.Object"))
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
        Expr::Var(name) => env.local_shape(name).is_none() && env.field_type(name).is_some(),
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
        CallStatementPlan::NativeArraysSort { target } => Some(RustStmt::Expr(
            collection_sort_projection(lower_expr(target, env, ctx)),
        )),
        CallStatementPlan::NativeCollectionsSort { target, comparator } => {
            let expr = if let Some(comparator) = comparator {
                collection_sort_by_comparator_projection(
                    lower_expr(target, env, ctx),
                    lower_expr(comparator, env, ctx),
                )
            } else {
                collection_sort_projection(lower_expr(target, env, ctx))
            };
            Some(RustStmt::Expr(expr))
        }
        CallStatementPlan::NativeArraysFill { target, value } => Some(RustStmt::Expr(
            array_fill_projection(lower_expr(target, env, ctx), lower_expr(value, env, ctx)),
        )),
        CallStatementPlan::NativeSystemPrintStream {
            stream,
            method,
            arg,
        } => Some(RustStmt::Expr(render_native_system_print_stream_stmt(
            stream, method, arg, env, ctx,
        ))),
        CallStatementPlan::NativeJavaExceptionPrintStackTrace => {
            Some(RustStmt::Expr(RustExpr::Raw(
                "{ /* Java printStackTrace omitted by translation runtime */ }".to_string(),
            )))
        }
    }
}

fn render_native_system_print_stream_stmt(
    stream: NativeSystemStream,
    method: NativePrintStreamMethod,
    arg: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let macro_name = match (stream, method) {
        (NativeSystemStream::Out, NativePrintStreamMethod::Print) => "print",
        (NativeSystemStream::Out, NativePrintStreamMethod::Println) => "println",
        (NativeSystemStream::Err, NativePrintStreamMethod::Print) => "eprint",
        (NativeSystemStream::Err, NativePrintStreamMethod::Println) => "eprintln",
    };
    let Some(arg) = arg else {
        return print_macro_projection(macro_name, None);
    };
    let lowered = lower_expr_for_slot(arg, env, Slot::DisplaySegment, ctx);
    print_macro_projection(macro_name, Some(lowered))
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
    if statement_expr_is_super_call(expr) {
        return Some(expr_stmt(RustExpr::Raw(
            "Ok::<(), JavaException>(())?".to_string(),
        )));
    }
    if !slot_expr_needs_result_propagation(expr, env, ctx) {
        return None;
    }
    Some(expr_stmt(lower_expr(expr, env, ctx)))
}

fn statement_expr_is_super_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            ..
        } if matches!(receiver.as_ref(), Expr::Super)
    )
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
    var_type: &JavaType,
    var_name: &str,
    iterable: &Expr,
    body: &Block,
    env: &ShapeEnv,
    wrap_returns_in_ok: bool,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustStmt {
    let mut loop_env = env.clone();
    let var_is_assigned = enhanced_for_var_is_assigned(var_name, body);
    if var_is_assigned && matches!(var_type, JavaType::String) {
        loop_env.insert_local_shape(var_name.to_string(), value_shape(JavaType::String));
        let iterator =
            sequence_iter_cloned_projection(lower_enhanced_for_iterable_expr(iterable, env, ctx));
        return render_enhanced_for_iterator(
            &format!("mut {var_name}"),
            &iterator,
            &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
        );
    }
    if static_string_sequence_field_iterable(iterable, env, ctx) {
        loop_env.insert_local_shape(var_name.to_string(), borrowed_string_content_shape());
        let iterator =
            sequence_iter_copied_projection(lower_enhanced_for_iterable_expr(iterable, env, ctx));
        return render_enhanced_for_iterator(
            var_name,
            &iterator,
            &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
        );
    }
    if matches!(var_type, JavaType::Primitive(_))
        && enhanced_for_iterable_element_type(iterable, env, ctx)
            .is_some_and(|element_ty| matches!(element_ty, JavaType::Primitive(_)))
    {
        loop_env.insert_local_shape(var_name.to_string(), value_shape(var_type.clone()));
        let iterator =
            sequence_iter_copied_projection(lower_enhanced_for_iterable_expr(iterable, env, ctx));
        let pattern = if var_is_assigned {
            format!("mut {var_name}")
        } else {
            var_name.to_string()
        };
        return render_enhanced_for_iterator(
            &pattern,
            &iterator,
            &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
        );
    }
    if !var_is_assigned
        && let Some(element_ty) = enhanced_for_iterable_element_type(iterable, env, ctx)
        && let Some(element_name) = concrete_java_type_name(&element_ty)
        && matches_source_java_enum_type(element_name, ctx)
    {
        loop_env.insert_local_shape(var_name.to_string(), value_shape(element_ty));
        let iterator =
            sequence_iter_copied_projection(lower_enhanced_for_iterable_expr(iterable, env, ctx));
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
    if let Some((iterator, key_ty, value_ty)) =
        lower_map_entry_set_expr(iterable, env, ctx, refinements)
    {
        loop_env.insert_local_shape(
            var_name.to_string(),
            borrowed_sequence_element_shape(JavaType::Named {
                name: "java.util.Map.Entry".to_string(),
                args: vec![key_ty.clone(), value_ty.clone()],
            }),
        );
        let loop_refinements = refinements.with_map_entry_local(var_name, key_ty, value_ty);
        return render_enhanced_for_iterator(
            var_name,
            &iterator,
            &lower_block_with_refinements(
                body,
                &mut loop_env,
                wrap_returns_in_ok,
                ctx,
                &loop_refinements,
            ),
        );
    }
    if let Some(element_ty) = enhanced_for_iterable_element_type(iterable, env, ctx) {
        loop_env.insert_local_shape(
            var_name.to_string(),
            borrowed_sequence_element_shape(element_ty),
        );
    }
    render_enhanced_for(
        var_name,
        &lower_enhanced_for_iterable_expr(iterable, env, ctx),
        &lower_block(body, &mut loop_env, wrap_returns_in_ok, ctx),
    )
}

fn enhanced_for_iterable_element_type(
    iterable: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    if let Some(owner) = source_java_enum_values_call_owner(iterable, env, ctx)
        && matches_source_java_enum_type(&owner, ctx)
    {
        return Some(JavaType::Named {
            name: owner,
            args: Vec::new(),
        });
    }
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = iterable
        && args.is_empty()
        && matches!(name.as_str(), "keySet" | "key_set")
        && let Some((key_ty, _, _)) =
            map_type_args_from_java_type(&crate::types::infer_expr_type(receiver, env.type_env()))
    {
        return Some(key_ty);
    }
    crate::types::sequence_element_type(&crate::types::infer_expr_type(iterable, env.type_env()))
        .or_else(|| {
            let shape = infer_expr_shape_with_context(
                iterable,
                env,
                ExprShapeContext {
                    signatures: Some(ctx.signatures),
                    representations: Some(ctx.representations),
                    facts: Some(ctx.facts),
                },
            );
            crate::types::sequence_element_type(&shape.java_ty)
        })
}

fn enhanced_for_var_is_assigned(var_name: &str, body: &Block) -> bool {
    let mut assigned = HashSet::new();
    collect_stmt_assigned_names(&body.statements, &mut assigned);
    assigned.contains(var_name)
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
        adapt_optional_instance_receiver_with_slot_plan(iterable, lowered, env, ctx, false, false)
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
    receiver_method_call_projection_exact(RustExpr::Var("self".to_string()), &rust_name, rust_args)
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
    let field_name = accessor_member_name(field_name);
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

fn accessor_member_name(field_name: &str) -> &str {
    field_name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(field_name)
}

fn lower_outer_this_field_name(name: &str, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustExpr> {
    if env.local_shape(name).is_some() {
        return None;
    }
    let current_type = ctx.current_type?;
    let outer_type = ctx.facts.captures.outer_this_type_for_type(current_type)?;
    signature_field_shape_for_type_aliases(ctx.signatures, outer_type, name)?;
    Some(outer_this_field_projection(name))
}

fn lower_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> RustExpr {
    lower_expr_with_refinements(expr, env, ctx, &ExprRefinements::default())
}

#[derive(Debug, Default)]
struct ExprRefinements {
    non_null_locals: HashSet<String>,
    exact_erased_object_locals: HashMap<String, JavaType>,
    map_entry_locals: HashMap<String, (JavaType, JavaType)>,
}

impl ExprRefinements {
    fn with_non_null_local(&self, name: &str) -> Self {
        let mut refinements = Self {
            non_null_locals: self.non_null_locals.clone(),
            exact_erased_object_locals: self.exact_erased_object_locals.clone(),
            map_entry_locals: self.map_entry_locals.clone(),
        };
        refinements.non_null_locals.insert(name.to_string());
        refinements
    }

    fn with_map_entry_local(&self, name: &str, key_ty: JavaType, value_ty: JavaType) -> Self {
        let mut refinements = Self {
            non_null_locals: self.non_null_locals.clone(),
            exact_erased_object_locals: self.exact_erased_object_locals.clone(),
            map_entry_locals: self.map_entry_locals.clone(),
        };
        refinements
            .map_entry_locals
            .insert(name.to_string(), (key_ty, value_ty));
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
        Expr::Var(name) => {
            if let Some(expr) = lower_outer_this_field_name(name, env, ctx) {
                expr
            } else if let Some(field_name) =
                source_java_enum_instance_field_accessor_name(name, ctx)
            {
                receiver_method_call_projection_exact(this_expr(), &field_name, Vec::new())
            } else if refinements.non_null_locals.contains(name) {
                lower_refined_non_null_var(name, env)
            } else {
                lower_bare_name(name, env)
            }
        }
        Expr::This => this_expr(),
        Expr::Super => super_expr(),
        Expr::FieldAccess(receiver, field) => {
            if let Some(nested_type) = lower_source_nested_type_access(receiver, field, env, ctx) {
                return nested_type;
            }
            if let Some(constant) =
                lower_source_associated_const_field_access(receiver, field, env, ctx)
            {
                return constant;
            }
            if let Some(enum_variant) =
                lower_java_enum_static_field_access(receiver, field, env, ctx)
            {
                return enum_variant;
            }
            if let Some(storage) = lower_static_storage_field_access(receiver, field, env, ctx) {
                return storage;
            }
            if let Some(field_name) =
                source_java_enum_field_accessor_name_for_receiver(receiver, field, env, ctx)
            {
                return receiver_method_call_projection_exact(
                    lower_expr_with_refinements(receiver, env, ctx, refinements),
                    &field_name,
                    Vec::new(),
                );
            }
            if field == "length"
                && let Some(length) =
                    lower_java_array_length_access(receiver, env, ctx, refinements)
            {
                return length;
            }
            if field == "length" && static_final_const_array_field_type(receiver, ctx).is_some() {
                return len_i32_projection(lower_expr_with_refinements(
                    receiver,
                    env,
                    ctx,
                    refinements,
                ));
            }
            if field == "length" && two_dimensional_array_expr_uses_nested_slice(receiver, env, ctx)
            {
                return len_i32_projection(lower_expr_with_refinements(
                    receiver,
                    env,
                    ctx,
                    refinements,
                ));
            }
            if field == "length"
                && let Some(expr) =
                    lower_optional_sequence_length_access(receiver, env, ctx, refinements)
            {
                return expr;
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
        Expr::Unary { op, expr } => {
            lower_unary_expr_with_refinements(*op, expr, env, ctx, refinements)
        }
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
            if let Some(expr) = lower_string_value_of_display_call(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
                refinements,
            ) {
                return expr;
            }
            if receiver.is_none() && is_java_get_class_name(name) && args.is_empty() {
                let current_type = ctx.current_type.unwrap_or("java.lang.Object");
                let rust_type = crate::slot_render::rust_name_for_java_named_type(current_type);
                return RustExpr::Raw(format!(
                    "JavaClass::<{rust_type}>::for_name({current_type:?})"
                ));
            }
            if let Some(expr) =
                lower_jdk_input_stream_super_read_call(receiver.as_deref(), name, args, env, ctx)
            {
                return expr;
            }
            if let Some(expr) = lower_source_input_stream_read_byte_array_call(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
                refinements,
            ) {
                return expr;
            }
            if let Some(expr) = lower_source_output_stream_write_byte_array_call(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
                refinements,
            ) {
                return expr;
            }
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
            if let Some(expr) = lower_java_objects_null_predicate_call_expr(
                receiver.as_deref(),
                name,
                args,
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
            if let Some(expr) = lower_source_java_enum_type_api_call(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
                refinements,
            ) {
                return expr;
            }
            if let Some(expr) = lower_arrays_as_list_single_object_array_call(
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
            ) {
                return expr;
            }
            if let Some(expr) = lower_map_get_collection_mutation_call(
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
            })
            .or_else(|| {
                source_context_resolved_static_method_fallback_signature(
                    receiver.as_deref(),
                    name,
                    args,
                    resolved.as_ref(),
                    env,
                    ctx,
                )
            })
            .or_else(|| {
                source_context_current_static_method_fallback_signature(
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
            let plan = apply_fallback_self_rust_name(plan, fallback_rust_name.clone());
            let plan = apply_fallback_static_owner_rust_name(plan, fallback_rust_name);
            let plan = retarget_native_java_string_call_from_shape(
                plan,
                receiver.as_deref(),
                name,
                args,
                env,
                ctx,
            );
            let plan =
                retarget_native_collection_size_from_shape(plan, receiver.as_deref(), name, args);
            let plan = retarget_native_collection_to_array_from_shape(
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
                } if receiver
                    .as_ref()
                    .is_none_or(|receiver| is_this_receiver_expr(receiver)) =>
                {
                    return lower_accessor_field(receiver, &field_name, env, ctx);
                }
                CallExprPlan::TrivialAccessorGetter { .. } => {
                    let Some(receiver) = receiver.as_deref() else {
                        unreachable!("current receiver accessors are handled above")
                    };
                    let rust_name = resolved
                        .as_ref()
                        .map(|resolved| resolved.rust_name.as_str())
                        .unwrap_or_else(|| name.as_str());
                    non_self_trivial_accessor_target(Some(receiver), rust_name)
                        .expect("non-self trivial accessor should have receiver")
                }
                CallExprPlan::Target(target) => target,
            };
            let target = retarget_static_owner_call_for_instance_field_receiver(
                target,
                receiver.as_deref(),
                env,
                ctx,
            );
            let target = retarget_java_logging_facade_call(target, receiver.as_deref(), name);
            let target = retarget_receiverless_current_static_call(
                target,
                context_fallback_signature.as_ref(),
            );
            let target = retarget_receiverless_resolved_static_owner_call(
                target,
                receiver.as_deref(),
                resolved.as_ref(),
                ctx,
            );
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
                ctx.facts,
            );
            let arg_plan = apply_dyn_trait_borrowed_return_arg_shapes(
                arg_plan,
                receiver.as_deref(),
                env,
                ctx,
                fallback_signature.as_ref(),
            );
            let arg_plan = apply_static_fallback_reference_arg_shapes(
                arg_plan,
                receiver.as_deref(),
                env,
                ctx,
                fallback_signature.as_ref(),
            );
            let arg_plan = emitted_call_target_signature_arg_plan(
                &target,
                receiver.as_deref(),
                args,
                env,
                ctx,
            )
            .unwrap_or(arg_plan);
            let lowered_args = lower_args_for_call_target_with_temporaries(
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
                    context_fallback_signature.as_ref(),
                )
            });
            let mut lowered = apply_call_target_with_lower_context(
                &target,
                lowered_receiver,
                lowered_args.args,
                ctx,
            );
            lowered = wrap_call_expr_with_argument_temporaries(lowered, lowered_args.temporaries);
            if slot_expr_needs_result_propagation(expr, env, ctx) {
                lowered = propagate_result_projection(lowered);
            }
            lowered
        }
        Expr::New {
            ty,
            args,
            body,
            resolved,
            ..
        } => {
            if let Some(unsupported) = unsupported_source_constructor_call_expr(ty, args, ctx) {
                return unsupported;
            }
            if let Some(anonymous) =
                lower_anonymous_interface_new_expr(ty, args, body.as_ref(), resolved.as_ref(), ctx)
            {
                return anonymous;
            }
            if let Some(lowered) =
                lower_java_string_constructor_expr(ty, args, env, ctx, refinements)
            {
                return lowered;
            }
            if let Some(lowered) =
                lower_boxed_primitive_string_constructor_expr(ty, args, env, ctx, refinements)
            {
                return lowered;
            }
            if let Some(lowered) =
                lower_native_collection_constructor_from_map_view(ty, args, env, ctx, refinements)
            {
                return lowered;
            }
            if concrete_java_type_name(ty)
                .map(crate::stdlib::canonical_external_receiver_type_key)
                .as_deref()
                == Some("URI")
                && args.len() == 1
            {
                let lowered = function_call_projection(
                    RustExpr::Raw("URI::parse".to_string()),
                    lower_args_with_plan(
                        args,
                        env,
                        ArgumentLoweringPlan::Unshaped {
                            arg_count: args.len(),
                        },
                        ctx,
                    ),
                );
                return propagate_result_projection(lowered);
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
                    args,
                    env,
                    ctx.signatures,
                    ty,
                    &target.rust_name,
                    ctx.facts,
                )
            };
            let rust_args = lower_constructor_args_with_plan(args, env, constructor_arg_plan, ctx);
            let mut lowered = apply_new_target_with_facts(&target, rust_args, ctx.facts);
            if external_constructor_returns_rust_result(ty, args.len(), ctx.facts) {
                lowered = propagate_result_projection(lowered);
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
            if java_type_is_erased_object_name(ty) {
                return lower_expr_with_refinements(expr, env, ctx, refinements);
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

fn lower_java_string_constructor_expr(
    ty: &JavaType,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !java_type_is_string_type(ty) || args.len() != 1 {
        return None;
    }
    let arg = &args[0];
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let actual = planned_method_call_return_shape(arg, env, ctx)
        .unwrap_or_else(|| infer_expr_shape_with_context(arg, env, shape_ctx));
    let actual_ty = actual
        .optional_element_type()
        .unwrap_or_else(|| actual.java_ty.clone());
    if !matches!(
        actual_ty,
        JavaType::Array(ref element)
            if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Char))
    ) {
        return None;
    }
    let mut target = RustSlotShape::for_storage(actual_ty);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;
    let chars =
        lower_expr_for_slot_with_refinements(arg, env, Slot::Argument(target), ctx, refinements);
    Some(char_sequence_collect_string_projection(chars))
}

fn lower_boxed_primitive_string_constructor_expr(
    ty: &JavaType,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if args.len() != 1 {
        return None;
    }
    let type_name = concrete_java_type_name(ty)?;
    let primitive = crate::stdlib::boxed_primitive_type(
        crate::stdlib::canonical_external_receiver_type_key(type_name)
            .rsplit('.')
            .next()
            .unwrap_or(type_name),
    )?;
    let helper = boxed_primitive_string_parse_helper(primitive)?;
    let arg = &args[0];
    let arg_ty = crate::types::infer_expr_type(arg, env.type_env());
    if !matches!(arg_ty, JavaType::String) {
        return None;
    }
    let mut target = RustSlotShape::for_storage(JavaType::String);
    target.ownership = OwnershipShape::SharedBorrow;
    let value =
        lower_expr_for_slot_with_refinements(arg, env, Slot::Argument(target), ctx, refinements);
    Some(function_call_projection(
        RustExpr::Raw(helper.to_string()),
        vec![value],
    ))
}

fn boxed_primitive_string_parse_helper(primitive: PrimitiveType) -> Option<&'static str> {
    match primitive {
        PrimitiveType::Byte => Some("java_parse_i8"),
        PrimitiveType::Short => Some("java_parse_i16"),
        PrimitiveType::Int => Some("java_parse_i32"),
        PrimitiveType::Long => Some("java_parse_i64"),
        PrimitiveType::Float => Some("java_parse_f32"),
        PrimitiveType::Double => Some("java_parse_f64"),
        PrimitiveType::Boolean | PrimitiveType::Char => None,
    }
}

fn java_type_is_string_type(ty: &JavaType) -> bool {
    matches!(ty, JavaType::String)
        || concrete_java_type_name(ty)
            .map(crate::stdlib::canonical_external_receiver_type_key)
            .as_deref()
            == Some("String")
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
    render_typed_single_input_method_reference_callback_with_facts(
        &plan_method_reference(target, method),
        &param_type,
        element_shape,
        ctx.facts,
    )
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
        || emit::emit_type(&erased_object_rust_type()),
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
        receiver_method_call_projection_exact(lowered, "unwrap", Vec::new())
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
                    map_entry_locals: refinements.map_entry_locals.clone(),
                }
            };
            let else_refinements = if else_non_null {
                refinements.with_non_null_local(local_name)
            } else {
                ExprRefinements {
                    non_null_locals: refinements.non_null_locals.clone(),
                    exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
                    map_entry_locals: refinements.map_entry_locals.clone(),
                }
            };
            (then_refinements, else_refinements)
        }
        TernaryNullRefinementPlan::None => (
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
                exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
                map_entry_locals: refinements.map_entry_locals.clone(),
            },
            ExprRefinements {
                non_null_locals: refinements.non_null_locals.clone(),
                exact_erased_object_locals: refinements.exact_erased_object_locals.clone(),
                map_entry_locals: refinements.map_entry_locals.clone(),
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
    if op == BinaryOp::Assign
        && let Some(expr) = lower_array_assignment_value_expr(left, right, env, ctx, refinements)
    {
        return expr;
    }
    if op == BinaryOp::Assign
        && let Some(expr) =
            lower_static_storage_assignment_value_expr(left, right, env, ctx, refinements)
    {
        return expr;
    }
    if op == BinaryOp::Assign
        && let Some(expr) =
            lower_qualified_static_storage_assignment_value_expr(left, right, env, ctx, refinements)
    {
        return expr;
    }
    if op == BinaryOp::Assign
        && let Some(expr) = lower_local_assignment_value_expr(left, right, env, ctx, refinements)
    {
        return expr;
    }
    if is_null_literal(right)
        && let Some(expr) =
            lower_assignment_null_comparison_expr(op, left, false, env, ctx, refinements)
    {
        return expr;
    }
    if is_null_literal(left)
        && let Some(expr) =
            lower_assignment_null_comparison_expr(op, right, true, env, ctx, refinements)
    {
        return expr;
    }
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
    let left_ty = numeric_operand_java_type(left, env, ctx);
    let right_ty = numeric_operand_java_type(right, env, ctx);
    let numeric_promotion = plan_numeric_binary_promotion(op, &left_ty, &right_ty);
    if matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        let shape_ctx = ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        };
        let left_shape = source_java_enum_field_accessor_shape_for_expr(left, env, ctx)
            .unwrap_or_else(|| {
                crate::expr_shape::infer_expr_shape_with_context(left, env, shape_ctx)
            });
        let right_shape = source_java_enum_field_accessor_shape_for_expr(right, env, ctx)
            .unwrap_or_else(|| {
                crate::expr_shape::infer_expr_shape_with_context(right, env, shape_ctx)
            });
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
        if let Some(plan) = numeric_promotion {
            return lower_numeric_binary_expr(
                op,
                lower_numeric_binary_operand(left, env, ctx, refinements),
                lower_numeric_binary_operand(right, env, ctx, refinements),
                plan,
            );
        }
        return lower_equality_comparison_expr_for_op(
            op,
            plan_equality_comparison(&left_shape, &right_shape),
            lower_expr_with_refinements(left, env, ctx, refinements),
            lower_expr_with_refinements(right, env, ctx, refinements),
        );
    }
    if let Some(plan) = numeric_promotion {
        return lower_numeric_binary_expr(
            op,
            lower_numeric_binary_operand(left, env, ctx, refinements),
            lower_numeric_binary_operand(right, env, ctx, refinements),
            plan,
        );
    }
    if let Some(expr) = lower_map_get_primitive_numeric_binary_expr(
        op,
        left,
        right,
        &left_ty,
        &right_ty,
        env,
        ctx,
        refinements,
    ) {
        return expr;
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

fn lower_array_assignment_value_expr(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::ArrayAccess(array, index) = target else {
        return None;
    };
    let lowered_target = lower_array_assignment_target_expr(array, index, env, ctx, refinements);
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let assignment_slot = target_assignment_slot_with_current_type(target, env, ctx, shape_ctx);
    let lowered_value =
        lower_expr_for_slot_with_refinements(value, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, value, &assignment_slot, env, ctx);
    let assigned_value =
        wrap_optional_assignment_value_if_needed(value, env, assignment_slot, lowered_value, ctx);
    let assigned_readback = lower_array_access_expr(array, index, env, ctx, refinements);
    Some(RustExpr::Raw(format!(
        "{{ {} = {}; {} }}",
        emit::emit_expr(&lowered_target),
        emit::emit_expr(&assigned_value),
        emit::emit_expr(&assigned_readback),
    )))
}

fn lower_array_assignment_target_for_stmt(
    target: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::ArrayAccess(array, index) = target else {
        return None;
    };
    Some(lower_array_assignment_target_expr(
        array,
        index,
        env,
        ctx,
        refinements,
    ))
}

fn lower_array_assignment_target_expr(
    array: &Expr,
    index: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Expr::ArrayAccess(outer_array, outer_index) = array
        && ndarray_element_type_after_optional(&crate::types::infer_expr_type(
            outer_array,
            env.type_env(),
        ))
        .is_some()
    {
        return array2_index_projection(
            lower_write_sequence_receiver(outer_array, env, ctx, refinements),
            lower_expr_for_slot(outer_index, env, index_slot(), ctx),
            lower_expr_for_slot(index, env, index_slot(), ctx),
        );
    }
    indexed_expr(
        lower_write_sequence_receiver(array, env, ctx, refinements),
        lower_expr_for_slot(index, env, index_slot(), ctx),
    )
}

fn lower_write_sequence_receiver(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    let Some(inner_ty) = shape.optional_element_type() else {
        return lowered;
    };
    if crate::types::sequence_element_type(&inner_ty).is_none()
        && crate::types::two_dimensional_array_element_type(&inner_ty).is_none()
    {
        return lowered;
    }
    project_optional_sequence_receiver(lowered, OptionalSequenceReceiverProjection::Mutable)
}

fn lower_source_associated_const_field_access(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let owner = static_storage_owner_type_name(receiver, env, ctx)?;
    let owner_ty = find_context_type_by_key(ctx.program, &owner)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))?;
    let field_decl = static_field_decl_by_name(owner_ty, field)?;
    if !crate::item_assembly::static_final_field_has_associated_const(owner_ty, field_decl) {
        return None;
    }
    let rust_owner = crate::slot_render::rust_name_for_java_named_type(&owner);
    Some(owner_associated_path_projection(
        &rust_owner,
        &field_decl.name,
    ))
}

fn lower_source_nested_type_access(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let owner = source_type_name_from_expr(receiver, env, ctx)?;
    let owner_ty = find_context_type_by_key(ctx.program, &owner)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))?;
    let nested = nested_type_decl_by_receiver_alias(owner_ty, field)?;
    Some(RustExpr::Var(
        crate::slot_render::rust_name_for_java_named_type(&nested.name),
    ))
}

fn lower_java_array_length_access(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if source_java_enum_values_call_owner(receiver, env, ctx)
        .is_some_and(|owner| matches_source_java_enum_type(&owner, ctx))
    {
        return Some(len_i32_projection(lower_expr_with_refinements(
            receiver,
            env,
            ctx,
            refinements,
        )));
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if !matches!(shape.java_ty, JavaType::Array(_)) {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    if let Some(plan) = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        "length",
        env.type_env(),
        ctx.facts,
    ) {
        return match plan.op {
            crate::member_access_plan::MemberAccessOp::Array2RowsAsI32
                if !two_dimensional_array_expr_uses_nested_slice(receiver, env, ctx) =>
            {
                Some(array2_rows_i32_projection(lowered_receiver))
            }
            crate::member_access_plan::MemberAccessOp::Array2ColsAsI32
            | crate::member_access_plan::MemberAccessOp::LenAsI32
            | crate::member_access_plan::MemberAccessOp::Array2RowsAsI32 => {
                Some(len_i32_projection(lowered_receiver))
            }
            _ => None,
        };
    }
    Some(len_i32_projection(lowered_receiver))
}

fn lower_source_java_enum_type_api_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let receiver = receiver?;
    let owner = source_type_name_from_expr(receiver, env, ctx)?;
    if !matches_source_java_enum_type(&owner, ctx) {
        return None;
    }
    let rust_owner = crate::slot_render::rust_name_for_java_named_type(&owner);
    match (name, args) {
        ("values", []) => Some(RustExpr::Raw(format!("{rust_owner}::values()"))),
        ("valueOf" | "value_of", [arg]) => {
            let arg = lower_expr_for_slot_with_refinements(
                arg,
                env,
                argument_slot(borrowed_string_content_shape()),
                ctx,
                refinements,
            );
            Some(RustExpr::Raw(format!(
                "{rust_owner}::value_of({})",
                emit::emit_expr(&arg)
            )))
        }
        _ => None,
    }
}

fn lower_arrays_as_list_single_object_array_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(receiver_name) = receiver? else {
        return None;
    };
    let receiver_key = env
        .type_env()
        .static_type_alias(receiver_name)
        .map(crate::stdlib::canonical_external_receiver_type_key)
        .unwrap_or_else(|| crate::stdlib::canonical_external_receiver_type_key(receiver_name));
    if !matches!(
        receiver_key.as_str(),
        "Arrays" | "java.util.Arrays" | "JavaUtilArrays"
    ) || !matches!(name, "asList" | "as_list")
    {
        return None;
    }
    let [arg] = args else {
        return None;
    };
    if source_java_enum_values_call_owner(arg, env, ctx)
        .is_some_and(|owner| matches_source_java_enum_type(&owner, ctx))
    {
        return Some(lower_expr(arg, env, ctx));
    }
    let java_ty = crate::types::infer_expr_type(arg, env.type_env());
    if matches!(java_ty, JavaType::Array(element) if !matches!(element.as_ref(), JavaType::Primitive(_)))
    {
        return Some(lower_expr(arg, env, ctx));
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
    match shape.java_ty {
        JavaType::Array(element) if !matches!(element.as_ref(), JavaType::Primitive(_)) => {
            Some(lower_expr(arg, env, ctx))
        }
        _ => None,
    }
}

fn source_java_enum_values_call_owner(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<String> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name != "values" || !args.is_empty() {
        return None;
    }
    source_type_name_from_expr(receiver, env, ctx)
}

fn lower_static_storage_field_access(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let owner = static_storage_owner_type_name(receiver, env, ctx)?;
    let field_name = env.type_static_storage_field_access_name(&owner, field)?;
    let rust_owner = crate::slot_render::rust_name_for_java_named_type(&owner);
    Some(qualified_static_storage_accessor_projection(
        &rust_owner,
        field_name,
    ))
}

fn lower_local_assignment_value_expr(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Var(_) = target else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let assignment_slot = target_assignment_slot_with_current_type(target, env, ctx, shape_ctx);
    let lowered_value =
        lower_expr_for_slot_with_refinements(value, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, value, &assignment_slot, env, ctx);
    let assigned_value =
        wrap_optional_assignment_value_if_needed(value, env, assignment_slot, lowered_value, ctx);
    let lowered_target = lower_expr(target, env, ctx);
    let target = emit::emit_expr(&lowered_target);
    Some(RustExpr::Raw(format!(
        "{{ {target} = {}; {target} }}",
        emit::emit_expr(&assigned_value)
    )))
}

fn lower_assignment_null_comparison_expr(
    op: BinaryOp,
    expr: &Expr,
    null_on_left: bool,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        return None;
    }
    let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        right,
    } = expr
    else {
        return None;
    };
    let Expr::Var(_) = left.as_ref() else {
        return None;
    };
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let assignment_slot = target_assignment_slot_with_current_type(left, env, ctx, shape_ctx);
    if slot_target_shape(&assignment_slot)
        .and_then(RustSlotShape::optional_element_type)
        .is_none()
    {
        return None;
    }
    let lowered_value =
        lower_expr_for_slot_with_refinements(right, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, right, &assignment_slot, env, ctx);
    let assigned_value =
        wrap_optional_assignment_value_if_needed(right, env, assignment_slot, lowered_value, ctx);
    let lowered_target = lower_expr(left, env, ctx);
    let target = emit::emit_expr(&lowered_target);
    let method = crate::operator_lowering::null_comparison_method(op, null_on_left)?;
    let predicate = match method {
        crate::operator_lowering::NullComparisonMethod::IsSome => "is_some",
        crate::operator_lowering::NullComparisonMethod::IsNone => "is_none",
    };
    Some(RustExpr::Raw(format!(
        "{{ {target} = {}; {target}.{predicate}() }}",
        emit::emit_expr(&assigned_value)
    )))
}

fn lower_static_storage_assignment_stmt(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustStmt> {
    let Expr::Var(name) = target else {
        return None;
    };
    let static_name = env.static_storage_field_access_name(name)?;
    let assignment_slot = static_storage_field_shape(name, env, ctx).map(Slot::Assignment)?;
    let lowered_value =
        lower_expr_for_slot_with_refinements(value, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, value, &assignment_slot, env, ctx);
    let assigned_value =
        wrap_optional_assignment_value_if_needed(value, env, assignment_slot, lowered_value, ctx);
    Some(expr_stmt(function_call_projection(
        static_storage_setter_path_projection(static_name),
        vec![assigned_value],
    )))
}

fn lower_static_storage_assignment_value_expr(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Var(name) = target else {
        return None;
    };
    let static_name = env.static_storage_field_access_name(name)?;
    let assignment_slot = static_storage_field_shape(name, env, ctx).map(Slot::Assignment)?;
    let lowered_value =
        lower_expr_for_slot_with_refinements(value, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, value, &assignment_slot, env, ctx);
    let assigned_value =
        wrap_optional_assignment_value_if_needed(value, env, assignment_slot, lowered_value, ctx);
    let getter = static_storage_accessor_projection(static_name);
    Some(RustExpr::Raw(format!(
        "{{ {}({}); {} }}",
        emit::emit_expr(&static_storage_setter_path_projection(static_name)),
        emit::emit_expr(&assigned_value),
        emit::emit_expr(&getter)
    )))
}

fn lower_qualified_static_storage_assignment_stmt(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustStmt> {
    let plan = qualified_static_storage_assignment_plan(target, env, ctx)?;
    let assigned_value = lower_static_storage_assigned_value(
        value,
        env,
        ctx,
        refinements,
        Slot::Assignment(plan.shape),
    );
    Some(expr_stmt(function_call_projection(
        qualified_static_storage_setter_path_projection(&plan.rust_owner, &plan.field_name),
        vec![assigned_value],
    )))
}

fn lower_qualified_static_storage_assignment_value_expr(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let plan = qualified_static_storage_assignment_plan(target, env, ctx)?;
    let assigned_value = lower_static_storage_assigned_value(
        value,
        env,
        ctx,
        refinements,
        Slot::Assignment(plan.shape),
    );
    let setter =
        qualified_static_storage_setter_path_projection(&plan.rust_owner, &plan.field_name);
    let getter = qualified_static_storage_accessor_projection(&plan.rust_owner, &plan.field_name);
    Some(RustExpr::Raw(format!(
        "{{ {}({}); {} }}",
        emit::emit_expr(&setter),
        emit::emit_expr(&assigned_value),
        emit::emit_expr(&getter)
    )))
}

struct QualifiedStaticStorageAssignmentPlan {
    rust_owner: String,
    field_name: String,
    shape: RustSlotShape,
}

fn qualified_static_storage_assignment_plan(
    target: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<QualifiedStaticStorageAssignmentPlan> {
    let Expr::FieldAccess(receiver, field) = target else {
        return None;
    };
    let owner = static_storage_owner_type_name(receiver, env, ctx)?;
    let field_name = env
        .type_static_storage_field_access_name(&owner, field)
        .map(str::to_string)?;
    let shape = ctx
        .signatures
        .field_shape(&owner, &field_name)
        .or_else(|| ctx.signatures.field_shape(&owner, field))
        .cloned()
        .or_else(|| env.field_shape(&field_name).cloned())?;
    let rust_owner = if ctx.current_type.is_some_and(|current| current == owner) {
        "Self".to_string()
    } else {
        crate::slot_render::rust_name_for_java_named_type(&owner)
    };
    Some(QualifiedStaticStorageAssignmentPlan {
        rust_owner,
        field_name,
        shape,
    })
}

fn lower_static_storage_assigned_value(
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
    assignment_slot: Slot,
) -> RustExpr {
    let lowered_value =
        lower_expr_for_slot_with_refinements(value, env, assignment_slot.clone(), ctx, refinements);
    let lowered_value =
        adapt_reused_owned_value_for_slot(lowered_value, value, &assignment_slot, env, ctx);
    wrap_optional_assignment_value_if_needed(value, env, assignment_slot, lowered_value, ctx)
}

fn static_storage_owner_type_name(
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<String> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() {
        return None;
    }
    if env
        .static_storage_field_access_name(receiver_name)
        .is_some()
        && let Some(shape) = static_storage_field_shape(receiver_name, env, ctx)
        && let Some(name) = concrete_java_type_name(&shape.java_ty)
    {
        return Some(name.to_string());
    }
    if env.field_type(receiver_name).is_some() {
        return None;
    }
    env.type_env()
        .static_type_alias(receiver_name)
        .map(str::to_string)
        .or_else(|| {
            find_context_type_by_key(ctx.program, receiver_name)
                .or_else(|| find_context_type_by_key(ctx.support_local_type_program, receiver_name))
                .or_else(|| {
                    find_context_type_by_key(ctx.source_summary_type_program, receiver_name)
                })
                .map(|ty| ty.name.clone())
        })
}

fn source_type_name_from_expr(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<String> {
    match expr {
        Expr::Var(_) => static_storage_owner_type_name(expr, env, ctx),
        Expr::FieldAccess(receiver, field) => {
            let owner = source_type_name_from_expr(receiver, env, ctx)?;
            let owner_ty = find_context_type_by_key(ctx.program, &owner)
                .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
                .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))?;
            nested_type_decl_by_receiver_alias(owner_ty, field).map(|nested| nested.name.clone())
        }
        _ => None,
    }
}

fn nested_type_decl_by_receiver_alias<'a>(
    owner_ty: &'a TypeDecl,
    receiver_name: &str,
) -> Option<&'a TypeDecl> {
    owner_ty.members.iter().find_map(|member| {
        let Member::Type(nested) = member else {
            return None;
        };
        type_simple_name_matches_receiver_alias(&nested.name, receiver_name).then_some(nested)
    })
}

fn matches_source_java_enum_type(type_name: &str, ctx: LowerCtx) -> bool {
    find_context_type_by_key(ctx.program, type_name)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, type_name))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, type_name))
        .is_some_and(|ty| ty.kind == TypeKind::Enum)
}

fn static_storage_field_shape(name: &str, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustSlotShape> {
    let static_name = env.static_storage_field_access_name(name).unwrap_or(name);
    ctx.current_type
        .and_then(|type_name| {
            ctx.signatures
                .field_shape(type_name, static_name)
                .or_else(|| ctx.signatures.field_shape(type_name, name))
                .cloned()
        })
        .or_else(|| env.field_shape(name).cloned())
}

fn lower_static_primitive_constant_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let actual = match expr {
        Expr::Var(name)
            if env.local_type(name).is_none() && env.static_field_access_name(name).is_some() =>
        {
            let static_name = env.static_field_access_name(name).unwrap_or(name);
            ctx.current_type
                .and_then(|type_name| {
                    ctx.signatures
                        .field_shape(type_name, static_name)
                        .or_else(|| ctx.signatures.field_shape(type_name, name))
                        .cloned()
                })
                .or_else(|| env.field_shape(static_name).cloned())
                .or_else(|| env.field_shape(name).cloned())
                .or_else(|| {
                    current_type_field_shape(ctx, static_name)
                        .or_else(|| current_type_field_shape(ctx, name))
                })
                .unwrap_or_else(|| {
                    RustSlotShape::conservative(crate::types::infer_expr_type(expr, env.type_env()))
                })
        }
        Expr::FieldAccess(receiver, field) => {
            let plan = crate::member_access_plan::plan_member_access_with_facts(
                receiver,
                field,
                env.type_env(),
                ctx.facts,
            )?;
            if !matches!(
                plan.op,
                crate::member_access_plan::MemberAccessOp::StaticConstant { .. }
            ) {
                return None;
            }
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
        }
        _ => return None,
    };
    if !matches!(actual.java_ty, JavaType::Primitive(_)) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(apply_expr_to_slot_via_plan(
        lowered, actual, slot, ctx.facts,
    ))
}

fn current_type_field_shape(ctx: LowerCtx, name: &str) -> Option<RustSlotShape> {
    let current_type = ctx.current_type?;
    [
        ctx.program,
        ctx.support_local_type_program,
        ctx.source_summary_type_program,
    ]
    .into_iter()
    .find_map(|program| find_context_type_by_key(program, current_type))
    .and_then(|ty| {
        ty.members.iter().find_map(|member| match member {
            Member::Field(field)
                if field.name == name || crate::parser::to_snake_case(&field.name) == name =>
            {
                Some(RustSlotShape::for_storage(field.ty.clone()))
            }
            _ => None,
        })
    })
}

fn lower_unary_expr_with_refinements(
    op: UnaryOp,
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if op == UnaryOp::Not
        && let Some(negated_equality) =
            lower_negated_java_equality_call_expr(expr, env, ctx, refinements)
    {
        return negated_equality;
    }
    if let Some(update) = lower_local_update_value_expr(op, expr, env, ctx) {
        return update;
    }
    lower_unary_expr(op, lower_expr_with_refinements(expr, env, ctx, refinements))
}

fn lower_local_update_value_expr(
    op: UnaryOp,
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if !matches!(
        op,
        UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec
    ) {
        return None;
    }
    let Expr::Var(_) = expr else {
        return None;
    };
    let target_expr = lower_expr(expr, env, ctx);
    let update_op = match op {
        UnaryOp::PreInc | UnaryOp::PostInc => "+=",
        UnaryOp::PreDec | UnaryOp::PostDec => "-=",
        _ => return None,
    };
    Some(match op {
        UnaryOp::PreInc | UnaryOp::PreDec => {
            java_update_expression_projection(target_expr, update_op, true)
        }
        UnaryOp::PostInc | UnaryOp::PostDec => {
            java_update_expression_projection(target_expr, update_op, false)
        }
        _ => return None,
    })
}

fn numeric_operand_java_type(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> JavaType {
    let inferred = crate::types::infer_expr_type(expr, env.type_env());
    if matches!(
        inferred,
        JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
    ) {
        return inferred;
    }
    if let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
        && let Some(lowered) = external_call_lowered_type_with_external_index(
            receiver.as_deref(),
            name,
            args.len(),
            ctx.facts,
            ctx.external_index,
            env.type_env(),
            ctx.current_type,
        )
        && let Some(java_ty) = lowered.java_ty()
        && matches!(
            java_ty,
            JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(*primitive)
        )
    {
        return java_ty.clone();
    }
    if let Some(java_ty) = numeric_receiver_method_signature_return_type(expr, env, ctx) {
        return java_ty;
    }
    if let Some(java_ty) = numeric_member_access_java_type(expr, env, ctx) {
        return java_ty;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shaped = actual_shape_for_slot_expr(expr, env, shape_ctx).java_ty;
    if matches!(
        shaped,
        JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
    ) {
        shaped
    } else if let Some(element_ty) = numeric_array_access_element_java_type(expr, env)
        && matches!(
            element_ty,
            JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
        )
    {
        element_ty
    } else {
        inferred
    }
}

fn numeric_receiver_method_signature_return_type(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    let (_, signature) =
        fallback_receiver_method_signature(receiver.as_deref(), name, args, env, ctx)?;
    let java_ty = signature.return_shape.as_ref()?.java_ty.clone();
    if matches!(
        java_ty,
        JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
    ) {
        Some(java_ty)
    } else {
        None
    }
}

fn numeric_member_access_java_type(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<JavaType> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    if let Some(java_ty) = numeric_static_field_java_type(receiver, field, env, ctx) {
        return Some(java_ty);
    }
    let plan = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        env.type_env(),
        ctx.facts,
    )?;
    if matches!(
        plan.java_ty,
        JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
    ) {
        Some(plan.java_ty)
    } else {
        None
    }
}

fn numeric_static_field_java_type(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    let owner = env
        .type_env()
        .static_type_alias(receiver_name)
        .unwrap_or(receiver_name);
    let java_ty = ctx
        .facts
        .external_field_for(owner, field)
        .and_then(|fact| fact.field_type.clone())
        .or_else(|| {
            crate::external_packs::external_field_for(owner, field).and_then(|fact| fact.field_type)
        })
        .or_else(|| {
            let receiver_ty = JavaType::Named {
                name: owner.to_string(),
                args: Vec::new(),
            };
            crate::stdlib::known_external_field_type_for_receiver(&receiver_ty, field)
        })?;
    if matches!(
        java_ty,
        JavaType::Primitive(primitive) if crate::types::is_numeric_primitive_type(primitive)
    ) {
        Some(java_ty)
    } else {
        None
    }
}

fn numeric_array_access_element_java_type(expr: &Expr, env: &ShapeEnv) -> Option<JavaType> {
    let Expr::ArrayAccess(array, _) = expr else {
        return None;
    };
    match array.as_ref() {
        Expr::Cast { ty, .. }
            if matches!(
                ty,
                JavaType::Primitive(primitive)
                    if crate::types::is_numeric_primitive_type(*primitive)
            ) =>
        {
            Some(ty.clone())
        }
        Expr::Var(name) => env.local_shape(name).and_then(|shape| {
            shape.sequence_element_type().or_else(|| {
                shape
                    .optional_element_type()
                    .and_then(|element_ty| crate::types::sequence_element_type(&element_ty))
            })
        }),
        _ => None,
    }
}

fn lower_numeric_binary_operand(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Some(cast) = lower_explicit_primitive_numeric_cast_operand(expr, env, ctx, refinements) {
        return cast;
    }
    lower_map_get_primitive_arithmetic_operand_info(expr, env, ctx, refinements)
        .map(|operand| operand.lowered)
        .unwrap_or_else(|| lower_expr_with_refinements(expr, env, ctx, refinements))
}

fn lower_explicit_primitive_numeric_cast_operand(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    let JavaType::Primitive(target) = ty else {
        return None;
    };
    if !crate::types::is_numeric_primitive_type(*target) {
        return None;
    }
    let lowered = lower_expr_with_refinements(inner, env, ctx, refinements);
    if *target == PrimitiveType::Char {
        return Some(RustExpr::Raw(format!(
            "char::from_u32((({}) as u32) & 0xffff).unwrap_or('\\0')",
            emit::emit_expr(&lowered)
        )));
    }
    Some(RustExpr::Cast {
        expr: Box::new(lowered),
        ty: crate::coercion::rust_primitive_type_name_policy(*target)
            .rust_name
            .to_string(),
    })
}

fn lower_map_get_primitive_numeric_binary_expr(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    left_ty: &JavaType,
    right_ty: &JavaType,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let left_operand = lower_map_get_primitive_arithmetic_operand_info(left, env, ctx, refinements);
    let right_operand =
        lower_map_get_primitive_arithmetic_operand_info(right, env, ctx, refinements);
    if left_operand.is_none() && right_operand.is_none() {
        return None;
    }
    let effective_left_ty = left_operand
        .as_ref()
        .map(|operand| operand.value_ty.clone())
        .unwrap_or_else(|| left_ty.clone());
    let effective_right_ty = right_operand
        .as_ref()
        .map(|operand| operand.value_ty.clone())
        .unwrap_or_else(|| right_ty.clone());
    let plan = plan_numeric_binary_promotion(op, &effective_left_ty, &effective_right_ty)?;
    Some(lower_numeric_binary_expr(
        op,
        left_operand
            .map(|operand| operand.lowered)
            .unwrap_or_else(|| lower_expr_with_refinements(left, env, ctx, refinements)),
        right_operand
            .map(|operand| operand.lowered)
            .unwrap_or_else(|| lower_expr_with_refinements(right, env, ctx, refinements)),
        plan,
    ))
}

struct MapGetPrimitiveArithmeticOperand {
    lowered: RustExpr,
    value_ty: JavaType,
}

fn lower_map_get_primitive_arithmetic_operand_info(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<MapGetPrimitiveArithmeticOperand> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if name != "get" || args.len() != 1 {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let receiver_ty = types::infer_expr_type(receiver, env.type_env());
    let (_, value_ty, _) = map_type_args_from_java_type(&receiver_ty)?;
    let value_ty = map_get_arithmetic_primitive_value_type(value_ty)?;
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
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    });
    let lowered_expr =
        apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx);
    Some(MapGetPrimitiveArithmeticOperand {
        lowered: optional_copied_expect_projection(lowered_expr),
        value_ty,
    })
}

fn map_get_arithmetic_primitive_value_type(value_ty: JavaType) -> Option<JavaType> {
    match value_ty {
        JavaType::Primitive(_) => Some(value_ty),
        JavaType::Named { name, args } if args.is_empty() => {
            crate::stdlib::boxed_primitive_type(&name).map(JavaType::Primitive)
        }
        _ => None,
    }
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
        BinaryOp::Ne => logical_not_projection(RustExpr::Raw(object_type_predicate)),
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
        BinaryOp::Ne => logical_not_projection(RustExpr::Raw(identity_expr)),
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
    let shape = static_storage_value_shape(value, env, ctx)
        .or_else(|| current_instance_field_shape_with_env(value, env, ctx))
        .unwrap_or(shape);
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

fn static_storage_value_shape(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<RustSlotShape> {
    let Expr::Var(name) = expr else {
        return None;
    };
    if env.static_storage_field_access_name(name).is_none() {
        return None;
    }
    static_storage_field_shape(name, env, ctx)
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
    let Some((_, signature)) = fallback_signature else {
        return plan;
    };
    match plan {
        ArgumentLoweringPlan::Unshaped { arg_count } if signature.params.len() == arg_count => {
            ArgumentLoweringPlan::Shaped(
                signature
                    .params
                    .iter()
                    .map(|param| param.shape.clone())
                    .collect(),
            )
        }
        ArgumentLoweringPlan::Shaped(shapes) if signature.params.len() == shapes.len() => {
            ArgumentLoweringPlan::Shaped(
                shapes
                    .into_iter()
                    .zip(signature.params.iter())
                    .map(|(shape, param)| {
                        refine_shaped_arg_with_fallback_owned_signature(shape, &param.shape)
                    })
                    .collect(),
            )
        }
        plan => plan,
    }
}

fn refine_shaped_arg_with_fallback_owned_signature(
    mut shape: RustSlotShape,
    fallback: &RustSlotShape,
) -> RustSlotShape {
    if matches!(
        shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && matches!(fallback.ownership, OwnershipShape::Owned)
        && java_types_have_same_erased_identity(&shape.java_ty, &fallback.java_ty)
    {
        shape.ownership = OwnershipShape::Owned;
        shape.view = fallback.view.clone();
        shape.dispatch = fallback.dispatch.clone();
    }
    shape
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
    receiver: ReceiverPlan,
    arg_shapes_are_authoritative: bool,
}

fn context_fallback_signature_for_method(
    owner: &str,
    method: &MethodDecl,
    ctx: LowerCtx,
) -> ContextFallbackMethodSignature {
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    if let Some(signature) = ctx
        .signatures
        .method_signature_by_rust_name(owner, &rust_name)
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(owner, &rust_name, method.params.len())
                .map(|(_, signature)| signature)
        })
    {
        return ContextFallbackMethodSignature {
            rust_name,
            arg_shapes: signature
                .params
                .iter()
                .map(|param| param.shape.clone())
                .collect(),
            receiver: signature.receiver,
            arg_shapes_are_authoritative: true,
        };
    }
    ContextFallbackMethodSignature {
        rust_name,
        arg_shapes: source_context_param_shapes(method, ctx),
        receiver: ReceiverPlan::Shared,
        arg_shapes_are_authoritative: false,
    }
}

fn context_fallback_signature_for_signature(
    rust_name: String,
    signature: &crate::signature::MethodSignaturePlan,
) -> ContextFallbackMethodSignature {
    ContextFallbackMethodSignature {
        rust_name,
        arg_shapes: signature
            .params
            .iter()
            .map(|param| param.shape.clone())
            .collect(),
        receiver: signature.receiver,
        arg_shapes_are_authoritative: false,
    }
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
        source_context_borrowed_param_shape(shape, &param.ty)
    } else {
        shape
    }
}

fn source_context_reference_param_should_borrow(java_ty: &JavaType, is_varargs: bool) -> bool {
    !is_varargs
        && matches!(
            java_ty,
            JavaType::String
                | JavaType::Named { .. }
                | JavaType::Unknown(_)
                | JavaType::Wildcard(_)
        )
}

fn source_context_borrowed_param_shape(
    mut shape: RustSlotShape,
    java_ty: &JavaType,
) -> RustSlotShape {
    shape = apply_signature_constraints(
        shape,
        &[SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        )],
    );
    if matches!(java_ty, JavaType::String) {
        shape = apply_signature_constraints(
            shape,
            &[SignatureConstraint::RequiredView(ViewShape::Str)],
        );
    }
    shape
}

fn apply_context_fallback_signature_arg_shapes(
    plan: ArgumentLoweringPlan,
    fallback_signature: Option<&ContextFallbackMethodSignature>,
    facts: &FactSet,
) -> ArgumentLoweringPlan {
    let Some(signature) = fallback_signature else {
        return plan;
    };
    match plan {
        ArgumentLoweringPlan::Unshaped { arg_count } if signature.arg_shapes.len() == arg_count => {
            ArgumentLoweringPlan::Shaped(signature.arg_shapes.clone())
        }
        ArgumentLoweringPlan::Shaped(shapes) if signature.arg_shapes.len() == shapes.len() => {
            if signature.arg_shapes_are_authoritative {
                return ArgumentLoweringPlan::Shaped(signature.arg_shapes.clone());
            }
            ArgumentLoweringPlan::Shaped(
                shapes
                    .into_iter()
                    .zip(signature.arg_shapes.iter())
                    .map(|(shape, fallback)| {
                        refine_shaped_arg_with_context_fallback_borrow(shape, fallback, facts)
                    })
                    .collect(),
            )
        }
        plan => plan,
    }
}

fn refine_shaped_arg_with_context_fallback_borrow(
    mut shape: RustSlotShape,
    fallback: &RustSlotShape,
    facts: &FactSet,
) -> RustSlotShape {
    if matches!(shape.ownership, OwnershipShape::Owned)
        && matches!(
            fallback.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && java_types_have_same_erased_identity(&shape.java_ty, &fallback.java_ty)
        && borrowed_content_type_policy(&shape.java_ty, facts)
            .policy
            .is_some()
    {
        shape.ownership = fallback.ownership;
        shape.view = fallback.view.clone();
        shape.dispatch = fallback.dispatch.clone();
    }
    shape
}

fn apply_dyn_trait_borrowed_return_arg_shapes(
    plan: ArgumentLoweringPlan,
    receiver: Option<&Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    fallback_signature: Option<&(String, &crate::signature::MethodSignaturePlan)>,
) -> ArgumentLoweringPlan {
    let Some(receiver) = receiver else {
        return plan;
    };
    if !receiver_expr_uses_dyn_trait_slots(receiver, env, ctx) {
        return plan;
    }
    let Some((_, signature)) = fallback_signature else {
        return plan;
    };
    let Some(return_shape) = signature.return_shape.as_ref() else {
        return plan;
    };
    if return_shape.ownership != OwnershipShape::SharedBorrow {
        return plan;
    }
    let mut shapes = match plan {
        ArgumentLoweringPlan::Shaped(shapes) => shapes,
        other => return other,
    };
    if shapes.len() != signature.params.len() {
        return ArgumentLoweringPlan::Shaped(shapes);
    }
    for (shape, param) in shapes.iter_mut().zip(signature.params.iter()) {
        if source_context_reference_param_should_borrow(&param.java_ty, param.is_varargs)
            && !java_types_have_same_erased_identity(&param.java_ty, &return_shape.java_ty)
        {
            shape.ownership = OwnershipShape::SharedBorrow;
        }
    }
    ArgumentLoweringPlan::Shaped(shapes)
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
    let ArgumentLoweringPlan::Unshaped { arg_count } = plan else {
        return plan;
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
        ArgumentLoweringPlan::Shaped(shapes) => {
            let actual_shapes: Vec<RustSlotShape> = args
                .iter()
                .map(|arg| {
                    infer_expr_shape_with_context(
                        arg,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    )
                })
                .collect();
            args.iter()
                .zip(shapes.iter())
                .enumerate()
                .map(|(index, (arg, shape))| {
                    let actual_shape = &actual_shapes[index];
                    let lowered = lower_non_optional_argument_for_optional_target(
                        arg,
                        env,
                        shape,
                        actual_shape,
                        ctx,
                    )
                    .or_else(|| {
                        lower_optional_argument_for_borrowed_target(
                            arg,
                            env,
                            shape,
                            actual_shape,
                            ctx,
                        )
                    })
                    .unwrap_or_else(|| {
                        lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx)
                    });
                    let lowered =
                        adapt_owned_native_borrow_argument(lowered, arg, shape, actual_shape);
                    let lowered = snapshot_same_call_shared_read_before_mut_borrow(
                        lowered, arg, shape, index, args, &shapes,
                    );
                    adapt_reused_owned_argument(lowered, arg, &shape, Some(actual_shape), env, ctx)
                })
                .collect()
        }
        ArgumentLoweringPlan::Unshaped { .. } => args
            .iter()
            .map(|arg| lower_unshaped_argument(arg, env, ctx))
            .collect(),
        ArgumentLoweringPlan::Slotted(slots) => args
            .iter()
            .zip(slots)
            .map(|(arg, slot)| lower_argument_for_explicit_slot(arg, env, slot, ctx))
            .collect(),
    }
}

fn adapt_owned_native_borrow_argument(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
) -> RustExpr {
    if !matches!(
        target_shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) || matches!(
        actual_shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) || target_shape.optional_element_type().is_some()
        || !(target_shape.view == ViewShape::Native
            || borrowed_two_dimensional_array_target_accepts_native_matrix(
                target_shape,
                actual_shape,
            ))
    {
        return lowered;
    }
    if target_shape.ownership == OwnershipShape::MutableBorrow && !matches!(source, Expr::Var(_)) {
        return lowered;
    }
    if rust_expr_is_constructed_optional_value(&lowered) {
        return lowered;
    }
    match target_shape.ownership {
        OwnershipShape::SharedBorrow => ensure_borrow_projection(
            lowered,
            Some(actual_shape),
            Some(target_shape),
            BorrowProjectionUse::Argument,
        ),
        OwnershipShape::MutableBorrow => ensure_borrow_projection(
            lowered,
            Some(actual_shape),
            Some(target_shape),
            BorrowProjectionUse::MutableSliceView,
        ),
        _ => lowered,
    }
}

fn borrowed_two_dimensional_array_target_accepts_native_matrix(
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
) -> bool {
    target_shape.view == ViewShape::Slice
        && actual_shape.view == ViewShape::Native
        && java_type_is_two_dimensional_array(&target_shape.java_ty)
        && same_concrete_java_type(&target_shape.java_ty, &actual_shape.java_ty)
}

fn java_type_is_two_dimensional_array(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Array(element) if matches!(element.as_ref(), JavaType::Array(_)))
}

#[derive(Debug, Clone)]
struct LoweredCallArgs {
    args: Vec<RustExpr>,
    temporaries: Vec<(String, RustExpr)>,
}

fn lower_args_for_call_target_with_temporaries(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
) -> LoweredCallArgs {
    let rust_args = lower_args_for_call_target(
        target,
        call_receiver,
        args,
        env,
        plan.clone(),
        resolved,
        ctx,
        fallback_signature,
    );
    let shapes = match plan {
        ArgumentLoweringPlan::Shaped(shapes) => shapes,
        ArgumentLoweringPlan::Slotted(slots) => slots
            .into_iter()
            .map(|slot| match slot {
                Slot::Argument(shape) => Some(shape),
                Slot::Index => Some(RustSlotShape::for_storage(JavaType::Primitive(
                    PrimitiveType::Int,
                ))),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default(),
        ArgumentLoweringPlan::Unshaped { .. } => Vec::new(),
    };
    if shapes.len() != args.len() {
        return LoweredCallArgs {
            args: rust_args,
            temporaries: Vec::new(),
        };
    }
    introduce_same_call_later_read_temporaries(args, shapes.as_slice(), rust_args)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SameCallArgumentAliasPlan {
    Keep,
    SnapshotSharedRead { local_name: String },
    TemporaryForLaterRead { temp_name: String },
}

fn introduce_same_call_later_read_temporaries(
    args: &[Expr],
    shapes: &[RustSlotShape],
    mut rust_args: Vec<RustExpr>,
) -> LoweredCallArgs {
    let mut temporaries = Vec::new();
    for index in 0..args.len() {
        if let SameCallArgumentAliasPlan::TemporaryForLaterRead { temp_name } =
            plan_same_call_later_read_alias(index, args, shapes, &rust_args)
            && let Some(lowered) = rust_args.get_mut(index)
        {
            let value = lowered.clone();
            *lowered = RustExpr::Var(temp_name.clone());
            temporaries.push((temp_name, value));
        }
    }
    LoweredCallArgs {
        args: rust_args,
        temporaries,
    }
}

fn plan_same_call_later_read_alias(
    index: usize,
    args: &[Expr],
    shapes: &[RustSlotShape],
    rust_args: &[RustExpr],
) -> SameCallArgumentAliasPlan {
    if shapes
        .get(index)
        .is_some_and(|shape| shape.ownership == OwnershipShape::MutableBorrow)
    {
        return SameCallArgumentAliasPlan::Keep;
    }
    if later_arg_reads_prior_mutable_alias(index, args, shapes, rust_args) {
        SameCallArgumentAliasPlan::TemporaryForLaterRead {
            temp_name: format!("__java_arg{index}"),
        }
    } else {
        SameCallArgumentAliasPlan::Keep
    }
}

fn later_arg_reads_prior_mutable_alias(
    index: usize,
    args: &[Expr],
    shapes: &[RustSlotShape],
    rust_args: &[RustExpr],
) -> bool {
    let Some(source) = args.get(index) else {
        return false;
    };
    args.iter()
        .take(index)
        .enumerate()
        .any(|(prior_index, prior_arg)| {
            let Some(base) = mutable_alias_base_name(prior_arg) else {
                return false;
            };
            let shape_is_mutable = shapes
                .get(prior_index)
                .is_some_and(|shape| shape.ownership == OwnershipShape::MutableBorrow);
            let rendered_is_mutable = rust_args
                .get(prior_index)
                .is_some_and(|arg| rendered_expr_is_mutable_alias(arg, &base));
            (shape_is_mutable || rendered_is_mutable) && expr_reads_local_name(source, &base)
        })
}

fn mutable_alias_base_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            mutable_alias_base_name(receiver)
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => mutable_alias_base_name(expr),
        _ => None,
    }
}

fn expr_reads_local_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(candidate) => candidate == name,
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr_reads_local_name(receiver, name)
        }
        Expr::ArrayAccess(array, index) => {
            expr_reads_local_name(array, name) || expr_reads_local_name(index, name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_reads_local_name(expr, name))
                || initializer
                    .iter()
                    .flatten()
                    .any(|expr| expr_reads_local_name(expr, name))
        }
        Expr::ArrayInitializer(items) => items.iter().any(|expr| expr_reads_local_name(expr, name)),
        Expr::Binary { left, right, .. } => {
            expr_reads_local_name(left, name) || expr_reads_local_name(right, name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_reads_local_name(condition, name)
                || expr_reads_local_name(then_expr, name)
                || expr_reads_local_name(else_expr, name)
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_deref()
                .is_some_and(|receiver| expr_reads_local_name(receiver, name))
                || args.iter().any(|arg| expr_reads_local_name(arg, name))
        }
        Expr::New { args, .. } => args.iter().any(|arg| expr_reads_local_name(arg, name)),
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_reads_local_name(expr, name)
        }
        Expr::Lambda { .. } | Expr::MethodReference { .. } | Expr::ClassLiteral(_) => false,
        Expr::Literal(_) | Expr::This | Expr::Super | Expr::Raw(_) => false,
    }
}

fn wrap_call_expr_with_argument_temporaries(
    call: RustExpr,
    temporaries: Vec<(String, RustExpr)>,
) -> RustExpr {
    if temporaries.is_empty() {
        return call;
    }
    let mut block = String::from("{ ");
    for (name, value) in temporaries {
        block.push_str("let ");
        block.push_str(&name);
        block.push_str(" = ");
        block.push_str(&emit::emit_expr(&value));
        block.push_str("; ");
    }
    block.push_str(&emit::emit_expr(&call));
    block.push_str(" }");
    RustExpr::Raw(block)
}

fn snapshot_same_call_shared_read_before_mut_borrow(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    index: usize,
    args: &[Expr],
    shapes: &[RustSlotShape],
) -> RustExpr {
    match plan_same_call_shared_read_alias(source, target_shape, index, args, shapes) {
        SameCallArgumentAliasPlan::SnapshotSharedRead { local_name } => {
            snapshot_rendered_shared_borrow_argument(lowered, &local_name)
        }
        SameCallArgumentAliasPlan::Keep
        | SameCallArgumentAliasPlan::TemporaryForLaterRead { .. } => lowered,
    }
}

fn plan_same_call_shared_read_alias(
    source: &Expr,
    target_shape: &RustSlotShape,
    index: usize,
    args: &[Expr],
    shapes: &[RustSlotShape],
) -> SameCallArgumentAliasPlan {
    let Expr::Var(name) = source else {
        return SameCallArgumentAliasPlan::Keep;
    };
    if target_shape.ownership != OwnershipShape::SharedBorrow {
        return SameCallArgumentAliasPlan::Keep;
    }
    if !args
        .iter()
        .zip(shapes)
        .enumerate()
        .any(|(other_index, (arg, shape))| {
            other_index != index
                && shape.ownership == OwnershipShape::MutableBorrow
                && matches!(arg, Expr::Var(other_name) if other_name == name)
        })
    {
        return SameCallArgumentAliasPlan::Keep;
    }
    SameCallArgumentAliasPlan::SnapshotSharedRead {
        local_name: name.clone(),
    }
}

fn lower_argument_for_explicit_slot(
    arg: &Expr,
    env: &ShapeEnv,
    slot: Slot,
    ctx: LowerCtx,
) -> RustExpr {
    if let Slot::Argument(shape) = slot {
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
            lower_non_optional_argument_for_optional_target(arg, env, &shape, &actual_shape, ctx)
                .or_else(|| {
                    lower_optional_argument_for_borrowed_target(
                        arg,
                        env,
                        &shape,
                        &actual_shape,
                        ctx,
                    )
                })
                .unwrap_or_else(|| {
                    lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx)
                });
        return adapt_reused_owned_argument(lowered, arg, &shape, Some(&actual_shape), env, ctx);
    }
    lower_expr_for_slot(arg, env, slot, ctx)
}

fn lower_constructor_args_with_plan(
    args: &[Expr],
    env: &ShapeEnv,
    plan: ArgumentLoweringPlan,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    match plan {
        ArgumentLoweringPlan::Shaped(shapes) => {
            let actual_shapes: Vec<RustSlotShape> = args
                .iter()
                .map(|arg| {
                    infer_expr_shape_with_context(
                        arg,
                        env,
                        ExprShapeContext {
                            signatures: Some(ctx.signatures),
                            representations: Some(ctx.representations),
                            facts: Some(ctx.facts),
                        },
                    )
                })
                .collect();
            args.iter()
                .zip(shapes)
                .enumerate()
                .map(|(index, (arg, shape))| {
                    let actual_shape = &actual_shapes[index];
                    let lowered = lower_non_optional_argument_for_optional_target(
                        arg,
                        env,
                        &shape,
                        actual_shape,
                        ctx,
                    )
                    .unwrap_or_else(|| {
                        lower_expr_for_slot(arg, env, argument_slot(shape.clone()), ctx)
                    });
                    adapt_reused_owned_argument(lowered, arg, &shape, Some(actual_shape), env, ctx)
                })
                .collect()
        }
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
                    adapt_reused_owned_argument(lowered, arg, &shape, Some(&shape), env, ctx)
                })
                .collect()
        }
        ArgumentLoweringPlan::Slotted(slots) => args
            .iter()
            .zip(slots)
            .map(|(arg, slot)| lower_argument_for_explicit_slot(arg, env, slot, ctx))
            .collect(),
    }
}

fn plan_constructor_arguments_with_signature_fallback(
    resolved: Option<&ResolvedConstructor>,
    args: &[Expr],
    env: &ShapeEnv,
    signatures: &crate::signature::SignaturePlan,
    constructed_ty: &JavaType,
    rust_name: &str,
    facts: &FactSet,
) -> ArgumentLoweringPlan {
    let arg_count = args.len();
    let plan = plan_constructor_arguments(resolved, arg_count, signatures);
    let ArgumentLoweringPlan::Unshaped { arg_count } = plan else {
        return plan;
    };
    let Some(type_name) = concrete_java_type_name(constructed_ty) else {
        return ArgumentLoweringPlan::Unshaped { arg_count };
    };
    if let Some(shapes) = external_constructor_arg_shapes_with_args(
        constructed_ty,
        rust_name,
        args,
        signatures,
        facts,
        env.type_env(),
    ) {
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
        lowered = propagate_result_projection(lowered);
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
    let clone_recovery = clone_recovery_decision_for_argument(arg, &shape, Some(&shape), env, ctx);
    if !matches!(clone_recovery, CloneRecoveryDecision::NoRecovery) {
        lowered = apply_clone_recovery_decision(lowered, clone_recovery);
    }
    if borrowed_self_receiver_can_read_fields(ctx)
        && shared_self_field_read_name(arg, env).is_some()
        && !matches!(shape.java_ty, JavaType::Primitive(_))
        && known_owned_argument_clone_recovery_with_facts(&shape, ctx.facts)
    {
        return apply_clone_recovery_decision(
            lowered,
            audited_clone_recovery_decision(CloneRecoveryIdempotence::AnyCloneRecovery),
        );
    }
    if reused_owned_argument_needs_clone(arg, &shape, Some(&shape), ctx) {
        return apply_clone_recovery_decision(
            lowered,
            audited_clone_recovery_decision(CloneRecoveryIdempotence::AnyCloneRecovery),
        );
    }
    if matches!(arg, Expr::ArrayAccess(_, _)) && indexed_access_needs_borrowed_place(&shape) {
        return force_shared_borrow_projection(lowered);
    }
    if matches!(shape.java_ty, JavaType::Primitive(_))
        && shape.ownership == OwnershipShape::SharedBorrow
    {
        return deref_borrowed_projection(lowered);
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
        return Some(apply_audited_clone_recovery(this_expr()));
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
    let known_exception_supertype =
        is_bare_this && current_type_extends_known_exception_target(&target_ty, ctx);
    if !same_type && !implemented_target && !known_exception_supertype {
        return None;
    }
    if known_exception_supertype {
        return Some(exception_self_as_supertype_facade_boundary_projection());
    }
    if target.ownership == OwnershipShape::Owned
        && matches!(
            ctx.current_receiver,
            Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
        )
    {
        if implemented_target && matches!(target.dispatch, DispatchShape::ErasedObject) {
            return Some(erased_object_box_projection(apply_audited_clone_recovery(
                this_expr(),
            )));
        }
        if implemented_target && let DispatchShape::BoxedDynTrait { trait_name } = &target.dispatch
        {
            return Some(boxed_dyn_trait_projection(
                apply_audited_clone_recovery(this_expr()),
                trait_name,
            ));
        }
        return Some(apply_audited_clone_recovery(this_expr()));
    }
    Some(this_expr())
}

fn current_type_extends_known_exception_target(target_ty: &JavaType, ctx: LowerCtx) -> bool {
    let Some(current_type) = ctx.current_type else {
        return false;
    };
    let Some(target_name) = concrete_java_type_name(target_ty) else {
        return false;
    };
    if !is_known_exception_supertype_name(target_name) {
        return false;
    }
    source_or_jdk_subtype_reaches(current_type, target_name, ctx.support_local_type_program)
}

fn source_or_jdk_subtype_reaches(actual_name: &str, target_name: &str, program: &Program) -> bool {
    if same_concrete_java_type_name(&JavaType::Unknown(actual_name.to_string()), target_name) {
        return true;
    }
    if crate::stdlib::is_known_java_subtype(erased_java_simple_name(actual_name), target_name) {
        return true;
    }
    let Some(ty) = find_type_decl_by_name(program, actual_name) else {
        return false;
    };
    let Some(super_name) = ty.extends.as_deref() else {
        return false;
    };
    same_concrete_java_type_name(
        &crate::parser::parse_hierarchy_type(super_name),
        target_name,
    ) || crate::stdlib::is_known_java_subtype(erased_java_simple_name(super_name), target_name)
        || source_or_jdk_subtype_reaches(super_name, target_name, program)
}

fn is_known_exception_supertype_name(name: &str) -> bool {
    matches!(
        erased_java_simple_name(name),
        "Throwable"
            | "Exception"
            | "RuntimeException"
            | "IllegalArgumentException"
            | "IllegalStateException"
    )
}

fn current_type_implements_target(target_ty: &JavaType, ctx: LowerCtx) -> bool {
    let Some(current_type) = ctx.current_type else {
        return false;
    };
    let mut visiting = HashSet::new();
    current_type_or_super_implements_target(current_type, target_ty, ctx, &mut visiting)
}

fn current_type_or_super_implements_target(
    type_name: &str,
    target_ty: &JavaType,
    ctx: LowerCtx,
    visiting: &mut HashSet<String>,
) -> bool {
    let key = crate::stdlib::canonical_external_receiver_type_key(type_name);
    if !visiting.insert(key.clone()) {
        return false;
    }
    let Some(ty) = find_context_type_decl_by_erased_name(ctx, &key) else {
        return false;
    };
    if ty.implements.iter().any(|implemented| {
        same_concrete_java_type_name(target_ty, implemented)
            || same_concrete_java_type(target_ty, &crate::parser::parse_hierarchy_type(implemented))
            || current_type_or_super_implements_target(implemented, target_ty, ctx, visiting)
    }) {
        return true;
    }
    ty.extends.as_deref().is_some_and(|super_name| {
        same_concrete_java_type_name(target_ty, super_name)
            || current_type_or_super_implements_target(super_name, target_ty, ctx, visiting)
    })
}

fn find_context_type_decl_by_erased_name<'a>(
    ctx: LowerCtx<'a>,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    find_super_type_decl_by_erased_name(ctx.program, type_name)
        .or_else(|| find_super_type_decl_by_erased_name(ctx.support_local_type_program, type_name))
        .or_else(|| find_super_type_decl_by_erased_name(ctx.source_summary_type_program, type_name))
}

fn find_type_decl_by_name<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_by_name_in_type(ty, type_name))
}

fn find_super_type_decl_by_erased_name<'a>(
    program: &'a Program,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    find_type_decl_by_name(program, type_name)
        .or_else(|| find_type_decl_by_name(program, erased_java_type_name(type_name)))
        .or_else(|| {
            find_unique_type_decl_by_simple_name(program, erased_java_simple_name(type_name))
        })
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

fn find_unique_type_decl_by_simple_name<'a>(
    program: &'a Program,
    simple_name: &str,
) -> Option<&'a TypeDecl> {
    let mut found = None;
    for ty in &program.types {
        for candidate in type_decl_and_nested_types(ty) {
            if candidate.name == simple_name {
                if found.is_some() {
                    return None;
                }
                found = Some(candidate);
            }
        }
    }
    found
}

fn type_decl_and_nested_types<'a>(ty: &'a TypeDecl) -> Vec<&'a TypeDecl> {
    let mut types = vec![ty];
    for member in &ty.members {
        if let Member::Type(nested) = member {
            types.extend(type_decl_and_nested_types(nested));
        }
    }
    types
}

fn erased_java_type_name(type_name: &str) -> &str {
    type_name.split('<').next().unwrap_or(type_name).trim()
}

fn erased_java_simple_name(type_name: &str) -> &str {
    erased_java_type_name(type_name)
        .rsplit(['.', '$'])
        .next()
        .unwrap_or_else(|| erased_java_type_name(type_name))
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
    Some(erased_object_borrowed_downcast_ref_projection(
        RustExpr::Var(local_name.to_string()),
        &rust_type,
        exact_erased_object_local_is_optional(local_name, env, ctx),
    ))
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
    mut plan: ArgumentLoweringPlan,
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
) -> Vec<RustExpr> {
    if args.len() == 1 && target.rust_method_name() == Some("write_int") {
        let shape = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int));
        return vec![lower_expr_for_slot(
            &args[0],
            env,
            argument_slot(shape),
            ctx,
        )];
    }
    if let CallTargetPlan::ExternalFacadeReceiver { rust_path, .. } = target
        && let Some(facade_plan) = facade_abi_argument_plan_for_rust_path(
            rust_path,
            args.len(),
            Some(args),
            env.type_env(),
            ctx.facts,
        )
    {
        plan = facade_plan;
    }
    if matches!(plan, ArgumentLoweringPlan::Unshaped { .. })
        && let Some(rust_method) = target.rust_method_name()
        && let Some(facade_plan) = facade_abi_argument_plan_for_rust_method_name(
            rust_method,
            args.len(),
            Some(args),
            env.type_env(),
            ctx.facts,
        )
    {
        plan = facade_plan;
    }
    if let Some(source_plan) = emitted_source_resolved_call_argument_plan(resolved, args.len(), ctx)
    {
        plan = source_plan;
    }
    if let Some(parse_plan) = native_numeric_parse_argument_plan(target, args.len()) {
        plan = parse_plan;
    }
    if matches!(plan, ArgumentLoweringPlan::Unshaped { .. })
        && args.len() == 1
        && !call_target_uses_display_arguments(target, call_receiver, args, env, ctx)
        && let Some(lowered) = lower_unshaped_argument_for_call_target(&args[0], target, env, ctx)
            .or_else(|| {
                call_receiver.and_then(|receiver| {
                    lower_unshaped_argument_for_receiver(&args[0], receiver, env, ctx)
                })
            })
    {
        return vec![lowered];
    }
    if matches!(plan, ArgumentLoweringPlan::Unshaped { .. })
        && let Some(signature_plan) =
            call_target_signature_arg_plan(target, call_receiver, args, env, ctx)
    {
        plan = signature_plan;
    }
    if let Some(packed) = fallback_signature
        .and_then(|signature| lower_varargs_call_args_with_signature(args, env, signature, ctx))
    {
        return packed;
    }
    if let Some(packed) = lower_resolved_varargs_call_args(args, env, resolved, ctx) {
        return packed;
    }
    match target {
        _ if call_target_uses_display_arguments(target, call_receiver, args, env, ctx) => args
            .iter()
            .map(|arg| lower_expr_for_slot(arg, env, Slot::DisplaySegment, ctx))
            .collect(),
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::StringFormat,
        } => lower_string_format_args(args, env, ctx),
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::StringJoin,
        } => lower_string_join_args(args, env, ctx),
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::StringValueOf,
        } => args
            .iter()
            .map(|arg| lower_expr_for_slot(arg, env, Slot::DisplaySegment, ctx))
            .collect(),
        CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::DecimalFormatFormat,
            ..
        } => lower_args_with_actual_shapes(args, env, ctx),
        CallTargetPlan::NativeJavaInstance {
            method:
                crate::call_plan::NativeJavaInstanceMethod::StringContains
                | crate::call_plan::NativeJavaInstanceMethod::StringStartsWith
                | crate::call_plan::NativeJavaInstanceMethod::StringEndsWith
                | crate::call_plan::NativeJavaInstanceMethod::StringReplace
                | crate::call_plan::NativeJavaInstanceMethod::StringReplaceFirst
                | crate::call_plan::NativeJavaInstanceMethod::StringReplaceAll
                | crate::call_plan::NativeJavaInstanceMethod::StringMatches
                | crate::call_plan::NativeJavaInstanceMethod::StringSplit
                | crate::call_plan::NativeJavaInstanceMethod::StringCompareTo
                | crate::call_plan::NativeJavaInstanceMethod::StringCompareToIgnoreCase
                | crate::call_plan::NativeJavaInstanceMethod::StringEqualsIgnoreCase,
            ..
        } if !matches!(plan, ArgumentLoweringPlan::Unshaped { .. }) => {
            lower_args_with_plan(args, env, plan, ctx)
        }
        CallTargetPlan::NativeJavaInstance {
            method:
                crate::call_plan::NativeJavaInstanceMethod::StringContains
                | crate::call_plan::NativeJavaInstanceMethod::StringStartsWith
                | crate::call_plan::NativeJavaInstanceMethod::StringEndsWith
                | crate::call_plan::NativeJavaInstanceMethod::StringReplace
                | crate::call_plan::NativeJavaInstanceMethod::StringCompareTo
                | crate::call_plan::NativeJavaInstanceMethod::StringCompareToIgnoreCase
                | crate::call_plan::NativeJavaInstanceMethod::StringEqualsIgnoreCase,
            ..
        } => lower_args_with_plan(
            args,
            env,
            string_pattern_compat_argument_plan(args.len()),
            ctx,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapInsert,
            ..
        } => lower_map_insert_args(args, env, plan, ctx),
        _ if external_facade_preserving_call_clones_leading_arg(target) => {
            if matches!(plan, ArgumentLoweringPlan::Unshaped { .. }) {
                lower_unshaped_external_facade_preserving_args(
                    target,
                    call_receiver,
                    args,
                    env,
                    ctx,
                )
            } else {
                apply_external_facade_preserving_leading_arg_clone(
                    lower_args_with_plan(args, env, plan, ctx),
                    args,
                    env,
                    ctx,
                )
            }
        }
        _ if external_generic_facade_target_borrows_actual_argument_shapes(target, env) => {
            lower_external_generic_facade_args_with_borrowed_actual_shapes(args, env, ctx)
        }
        _ if external_generic_facade_target_keeps_argument_shapes(target, env, ctx) => {
            lower_external_generic_facade_args_with_actual_shapes(args, env, plan, ctx)
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

fn native_numeric_parse_argument_plan(
    target: &CallTargetPlan<'_>,
    arg_count: usize,
) -> Option<ArgumentLoweringPlan> {
    let CallTargetPlan::NativeJavaStatic { method } = target else {
        return None;
    };
    match method {
        crate::call_plan::NativeJavaStaticMethod::IntegerParseInt if matches!(arg_count, 1 | 2) => {
            let mut shapes = vec![borrowed_string_content_shape()];
            if arg_count == 2 {
                shapes.push(value_shape(JavaType::Primitive(PrimitiveType::Int)));
            }
            Some(ArgumentLoweringPlan::Shaped(shapes))
        }
        crate::call_plan::NativeJavaStaticMethod::FloatParseFloat
        | crate::call_plan::NativeJavaStaticMethod::DoubleParseDouble
            if arg_count == 1 =>
        {
            Some(ArgumentLoweringPlan::Shaped(vec![
                borrowed_string_content_shape(),
            ]))
        }
        _ => None,
    }
}

fn emitted_source_resolved_call_argument_plan(
    resolved: Option<&ResolvedCall>,
    arg_count: usize,
    ctx: LowerCtx,
) -> Option<ArgumentLoweringPlan> {
    let resolved = resolved?;
    let owner = find_context_type_by_key(ctx.program, &resolved.declaring_type)?;
    let exact_method = owner.members.iter().find_map(|member| match member {
        Member::Method(method)
            if method.params.len() == arg_count
                && method_staticness_matches_resolved(method, resolved.kind)
                && method_effective_rust_name(method) == resolved.rust_name =>
        {
            Some(method)
        }
        _ => None,
    });
    let method = exact_method.or_else(|| {
        let mut matches = owner.members.iter().filter_map(|member| match member {
            Member::Method(method)
                if method.params.len() == arg_count
                    && method_staticness_matches_resolved(method, resolved.kind)
                    && method_rust_name_matches_resolved(method, &resolved.rust_name) =>
            {
                Some(method)
            }
            _ => None,
        });
        let first = matches.next()?;
        if matches.next().is_none() {
            Some(first)
        } else {
            None
        }
    })?;
    let signature = ctx.signatures.required_method_signature(owner, method);
    Some(ArgumentLoweringPlan::Shaped(
        signature
            .params
            .iter()
            .map(|param| param.shape.clone())
            .collect(),
    ))
}

fn method_effective_rust_name(method: &MethodDecl) -> String {
    method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}

fn method_staticness_matches_resolved(method: &MethodDecl, resolved_kind: CallKind) -> bool {
    let method_is_static = method.modifiers.contains(&Modifier::Static);
    matches!(
        (resolved_kind, method_is_static),
        (CallKind::Static, true) | (CallKind::Instance, false)
    )
}

fn method_rust_name_matches_resolved(method: &MethodDecl, resolved_rust_name: &str) -> bool {
    let base = crate::parser::to_snake_case(&method.name);
    resolved_rust_name == base
        || resolved_rust_name
            .strip_prefix(&base)
            .is_some_and(|suffix| suffix.starts_with('_'))
}

fn call_target_uses_display_arguments(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    match target {
        target if target.is_fallback_receiver_method("append") => {
            call_receiver.is_some_and(|receiver| {
                receiver_type_uses_java_append_display_arguments(receiver, env)
            }) && args
                .iter()
                .any(|arg| expr_needs_erased_object_display_argument(arg, env, ctx))
        }
        CallTargetPlan::ExternalFacadeReceiver {
            rust_path: rust_name,
            ..
        } => {
            matches!(rust_name.rsplit("::").next().unwrap_or(rust_name), "append")
                && args
                    .iter()
                    .any(|arg| expr_needs_erased_object_display_argument(arg, env, ctx))
        }
        CallTargetPlan::ExternalCrateReceiver { rust_name, .. } => {
            matches!(rust_name.as_str(), "append")
                && args
                    .iter()
                    .any(|arg| expr_needs_erased_object_display_argument(arg, env, ctx))
        }
        CallTargetPlan::StaticOwner { owner, rust_name } => {
            matches!(rust_name.as_str(), "valueOf" | "value_of")
                && owner.rsplit("::").next().unwrap_or(owner) == "String"
        }
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::StringValueOf,
        } => true,
        _ => false,
    }
}

fn expr_needs_erased_object_display_argument(arg: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    let actual_shape = infer_expr_shape_with_context(
        arg,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    external_generic_facade_arg_shape_is_erased_object_handoff(&actual_shape)
}

fn receiver_type_uses_java_append_display_arguments(receiver: &Expr, env: &ShapeEnv) -> bool {
    let receiver_ty = types::infer_expr_type(receiver, env.type_env());
    let receiver_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or_else(|| receiver_ty.clone());
    let name = match &receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.as_str(),
        JavaType::String => "String",
        JavaType::Void | JavaType::Primitive(_) | JavaType::Array(_) | JavaType::Wildcard(_) => {
            return false;
        }
    };
    matches!(
        name.rsplit('.').next().unwrap_or(name),
        "StringBuilder" | "StringBuffer" | "Appendable"
    )
}

fn lower_string_value_of_display_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !matches!(name, "valueOf" | "value_of") || args.len() != 1 {
        return None;
    }
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    let owner = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if owner != "String" {
        return None;
    }
    Some(apply_java_display_plan(
        lower_expr_for_slot_with_refinements(&args[0], env, Slot::DisplaySegment, ctx, refinements),
        JavaDisplayPlan::FormatValue,
    ))
}

fn lower_args_with_actual_shapes(args: &[Expr], env: &ShapeEnv, ctx: LowerCtx) -> Vec<RustExpr> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    args.iter()
        .map(|arg| {
            let actual_shape = infer_expr_shape_with_context(arg, env, shape_ctx);
            lower_expr_for_slot(arg, env, argument_slot(actual_shape), ctx)
        })
        .collect()
}

fn call_target_signature_arg_plan(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ArgumentLoweringPlan> {
    let receiver_owner;
    let (owner_key, rust_name) = match target {
        CallTargetPlan::StaticOwner {
            owner, rust_name, ..
        } => {
            let owner_key = if owner == "Self" {
                ctx.current_type.unwrap_or(owner)
            } else {
                owner
            };
            (owner_key, rust_name.as_str())
        }
        CallTargetPlan::StaticSelf { rust_name } => (ctx.current_type?, *rust_name),
        CallTargetPlan::FallbackSelf { rust_name } | CallTargetPlan::FallbackFree { rust_name } => {
            (ctx.current_type?, rust_name.as_str())
        }
        CallTargetPlan::InstanceSelf { rust_name } => (ctx.current_type?, *rust_name),
        CallTargetPlan::InstanceReceiver {
            receiver,
            rust_name,
        } => {
            receiver_owner = receiver_signature_owner(receiver, env)?;
            (receiver_owner.as_str(), *rust_name)
        }
        CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name,
        } => {
            receiver_owner = receiver_signature_owner(receiver, env)?;
            (receiver_owner.as_str(), rust_name.as_str())
        }
        CallTargetPlan::ExternalCrateReceiver {
            receiver,
            rust_name,
            ..
        } => {
            receiver_owner = receiver_signature_owner(receiver, env)?;
            (receiver_owner.as_str(), rust_name.as_str())
        }
        CallTargetPlan::ExternalFacadeReceiver { rust_path, .. } => (
            {
                receiver_owner = receiver_signature_owner(call_receiver?, env)?;
                receiver_owner.as_str()
            },
            rust_path.rsplit("::").next().unwrap_or(rust_path),
        ),
        _ => return None,
    };
    let signature =
        method_signature_for_owner_aliases(ctx.signatures, owner_key, rust_name, args.len())?;
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

fn emitted_call_target_signature_arg_plan(
    target: &CallTargetPlan<'_>,
    call_receiver: Option<&Expr>,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ArgumentLoweringPlan> {
    let owner_key = match target {
        CallTargetPlan::StaticOwner { owner, .. } if owner == "Self" => ctx.current_type?,
        CallTargetPlan::StaticOwner { owner, .. } => owner.as_str(),
        CallTargetPlan::StaticSelf { .. }
        | CallTargetPlan::FallbackSelf { .. }
        | CallTargetPlan::FallbackFree { .. }
        | CallTargetPlan::InstanceSelf { .. } => ctx.current_type?,
        _ => return None,
    };
    if !static_context_owner_is_emitted(owner_key, ctx) {
        return None;
    }
    call_target_signature_arg_plan(target, call_receiver, args, env, ctx)
}

fn method_signature_for_owner_aliases<'a>(
    signatures: &'a crate::signature::SignaturePlan,
    owner_key: &str,
    rust_name: &str,
    arg_count: usize,
) -> Option<&'a crate::signature::MethodSignaturePlan> {
    let mut owners = vec![owner_key.to_string()];
    let canonical = crate::stdlib::canonical_external_receiver_type_key(owner_key);
    if canonical != owner_key {
        owners.push(canonical.clone());
    }
    let rust_owner = crate::slot_render::rust_name_for_java_named_type(&canonical);
    if rust_owner != owner_key && !owners.iter().any(|owner| owner == &rust_owner) {
        owners.push(rust_owner);
    }
    owners.iter().find_map(|owner| {
        signatures
            .method_signature_by_rust_name(owner, rust_name)
            .or_else(|| {
                signatures
                    .method_signature_by_base_name_and_arity(owner, rust_name, arg_count)
                    .map(|(_, signature)| signature)
            })
    })
}

fn receiver_signature_owner(receiver: &Expr, env: &ShapeEnv) -> Option<String> {
    concrete_java_type_name(&crate::types::infer_expr_type(receiver, env.type_env()))
        .map(ToOwned::to_owned)
}

fn external_facade_preserving_call_clones_leading_arg(target: &CallTargetPlan<'_>) -> bool {
    let Some(rust_name) = target.rust_method_name() else {
        return false;
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
    let lowered = args
        .iter()
        .map(|arg| {
            let lowered = lower_unshaped_argument_for_call_target(arg, target, env, ctx)
                .or_else(|| {
                    call_receiver.and_then(|receiver| {
                        lower_unshaped_argument_for_receiver(arg, receiver, env, ctx)
                    })
                })
                .unwrap_or_else(|| lower_unshaped_argument(arg, env, ctx));
            lowered
        })
        .collect();
    apply_external_facade_preserving_leading_arg_clone(lowered, args, env, ctx)
}

fn apply_external_facade_preserving_leading_arg_clone(
    mut lowered: Vec<RustExpr>,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    if let Some(first) = lowered.first_mut()
        && let Some(arg) = args.first()
        && external_facade_preserving_arg_needs_clone(arg, env, ctx)
    {
        *first = apply_clone_recovery_decision(
            first.clone(),
            audited_clone_recovery_decision(CloneRecoveryIdempotence::DirectClone),
        );
    }
    lowered
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
        && (known_owned_argument_clone_recovery_with_facts(&shape, ctx.facts)
            || optional_owned_argument_clone_recovery(&shape, ctx.facts))
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
        return lower_explicit_varargs_array_call_args_from_params(
            args,
            env,
            &signature.params,
            ctx,
        );
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
        return lower_explicit_varargs_array_call_args_from_params(
            args,
            env,
            &signature.params,
            ctx,
        );
    }
    lower_varargs_call_args_from_params(args, env, &signature.params, ctx)
}

fn lower_resolved_constructor_varargs_args(
    args: &[Expr],
    env: &ShapeEnv,
    resolved: &ResolvedConstructor,
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let signature = ctx
        .signatures
        .constructor_signature_by_rust_name(&resolved.declaring_type, &resolved.rust_name)?;
    let varargs_param = signature.params.last()?;
    if !varargs_param.is_varargs {
        return None;
    }
    let fixed_count = signature.params.len().saturating_sub(1);
    if args.len() < fixed_count {
        return None;
    }
    if args.len() == signature.params.len() && last_arg_is_explicit_varargs_array(args, env, ctx) {
        return lower_explicit_varargs_array_call_args_from_params(
            args,
            env,
            &signature.params,
            ctx,
        );
    }
    lower_varargs_call_args_from_params(args, env, &signature.params, ctx)
}

fn lower_explicit_varargs_array_call_args_from_params(
    args: &[Expr],
    env: &ShapeEnv,
    params: &[crate::signature::ParamPlan],
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let varargs_param = params.last()?;
    if !varargs_param.is_varargs || args.len() != params.len() {
        return None;
    }
    let fixed_count = params.len().saturating_sub(1);
    let mut lowered = lower_fixed_varargs_prefix_args(args, env, params, ctx)?;
    let last = args.get(fixed_count)?;
    let lowered_arg =
        lower_expr_for_slot(last, env, argument_slot(varargs_param.shape.clone()), ctx);
    let actual_shape = infer_expr_shape_with_context(
        last,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    lowered.push(adapt_reused_owned_argument(
        lowered_arg,
        last,
        &varargs_param.shape,
        Some(&actual_shape),
        env,
        ctx,
    ));
    Some(lowered)
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
    let element_shape = project_java_value_shape(Some(ctx.representations), element_ty);
    let fixed_count = params.len().saturating_sub(1);
    let mut lowered = lower_fixed_varargs_prefix_args(args, env, params, ctx)?;
    let spread_tail = args
        .len()
        .saturating_sub(fixed_count)
        .checked_sub(1)
        .filter(|_| {
            args.last()
                .is_some_and(|_| last_arg_is_explicit_varargs_array(args, env, ctx))
        })
        .and_then(|fixed_vararg_count| {
            if fixed_vararg_count == 0 {
                None
            } else {
                Some(fixed_vararg_count)
            }
        });
    let vararg_value_count = spread_tail
        .map(|fixed_vararg_count| fixed_count + fixed_vararg_count)
        .unwrap_or(args.len());
    let varargs = args
        .iter()
        .skip(fixed_count)
        .take(vararg_value_count.saturating_sub(fixed_count))
        .map(|arg| lower_varargs_element_expr(arg, env, &element_shape, ctx))
        .collect::<Vec<_>>();
    let packed_vec = varargs_packed_vec_projection(varargs);
    let packed_vec = if spread_tail.is_some() {
        let tail = args.last()?;
        let tail = lower_expr_for_slot(tail, env, argument_slot(varargs_param.shape.clone()), ctx);
        varargs_spread_extend_projection(packed_vec, tail)
    } else {
        packed_vec
    };
    let packed = if varargs_param.shape.ownership == OwnershipShape::SharedBorrow {
        force_shared_borrow_projection(packed_vec)
    } else {
        packed_vec
    };
    lowered.push(packed);
    Some(lowered)
}

fn lower_fixed_varargs_prefix_args(
    args: &[Expr],
    env: &ShapeEnv,
    params: &[crate::signature::ParamPlan],
    ctx: LowerCtx,
) -> Option<Vec<RustExpr>> {
    let fixed_count = params.len().checked_sub(1)?;
    if args.len() < fixed_count {
        return None;
    }
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
            env,
            ctx,
        ));
    }
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
    let mut lowered = lower_expr(arg, env, ctx);
    if matches!(element_shape.dispatch, DispatchShape::ErasedObject)
        && reused_owned_argument_needs_clone(arg, &actual_shape, Some(&actual_shape), ctx)
    {
        lowered = apply_audited_clone_recovery(lowered);
    }
    if actual_shape.ownership == OwnershipShape::SharedBorrow
        && shape_is_owned_non_erased(element_shape)
        && known_owned_argument_clone_recovery_with_facts(&actual_shape, ctx.facts)
        && slot_conversion_plan_contains_external_into(&plan.conversion)
    {
        let cloned = apply_audited_clone_recovery(lowered);
        return apply_slot_expr_conversion(cloned, &plan);
    }
    lowered = apply_clone_recovery_decision(
        lowered,
        clone_recovery_decision_for_argument(arg, element_shape, Some(&actual_shape), env, ctx),
    );
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
    target.rust_method_name().is_some_and(|rust_name| {
        matches!(
            rust_name,
            "map" | "map_to_pair" | "filter" | "flat_map" | "map_partitions"
        )
    })
}

fn call_target_is_map_partitions(target: &CallTargetPlan<'_>) -> bool {
    target.rust_method_name().is_some_and(|rust_name| {
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
    Some(function_call_projection(
        RustExpr::Var(callback_name.clone()),
        lowered_args,
    ))
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
    apply_audited_clone_recovery(lowered)
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
            Some(
                render_typed_single_input_method_reference_callback_with_facts(
                    &MethodReferencePlan::AssociatedFunction { owner, method },
                    &param_ty,
                    element_shape,
                    ctx.facts,
                ),
            )
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
    if let Some(return_ty) = external_receiver_call_return_type_with_external_index(
        receiver,
        ctx.external_index,
        env.type_env(),
    ) && let Some(element_ty) = crate::types::sequence_element_type(&return_ty)
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
        let receiver_shape = infer_expr_shape_with_context(
            receiver,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
        );
        if receiver_shape.sequence_element_type().is_some() {
            return Some(receiver_shape);
        }
        return stream_like_call_return_shape(receiver, env, ctx).or(Some(receiver_shape));
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
        external_call_lowered_type_from_static_owner_plan_with_external_index(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            ctx.facts,
            ctx.external_index,
            env.type_env(),
        )
    })?;
    if let LoweredType::Slot(shape) = lowered {
        return Some((*shape).clone());
    }
    lowered
        .java_ty()
        .map(fallback_local_shape_from_declared_type)
}

fn preferred_planned_or_external_call_result_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let planned = planned_method_call_return_shape(expr, env, ctx);
    let external = external_call_expr_result_shape(expr, env, ctx)
        .or_else(|| known_external_call_result_shape_from_syntax(expr, ctx));
    match (planned, external) {
        (Some(_planned), Some(external)) if external.optional_element_type().is_some() => {
            Some(external)
        }
        (Some(planned), _) => Some(planned),
        (None, external) => external,
    }
}

fn known_external_call_result_shape_from_syntax(
    expr: &Expr,
    _ctx: LowerCtx,
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
    if args.len() == 1
        && matches!(
            name.as_str(),
            "getResourceAsStream" | "get_resource_as_stream"
        )
    {
        let optional_stream = crate::types::optional_java_type(JavaType::Named {
            name: "InputStream".to_string(),
            args: Vec::new(),
        });
        return Some(fallback_local_shape_from_declared_type(&optional_stream));
    }
    let Expr::Var(receiver_name) = receiver.as_ref() else {
        return None;
    };
    let owner = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    [name.to_string(), crate::parser::to_snake_case(name)]
        .into_iter()
        .find_map(|method| {
            crate::stdlib::known_external_method_return_type(Some(&owner), &method, args.len()).map(
                |return_ty| {
                    let lowered = known_external_call_lowered_type(
                        Some(&owner),
                        &method,
                        args.len(),
                        return_ty,
                    );
                    lowered
                        .java_ty()
                        .map(fallback_local_shape_from_declared_type)
                        .unwrap_or_else(|| value_shape(JavaType::Unknown("Object".to_string())))
                },
            )
        })
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
        return function_call_projection(self_associated_path_projection(rust_name), args);
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
    }) || receiver_is_known_string_producing_call(receiver);
    if !receiver_is_string_like {
        return plan;
    }
    let method = match (name, args.len()) {
        ("length", 0) => NativeJavaInstanceMethod::StringLen,
        ("isEmpty", 0) => NativeJavaInstanceMethod::StringIsEmpty,
        ("trim", 0) => NativeJavaInstanceMethod::StringTrim,
        ("toLowerCase", 0) => NativeJavaInstanceMethod::StringToLowercase,
        ("toUpperCase", 0) => NativeJavaInstanceMethod::StringToUppercase,
        ("toString", 0) => NativeJavaInstanceMethod::StringToString,
        ("contains", 1) => NativeJavaInstanceMethod::StringContains,
        ("startsWith", 1) => NativeJavaInstanceMethod::StringStartsWith,
        ("endsWith", 1) => NativeJavaInstanceMethod::StringEndsWith,
        ("replace", 2) => NativeJavaInstanceMethod::StringReplace,
        ("replaceFirst" | "replace_first", 2) => NativeJavaInstanceMethod::StringReplaceFirst,
        ("replaceAll" | "replace_all", 2) => NativeJavaInstanceMethod::StringReplaceAll,
        ("matches", 1) => NativeJavaInstanceMethod::StringMatches,
        ("split", 1 | 2) => NativeJavaInstanceMethod::StringSplit,
        ("substring", 1 | 2) => NativeJavaInstanceMethod::StringSubstring,
        ("toCharArray" | "to_char_array", 0) => NativeJavaInstanceMethod::StringToCharArray,
        ("hashCode" | "hash_code", 0) => NativeJavaInstanceMethod::StringHashCode,
        ("getBytes" | "get_bytes", 0 | 1) => NativeJavaInstanceMethod::StringGetBytes,
        ("indexOf" | "index_of", 1 | 2) => NativeJavaInstanceMethod::StringIndexOf,
        ("lastIndexOf" | "last_index_of", 1 | 2) => NativeJavaInstanceMethod::StringLastIndexOf,
        ("charAt" | "char_at", 1) => NativeJavaInstanceMethod::StringCharAt,
        ("compareTo" | "compare_to", 1) => NativeJavaInstanceMethod::StringCompareTo,
        ("compareToIgnoreCase" | "compare_to_ignore_case", 1) => {
            NativeJavaInstanceMethod::StringCompareToIgnoreCase
        }
        ("equalsIgnoreCase" | "equals_ignore_case", 1) => {
            NativeJavaInstanceMethod::StringEqualsIgnoreCase
        }
        _ => return plan,
    };
    CallExprPlan::Target(CallTargetPlan::NativeJavaInstance { receiver, method })
}

fn receiver_is_known_string_producing_call(receiver: &Expr) -> bool {
    matches!(
        receiver,
        Expr::Call { name, args, .. }
            if matches!(
                (name.as_str(), args.len()),
                ("substring", 1 | 2)
                    | ("toLowerCase" | "to_lower_case", 0)
                    | ("toUpperCase" | "to_upper_case", 0)
                    | ("toString" | "to_string", 0)
                    | ("replace", 2)
                    | ("replaceFirst" | "replace_first", 2)
                    | ("replaceAll" | "replace_all", 2)
            )
    )
}

fn retarget_native_collection_to_array_from_shape<'a>(
    plan: CallExprPlan<'a>,
    receiver: Option<&'a Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> CallExprPlan<'a> {
    if matches!(
        plan,
        CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod { .. })
    ) {
        return plan;
    }
    if !matches!((name, args.len()), ("toArray" | "to_array", 0 | 1)) {
        return plan;
    }
    let Some(receiver) = receiver else {
        return plan;
    };
    if is_java_collection_view_call(receiver) {
        return CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
            receiver,
            method: NativeCollectionMethod::ToArray,
        });
    }
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
    let receiver_is_collection_like = receiver_shapes.iter().any(|shape| {
        let receiver_ty = crate::types::optional_element_type(&shape.java_ty)
            .unwrap_or_else(|| shape.java_ty.clone());
        crate::stdlib::java_collection_kind_for_type(&receiver_ty).is_some()
    });
    if !receiver_is_collection_like {
        return plan;
    }
    CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
        receiver,
        method: NativeCollectionMethod::ToArray,
    })
}

fn retarget_native_collection_size_from_shape<'a>(
    plan: CallExprPlan<'a>,
    receiver: Option<&'a Expr>,
    name: &str,
    args: &[Expr],
) -> CallExprPlan<'a> {
    if matches!(
        plan,
        CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod { .. })
    ) {
        return plan;
    }
    if name != "size" || !args.is_empty() {
        return plan;
    }
    let Some(receiver) = receiver else {
        return plan;
    };
    if !is_java_collection_view_call(receiver) {
        return plan;
    }
    CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
        receiver,
        method: NativeCollectionMethod::Len,
    })
}

fn is_java_collection_view_call(receiver: &Expr) -> bool {
    matches!(
        receiver,
        Expr::Call { name, args, .. }
            if args.is_empty() && matches!(name.as_str(), "keySet" | "key_set")
    )
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
    if target
        .rust_method_name()
        .is_some_and(|method| matches!(method, "writeObject" | "write_object"))
    {
        let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
        if concrete_java_type_name(&receiver_ty).is_some_and(|name| {
            crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "ObjectOutputStream")
        }) {
            return true;
        }
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    let effective_facts = crate::effective_facts::EffectiveFacts::new(ctx.facts);
    if !effective_facts.is_final_type(&receiver_ty) {
        return false;
    }
    crate::stdlib::java_collection_kind_for_type(&receiver_ty).is_some()
        || concrete_java_type_name(&receiver_ty).is_some_and(|name| {
            ctx.facts.external_library_pack_for_type(name).is_some()
                || crate::external_packs::external_library_pack_for_type(name).is_some()
        })
}

fn external_generic_facade_target_borrows_actual_argument_shapes(
    target: &CallTargetPlan<'_>,
    env: &ShapeEnv,
) -> bool {
    let Some(receiver) = target.receiver_expr() else {
        return false;
    };
    if !target
        .rust_method_name()
        .is_some_and(|method| method == "append")
    {
        return false;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    concrete_java_type_name(&receiver_ty).is_some_and(|name| {
        let simple = name.rsplit('.').next().unwrap_or(name);
        matches!(simple, "StringBuilder" | "StringBuffer")
    })
}

fn lower_external_generic_facade_args_with_borrowed_actual_shapes(
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Vec<RustExpr> {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    args.iter()
        .map(|arg| {
            let mut actual_shape = infer_expr_shape_with_context(arg, env, shape_ctx);
            if !matches!(actual_shape.java_ty, JavaType::Primitive(_))
                && !matches!(arg, Expr::Ternary { .. })
            {
                actual_shape.ownership = OwnershipShape::SharedBorrow;
            }
            lower_expr_for_slot(arg, env, argument_slot(actual_shape), ctx)
        })
        .collect()
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
                if external_generic_facade_arg_shape_is_unspecified_object(&shape)
                    || external_generic_facade_arg_shape_is_erased_object_handoff(&shape)
                {
                    let actual_shape = generic_facade_actual_argument_shape(arg, env, ctx)
                        .unwrap_or_else(|| {
                            infer_expr_shape_with_context(
                                arg,
                                env,
                                ExprShapeContext {
                                    signatures: Some(ctx.signatures),
                                    representations: Some(ctx.representations),
                                    facts: Some(ctx.facts),
                                },
                            )
                        });
                    let lowered =
                        lower_expr_for_slot(arg, env, argument_slot(actual_shape.clone()), ctx);
                    let lowered = clone_indexed_erased_handoff_arg_if_needed(
                        lowered,
                        arg,
                        &original_shape,
                        &actual_shape,
                    );
                    let lowered = wrap_erased_handoff_arg_for_optional_target(
                        lowered,
                        &original_shape,
                        &actual_shape,
                        ctx.facts,
                    );
                    adapt_reused_owned_argument(
                        lowered,
                        arg,
                        &actual_shape,
                        Some(&actual_shape),
                        env,
                        ctx,
                    )
                } else {
                    let actual_shape = generic_facade_actual_argument_shape(arg, env, ctx)
                        .unwrap_or_else(|| {
                            infer_expr_shape_with_context(
                                arg,
                                env,
                                ExprShapeContext {
                                    signatures: Some(ctx.signatures),
                                    representations: Some(ctx.representations),
                                    facts: Some(ctx.facts),
                                },
                            )
                        });
                    let lowered = lower_optional_argument_for_borrowed_target(
                        arg,
                        env,
                        &original_shape,
                        &actual_shape,
                        ctx,
                    )
                    .unwrap_or_else(|| lower_expr_for_slot(arg, env, argument_slot(shape), ctx));
                    adapt_reused_owned_argument(
                        lowered,
                        arg,
                        &original_shape,
                        Some(&actual_shape),
                        env,
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
        ArgumentLoweringPlan::Slotted(slots) => args
            .iter()
            .zip(slots)
            .map(|(arg, slot)| lower_argument_for_explicit_slot(arg, env, slot, ctx))
            .collect(),
    }
}

fn generic_facade_actual_argument_shape(
    arg: &Expr,
    _env: &ShapeEnv,
    _ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let Expr::New { ty, resolved, .. } = arg else {
        return None;
    };
    let constructed_ty = resolved
        .as_ref()
        .map(|resolved| resolved.constructed_type.clone())
        .unwrap_or_else(|| ty.clone());
    if concrete_java_type_name(&constructed_ty)
        .is_some_and(|name| crate::stdlib::canonical_external_receiver_type_key(name) == "Object")
    {
        return None;
    }
    Some(RustSlotShape::for_storage(constructed_ty))
}

fn external_generic_facade_arg_shape_is_unspecified_object(shape: &RustSlotShape) -> bool {
    matches!(&shape.java_ty, JavaType::Unknown(_))
}

fn external_generic_facade_arg_shape_is_erased_object_handoff(shape: &RustSlotShape) -> bool {
    if matches!(shape.dispatch, DispatchShape::ErasedObject)
        || matches!(shape.view, ViewShape::ErasedObjectView)
    {
        return true;
    }
    shape.optional_element_type().is_some_and(|element_ty| {
        let element_shape = crate::conversion_plan::optional_element_slot_shape(shape, element_ty);
        matches!(element_shape.dispatch, DispatchShape::ErasedObject)
            || matches!(element_shape.view, ViewShape::ErasedObjectView)
    })
}

fn wrap_erased_handoff_arg_for_optional_target(
    lowered: RustExpr,
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    if target_shape.optional_element_type().is_some()
        && actual_shape.optional_element_type().is_none()
    {
        planned_optional_value_for_target(lowered, actual_shape.clone(), target_shape, facts)
    } else {
        lowered
    }
}

fn clone_indexed_erased_handoff_arg_if_needed(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
) -> RustExpr {
    if !matches!(source, Expr::ArrayAccess(_, _)) {
        return lowered;
    }
    if !external_generic_facade_arg_shape_is_erased_object_handoff(target_shape)
        && !matches!(actual_shape.dispatch, DispatchShape::ErasedObject)
    {
        return lowered;
    }
    clone_erased_object_ref_once(lowered)
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
                adapt_reused_owned_argument(lowered, source, &shape, Some(&actual_shape), env, ctx)
            })
            .collect(),
        ArgumentLoweringPlan::Unshaped { .. } => lower_args_with_plan(args, env, plan, ctx),
        ArgumentLoweringPlan::Slotted(_) => lower_args_with_plan(args, env, plan, ctx),
    }
}

fn lower_non_optional_argument_for_optional_target(
    arg: &Expr,
    env: &ShapeEnv,
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if target_shape.ownership != OwnershipShape::Owned
        || actual_shape.optional_element_type().is_some()
    {
        return None;
    }
    let target_element_ty = target_shape.optional_element_type()?;
    if !same_concrete_java_type(&actual_shape.java_ty, &target_element_ty) {
        return None;
    }
    if shape_is_erased_object_boundary(target_shape) {
        return None;
    }
    let element_target =
        optional_element_slot_shape_with_facts(target_shape, target_element_ty.clone(), ctx.facts);
    let lowered = lower_expr_with_refinements(arg, env, ctx, &ExprRefinements::default());
    let clone_recovery = if target_element_ty == JavaType::String
        && actual_shape.ownership == OwnershipShape::SharedBorrow
    {
        CloneRecoveryDecision::NoRecovery
    } else {
        clone_recovery_decision_for_argument(arg, &element_target, Some(actual_shape), env, ctx)
    };
    let recovered = apply_clone_recovery_decision(lowered, clone_recovery);
    let wrapped =
        planned_optional_value_for_target(recovered, actual_shape.clone(), target_shape, ctx.facts);
    Some(normalize_owned_string_optional_argument(
        wrapped,
        target_shape,
    ))
}

fn normalize_owned_string_optional_argument(
    expr: RustExpr,
    target_shape: &RustSlotShape,
) -> RustExpr {
    if target_shape.ownership != OwnershipShape::Owned
        || target_shape.optional_element_type() != Some(JavaType::String)
    {
        return expr;
    }
    let rendered = emit::emit_expr(&expr);
    let Some(inner) = rendered
        .strip_prefix("Some(")
        .and_then(|inner| inner.strip_suffix(".as_str())"))
    else {
        return expr;
    };
    RustExpr::Raw(format!("Some({inner})"))
}

fn lower_optional_argument_for_borrowed_target(
    arg: &Expr,
    env: &ShapeEnv,
    target_shape: &RustSlotShape,
    actual_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if let Some(lowered) =
        lower_current_instance_field_for_borrowed_dyn_argument(arg, env, target_shape, ctx)
    {
        return Some(lowered);
    }
    if !matches!(
        target_shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        return None;
    }
    let target_element = target_shape
        .optional_element_type()
        .unwrap_or_else(|| target_shape.java_ty.clone());
    if let Some(actual_element) = actual_shape.optional_element_type() {
        if !same_concrete_java_type(&actual_element, &target_element)
            && !java_type_is_erased_object_name(&target_element)
        {
            return None;
        }
        let lowered = lower_expr_with_refinements(arg, env, ctx, &ExprRefinements::default());
        if optional_borrowed_expr_already_matches_target(arg, target_shape, env, ctx) {
            return Some(lowered);
        }
        let lowered_type = optional_element_borrow_lowered_type_with_facts(
            actual_shape.clone(),
            actual_element,
            None,
            ctx.facts,
        )?;
        let plan = plan_lowered_slot_conversion(
            &lowered_type,
            Slot::Argument(target_shape.clone()),
            ctx.facts,
        );
        return Some(apply_lowered_slot_expr_conversion(
            crate::coercion::LoweredExpr {
                expr: lowered,
                actual: plan.actual.clone(),
            },
            &plan,
        ));
    }
    if !java_type_is_erased_object_name(&target_element)
        && !same_concrete_java_type(&actual_shape.java_ty, &target_element)
    {
        return None;
    }
    if target_shape.optional_element_type().is_none()
        && !java_type_is_erased_object_name(&target_element)
    {
        return None;
    }
    if !matches!(arg, Expr::Literal(Literal::String(_))) {
        return None;
    }
    let lowered = lower_expr_with_refinements(arg, env, ctx, &ExprRefinements::default());
    match lowered {
        RustExpr::Raw(rendered) => Some(RustExpr::Raw(format!("Some(&{rendered})"))),
        _ => None,
    }
}

fn lower_current_instance_field_for_borrowed_dyn_argument(
    arg: &Expr,
    env: &ShapeEnv,
    target_shape: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if target_shape.ownership != OwnershipShape::SharedBorrow {
        return None;
    }
    let DispatchShape::DynTrait { trait_name } = &target_shape.dispatch else {
        return None;
    };
    let mut field_shape = current_instance_field_shape_with_env(arg, env, ctx)?;
    let field_element_ty = field_shape
        .optional_element_type()
        .unwrap_or_else(|| field_shape.java_ty.clone());
    if !same_concrete_java_type(&field_element_ty, &target_shape.java_ty) {
        return None;
    }
    if !matches!(
        field_shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: ref field_trait
        } if field_trait == trait_name
    ) && !object_safe_trait_argument_boundary(&target_shape.java_ty, ctx.facts)
    {
        return None;
    }
    field_shape.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: trait_name.clone(),
    };
    let lowered = lower_expr_with_refinements(arg, env, ctx, &ExprRefinements::default());
    Some(apply_expr_to_slot_via_plan(
        lowered,
        field_shape,
        &Slot::Argument(target_shape.clone()),
        ctx.facts,
    ))
}

fn object_safe_trait_argument_boundary(java_ty: &JavaType, facts: &FactSet) -> bool {
    match java_ty {
        JavaType::Named { name, args } if args.is_empty() => {
            facts.object_safe_trait_fact_for_name(name) == Some(true)
        }
        _ => false,
    }
}

fn optional_array_lowered_type_for_slice_slot(
    actual_shape: &RustSlotShape,
    target_shape: &RustSlotShape,
) -> Option<LoweredType> {
    let target_ty = crate::types::optional_element_type(&target_shape.java_ty)
        .unwrap_or_else(|| target_shape.java_ty.clone());
    if target_shape.view != ViewShape::Slice || !matches!(target_ty, JavaType::Array(_)) {
        return None;
    }
    let actual_element = actual_shape.optional_element_type()?;
    if !same_concrete_java_type(&actual_element, &target_ty) {
        return None;
    }
    optional_element_borrow_lowered_type(actual_shape.clone(), actual_element, None)
}

fn optional_element_borrow_lowered_type(
    option_shape: RustSlotShape,
    element_ty: JavaType,
    element_ownership: Option<OwnershipShape>,
) -> Option<LoweredType> {
    optional_element_borrow_lowered_type_with_facts(
        option_shape,
        element_ty,
        element_ownership,
        &FactSet::default(),
    )
}

fn optional_element_borrow_lowered_type_with_facts(
    option_shape: RustSlotShape,
    element_ty: JavaType,
    element_ownership: Option<OwnershipShape>,
    facts: &FactSet,
) -> Option<LoweredType> {
    LoweredType::optional_element_borrow_from_option_shape(
        option_shape,
        element_ty,
        element_ownership,
        facts,
    )
}

fn optional_element_borrow_lowered_expr(
    option_expr: RustExpr,
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    element_ownership: OwnershipShape,
) -> Option<crate::coercion::LoweredExpr> {
    let actual = optional_element_borrow_lowered_type(
        option_shape.clone(),
        element_ty,
        Some(element_ownership),
    )?;
    let expr = project_optional_element_borrow_carrier(option_expr, element_ownership)?;
    Some(crate::coercion::LoweredExpr { expr, actual })
}

fn optional_element_borrow_lowered_expr_with_facts(
    option_expr: RustExpr,
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    element_ownership: OwnershipShape,
    facts: &FactSet,
) -> Option<crate::coercion::LoweredExpr> {
    let actual = optional_element_borrow_lowered_type_with_facts(
        option_shape.clone(),
        element_ty,
        Some(element_ownership),
        facts,
    )?;
    let expr = project_optional_element_borrow_carrier(option_expr, element_ownership)?;
    Some(crate::coercion::LoweredExpr { expr, actual })
}

fn adapt_reused_owned_argument(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    if let Some(reborrowed) = reborrow_reused_optional_mut_borrow_argument(
        &lowered,
        source,
        target_shape,
        actual_shape,
        ctx,
    ) {
        return reborrowed;
    }
    if let Some(borrowed) =
        normalize_reused_local_clone_for_borrowed_argument(&lowered, source, target_shape)
    {
        return borrowed;
    }
    if let Some(adapted) = adapt_reused_owned_optional_argument(
        lowered.clone(),
        source,
        target_shape,
        actual_shape,
        env,
        ctx,
    ) {
        return adapted;
    }
    apply_clone_recovery_decision(
        lowered,
        clone_recovery_decision_for_argument(source, target_shape, actual_shape, env, ctx),
    )
}

fn adapt_reused_owned_optional_argument(
    lowered: RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if target_shape.optional_element_type().is_some() || !shape_is_owned(target_shape) {
        return None;
    }
    let actual_shape = optional_owned_clone_recovery_source_shape(source, actual_shape, env, ctx)?;
    let name = move_recovery_source_name(source)?;
    let move_use = MoveUseContext::new(ctx);
    if !move_use.has_later_or_repeated_current_read(name)
        && !move_use
            .binding_use_count(name)
            .is_some_and(|use_count| use_count > 1)
    {
        return None;
    }
    let element_ty = actual_shape.optional_element_type()?;
    let element_shape =
        optional_element_slot_shape_with_facts(&actual_shape, element_ty, ctx.facts);
    if !borrowed_element_target_matches_element(target_shape, &element_shape, ctx.facts) {
        return None;
    }
    let rendered = emit::emit_expr(&lowered);
    let cloned_unwrap = format!("{name}.clone().unwrap()");
    if rendered.contains(&cloned_unwrap) {
        if optional_shape_contains_erased_object(&actual_shape) {
            return Some(RustExpr::Raw(rendered.replacen(
                &cloned_unwrap,
                &format!(
                    "{name}.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
                ),
                1,
            )));
        }
        return Some(RustExpr::Raw(rendered.replacen(
            &cloned_unwrap,
            &format!(
                "{name}.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
            ),
            1,
        )));
    }
    let consuming_unwrap = format!("{name}.unwrap()");
    if rendered.contains(&consuming_unwrap) {
        if optional_shape_contains_erased_object(&actual_shape) {
            return Some(RustExpr::Raw(rendered.replacen(
                &consuming_unwrap,
                &format!(
                    "{name}.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
                ),
                1,
            )));
        }
        return Some(RustExpr::Raw(rendered.replacen(
            &consuming_unwrap,
            &format!(
                "{name}.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
            ),
            1,
        )));
    }
    if rendered.contains(".unwrap()") || rendered.contains(".expect(\"Java null receiver\")") {
        return Some(lowered);
    }
    let owned_element = if optional_shape_contains_erased_object(&actual_shape) {
        RustExpr::Raw(format!(
            "{rendered}.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        ))
    } else {
        RustExpr::Raw(format!(
            "{rendered}.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        ))
    };
    Some(apply_expr_to_slot_via_plan(
        owned_element,
        element_shape,
        &argument_slot(target_shape.clone()),
        ctx.facts,
    ))
}

fn optional_shape_contains_erased_object(shape: &RustSlotShape) -> bool {
    shape
        .optional_element_type()
        .as_ref()
        .is_some_and(java_type_is_erased_object_name)
        || matches!(shape.dispatch, DispatchShape::ErasedObject)
        || matches!(shape.view, ViewShape::ErasedObjectView)
}

fn move_recovery_source_name(source: &Expr) -> Option<&str> {
    match source {
        Expr::Var(name) => Some(name),
        Expr::Cast { expr, .. } => move_recovery_source_name(expr),
        _ => None,
    }
}

fn optional_owned_clone_recovery_source_shape(
    source: &Expr,
    actual_shape: Option<&RustSlotShape>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    if let Some(shape) = actual_shape
        && shape.optional_element_type().is_some()
        && shape.ownership == OwnershipShape::Owned
    {
        return Some(shape.clone());
    }
    if let Expr::Cast { expr, .. } = source {
        let shape = infer_expr_shape_with_context(
            expr,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
        );
        if shape.optional_element_type().is_some() && shape.ownership == OwnershipShape::Owned {
            return Some(shape);
        }
        return optional_owned_clone_recovery_source_shape(expr, None, env, ctx);
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CloneRecoveryDecision {
    NoRecovery,
    CloneOptionalBeforeConsumingUnwrap {
        name: String,
        erased_object: bool,
    },
    ApplyRecoveryCoercion {
        coercion: Coercion,
        idempotence: CloneRecoveryIdempotence,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloneRecoveryIdempotence {
    DirectClone,
    AnyCloneRecovery,
}

#[derive(Debug, Clone, Copy)]
struct MoveUseContext<'a> {
    ctx: LowerCtx<'a>,
}

impl<'a> MoveUseContext<'a> {
    fn new(ctx: LowerCtx<'a>) -> Self {
        Self { ctx }
    }

    fn has_later_read(&self, name: &str) -> bool {
        self.ctx
            .later_read_names
            .is_some_and(|later_reads| later_reads.contains(name))
    }

    fn has_catch_future_use(&self, name: &str) -> bool {
        self.ctx
            .catch_future_use_names
            .is_some_and(|future_uses| future_uses.contains(name))
    }

    fn has_exclusive_terminal_branch_read(&self, name: &str) -> bool {
        self.ctx
            .exclusive_terminal_branch_read_names
            .is_some_and(|exclusive_reads| exclusive_reads.contains(name))
    }

    fn current_stmt_read_count(&self, name: &str) -> Option<usize> {
        self.ctx
            .current_stmt_read_counts
            .and_then(|current_counts| current_counts.get(name).copied())
    }

    fn has_repeated_current_stmt_read(&self, name: &str) -> bool {
        self.current_stmt_read_count(name)
            .is_some_and(|count| count > 1)
    }

    fn has_later_or_repeated_current_read(&self, name: &str) -> bool {
        self.has_later_read(name) || self.has_repeated_current_stmt_read(name)
    }

    fn has_later_read_facts(&self) -> bool {
        self.ctx.later_read_names.is_some()
    }

    fn binding_use_count(&self, name: &str) -> Option<usize> {
        current_callable_binding_use_count(name, self.ctx)
    }
}

fn clone_recovery_decision_for_argument(
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> CloneRecoveryDecision {
    if let Some(optional_actual) =
        reused_actual_optional_owned_argument_needs_clone(source, actual_shape, env, ctx)
    {
        if let Some(name) = move_recovery_source_name(source) {
            return CloneRecoveryDecision::CloneOptionalBeforeConsumingUnwrap {
                name: name.to_string(),
                erased_object: optional_shape_contains_erased_object(&optional_actual),
            };
        }
    }
    if matches!(
        target_shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        return CloneRecoveryDecision::NoRecovery;
    }
    if shared_borrow_argument_needs_owned_clone(target_shape, actual_shape, ctx) {
        return CloneRecoveryDecision::ApplyRecoveryCoercion {
            coercion: Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
            idempotence: CloneRecoveryIdempotence::DirectClone,
        };
    }
    if reused_owned_argument_needs_clone(source, target_shape, actual_shape, ctx) {
        return CloneRecoveryDecision::ApplyRecoveryCoercion {
            coercion: Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
            idempotence: CloneRecoveryIdempotence::AnyCloneRecovery,
        };
    }
    CloneRecoveryDecision::NoRecovery
}

fn reused_actual_optional_owned_argument_needs_clone(
    arg: &Expr,
    actual_shape: Option<&RustSlotShape>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let actual_shape = optional_owned_clone_recovery_source_shape(arg, actual_shape, env, ctx)?;
    if !optional_owned_argument_clone_recovery(&actual_shape, ctx.facts) {
        return None;
    }
    let name = move_recovery_source_name(arg)?;
    MoveUseContext::new(ctx)
        .has_later_or_repeated_current_read(name)
        .then_some(actual_shape)
}

fn audited_clone_recovery_decision(idempotence: CloneRecoveryIdempotence) -> CloneRecoveryDecision {
    CloneRecoveryDecision::ApplyRecoveryCoercion {
        coercion: Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::AuditRequired,
        },
        idempotence,
    }
}

fn apply_clone_recovery_decision(lowered: RustExpr, decision: CloneRecoveryDecision) -> RustExpr {
    if let CloneRecoveryDecision::CloneOptionalBeforeConsumingUnwrap {
        name,
        erased_object,
    } = decision
    {
        let rendered = emit::emit_expr(&lowered);
        let consuming_unwrap = format!("{name}.unwrap()");
        if rendered.contains(&consuming_unwrap) {
            let replacement = if erased_object {
                format!(
                    "{name}.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
                )
            } else {
                format!(
                    "{name}.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
                )
            };
            return RustExpr::Raw(rendered.replacen(&consuming_unwrap, &replacement, 1));
        }
        return apply_audited_clone_recovery(lowered);
    }
    let CloneRecoveryDecision::ApplyRecoveryCoercion {
        coercion,
        idempotence,
    } = decision
    else {
        return lowered;
    };
    let rendered = emit::emit_expr(&lowered);
    if clone_recovery_idempotence_matches(&rendered, idempotence) {
        return lowered;
    }
    apply_coercions(lowered, &[coercion])
}

fn clone_recovery_idempotence_matches(
    rendered: &str,
    idempotence: CloneRecoveryIdempotence,
) -> bool {
    match idempotence {
        CloneRecoveryIdempotence::DirectClone => rendered_expr_has_direct_clone_recovery(rendered),
        CloneRecoveryIdempotence::AnyCloneRecovery => rendered_expr_has_clone_recovery(rendered),
    }
}

fn reborrow_reused_optional_mut_borrow_argument(
    lowered: &RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
    actual_shape: Option<&RustSlotShape>,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(name) = source else {
        return None;
    };
    if target_shape.ownership != OwnershipShape::MutableBorrow
        || target_shape.optional_element_type().is_none()
        || !actual_shape.is_some_and(|actual_shape| {
            actual_shape.ownership == OwnershipShape::MutableBorrow
                && actual_shape.optional_element_type() == target_shape.optional_element_type()
        })
        || !current_callable_binding_has_later_read(name, ctx)
    {
        return None;
    }
    reborrow_optional_mut_projection(lowered)
}

fn normalize_reused_local_clone_for_borrowed_argument(
    lowered: &RustExpr,
    source: &Expr,
    target_shape: &RustSlotShape,
) -> Option<RustExpr> {
    let Expr::Var(name) = source else {
        return None;
    };
    normalize_reused_clone_for_borrowed_argument(lowered, name, target_shape)
}

fn adapt_reused_owned_value_for_slot(
    lowered: RustExpr,
    source: &Expr,
    slot: &Slot,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let Some(target_shape) = slot_target_shape(slot) else {
        return lowered;
    };
    apply_clone_recovery_decision(
        lowered,
        clone_recovery_decision_for_value(source, target_shape, env, slot, ctx),
    )
}

fn clone_recovery_decision_for_value(
    source: &Expr,
    target_shape: &RustSlotShape,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
) -> CloneRecoveryDecision {
    if reused_owned_value_needs_clone(source, target_shape, env, slot, ctx) {
        CloneRecoveryDecision::ApplyRecoveryCoercion {
            coercion: Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
            idempotence: CloneRecoveryIdempotence::AnyCloneRecovery,
        }
    } else {
        CloneRecoveryDecision::NoRecovery
    }
}

fn reused_owned_value_needs_clone(
    source: &Expr,
    target_shape: &RustSlotShape,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
) -> bool {
    let Expr::Var(name) = source else {
        return false;
    };
    let move_use = MoveUseContext::new(ctx);
    if !shape_is_owned_non_erased(target_shape) {
        return false;
    }
    let actual_shape = actual_shape_for_slot_expr_with_slot(
        source,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
        slot,
    );
    if actual_shape.ownership == OwnershipShape::SharedBorrow
        || actual_shape.java_ty != target_shape.java_ty
        || matches!(actual_shape.java_ty, JavaType::Primitive(_))
    {
        return false;
    }
    if !known_owned_argument_clone_recovery(target_shape)
        && !known_named_clone_recovery(target_shape)
        && !known_owned_argument_clone_recovery_with_facts(target_shape, ctx.facts)
        && !known_owned_argument_clone_recovery_with_facts(&actual_shape, ctx.facts)
    {
        return false;
    }
    if move_use.has_later_read(name) {
        return true;
    }
    move_use.has_repeated_current_stmt_read(name)
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
        let move_use = MoveUseContext::new(ctx);
        return optional_owned_argument_clone_recovery(target_shape, ctx.facts)
            && matches!(arg, Expr::Var(_))
            && move_use.has_later_or_repeated_current_read(match arg {
                Expr::Var(name) => name,
                _ => unreachable!("checked above"),
            });
    }
    if actual_shape
        .is_some_and(|actual_shape| actual_shape.ownership == OwnershipShape::SharedBorrow)
    {
        return false;
    }
    if (shape_is_erased_object_sequence_like(target_shape)
        || actual_shape.is_some_and(shape_is_erased_object_sequence_like))
        && target_shape.ownership == OwnershipShape::Owned
    {
        let Expr::Var(name) = arg else {
            return false;
        };
        let move_use = MoveUseContext::new(ctx);
        if move_use.has_catch_future_use(name) {
            return true;
        }
        return move_use.has_later_or_repeated_current_read(name);
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
    let move_use = MoveUseContext::new(ctx);
    if move_use.has_catch_future_use(name) {
        return true;
    }
    if move_use.has_exclusive_terminal_branch_read(name) && !move_use.has_later_read(name) {
        return false;
    }
    if move_use.has_later_read_facts() {
        return move_use.has_later_or_repeated_current_read(name);
    }
    if shape_is_erased_object_sequence_like(target_shape) {
        return move_use.has_repeated_current_stmt_read(name);
    }
    if let Some(current_count) = move_use.current_stmt_read_count(name) {
        return current_count > 1;
    }
    move_use
        .binding_use_count(name)
        .is_some_and(|use_count| use_count > 1)
}

fn shape_is_erased_object_sequence_like(shape: &RustSlotShape) -> bool {
    java_type_is_erased_object_sequence_like(&shape.java_ty)
        || shape.sequence_element_type().is_some_and(|element_ty| {
            java_type_is_erased_object_name(&element_ty)
                || java_type_is_erased_object_sequence_like(&element_ty)
        })
}

fn java_type_is_erased_object_sequence_like(ty: &JavaType) -> bool {
    crate::types::sequence_element_type(ty).is_some_and(|element_ty| {
        java_type_is_erased_object_name(&element_ty)
            || java_type_is_erased_object_sequence_like(&element_ty)
    })
}

fn known_owned_argument_clone_recovery(target_shape: &RustSlotShape) -> bool {
    matches!(target_shape.java_ty, JavaType::String)
}

fn optional_owned_argument_clone_recovery(target_shape: &RustSlotShape, facts: &FactSet) -> bool {
    if optional_shape_contains_erased_object(target_shape) {
        return true;
    }
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

fn string_pattern_compat_argument_plan(arg_count: usize) -> ArgumentLoweringPlan {
    ArgumentLoweringPlan::Shaped(vec![borrowed_string_content_shape(); arg_count])
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

fn lower_string_join_args(args: &[Expr], env: &ShapeEnv, ctx: LowerCtx) -> Vec<RustExpr> {
    let Some((delimiter, values)) = args.split_first() else {
        return Vec::new();
    };
    let mut lowered = Vec::with_capacity(args.len());
    lowered.push(lower_expr_for_slot(
        delimiter,
        env,
        argument_slot(borrowed_string_content_shape()),
        ctx,
    ));
    lowered.extend(values.iter().map(|arg| {
        lower_expr_for_slot(
            arg,
            env,
            argument_slot(borrowed_string_content_shape()),
            ctx,
        )
    }));
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
    if let Some(cast_receiver) =
        lower_explicit_external_facade_cast_receiver(target, receiver, env, ctx, refinements)
    {
        return cast_receiver;
    }
    if let Some(lowered) =
        adapt_external_facade_receiver_slot(target, receiver, lowered.clone(), env, ctx)
    {
        return lowered;
    }
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
            adapt_optional_instance_receiver(receiver, lowered, env, ctx)
        }
        _ => lowered,
    }
}

fn adapt_external_facade_receiver_slot(
    target: &CallTargetPlan<'_>,
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let CallTargetPlan::ExternalFacadeReceiver { receiver_abi, .. } = target else {
        return None;
    };
    let actual_shape = infer_expr_shape_with_context(
        receiver,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let receiver_slot_ty = actual_shape
        .optional_element_type()
        .unwrap_or_else(|| actual_shape.java_ty.clone());
    let slot_plan = facade_receiver_slot_plan(*receiver_abi, receiver_slot_ty, ctx.facts);
    let slot = slot_plan.slot?;
    let lowered = crate::coercion::LoweredExpr {
        expr: lowered,
        actual: LoweredType::Slot(Box::new(actual_shape)),
    };
    let plan = plan_lowered_slot_conversion(&lowered.actual, slot, ctx.facts);
    if receiver_method_autoref_satisfies_facade_slot(*receiver_abi, &plan) {
        return None;
    }
    Some(parenthesize_borrowed_method_receiver(
        apply_lowered_slot_expr_conversion(lowered, &plan),
    ))
}

fn adapt_call_target_receiver_with_signature(
    target: &CallTargetPlan<'_>,
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
    fallback_signature: Option<&crate::signature::MethodSignaturePlan>,
    context_fallback_signature: Option<&ContextFallbackMethodSignature>,
) -> RustExpr {
    if let Some(cast_receiver) =
        lower_explicit_external_facade_cast_receiver(target, receiver, env, ctx, refinements)
    {
        return cast_receiver;
    }
    if let Some(lowered) =
        adapt_external_facade_receiver_slot(target, receiver, lowered.clone(), env, ctx)
    {
        return lowered;
    }
    if refined_non_null_receiver(receiver, refinements) {
        if target.adapts_optional_instance_receiver() {
            let target_method = target.rust_method_name();
            let mutable = fallback_signature
                .is_some_and(|signature| matches!(signature.receiver, ReceiverPlan::Mutable))
                || context_fallback_signature
                    .is_some_and(|signature| matches!(signature.receiver, ReceiverPlan::Mutable))
                || target_method.is_some_and(|method| {
                    optional_receiver_method_requires_mutable_signature(receiver, method, env, ctx)
                })
                || target.mutates_receiver();
            let lowered =
                lower_expr_with_refinements(receiver, env, ctx, &ExprRefinements::default());
            return adapt_optional_instance_receiver_with_slot_plan(
                receiver, lowered, env, ctx, mutable, true,
            );
        }
        return lowered;
    }
    if target.adapts_optional_instance_receiver()
        && let Some(method) = target.rust_method_name()
        && optional_receiver_method_requires_mutable_signature(receiver, method, env, ctx)
    {
        return adapt_optional_instance_receiver_with_slot_plan(
            receiver, lowered, env, ctx, true, true,
        );
    }
    if target.adapts_optional_instance_receiver()
        && (fallback_signature
            .is_some_and(|signature| matches!(signature.receiver, ReceiverPlan::Mutable))
            || context_fallback_signature
                .is_some_and(|signature| matches!(signature.receiver, ReceiverPlan::Mutable)))
    {
        return adapt_optional_instance_receiver_with_slot_plan(
            receiver, lowered, env, ctx, true, true,
        );
    }
    if target.adapts_optional_instance_receiver() && target.mutates_receiver() {
        return adapt_optional_instance_receiver_with_slot_plan(
            receiver, lowered, env, ctx, true, true,
        );
    }
    adapt_call_target_receiver(target, receiver, lowered, env, ctx, refinements)
}

fn optional_receiver_method_requires_mutable_signature(
    receiver: &Expr,
    rust_method: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let Some(receiver_ty) = receiver_shape.optional_element_type() else {
        return false;
    };
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = receiver_ty else {
        return false;
    };
    let mut candidates = vec![
        name.clone(),
        crate::slot_render::rust_name_for_java_named_type(&name),
    ];
    for nested in nested_type_owner_candidates_for_receiver(&name, ctx) {
        candidates.push(nested.clone());
        candidates.push(crate::slot_render::rust_name_for_java_named_type(&nested));
    }
    candidates.into_iter().any(|candidate| {
        ctx.signatures
            .method_signature_by_rust_name(&candidate, rust_method)
            .or_else(|| {
                ctx.signatures
                    .method_signature_by_base_name_and_arity(&candidate, rust_method, 0)
                    .map(|(_, signature)| signature)
            })
            .is_some_and(|signature| matches!(signature.receiver, ReceiverPlan::Mutable))
    })
}

fn lower_explicit_external_facade_cast_receiver(
    target: &CallTargetPlan<'_>,
    receiver: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if !matches!(target, CallTargetPlan::ExternalFacadeReceiver { .. }) {
        return None;
    }
    let Expr::Cast { ty, expr: inner } = receiver else {
        return None;
    };
    let cast_type = concrete_java_type_name(ty)?;
    let cast_type = crate::stdlib::canonical_external_receiver_type_key(cast_type);
    let rust_type = crate::slot_render::rust_name_for_java_named_type(&cast_type);
    let lowered_inner = lower_expr_with_refinements(inner, env, ctx, refinements);
    Some(explicit_java_facade_cast_placeholder_projection(
        lowered_inner,
        &rust_type,
    ))
}

fn lower_jdk_input_stream_super_read_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if !matches!(receiver, Some(Expr::Super)) || name != "read" || args.len() != 3 {
        return None;
    }
    if !current_type_extends_jdk_input_stream_facade(ctx) {
        return None;
    }
    let synthetic_receiver = Expr::FieldAccess(Box::new(Expr::This), "in".to_string());
    let target = plan_call_expr_with_lower_context(
        Some(&synthetic_receiver),
        "read_3",
        args,
        None,
        ctx,
        env,
    );
    let CallExprPlan::Target(target) = target else {
        return None;
    };
    let arg_plan = plan_call_arguments_with_facts_and_args(
        Some(&synthetic_receiver),
        "read_3",
        None,
        args,
        ctx.signatures,
        ctx.facts,
        env.type_env(),
    );
    let rust_args = lower_args_for_call_target(
        &target,
        Some(&synthetic_receiver),
        args,
        env,
        arg_plan,
        None,
        ctx,
        None,
    );
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, &ExprRefinements::default());
        adapt_call_target_receiver(
            &target,
            receiver,
            lowered,
            env,
            ctx,
            &ExprRefinements::default(),
        )
    });
    Some(propagate_result_projection(
        apply_call_target_with_lower_context(&target, lowered_receiver, rust_args, ctx),
    ))
}

fn lower_source_output_stream_write_byte_array_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if name != "write" || args.len() != 1 {
        return None;
    }
    let receiver = receiver?;
    let receiver_name = source_receiver_type_name(receiver, env, ctx)?;
    let signature = ctx
        .signatures
        .method_signature_by_rust_name(&receiver_name, "write_byte_array_int_int")?;
    if signature.params.len() != 3 || !java_type_is_byte_array_like(&signature.params[0].java_ty) {
        return None;
    }
    if !expr_is_byte_array_like(&args[0], env, ctx) {
        return None;
    }
    let byte_slot = Slot::Argument(signature.params[0].shape.clone());
    let lowered_bytes =
        lower_expr_for_slot_with_refinements(&args[0], env, byte_slot, ctx, refinements);
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    let lowered_receiver = adapt_optional_instance_receiver_with_slot_plan(
        receiver,
        lowered_receiver,
        env,
        ctx,
        true,
        true,
    );
    let receiver_text = emit::emit_expr(&lowered_receiver);
    let bytes_text = emit::emit_expr(&lowered_bytes);
    Some(RustExpr::Raw(format!(
        "{{ let __java_write_bytes = {bytes_text}; let __java_write_len = __java_write_bytes.len() as i32; {receiver_text}.write_byte_array_int_int(__java_write_bytes, 0, __java_write_len) }}"
    )))
}

fn lower_source_input_stream_read_byte_array_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if name != "read" || args.len() != 1 {
        return None;
    }
    let receiver = receiver?;
    let receiver_name = source_receiver_type_name(receiver, env, ctx)?;
    let (read_method, byte_shape) = ctx
        .signatures
        .method_signature_by_rust_name(&receiver_name, "read_3")
        .map(|signature| ("read_3".to_string(), signature.params[0].shape.clone()))
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(&receiver_name, "read", 3)
                .map(|(rust_name, signature)| (rust_name, signature.params[0].shape.clone()))
        })
        .or_else(|| {
            java_platform_input_stream_read_3_byte_slot(&receiver_name)
                .map(|shape| ("read_3".to_string(), shape))
        })?;
    if !java_type_is_byte_array_like(&byte_shape.java_ty) {
        return None;
    }
    if !expr_is_byte_array_like(&args[0], env, ctx) {
        return None;
    }
    let byte_slot = Slot::Argument(byte_shape);
    let lowered_bytes =
        lower_expr_for_slot_with_refinements(&args[0], env, byte_slot, ctx, refinements);
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    let lowered_receiver = adapt_optional_instance_receiver_with_slot_plan(
        receiver,
        lowered_receiver,
        env,
        ctx,
        false,
        true,
    );
    let receiver_text = emit::emit_expr(&lowered_receiver);
    let bytes_text = emit::emit_expr(&lowered_bytes);
    Some(RustExpr::Raw(format!(
        "{{ let __java_read_bytes = {bytes_text}; let __java_read_len = __java_read_bytes.len() as i32; {receiver_text}.{read_method}(__java_read_bytes, 0, __java_read_len)? }}"
    )))
}

fn java_type_is_byte_array_like(java_ty: &JavaType) -> bool {
    let java_ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
    matches!(
        java_ty,
        JavaType::Array(element)
            if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
    )
}

fn expr_is_byte_array_like(expr: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    if java_type_is_byte_array_like(&types::infer_expr_type(expr, env.type_env())) {
        return true;
    }
    let shape = actual_shape_for_slot_expr(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    java_type_is_byte_array_like(&shape.java_ty)
}

fn java_platform_input_stream_read_3_byte_slot(receiver_name: &str) -> Option<RustSlotShape> {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if !matches!(
        canonical.as_str(),
        "InputStream"
            | "java.io.InputStream"
            | "FileInputStream"
            | "java.io.FileInputStream"
            | "ByteArrayInputStream"
            | "java.io.ByteArrayInputStream"
            | "BufferedInputStream"
            | "java.io.BufferedInputStream"
            | "DataInputStream"
            | "java.io.DataInputStream"
            | "GZIPInputStream"
            | "java.util.zip.GZIPInputStream"
            | "ObjectInputStream"
            | "java.io.ObjectInputStream"
            | "FilterInputStream"
            | "java.io.FilterInputStream"
    ) {
        return None;
    }
    let mut shape = RustSlotShape::for_storage(JavaType::Array(Box::new(JavaType::Primitive(
        PrimitiveType::Byte,
    ))));
    shape.ownership = OwnershipShape::MutableBorrow;
    shape.mutability = MutabilityShape::Mutable;
    shape.view = ViewShape::Slice;
    Some(shape)
}

fn current_type_extends_jdk_input_stream_facade(ctx: LowerCtx) -> bool {
    let Some(current_type) = ctx.current_type else {
        return false;
    };
    let Some(decl) = find_context_type_by_key(ctx.program, current_type)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, current_type))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, current_type))
    else {
        return false;
    };
    decl.extends.as_deref().is_some_and(|extends| {
        matches!(
            crate::stdlib::canonical_external_receiver_type_key(extends).as_str(),
            "InputStream"
                | "FilterInputStream"
                | "GZIPInputStream"
                | "java.io.InputStream"
                | "java.io.FilterInputStream"
                | "java.util.zip.GZIPInputStream"
        )
    })
}

fn refined_non_null_receiver(receiver: &Expr, refinements: &ExprRefinements) -> bool {
    matches!(receiver, Expr::Var(name) if refinements.non_null_locals.contains(name))
}

fn optional_borrowed_expr_already_matches_target(
    expr: &Expr,
    target_shape: &RustSlotShape,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    if target_shape.ownership != OwnershipShape::SharedBorrow {
        return false;
    }
    let Some(target_element_ty) = target_shape.optional_element_type() else {
        return false;
    };
    let Some(LoweredType::OptionalElementBorrow { element_shape, .. }) =
        native_collection_optional_element_borrow_lowered_type(expr, env, ctx)
    else {
        return false;
    };
    if !same_concrete_java_type(&element_shape.java_ty, &target_element_ty) {
        return false;
    }
    let target_element =
        crate::conversion_plan::optional_element_slot_shape(target_shape, target_element_ty);
    target_element.ownership == element_shape.ownership
        && target_element.view == element_shape.view
        && target_element.dispatch == element_shape.dispatch
}

fn native_collection_optional_element_borrow_lowered_type(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<LoweredType> {
    let Expr::Call {
        receiver: call_receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
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
        return None;
    };
    let lowered = native_collection_lowered_type(receiver, method, env.type_env())?;
    matches!(lowered, LoweredType::OptionalElementBorrow { .. }).then_some(lowered)
}

fn adapt_optional_instance_receiver(
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    adapt_optional_instance_receiver_with_slot_plan(receiver, lowered, env, ctx, false, true)
}

fn retarget_static_owner_call_for_instance_field_receiver<'a>(
    target: CallTargetPlan<'a>,
    receiver: Option<&'a Expr>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> CallTargetPlan<'a> {
    let should_retarget =
        receiver.is_some_and(|receiver| receiver_is_instance_or_outer_field(receiver, env, ctx));
    retarget_static_call_to_receiver_if(target, receiver, should_retarget)
}

fn retarget_java_logging_facade_call<'a>(
    target: CallTargetPlan<'a>,
    receiver: Option<&'a Expr>,
    name: &str,
) -> CallTargetPlan<'a> {
    let Some(receiver) = receiver else {
        return target;
    };
    if !java_logging_receiver_name(receiver).is_some_and(|receiver_name| {
        matches!(
            crate::stdlib::canonical_external_receiver_type_key(receiver_name).as_str(),
            "logger" | "Logger"
        )
    }) || !matches!(name, "info" | "warn" | "debug" | "error" | "trace")
    {
        return target;
    }
    CallTargetPlan::StaticOwner {
        owner: "logger".to_string(),
        rust_name: crate::parser::to_snake_case(name),
    }
}

fn java_logging_receiver_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(receiver_name) => Some(receiver_name),
        Expr::FieldAccess(_, field) => Some(field),
        _ => None,
    }
}

fn receiver_is_instance_or_outer_field(receiver: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    if current_instance_field_shape_with_env(receiver, env, ctx).is_some() {
        return true;
    }
    let Expr::Var(name) = receiver else {
        return false;
    };
    env.field_type(name).is_some() || lower_outer_this_field_name(name, env, ctx).is_some()
}

fn adapt_optional_instance_receiver_with_slot_plan(
    receiver: &Expr,
    lowered: RustExpr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    mutable: bool,
    legacy_unchecked: bool,
) -> RustExpr {
    if let Some(LoweredType::OptionalElementBorrow { element_shape, .. }) =
        native_collection_optional_element_borrow_lowered_type(receiver, env, ctx)
    {
        let plan =
            optional_borrowed_receiver_slot_plan(element_shape.java_ty.clone(), mutable, ctx.facts);
        let projected = project_optional_receiver(lowered, plan.projection);
        let projected = crate::coercion::LoweredExpr {
            expr: projected,
            actual: LoweredType::Slot(element_shape),
        };
        let conversion_plan = plan_lowered_slot_conversion(&projected.actual, plan.slot, ctx.facts);
        if receiver_method_autoref_satisfies_optional_slot(&conversion_plan) {
            return projected.expr;
        }
        return apply_lowered_slot_expr_conversion(projected, &conversion_plan);
    }

    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let inferred_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let mut receiver_shapes = vec![inferred_shape];
    if let Some(field_shape) = current_instance_field_shape_with_env(receiver, env, ctx) {
        receiver_shapes.push(field_shape);
    }
    let Some(receiver_ty) = receiver_shapes
        .iter()
        .find_map(RustSlotShape::optional_element_type)
    else {
        return lowered;
    };
    let option_shape = receiver_shapes
        .into_iter()
        .find(|shape| {
            shape
                .optional_element_type()
                .is_some_and(|ty| ty == receiver_ty)
        })
        .expect("receiver type was selected from receiver_shapes");
    let plan = optional_receiver_slot_plan(receiver_ty, mutable, legacy_unchecked, ctx.facts);
    let projected = project_optional_receiver(lowered, plan.projection);
    let Some(actual) = plan.optional_element_borrow_type(option_shape) else {
        return projected;
    };
    let LoweredType::OptionalElementBorrow { element_shape, .. } = actual else {
        return projected;
    };
    let projected = crate::coercion::LoweredExpr {
        expr: projected,
        actual: LoweredType::Slot(element_shape),
    };
    let conversion_plan = plan_lowered_slot_conversion(&projected.actual, plan.slot, ctx.facts);
    if receiver_method_autoref_satisfies_optional_slot(&conversion_plan) {
        return projected.expr;
    }
    apply_lowered_slot_expr_conversion(projected, &conversion_plan)
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
    let mutable = match method {
        NativeCollectionMethod::ListPush
        | NativeCollectionMethod::SetInsert
        | NativeCollectionMethod::Extend
        | NativeCollectionMethod::MapInsert => true,
        NativeCollectionMethod::Len
        | NativeCollectionMethod::IsEmpty
        | NativeCollectionMethod::ListHashCode
        | NativeCollectionMethod::ListGet
        | NativeCollectionMethod::ListIndexOf
        | NativeCollectionMethod::SetHashCode
        | NativeCollectionMethod::MapHashCode
        | NativeCollectionMethod::MapGet
        | NativeCollectionMethod::MapContainsKey
        | NativeCollectionMethod::MapKeys
        | NativeCollectionMethod::MapValues
        | NativeCollectionMethod::MapIter
        | NativeCollectionMethod::Iterator
        | NativeCollectionMethod::ToArray => false,
    };
    adapt_optional_instance_receiver_with_slot_plan(receiver, lowered, env, ctx, mutable, true)
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
    lower_java_equality_call_expr_for_op(
        BinaryOp::Eq,
        receiver,
        name,
        args,
        resolved,
        env,
        ctx,
        refinements,
    )
}

fn lower_negated_java_equality_call_expr(
    expr: &Expr,
    env: &ShapeEnv,
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
    lower_java_equality_call_expr_for_op(
        BinaryOp::Ne,
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        env,
        ctx,
        refinements,
    )
}

fn lower_java_equality_call_expr_for_op(
    op: BinaryOp,
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
    let left_shape = source_java_enum_field_accessor_shape_for_expr(left, env, ctx)
        .unwrap_or_else(|| crate::expr_shape::infer_expr_shape_with_context(left, env, shape_ctx));
    let right_shape = source_java_enum_field_accessor_shape_for_expr(right, env, ctx)
        .unwrap_or_else(|| crate::expr_shape::infer_expr_shape_with_context(right, env, shape_ctx));
    let plan = if use_fact_aware_plan {
        plan_equality_comparison_with_facts(&left_shape, &right_shape, ctx.facts)
    } else {
        plan_equality_comparison(&left_shape, &right_shape)
    };
    Some(lower_equality_comparison_expr_for_op(
        op,
        plan,
        lower_expr_with_refinements(left, env, ctx, refinements),
        lower_expr_with_refinements(right, env, ctx, refinements),
    ))
}

fn lower_java_objects_null_predicate_call_expr(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    if args.len() != 1 {
        return None;
    }
    let target = match plan_call_expr_with_lower_context(receiver, name, args, None, ctx, env) {
        CallExprPlan::Target(target) => target,
        CallExprPlan::TrivialAccessorGetter { .. } => return None,
    };
    let is_non_null = match target {
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::ObjectsNonNull,
        } => true,
        CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::ObjectsIsNull,
        } => false,
        _ => return None,
    };
    let arg = &args[0];
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_shape = infer_expr_shape_with_context(arg, env, shape_ctx);
    if arg_shape.optional_element_type().is_some() {
        let lowered = lower_expr_with_refinements(arg, env, ctx, refinements);
        let method = if is_non_null { "is_some" } else { "is_none" };
        return Some(receiver_method_call_projection_exact(
            lowered,
            method,
            Vec::new(),
        ));
    }
    Some(RustExpr::Literal(
        if is_non_null { "true" } else { "false" }.to_string(),
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
    typed_none_projection(&rust_ty)
}

fn lower_anonymous_class_new_for_boxed_dyn_slot(
    expr: &Expr,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::New {
        ty,
        args,
        body: Some(_),
        ..
    } = expr
    else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let target = slot_target_shape(slot)?;
    let trait_name = match &target.dispatch {
        DispatchShape::BoxedDynTrait { trait_name } | DispatchShape::DynTrait { trait_name } => {
            trait_name
        }
        _ => return None,
    };
    let constructed_name = concrete_java_type_name(ty)?;
    let constructed_trait = anonymous_class_trait_name(constructed_name, ctx)?;
    let target_trait = concrete_java_type_name(&target.java_ty)
        .map(crate::slot_render::rust_name_for_java_named_type);
    if *trait_name != constructed_trait
        && target_trait.as_ref() != Some(trait_name)
        && !trait_name.ends_with(&constructed_trait)
    {
        return None;
    }
    Some(anonymous_dyn_trait_projection(trait_name))
}

fn lower_anonymous_interface_new_expr(
    ty: &JavaType,
    args: &[Expr],
    body: Option<&Vec<Member>>,
    resolved: Option<&ResolvedConstructor>,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if !args.is_empty() {
        return None;
    }
    if let Some(resolved) = resolved
        && resolved.rust_name == "anonymous"
    {
        let trait_name =
            crate::slot_render::rust_name_for_java_named_type(&resolved.declaring_type);
        return Some(anonymous_dyn_trait_projection(&trait_name));
    }
    body?;
    let constructed_name = concrete_java_type_name(ty)?;
    let trait_name = anonymous_class_trait_name(constructed_name, ctx)?;
    Some(anonymous_dyn_trait_projection(&trait_name))
}

fn anonymous_class_trait_name(constructed_name: &str, ctx: LowerCtx) -> Option<String> {
    lower_find_type_decl(ctx.program, constructed_name)
        .or_else(|| lower_find_type_decl(ctx.support_local_type_program, constructed_name))
        .filter(|ty| ty.kind == TypeKind::Interface)
        .map(|ty| crate::slot_render::rust_name_for_java_named_type(&ty.name))
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
    if let Expr::Var(name) = expr
        && env.static_storage_field_access_name(name).is_some()
        && slot_expects_optional(&slot)
        && static_storage_field_shape(name, env, ctx)
            .is_some_and(|shape| shape.optional_element_type().is_some())
    {
        return lower_expr_with_refinements(expr, env, ctx, refinements);
    }
    if let Some(expr) = lower_literal_for_slot(expr, &slot) {
        return expr;
    }
    if let Some(expr) = lower_static_primitive_constant_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) = lower_anonymous_class_new_for_boxed_dyn_slot(expr, &slot, ctx) {
        return expr;
    }
    if let Some(expr) = lower_static_initialized_singleton_field_for_slot(expr, &slot, ctx) {
        return expr;
    }
    if let Some(expr) = lower_static_final_const_array_clone_for_slot(expr, &slot, env, ctx) {
        return expr;
    }
    if let Some(expr) = lower_static_final_const_array_for_slice_slot(expr, &slot, env, ctx) {
        return expr;
    }
    if let Some(expr) = lower_java_enum_static_field_access_for_slot(expr, env, &slot, ctx) {
        return expr;
    }
    if let Some(expr) =
        lower_source_java_enum_field_accessor_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) =
        lower_typed_collection_value_cast_source_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) = lower_erased_object_cast_for_erased_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) =
        lower_target_typed_external_call_cast_source_for_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(expr) = lower_erased_external_call_cast_for_slot(expr, env, &slot, ctx, refinements)
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
    if let Some(expr) = lower_closed_enum_cast_for_matching_slot(expr, env, &slot, ctx, refinements)
    {
        return expr;
    }
    if let Some(stream_expr) = lower_stream_expr(expr, env, ctx) {
        let plan = plan_lowered_slot_conversion(&stream_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(stream_expr, &plan);
    }
    if let Some(graphics_expr) =
        lower_create_graphics_expr_for_graphics_slot(expr, env, &slot, ctx, refinements)
    {
        return graphics_expr;
    }
    if let Some(resource_stream_expr) =
        lower_get_resource_as_stream_expr_for_optional_slot(expr, env, &slot, ctx, refinements)
    {
        return resource_stream_expr;
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
    if let Some(map_get_to_array_expr) =
        lower_map_get_to_array_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return map_get_to_array_expr;
    }
    if let Some(map_entry_expr) =
        lower_map_entry_accessor_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return map_entry_expr;
    }
    if let Some(map_keys_expr) =
        lower_map_key_set_expr_for_set_slot(expr, env, &slot, ctx, refinements)
    {
        return map_keys_expr;
    }
    if let Some(map_values_expr) =
        lower_map_values_expr_for_set_slot(expr, env, &slot, ctx, refinements)
    {
        return map_values_expr;
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
    if let Some(object_to_string_expr) =
        lower_java_object_to_string_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&object_to_string_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(object_to_string_expr, &plan);
    }
    if let Some(source_call_expr) =
        lower_source_receiver_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        let plan = plan_lowered_slot_conversion(&source_call_expr.actual, slot, ctx.facts);
        return apply_lowered_slot_expr_conversion(source_call_expr, &plan);
    }
    if let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
        && let Some(read_expr) = lower_source_input_stream_read_byte_array_call(
            receiver.as_deref(),
            name,
            args,
            env,
            ctx,
            refinements,
        )
    {
        return read_expr;
    }
    if let Some(external_expr) =
        lower_exact_generic_external_call_for_slot(expr, env, &slot, ctx, refinements)
    {
        return external_expr;
    }
    if let Some(mut external_expr) =
        lower_external_call_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        if slot_expr_needs_result_propagation(expr, env, ctx) {
            external_expr.expr = propagate_result_projection(external_expr.expr);
        }
        let plan = plan_lowered_slot_conversion(&external_expr.actual, slot, ctx.facts);
        let lowered = apply_lowered_slot_expr_conversion(external_expr, &plan);
        return if expr_is_object_input_stream_read_object_call(expr) {
            rewrite_deserialization_downcast_message(lowered)
        } else {
            lowered
        };
    }
    if let Some(constructor_expr) =
        lower_external_crate_constructor_expr_for_slot(expr, env, &slot, ctx, refinements)
    {
        return constructor_expr;
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
    let mut actual = preferred_planned_or_external_call_result_shape(expr, env, ctx)
        .or_else(|| optional_slot_non_optional_expr_shape(expr, env, &slot, shape_ctx))
        .unwrap_or_else(|| actual_shape_for_slot_expr_with_slot(expr, env, shape_ctx, &slot));
    if let Some(refined_actual) =
        refined_non_null_optional_element_shape_for_slot(expr, &actual, &slot, refinements)
    {
        actual = refined_actual;
    }
    if let Some(getter_actual) = optional_getter_actual_shape_for_slot(expr, &slot) {
        actual = getter_actual;
    }
    if let Some(display_actual) = optional_current_method_display_actual_shape(expr, &slot, ctx) {
        actual = display_actual;
    }
    if let Expr::FieldAccess(receiver, field) = expr {
        if let Some(enum_field_actual) =
            source_java_enum_field_accessor_shape_for_receiver(receiver, field, env, ctx)
        {
            actual = enum_field_actual;
        }
    } else if let Some(enum_field_actual) =
        source_java_enum_instance_field_accessor_shape(expr, ctx)
    {
        actual = enum_field_actual;
    }
    if let Some(optional_field) =
        lower_optional_receiver_field_access_for_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return optional_field;
    }
    if let Some(optional_array_slice) =
        lower_optional_array_expr_for_slice_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return optional_array_slice;
    }
    if let Some(recovered_field) =
        lower_shared_self_field_owned_recovery_for_slot(expr, env, &slot, &actual, ctx, refinements)
    {
        return recovered_field;
    }
    if let Some(recovered_field) = lower_borrowed_receiver_field_owned_recovery_for_slot(
        expr,
        env,
        &slot,
        &actual,
        ctx,
        refinements,
    ) {
        return recovered_field;
    }
    if let Some(optional_field) = lower_optional_self_field_borrowed_carrier_for_slot(
        expr,
        env,
        &slot,
        &actual,
        ctx,
        refinements,
    ) {
        return optional_field;
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
        lowered_expr = propagate_result_projection(lowered_expr);
    }
    if matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment)
        && matches!(expr, Expr::Var(name) if refinements.non_null_locals.contains(name))
    {
        return apply_java_display_plan(lowered_expr, JavaDisplayPlan::ConcatValue);
    }
    if let Some(upcast) = lower_source_subtype_to_supertype_slot(&lowered_expr, &actual, &slot, ctx)
    {
        return upcast;
    }
    if let Some(unwrapped) =
        lower_nullable_resource_stream_call_for_concrete_slot(expr, &slot, &lowered_expr)
    {
        return unwrapped;
    }
    let plan = plan_slot_conversion(actual, slot, ctx.facts);
    apply_slot_expr_conversion(lowered_expr, &plan)
}

fn lower_external_crate_constructor_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::New {
        ty, args, resolved, ..
    } = expr
    else {
        return None;
    };
    let target = plan_new_target_for_args_with_facts(
        ty,
        resolved.as_ref(),
        ctx.representations,
        ctx.facts,
        args,
        env.type_env(),
    );
    let external = target.external_crate?;
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let actual = LoweredType::rust_scalar_return(
        ty.clone(),
        format!("{}::{}", external.crate_name, external.type_name),
    );
    let plan = plan_lowered_slot_conversion(&actual, slot.clone(), ctx.facts);
    Some(apply_lowered_slot_expr_conversion(
        crate::coercion::LoweredExpr {
            expr: lowered,
            actual,
        },
        &plan,
    ))
}

fn lower_source_subtype_to_supertype_slot(
    lowered_expr: &RustExpr,
    actual: &RustSlotShape,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    if actual.ownership != OwnershipShape::Owned {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if target.ownership != OwnershipShape::Owned {
        return None;
    }
    let actual_name = concrete_java_type_name(&actual.java_ty)?;
    let target_name = concrete_java_type_name(&target.java_ty)?;
    if same_concrete_java_type_name(&actual.java_ty, target_name) {
        return None;
    }
    if !source_subtype_conversion_available(actual_name, target_name, ctx) {
        return None;
    }
    Some(into_projection(lowered_expr.clone()))
}

fn source_subtype_conversion_available(
    actual_name: &str,
    target_name: &str,
    ctx: LowerCtx,
) -> bool {
    let actual_is_source = find_type_decl_by_name(ctx.support_local_type_program, actual_name)
        .is_some()
        || find_type_decl_by_name(ctx.source_summary_type_program, actual_name).is_some();
    if !actual_is_source {
        return false;
    }
    if source_external_super_conversion_available(actual_name, target_name, ctx) {
        return true;
    }
    let target_is_source = find_type_decl_by_name(ctx.program, target_name).is_some()
        || (ctx.context_sources_are_emitted
            && (find_type_decl_by_name(ctx.support_local_type_program, target_name).is_some()
                || find_type_decl_by_name(ctx.source_summary_type_program, target_name).is_some()));
    target_is_source
        && (source_or_jdk_subtype_reaches(actual_name, target_name, ctx.support_local_type_program)
            || source_or_jdk_subtype_reaches(
                actual_name,
                target_name,
                ctx.source_summary_type_program,
            ))
}

fn source_external_super_conversion_available(
    actual_name: &str,
    target_name: &str,
    ctx: LowerCtx,
) -> bool {
    let Some(actual_ty) = find_type_decl_by_name(ctx.support_local_type_program, actual_name)
        .or_else(|| find_type_decl_by_name(ctx.source_summary_type_program, actual_name))
    else {
        return false;
    };
    crate::stdlib::resolved_external_super_names_for_method_queries(
        ctx.facts,
        actual_ty,
        ctx.support_local_type_program,
    )
    .into_iter()
    .any(|super_name| {
        ctx.facts
            .external_conversion_adapter_for(&super_name, target_name)
            .is_some_and(|adapter| {
                matches!(
                    adapter.kind,
                    crate::facts::ExternalConversionAdapterKind::Into
                )
            })
            || crate::external_packs::external_conversion_adapter_for(&super_name, target_name)
                .is_some_and(|adapter| {
                    matches!(
                        adapter.kind,
                        crate::facts::ExternalConversionAdapterKind::Into
                    )
                })
    })
}

fn lower_nullable_resource_stream_call_for_concrete_slot(
    expr: &Expr,
    slot: &Slot,
    lowered_expr: &RustExpr,
) -> Option<RustExpr> {
    let Expr::Call { name, args, .. } = expr else {
        return None;
    };
    if !matches!(
        name.as_str(),
        "getResourceAsStream" | "get_resource_as_stream"
    ) || args.len() != 1
    {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some()
        || !matches!(
            target.java_ty,
            JavaType::Named { ref name, .. } if name == "InputStream"
        )
    {
        return None;
    }
    Some(unwrap_java_null_receiver_projection(lowered_expr.clone()))
}

fn optional_getter_actual_shape_for_slot(expr: &Expr, slot: &Slot) -> Option<RustSlotShape> {
    if slot_targets_erased_object(slot) {
        return None;
    }
    let target = slot_target_shape(slot)?;
    target.optional_element_type()?;
    let Expr::Call {
        receiver: Some(_),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if !args.is_empty() || !crate::java_naming::java_getter_method_name(name) {
        return None;
    }
    Some(target.clone())
}

fn optional_current_method_display_actual_shape(
    expr: &Expr,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    if !matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment) {
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
    if !args.is_empty() {
        return None;
    }
    if receiver
        .as_ref()
        .is_some_and(|receiver| !is_this_receiver_expr(receiver))
    {
        return None;
    }
    let rust_name = resolved
        .as_ref()
        .map(|resolved| resolved.rust_name.as_str())
        .unwrap_or(name);
    let snake_name = crate::parser::to_snake_case(name);
    let mut owners = Vec::new();
    if let Some(current_type) = ctx.current_type {
        owners.push(current_type);
    }
    if let Some(resolved) = resolved {
        owners.push(resolved.declaring_type.as_str());
    }
    owners.sort_unstable();
    owners.dedup();
    owners.into_iter().find_map(|owner| {
        ctx.signatures
            .method_return_shape_by_rust_name(owner, rust_name)
            .or_else(|| {
                ctx.signatures
                    .method_return_shape_by_rust_name(owner, &snake_name)
            })
            .filter(|shape| shape.optional_element_type().is_some())
    })
}

fn lower_optional_array_expr_for_slice_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    let target_ty = crate::types::optional_element_type(&target.java_ty)
        .unwrap_or_else(|| target.java_ty.clone());
    if target.view != ViewShape::Slice || !matches!(target_ty, JavaType::Array(_)) {
        return None;
    }
    let actual_element = actual.optional_element_type()?;
    if !same_concrete_java_type(&actual_element, &target_ty) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let lowered_type = optional_array_lowered_type_for_slice_slot(actual, target)?;
    let plan = plan_lowered_slot_conversion(&lowered_type, slot.clone(), ctx.facts);
    Some(apply_lowered_slot_expr_conversion(
        crate::coercion::LoweredExpr {
            expr: lowered,
            actual: plan.actual.clone(),
        },
        &plan,
    ))
}

fn lower_optional_receiver_field_access_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    if is_this_receiver_expr(receiver) {
        return None;
    }
    if matches!(receiver.as_ref(), Expr::Var(name) if bare_name_looks_like_static_owner(name)) {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)?;
    let receiver_owner = concrete_java_type_name(&receiver_ty).map(str::to_string);
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    if field == "length"
        && let Some(plan) = crate::member_access_plan::plan_member_access_with_facts(
            receiver,
            field,
            env.type_env(),
            ctx.facts,
        )
    {
        let unwrapped_receiver = emit::emit_expr(&adapt_optional_instance_receiver_with_slot_plan(
            receiver,
            lowered_receiver.clone(),
            env,
            ctx,
            false,
            false,
        ));
        let lowered = match plan.op {
            crate::member_access_plan::MemberAccessOp::LenAsI32 => {
                len_i32_projection(RustExpr::Raw(unwrapped_receiver.clone()))
            }
            crate::member_access_plan::MemberAccessOp::Array2RowsAsI32
                if two_dimensional_array_expr_uses_nested_slice(receiver, env, ctx) =>
            {
                len_i32_projection(RustExpr::Raw(unwrapped_receiver.clone()))
            }
            crate::member_access_plan::MemberAccessOp::Array2RowsAsI32 => {
                array2_rows_i32_projection(RustExpr::Raw(unwrapped_receiver.clone()))
            }
            crate::member_access_plan::MemberAccessOp::Array2ColsAsI32 => {
                len_i32_projection(RustExpr::Raw(unwrapped_receiver.clone()))
            }
            _ => RustExpr::Raw(format!(
                "{unwrapped_receiver}.{}",
                crate::parser::to_snake_case(accessor_member_name(field))
            )),
        };
        return Some(apply_expr_to_slot_via_plan(
            lowered,
            crate::slot_shape::RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
            slot,
            ctx.facts,
        ));
    }
    let field_name = crate::parser::to_snake_case(accessor_member_name(field));
    let field_shape = optional_receiver_field_shape(field, env, ctx, receiver_owner.as_deref());
    let actual = optional_receiver_field_actual_shape_for_slot(
        field,
        env,
        slot,
        ctx,
        receiver_owner.as_deref(),
    )
    .or_else(|| field_shape.clone())
    .unwrap_or_else(|| actual.clone());
    let lowered_receiver = adapt_optional_instance_receiver_with_slot_plan(
        receiver,
        lowered_receiver,
        env,
        ctx,
        false,
        false,
    );
    let mut lowered = RustExpr::Raw(format!(
        "{}.{field_name}",
        emit::emit_expr(&lowered_receiver)
    ));
    if let Some(field_shape) = field_shape.as_ref()
        && field_shape.optional_element_type().is_some()
        && optional_field_borrowed_carrier_slot_is_element_projection(field_shape, slot)
    {
        return apply_optional_field_borrowed_carrier_to_slot(
            lowered,
            field_shape,
            slot.clone(),
            ctx,
        );
    }
    if shape_is_owned(&actual) && !matches!(actual.java_ty, JavaType::Primitive(_)) {
        lowered = apply_audited_clone_recovery(lowered);
    }
    Some(apply_expr_to_slot_via_plan(
        lowered, actual, slot, ctx.facts,
    ))
}

fn optional_field_borrowed_carrier_slot_is_element_projection(
    option_shape: &RustSlotShape,
    slot: &Slot,
) -> bool {
    let Some(target) = slot_target_shape(slot) else {
        return false;
    };
    let Some(element_ty) = option_shape.optional_element_type() else {
        return false;
    };
    target.optional_element_type().is_some()
        || same_concrete_java_type(&target.java_ty, &element_ty)
}

fn optional_receiver_field_actual_shape_for_slot(
    field: &str,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    receiver_owner: Option<&str>,
) -> Option<RustSlotShape> {
    let target = slot_target_shape(slot)?;
    target.optional_element_type()?;
    let field_shape = optional_receiver_field_shape(field, env, ctx, receiver_owner)?;
    if field_shape.java_ty != target.java_ty {
        return None;
    }
    Some(field_shape)
}

fn optional_receiver_field_shape(
    field: &str,
    env: &ShapeEnv,
    ctx: LowerCtx,
    receiver_owner: Option<&str>,
) -> Option<RustSlotShape> {
    let field_name = accessor_member_name(field);
    let alias = crate::parser::to_snake_case(field_name);
    let field_shape = receiver_owner
        .and_then(|owner| signature_field_shape_for_member(ctx, owner, field_name, &alias))
        .or_else(|| env.field_shape(field_name).cloned())
        .or_else(|| env.field_shape(&alias).cloned())?;
    field_shape.optional_element_type()?;
    Some(field_shape)
}

fn signature_field_shape_for_member(
    ctx: LowerCtx,
    owner: &str,
    field_name: &str,
    alias: &str,
) -> Option<RustSlotShape> {
    let owner_alias = owner
        .rsplit(['.', '$'])
        .next()
        .filter(|simple| *simple != owner);
    [Some(owner), owner_alias]
        .into_iter()
        .flatten()
        .flat_map(|owner| {
            [
                ctx.signatures.field_shape(owner, field_name),
                ctx.signatures.field_shape(owner, alias),
            ]
        })
        .flatten()
        .next()
        .cloned()
}

fn lower_static_initialized_singleton_field_for_slot(
    expr: &Expr,
    slot: &Slot,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let Expr::Var(field_name) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    if target.ownership != OwnershipShape::Owned {
        return None;
    }
    let current_type = ctx.current_type?;
    let ty = find_context_type_by_key(ctx.program, current_type)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, current_type))?;
    let field = ty.members.iter().find_map(|member| {
        let Member::Field(field) = member else {
            return None;
        };
        (field.name == *field_name && field.modifiers.contains(&Modifier::Static)).then_some(field)
    })?;
    let Expr::New {
        ty: initializer_ty,
        args,
        body,
        ..
    } = field.initializer.as_ref()?
    else {
        return None;
    };
    if !args.is_empty()
        || body.as_ref().is_some_and(|members| !members.is_empty())
        || !same_concrete_java_type(initializer_ty, &target.java_ty)
        || !same_concrete_java_type(&field.ty, &target.java_ty)
    {
        return None;
    }
    let rust_name =
        crate::slot_render::rust_name_for_java_named_type(concrete_java_type_name(initializer_ty)?);
    Some(function_call_projection(
        owner_associated_path_projection(&rust_name, "new"),
        Vec::new(),
    ))
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
    if receiver
        .as_ref()
        .is_none_or(|receiver| is_this_receiver_expr(receiver))
    {
        let field_expr = Expr::Var(field_name);
        return Some(lower_expr_for_slot_with_refinements(
            &field_expr,
            env,
            slot.clone(),
            ctx,
            refinements,
        ));
    }
    None
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
    let rust_ty = emit::emit_type(&erased_object_rust_type());
    Some(typed_none_projection(&rust_ty))
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
        if let Some(created) = lower_ragged_array_creation_for_slot(&array_plan, env, target, ctx) {
            return Some(created);
        }
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
            if let Some(recovered) =
                lower_reused_owned_array_child(child, env, &element_shape, ctx, refinements)
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

fn lower_ragged_array_creation_for_slot(
    array_plan: &crate::array_plan::ArrayExprPlan<'_>,
    env: &ShapeEnv,
    target: &RustSlotShape,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let crate::array_plan::ArrayExprPlan::Creation { dimensions, .. } = array_plan else {
        return None;
    };
    if dimensions.len() != 1 {
        return None;
    }
    if crate::types::two_dimensional_array_element_type(&target.java_ty).is_none()
        || crate::types::ndarray_array2_element_type(&target.java_ty).is_some()
    {
        return None;
    }
    let len = lower_expr_for_slot(&dimensions[0], env, index_slot(), ctx);
    Some(ragged_array_outer_creation_projection(len))
}

fn lower_array_access_expr(
    array: &Expr,
    index: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Some(expr) = lower_primitive_casted_array_access(array, index, env, ctx, refinements) {
        return expr;
    }
    if let Expr::ArrayAccess(outer_array, outer_index) = array
        && two_dimensional_element_type_after_optional(&crate::types::infer_expr_type(
            outer_array,
            env.type_env(),
        ))
        .is_some()
    {
        if two_dimensional_array_expr_uses_nested_slice(outer_array, env, ctx) {
            return indexed_expr(
                indexed_expr(
                    lower_read_sequence_receiver(outer_array, env, ctx, refinements),
                    lower_expr_for_slot(outer_index, env, index_slot(), ctx),
                ),
                lower_expr_for_slot(index, env, index_slot(), ctx),
            );
        }
        return array2_index_projection(
            lower_read_sequence_receiver(outer_array, env, ctx, refinements),
            lower_expr_for_slot(outer_index, env, index_slot(), ctx),
            lower_expr_for_slot(index, env, index_slot(), ctx),
        );
    }

    if let Some(row_element_ty) =
        ndarray_element_type_after_optional(&crate::types::infer_expr_type(array, env.type_env()))
    {
        if two_dimensional_array_expr_uses_nested_slice(array, env, ctx) {
            return indexed_expr(
                lower_read_sequence_receiver(array, env, ctx, refinements),
                lower_expr_for_slot(index, env, index_slot(), ctx),
            );
        }
        let row_expr = receiver_method_call_projection_exact(
            lower_read_sequence_receiver(array, env, ctx, refinements),
            "row",
            vec![lower_expr_for_slot(index, env, index_slot(), ctx)],
        );
        return to_owned_sequence_projection(
            row_expr,
            &CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Array(Box::new(row_element_ty)),
            },
        );
    }

    let lowered_array = lower_sequence_get_array_access_receiver(array, env, ctx, refinements)
        .unwrap_or_else(|| lower_read_sequence_receiver(array, env, ctx, refinements));

    indexed_expr(
        lowered_array,
        lower_expr_for_slot(index, env, index_slot(), ctx),
    )
}

fn lower_primitive_casted_array_access(
    array: &Expr,
    index: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = array else {
        return None;
    };
    let JavaType::Primitive(target) = ty else {
        return None;
    };
    if !crate::types::is_numeric_primitive_type(*target) {
        return None;
    }
    let lowered_array = lower_sequence_get_array_access_receiver(inner, env, ctx, refinements)
        .unwrap_or_else(|| lower_read_sequence_receiver(inner, env, ctx, refinements));
    let indexed = indexed_expr(
        lowered_array,
        lower_expr_for_slot(index, env, index_slot(), ctx),
    );
    Some(RustExpr::Cast {
        expr: Box::new(indexed),
        ty: crate::coercion::rust_primitive_type_name_policy(*target)
            .rust_name
            .to_string(),
    })
}

fn static_final_const_array_field_type<'a>(expr: &Expr, ctx: LowerCtx<'a>) -> Option<&'a JavaType> {
    let field_name = static_final_const_array_field_name(expr)?;
    let current_type = ctx.current_type?;
    [
        ctx.program,
        ctx.support_local_type_program,
        ctx.source_summary_type_program,
    ]
    .into_iter()
    .find_map(|program| find_context_type_by_key(program, current_type))
    .and_then(|ty| static_final_const_array_field_type_for_type(ty, field_name))
}

fn static_slice_const_array_expr_type(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<JavaType> {
    if let Some(java_ty) = static_final_const_array_field_type(expr, ctx) {
        return Some(java_ty.clone());
    }
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    let plan = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        env.type_env(),
        ctx.facts,
    )?;
    if !matches!(
        plan.op,
        crate::member_access_plan::MemberAccessOp::StaticConstant { .. }
    ) || !matches!(plan.java_ty, JavaType::Array(_))
    {
        return None;
    }
    Some(plan.java_ty)
}

fn two_dimensional_array_expr_uses_nested_slice(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    static_slice_const_array_expr_type(expr, env, ctx)
        .as_ref()
        .is_some_and(|java_ty| two_dimensional_element_type_after_optional(java_ty).is_some())
}

fn static_final_const_array_field_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field)
        }
        _ => None,
    }
}

fn static_final_const_array_field_type_for_type<'a>(
    ty: &'a TypeDecl,
    name: &str,
) -> Option<&'a JavaType> {
    ty.members.iter().find_map(|member| {
        let Member::Field(field) = member else {
            return None;
        };
        if !field.modifiers.contains(&Modifier::Static)
            || !field.modifiers.contains(&Modifier::Final)
            || !static_final_field_name_matches(field, name)
            || !matches!(field.ty, JavaType::Array(_))
            || !field
                .initializer
                .as_ref()
                .is_some_and(static_final_array_initializer_is_const_like)
        {
            return None;
        }
        Some(&field.ty)
    })
}

fn lower_static_final_const_array_for_slice_slot(
    expr: &Expr,
    slot: &Slot,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    let const_ty = static_slice_const_array_expr_type(expr, env, ctx)?;
    if !same_concrete_java_type(&const_ty, &target.java_ty) {
        return None;
    }
    let lowered = lower_expr(expr, env, ctx);
    if two_dimensional_element_type_after_optional(&const_ty).is_some() {
        return array2_from_rows_projection(lowered, target.ownership);
    }
    let rendered = emit::emit_expr(&lowered);
    match target.ownership {
        OwnershipShape::SharedBorrow if target.view == ViewShape::Slice => Some(lowered),
        OwnershipShape::Owned => Some(to_owned_sequence_projection(
            RustExpr::Raw(rendered),
            &CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: const_ty.clone(),
            },
        )),
        OwnershipShape::MutableBorrow
        | OwnershipShape::CopyValue
        | OwnershipShape::SharedBorrow => None,
    }
}

fn lower_static_final_const_array_clone_for_slot(
    expr: &Expr,
    slot: &Slot,
    env: &ShapeEnv,
    ctx: LowerCtx,
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
    if name != "clone" || !args.is_empty() {
        return None;
    }
    lower_static_final_const_array_for_slice_slot(receiver, slot, env, ctx)
}

fn static_final_field_name_matches(field: &FieldDecl, name: &str) -> bool {
    field.name == name || crate::parser::to_snake_case(&field.name) == name
}

fn static_final_array_initializer_is_const_like(expr: &Expr) -> bool {
    match expr {
        Expr::ArrayInitializer(_) => true,
        Expr::ArrayCreation {
            initializer: Some(_),
            ..
        } => true,
        _ => false,
    }
}

fn two_dimensional_element_type_after_optional(java_ty: &JavaType) -> Option<JavaType> {
    crate::types::two_dimensional_array_element_type(java_ty).or_else(|| {
        crate::types::optional_element_type(java_ty)
            .as_ref()
            .and_then(crate::types::two_dimensional_array_element_type)
    })
}

fn ndarray_element_type_after_optional(java_ty: &JavaType) -> Option<JavaType> {
    crate::types::ndarray_array2_element_type(java_ty).or_else(|| {
        crate::types::optional_element_type(java_ty)
            .as_ref()
            .and_then(crate::types::ndarray_array2_element_type)
    })
}

fn lower_read_sequence_receiver(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let shape = infer_expr_shape_with_context(expr, env, shape_ctx);
    let Some(inner_ty) = shape.optional_element_type() else {
        return lowered;
    };
    if crate::types::sequence_element_type(&inner_ty).is_none()
        && crate::types::two_dimensional_array_element_type(&inner_ty).is_none()
    {
        return lowered;
    }
    project_optional_sequence_receiver(lowered, OptionalSequenceReceiverProjection::Shared)
}

fn lower_sequence_get_array_access_receiver(
    expr: &Expr,
    env: &ShapeEnv,
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
    crate::types::sequence_element_type(&element_ty)?;

    let mut receiver_expr = lower_expr_with_refinements(receiver, env, ctx, refinements);
    if optional_sequence_ty.is_some() {
        receiver_expr = adapt_optional_instance_receiver_with_slot_plan(
            receiver,
            receiver_expr,
            env,
            ctx,
            false,
            false,
        );
    }
    let index_expr = lower_expr_for_slot(&args[0], env, index_slot(), ctx);
    Some(sequence_get_expect_projection(receiver_expr, index_expr))
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
    crate::types::ndarray_array2_element_type(&crate::types::infer_expr_type(
        array,
        env.type_env(),
    ))?;
    Some(array2_row_mut_assign_stmt(
        lower_expr(array, env, ctx),
        lower_expr_for_slot(index, env, index_slot(), ctx),
        lower_array2_row_replacement_value_expr(value, env, ctx),
    ))
}

fn lower_array2_row_replacement_value_expr(
    value: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let lowered = lower_expr(value, env, ctx);
    if matches!(
        value,
        Expr::ArrayCreation {
            initializer: Some(_),
            ..
        } | Expr::ArrayInitializer(_)
    ) {
        return force_shared_borrow_projection(lowered);
    }
    lowered
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
    let cloned = apply_audited_clone_recovery(lowered);
    Some(apply_slot_expr_conversion(cloned, &plan))
}

fn lower_reused_owned_array_child(
    child: &Expr,
    env: &ShapeEnv,
    element_shape: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let slot = argument_slot(element_shape.clone());
    let lowered = lower_expr_for_slot_with_refinements(child, env, slot.clone(), ctx, refinements);
    let recovered = adapt_reused_owned_value_for_slot(lowered.clone(), child, &slot, env, ctx);
    (recovered != lowered).then_some(recovered)
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

fn lower_erased_object_cast_for_erased_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, expr: inner } = expr else {
        return None;
    };
    if !java_type_is_erased_object_name(ty)
        || !(slot_targets_erased_object_type(slot) || matches!(slot, Slot::Value))
    {
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

fn slot_targets_erased_object_type(slot: &Slot) -> bool {
    if slot_targets_erased_object(slot) {
        return true;
    }
    let Some(target) = slot_target_shape(slot) else {
        return false;
    };
    if java_type_is_erased_object_name(&target.java_ty) {
        return true;
    }
    target
        .optional_element_type()
        .is_some_and(|element_ty| java_type_is_erased_object_name(&element_ty))
}

fn java_type_is_erased_object_name(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
        if erased_java_simple_name(name).eq_ignore_ascii_case("Object")
            || erased_java_type_name(name).eq_ignore_ascii_case("java.lang.Object"))
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
    let plan = retarget_native_collection_to_array_from_shape(
        plan,
        receiver.as_deref(),
        name,
        args,
        env,
        ctx,
    );
    let CallExprPlan::Target(target_plan) = plan else {
        return None;
    };
    if !target_plan.uses_receiver_argument_lowering() {
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
        lowered_expr = propagate_result_projection(lowered_expr);
    }
    let actual = target_typed_call_cast_actual_shape(inner, env, ctx, &target_ty, target);
    if shape_is_erased_object_boundary(&actual)
        && let Some(handoff) =
            erased_external_call_trait_object_cast_handoff(lowered_expr.clone(), ty, target)
    {
        return Some(handoff);
    }
    Some(rewrite_deserialization_downcast_message(
        apply_expr_to_slot_via_plan(lowered_expr, actual, slot, ctx.facts),
    ))
}

fn lower_erased_external_call_cast_for_slot(
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
    let call_shape = external_call_expr_result_shape(inner, env, ctx)?;
    if !shape_is_erased_object_boundary(&call_shape) {
        return None;
    }
    let lowered = lower_external_call_expr_for_slot(inner, env, slot, ctx, refinements)?;
    if let Some(handoff) =
        erased_external_call_trait_object_cast_handoff(lowered.expr.clone(), ty, target)
    {
        return Some(handoff);
    }
    let plan = plan_lowered_slot_conversion(&lowered.actual, slot.clone(), ctx.facts);
    Some(rewrite_deserialization_downcast_message(
        apply_lowered_slot_expr_conversion(lowered, &plan),
    ))
}

fn expr_is_object_input_stream_read_object_call(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(_),
            name,
            args,
            ..
        } if args.is_empty() && matches!(name.as_str(), "readObject" | "read_object")
    )
}

fn rewrite_deserialization_downcast_message(expr: RustExpr) -> RustExpr {
    rewrite_downcast_expect_message(expr, "Java deserialization type mismatch")
}

fn erased_external_call_trait_object_cast_handoff(
    lowered: RustExpr,
    cast_ty: &JavaType,
    target: &RustSlotShape,
) -> Option<RustExpr> {
    let trait_name = match &target.dispatch {
        DispatchShape::DynTrait { trait_name } | DispatchShape::BoxedDynTrait { trait_name } => {
            trait_name
        }
        _ => return None,
    };
    let java_name = match cast_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.as_str(),
        _ => trait_name.as_str(),
    };
    Some(unsupported_deserialization_cast_handoff_projection(
        lowered, java_name,
    ))
}

fn target_typed_call_cast_actual_shape(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    target_ty: &JavaType,
    slot_target: &RustSlotShape,
) -> RustSlotShape {
    if let Some(call_shape) = external_call_expr_result_shape(expr, env, ctx) {
        if shape_is_erased_object_boundary(&call_shape) {
            return call_shape;
        }
        if external_call_shape_already_matches_cast_slot(&call_shape, target_ty, slot_target) {
            return call_shape;
        }
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
        return Some(lower_present_expr_for_optional_target(
            inner,
            env,
            target,
            target_ty,
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

fn lower_closed_enum_cast_for_matching_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Cast { ty, .. } = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    if !matches!(target.dispatch, DispatchShape::ClosedEnum { .. })
        || target.ownership != OwnershipShape::Owned
        || !same_concrete_java_type(ty, &target.java_ty)
    {
        return None;
    }
    Some(lower_expr_with_refinements(expr, env, ctx, refinements))
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
    let actual = RustSlotShape::for_storage(JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    });
    if !constructor_result_can_flow_into_concrete_slot(&actual, &target_shape, ctx.facts) {
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
        args,
        env,
        ctx.signatures,
        ty,
        &target.rust_name,
        ctx.facts,
    );
    let rust_args = lower_constructor_args_with_plan(args, env, constructor_arg_plan, ctx);
    let lowered = named_constructor_expr(type_name, &target.rust_name, rust_args);
    let plan = plan_slot_conversion(actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn constructor_result_can_flow_into_concrete_slot(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> bool {
    if same_concrete_java_type(&target.java_ty, &actual.java_ty) {
        return true;
    }
    let plan = plan_slot_conversion(actual.clone(), Slot::Assignment(target.clone()), facts);
    matches!(
        plan.conversion,
        SlotConversionPlan::Shape(shape)
            if matches!(shape.dispatch, DispatchCoercionPolicy::ExternalInto { .. })
    )
}

fn planned_optional_value_for_target(
    value: RustExpr,
    actual_element_shape: RustSlotShape,
    target_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let construction =
        optional_slot_construction_plan_for_actual(&actual_element_shape, target_shape, facts);
    construct_optional_value_for_target(value, construction)
}

fn planned_optional_assignment_value_for_target(
    lowered_value: RustExpr,
    actual: RustSlotShape,
    slot: Slot,
    target: &RustSlotShape,
    already_optional_result: bool,
    facts: &FactSet,
) -> RustExpr {
    let conversion_plan = plan_slot_conversion(actual.clone(), slot, facts);
    let mut target_element = target.clone();
    target_element.java_ty = actual.java_ty.clone();
    let preconverted_some_plan = OptionalSlotConstructionPlan {
        target: target.clone(),
        element_shape: target_element.clone(),
        element_plan: shape_coercion_plan_with_facts(&target_element, &target_element, facts),
        wrapper_ownership: OwnershipShape::Owned,
    };
    render_adapt_optional_assignment_value_for_target(
        lowered_value,
        actual,
        conversion_plan,
        preconverted_some_plan,
        already_optional_result,
    )
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
    if let Some(expr) = lower_optional_self_field_borrowed_option_recovery(
        expr,
        env,
        target,
        actual,
        ctx,
        refinements,
    ) {
        return Some(expr);
    }
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
    if let Some(expr) = lower_optional_self_field_read_owned_recovery(
        expr,
        env,
        slot,
        target,
        actual,
        ctx,
        refinements,
    ) {
        return Some(expr);
    }
    if let Some(expr) = lower_shared_self_field_erased_object_owned_recovery(
        expr,
        env,
        slot,
        actual,
        ctx,
        refinements,
    ) {
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
                let recovered = apply_audited_clone_recovery(lowered);
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
            let recovered = apply_audited_clone_recovery(lowered);
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
    let mut borrowed_actual = actual.clone();
    borrowed_actual.ownership = OwnershipShape::SharedBorrow;
    let plan = plan_slot_conversion(borrowed_actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn lower_shared_self_field_erased_object_owned_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    if !shape_is_owned(target) || !shape_is_erased_object_boundary(target) {
        return None;
    }
    if let Some(recovered) =
        lower_optional_self_field_unwrap_erased_object_recovery(expr, env, slot, ctx, refinements)
    {
        return Some(recovered);
    }
    shared_self_field_read_name_with_ctx(expr, env, ctx)?;
    if let Some(element_ty) = actual.optional_element_type() {
        if matches!(element_ty, JavaType::Primitive(_)) {
            return None;
        }
        let element_shape = optional_element_slot_shape_with_facts(actual, element_ty, ctx.facts);
        if !known_owned_argument_clone_recovery_with_facts(&element_shape, ctx.facts)
            && !known_named_clone_recovery(&element_shape)
        {
            return None;
        }
        let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
        return apply_optional_field_borrowed_carrier_to_slot(lowered, actual, slot.clone(), ctx);
    }
    if matches!(actual.java_ty, JavaType::Primitive(_)) {
        return None;
    }
    if !known_owned_argument_clone_recovery_with_facts(actual, ctx.facts)
        && !known_named_clone_recovery(actual)
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let mut borrowed_actual = actual.clone();
    borrowed_actual.ownership = OwnershipShape::SharedBorrow;
    let plan = plan_slot_conversion(borrowed_actual, slot.clone(), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn lower_optional_self_field_unwrap_erased_object_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    let element_ty = receiver_shape.optional_element_type()?;
    if matches!(element_ty, JavaType::Primitive(_)) {
        return None;
    }
    let element_shape =
        optional_element_slot_shape_with_facts(&receiver_shape, element_ty, ctx.facts);
    if !known_owned_argument_clone_recovery_with_facts(&element_shape, ctx.facts)
        && !known_named_clone_recovery(&element_shape)
    {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    apply_optional_field_borrowed_carrier_to_slot(
        lowered_receiver,
        &receiver_shape,
        slot.clone(),
        ctx,
    )
}

fn lower_borrowed_receiver_field_owned_recovery_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    if !shape_is_owned(target) || matches!(target.java_ty, JavaType::Primitive(_)) {
        return None;
    }
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    if is_this_receiver_expr(receiver) {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_shape = infer_expr_shape_with_context(receiver, env, shape_ctx);
    if receiver_shape.ownership != OwnershipShape::SharedBorrow {
        return None;
    }
    let field_name = accessor_member_name(field);
    let alias = crate::parser::to_snake_case(field_name);
    let receiver_owner = concrete_java_type_name(&receiver_shape.java_ty);
    let field_actual = receiver_owner
        .and_then(|owner| signature_field_shape_for_member(ctx, owner, field_name, &alias))
        .unwrap_or_else(|| actual.clone());
    if let Some(element_ty) = field_actual.optional_element_type() {
        if target.optional_element_type().is_some()
            || target.java_ty != element_ty
            || (!known_owned_argument_clone_recovery_with_facts(target, ctx.facts)
                && !known_named_clone_recovery(target))
        {
            return None;
        }
        let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
        return apply_optional_field_borrowed_carrier_to_slot(
            lowered,
            &field_actual,
            slot.clone(),
            ctx,
        );
    }
    if field_actual.java_ty != target.java_ty
        || (!known_owned_argument_clone_recovery_with_facts(&field_actual, ctx.facts)
            && !known_named_clone_recovery(&field_actual))
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let recovered = apply_audited_clone_recovery(lowered);
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
    apply_optional_field_borrowed_carrier_to_slot(
        lowered,
        &field_actual,
        Slot::Return(target.clone()),
        ctx,
    )
}

fn lower_optional_self_field_borrowed_option_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    target: &RustSlotShape,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    shared_self_field_read_name_with_ctx(expr, env, ctx)?;
    if target.ownership != OwnershipShape::SharedBorrow
        || target.optional_element_type().is_none()
        || actual.optional_element_type().is_none()
        || target.java_ty != actual.java_ty
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let plan = plan_slot_conversion(actual.clone(), Slot::Return(target.clone()), ctx.facts);
    Some(apply_slot_expr_conversion(lowered, &plan))
}

fn lower_optional_self_field_borrowed_carrier_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    actual: &RustSlotShape,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some()
        || !matches!(
            target.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
    {
        return None;
    }
    shared_self_field_read_name_with_ctx(expr, env, ctx)?;
    let field_actual = if actual.optional_element_type().is_some() {
        actual.clone()
    } else {
        current_instance_field_shape_with_env(expr, env, ctx)?
    };
    let element_ty = field_actual.optional_element_type()?;
    if !same_concrete_java_type(&target.java_ty, &element_ty) {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    let lowered = optional_element_borrow_lowered_expr_with_facts(
        lowered,
        &field_actual,
        element_ty,
        target.ownership,
        ctx.facts,
    )?;
    let plan = plan_lowered_slot_conversion(&lowered.actual, slot.clone(), ctx.facts);
    Some(apply_lowered_slot_expr_conversion(lowered, &plan))
}

fn lower_optional_self_field_read_owned_recovery(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    let element_ty = field_actual.optional_element_type()?;
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
    apply_optional_field_borrowed_carrier_to_slot(lowered, &field_actual, slot.clone(), ctx)
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
    apply_optional_field_borrowed_carrier_to_slot(
        lowered_receiver,
        &receiver_shape,
        Slot::Return(target.clone()),
        ctx,
    )
}

fn apply_optional_field_borrowed_carrier_to_slot(
    option_expr: RustExpr,
    option_shape: &RustSlotShape,
    slot: Slot,
    ctx: LowerCtx,
) -> Option<RustExpr> {
    let element_ty = option_shape.optional_element_type()?;
    let lowered = optional_element_borrow_lowered_expr(
        option_expr,
        option_shape,
        element_ty,
        OwnershipShape::SharedBorrow,
    )?;
    let plan = plan_lowered_slot_conversion(&lowered.actual, slot, ctx.facts);
    Some(apply_lowered_slot_expr_conversion(lowered, &plan))
}

fn borrowed_self_receiver_can_read_fields(ctx: LowerCtx) -> bool {
    matches!(
        ctx.current_receiver,
        Some(ReceiverPlan::Shared | ReceiverPlan::Mutable)
    )
}

fn shared_self_field_read_name<'a>(expr: &'a Expr, env: &ShapeEnv) -> Option<&'a str> {
    match expr {
        Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => {
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
    if let Expr::Cast { expr, .. } = expr {
        return slot_expr_needs_result_propagation(expr, env, ctx);
    }
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
    let key = CallableKey::method(
        receiver_name.to_string(),
        name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    if ctx
        .facts
        .facade_abi_for_key(&key)
        .is_some_and(|abi| matches!(abi.result, ResultAbi::ResultWrapped))
    {
        return true;
    }
    if ctx
        .facts
        .external_rust_result_for_receiver(Some(receiver_name), name, Some(arg_count))
        .is_some_and(|fact| fact.returns_result)
    {
        return true;
    }
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

fn lower_literal_for_slot(expr: &Expr, slot: &Slot) -> Option<RustExpr> {
    let Expr::Literal(literal) = expr else {
        return None;
    };
    literal_slot_plan(literal, slot).map(|plan| apply_literal_slot_plan(&plan))
}

fn char_literal_expr_from_integral_expr(expr: &Expr) -> Option<RustExpr> {
    let Expr::Literal(literal) = expr else {
        return None;
    };
    java_integral_literal_char(literal).map(|ch| RustExpr::Literal(format!("{ch:?}")))
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
    let Expr::Var(name) = expr else {
        return None;
    };
    let target = slot_target_shape(slot)?;
    if target.optional_element_type().is_some() {
        return None;
    }
    if shape_is_erased_object_boundary(target) {
        return None;
    }
    let element_ty = actual.optional_element_type()?;
    let element_shape = optional_element_slot_shape_with_facts(actual, element_ty, ctx.facts);
    if !borrowed_element_target_matches_element(target, &element_shape, ctx.facts) {
        return None;
    }
    if matches!(
        target.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
        if target.ownership == OwnershipShape::SharedBorrow
            && actual.ownership == OwnershipShape::SharedBorrow
            && actual.view == target.view
        {
            let actual = optional_element_borrow_lowered_type_with_facts(
                actual.clone(),
                element_shape.java_ty,
                Some(OwnershipShape::SharedBorrow),
                ctx.facts,
            )?;
            let rendered = emit::emit_expr(&lowered);
            let expr = RustExpr::Raw(format!("{rendered}.expect(\"Java null receiver\")"));
            let lowered = crate::coercion::LoweredExpr { expr, actual };
            let plan = plan_lowered_slot_conversion(&lowered.actual, slot.clone(), ctx.facts);
            return Some(apply_lowered_slot_expr_conversion(lowered, &plan));
        }
        let lowered = optional_element_borrow_lowered_expr_with_facts(
            lowered,
            actual,
            element_shape.java_ty,
            target.ownership.clone(),
            ctx.facts,
        )?;
        let plan = plan_lowered_slot_conversion(&lowered.actual, slot.clone(), ctx.facts);
        return Some(apply_lowered_slot_expr_conversion(lowered, &plan));
    }
    if !shape_is_owned(target) {
        return None;
    }
    if !known_owned_argument_clone_recovery(target)
        && !known_named_clone_recovery(target)
        && !known_owned_argument_clone_recovery_with_facts(target, ctx.facts)
        && !known_owned_argument_clone_recovery_with_facts(&element_shape, ctx.facts)
    {
        return None;
    }
    let lowered = lower_expr_with_refinements(expr, env, ctx, refinements);
    if actual.ownership == OwnershipShape::Owned
        && !matches!(slot, Slot::Return(_))
        && MoveUseContext::new(ctx).has_later_or_repeated_current_read(name)
    {
        let plan = plan_slot_conversion(actual.clone(), slot.clone(), ctx.facts);
        return Some(apply_slot_expr_conversion(
            apply_clone_projection(lowered),
            &plan,
        ));
    }
    let lowered = reused_optional_unwrap_lowered_expr(
        lowered,
        actual,
        element_shape.java_ty,
        matches!(slot, Slot::Return(_)),
    )?;
    let plan = plan_lowered_slot_conversion(&lowered.actual, slot.clone(), ctx.facts);
    Some(apply_lowered_slot_expr_conversion(lowered, &plan))
}

fn reused_optional_unwrap_lowered_expr(
    option_expr: RustExpr,
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    move_owned_carrier: bool,
) -> Option<crate::coercion::LoweredExpr> {
    if option_shape.ownership == OwnershipShape::SharedBorrow {
        let actual = optional_element_borrow_lowered_type(
            option_shape.clone(),
            element_ty,
            Some(OwnershipShape::SharedBorrow),
        )?;
        return Some(crate::coercion::LoweredExpr {
            expr: option_expr,
            actual,
        });
    }
    if move_owned_carrier && option_shape.ownership == OwnershipShape::Owned {
        let actual = optional_element_borrow_lowered_type(
            option_shape.clone(),
            element_ty,
            Some(OwnershipShape::Owned),
        )?;
        return Some(crate::coercion::LoweredExpr {
            expr: option_expr,
            actual,
        });
    }
    optional_element_borrow_lowered_expr(
        option_expr,
        option_shape,
        element_ty,
        OwnershipShape::SharedBorrow,
    )
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
    Some(apply_java_display_plan(
        lowered,
        JavaDisplayPlan::OptionalValue,
    ))
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
    let recovered = apply_audited_clone_recovery(lowered);
    Some(planned_optional_value_for_target(
        recovered,
        actual.clone(),
        target,
        ctx.facts,
    ))
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
    const MAP_ENTRY_VALUE_ACCESSOR: &str = concat!("get", "Value");
    if let Expr::Var(local_name) = receiver
        && let Some((key_ty, value_ty)) = refinements
            .map_entry_locals
            .get(local_name)
            .cloned()
            .or_else(|| map_entry_local_types_from_env(local_name, env))
    {
        let (field_index, actual_ty) = match name.as_str() {
            "getKey" | "get_key" => (
                0,
                map_entry_accessor_type_or_slot_target(&key_ty, slot).unwrap_or(key_ty),
            ),
            accessor if accessor == MAP_ENTRY_VALUE_ACCESSOR || accessor == "get_value" => {
                (1, value_ty)
            }
            _ => return None,
        };
        let entry_access = tuple_field_projection(RustExpr::Var(local_name.clone()), field_index);
        let actual = crate::shape_projection::borrowed_sequence_element_shape(actual_ty);
        return Some(apply_expr_to_slot_via_plan(
            entry_access,
            actual,
            slot,
            ctx.facts,
        ));
    }
    let (next_expr, key_ty, value_ty) = lower_map_entry_next_expr(receiver, env, ctx, refinements)?;
    let (field_index, actual_ty) = match name.as_str() {
        "getKey" | "get_key" => (
            0,
            map_entry_accessor_type_or_slot_target(&key_ty, slot).unwrap_or(key_ty),
        ),
        accessor if accessor == MAP_ENTRY_VALUE_ACCESSOR || accessor == "get_value" => {
            (1, value_ty)
        }
        _ => return None,
    };
    let entry_access = tuple_field_projection(next_expr, field_index);
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

fn map_entry_local_types_from_env(
    local_name: &str,
    env: &ShapeEnv,
) -> Option<(JavaType, JavaType)> {
    let shape = env.local_shape(local_name)?;
    let ty = crate::types::optional_element_type(&shape.java_ty)
        .unwrap_or_else(|| shape.java_ty.clone());
    map_entry_type_args_from_java_type(&ty)
}

fn map_entry_type_args_from_java_type(ty: &JavaType) -> Option<(JavaType, JavaType)> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    if !matches!(
        name.rsplit('.')
            .next()
            .unwrap_or(name)
            .rsplit('$')
            .next()
            .unwrap_or(name),
        "Entry"
    ) {
        return None;
    }
    let key_ty = args.first()?.clone();
    let value_ty = args
        .get(1)
        .filter(|ty| !matches!(ty, JavaType::Wildcard(_)))
        .cloned()
        .unwrap_or(JavaType::Unknown("map_entry_value".to_string()));
    Some((key_ty, value_ty))
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
    Some((iterator_next_expect_projection(iter_expr), key_ty, value_ty))
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
        receiver_expr = adapt_optional_instance_receiver_with_slot_plan(
            receiver,
            receiver_expr,
            env,
            ctx,
            false,
            false,
        );
    }
    Some((
        RustExpr::Raw(render_sequence_iteration_source(
            &emit::emit_expr(&receiver_expr),
            false,
            SequenceIterationProjection::Borrowed,
        )),
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
    let receiver_name = if is_this_receiver_expr(receiver) {
        ctx.current_type?.to_string()
    } else {
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
        match receiver_ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
            _ => return None,
        }
    };
    let snake_name = crate::parser::to_snake_case(name);
    if crate::stdlib::is_java_platform_facade_type_name(&receiver_name)
        && (crate::stdlib::known_external_method_return_type(
            Some(&receiver_name),
            name,
            args.len(),
        )
        .is_some()
            || crate::stdlib::known_external_method_return_type(
                Some(&receiver_name),
                &snake_name,
                args.len(),
            )
            .is_some())
    {
        return None;
    }
    if let Some(resolved) = resolved
        && context_type_key_is_summary_only(ctx, &resolved.declaring_type)
        && let Some(shape) = source_summary_declared_method_return_shape(
            &resolved.declaring_type,
            name,
            args.len(),
            ctx,
        )
    {
        return Some(shape);
    }
    if context_type_key_is_summary_only(ctx, &receiver_name)
        && let Some(shape) =
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
    if args.is_empty()
        && let Some(shape) =
            getter_field_return_shape_from_receiver(&receiver_name, name, ctx.signatures)
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

fn getter_field_return_shape_from_receiver(
    receiver_name: &str,
    method_name: &str,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    let field_name = crate::java_naming::java_getter_field_name(method_name)?;
    signature_field_shape_for_type_aliases(signatures, receiver_name, &field_name).cloned()
}

fn source_summary_declared_method_return_shape(
    receiver_name: &str,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    let ty = find_context_type_by_key(ctx.program, receiver_name)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, receiver_name))
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
        return Some(source_summary_typed_erased_facade_return_shape(
            apply_signature_constraints(
                shape,
                &[SignatureConstraint::RequiredStorageNullability(
                    effect.return_nullability,
                )],
            ),
            &method.return_type,
            ctx.facts,
        ));
    }
    let body_constraints = crate::signature::source_method_return_nullability_constraints(method);
    if !body_constraints.is_empty() {
        return Some(source_summary_typed_erased_facade_return_shape(
            apply_signature_constraints(shape, &body_constraints),
            &method.return_type,
            ctx.facts,
        ));
    }
    if !matches!(
        crate::stdlib::source_summary_method_return_nullability(&method.return_type),
        NullabilityState::DefinitelyNonNull
    ) {
        return Some(source_summary_typed_erased_facade_return_shape(
            shape,
            &method.return_type,
            ctx.facts,
        ));
    }
    Some(source_summary_typed_erased_facade_return_shape(
        apply_signature_constraints(
            shape,
            &[SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNonNull,
            )],
        ),
        &method.return_type,
        ctx.facts,
    ))
}

fn source_summary_typed_erased_facade_return_shape(
    mut shape: RustSlotShape,
    return_type: &JavaType,
    facts: &FactSet,
) -> RustSlotShape {
    if source_summary_return_should_keep_typed_erased_facade(return_type, facts) {
        crate::shape_projection::apply_required_dispatch(&mut shape, DispatchShape::Concrete);
    }
    shape
}

fn source_summary_return_should_keep_typed_erased_facade(
    return_type: &JavaType,
    facts: &FactSet,
) -> bool {
    let name = match return_type {
        JavaType::Named { name, args } if args.is_empty() => name.as_str(),
        JavaType::Unknown(name) => name.as_str(),
        _ => return false,
    };
    if matches!(name, "Object" | "java.lang.Object") {
        return false;
    }
    let erased = JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    };
    let effective_facts = crate::effective_facts::EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(&erased),
        Some(HierarchyStrategy::ErasedObject)
    )
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
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    for owner in owner_candidates {
        let Some(ty) = find_context_type_by_key(ctx.program, &owner)
            .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
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
        if matching_methods.is_empty() {
            continue;
        }
        if static_context_owner_is_emitted(&owner, ctx) {
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
                .method_signature_by_base_name_and_arg_types_with_facts(
                    &ty.name, name, &arg_types, ctx.facts,
                )
                .or_else(|| {
                    signatures
                        .method_signature_by_base_name_and_arg_types(&ty.name, name, &arg_types)
                })
                .or_else(|| {
                    signatures.method_signature_by_base_name_and_arity(&ty.name, name, args.len())
                })
            {
                return Some(ContextFallbackMethodSignature {
                    rust_name,
                    arg_shapes: non_emitted_static_context_arg_shapes_from_signature(signature),
                    receiver: signature.receiver,
                    arg_shapes_are_authoritative: true,
                });
            }
            if matching_methods.len() == 1 {
                let method = matching_methods[0];
                return Some(non_emitted_static_context_fallback_signature_for_method(
                    &owner, method, ctx,
                ));
            }
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
            .method_signature_by_base_name_and_arg_types_with_facts(
                &ty.name, name, &arg_types, ctx.facts,
            )
            .or_else(|| {
                signatures.method_signature_by_base_name_and_arg_types(&ty.name, name, &arg_types)
            })
            .or_else(|| {
                signatures.method_signature_by_base_name_and_arity(&ty.name, name, args.len())
            })
        {
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: non_emitted_static_context_arg_shapes_from_signature(signature),
                receiver: signature.receiver,
                arg_shapes_are_authoritative: true,
            });
        }
        let rust_name = matching_methods[0]
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&matching_methods[0].name));
        return Some(ContextFallbackMethodSignature {
            rust_name: rust_name.clone(),
            arg_shapes: ctx
                .signatures
                .method_signature_by_rust_name(&owner, &rust_name)
                .map(non_emitted_static_context_arg_shapes_from_signature)
                .unwrap_or_else(|| {
                    non_emitted_static_context_arg_shapes_for_method(matching_methods[0], ctx)
                }),
            receiver: ctx
                .signatures
                .method_signature_by_rust_name(&owner, &rust_name)
                .map(|signature| signature.receiver)
                .unwrap_or(ReceiverPlan::Shared),
            arg_shapes_are_authoritative: true,
        });
    }
    None
}

fn source_context_resolved_static_method_fallback_signature(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ContextFallbackMethodSignature> {
    if receiver.is_some() {
        return None;
    }
    let resolved = resolved?;
    if resolved.kind != CallKind::Static {
        return None;
    }
    let owner = &resolved.declaring_type;
    let ty = find_context_type_by_key(ctx.program, owner)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, owner))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, owner))?;
    let matching_methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if method.modifiers.contains(&Modifier::Static)
                    && method.name == name
                    && method.params.len() == args.len() =>
            {
                Some(method)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if matching_methods.is_empty() {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    let owner_is_emitted = static_context_owner_is_emitted(owner, ctx);
    if owner_is_emitted
        && let Some(signature) = ctx
            .signatures
            .method_signature_by_rust_name(owner, &resolved.rust_name)
    {
        return Some(context_fallback_signature_for_signature(
            resolved.rust_name.clone(),
            signature,
        ));
    }
    if let Some((rust_name, signature)) = ctx
        .signatures
        .method_signature_by_base_name_and_arg_types_with_facts(owner, name, &arg_types, ctx.facts)
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arg_types(owner, name, &arg_types)
        })
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(owner, name, args.len())
        })
    {
        return Some(ContextFallbackMethodSignature {
            rust_name,
            arg_shapes: if owner_is_emitted {
                signature
                    .params
                    .iter()
                    .map(|param| param.shape.clone())
                    .collect()
            } else {
                non_emitted_static_context_arg_shapes_from_signature(signature)
            },
            receiver: signature.receiver,
            arg_shapes_are_authoritative: true,
        });
    }
    if matching_methods.len() == 1 {
        let method = matching_methods[0];
        return Some(if owner_is_emitted {
            context_fallback_signature_for_method(owner, method, ctx)
        } else {
            non_emitted_static_context_fallback_signature_for_method(owner, method, ctx)
        });
    }
    None
}

fn non_emitted_static_context_fallback_signature_for_method(
    owner: &str,
    method: &MethodDecl,
    ctx: LowerCtx,
) -> ContextFallbackMethodSignature {
    let mut signature = context_fallback_signature_for_method(owner, method, ctx);
    signature.arg_shapes = non_emitted_static_context_arg_shapes_for_method(method, ctx);
    signature.arg_shapes_are_authoritative = true;
    signature
}

fn non_emitted_static_context_arg_shapes_from_signature(
    signature: &crate::signature::MethodSignaturePlan,
) -> Vec<RustSlotShape> {
    signature
        .params
        .iter()
        .map(non_emitted_static_context_arg_shape_from_param_plan)
        .collect()
}

fn non_emitted_static_context_arg_shapes_for_method(
    method: &MethodDecl,
    ctx: LowerCtx,
) -> Vec<RustSlotShape> {
    method
        .params
        .iter()
        .map(|param| {
            non_emitted_static_context_arg_shape(
                source_context_param_shape(param, ctx),
                &param.ty,
                param.is_varargs,
            )
        })
        .collect()
}

fn non_emitted_static_context_arg_shape_from_param_plan(
    param: &crate::signature::ParamPlan,
) -> RustSlotShape {
    non_emitted_static_context_arg_shape(param.shape.clone(), &param.java_ty, param.is_varargs)
}

fn non_emitted_static_context_arg_shape(
    mut shape: RustSlotShape,
    java_ty: &JavaType,
    is_varargs: bool,
) -> RustSlotShape {
    if matches!(shape.ownership, OwnershipShape::Owned)
        && source_context_reference_param_should_borrow(java_ty, is_varargs)
    {
        shape = source_context_borrowed_param_shape(shape, java_ty);
    }
    shape
}

fn source_context_current_static_method_fallback_signature(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<ContextFallbackMethodSignature> {
    if receiver.is_some() {
        return None;
    }
    let current_type = ctx.current_type?;
    let ty = find_context_type_by_key(ctx.program, current_type)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, current_type))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, current_type))?;
    let matching_methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if method.modifiers.contains(&Modifier::Static)
                    && method.name == name
                    && method.params.len() == args.len() =>
            {
                Some(method)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if matching_methods.is_empty() {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    if let Some((rust_name, signature)) = ctx
        .signatures
        .method_signature_by_base_name_and_arg_types_with_facts(
            current_type,
            name,
            &arg_types,
            ctx.facts,
        )
        .or_else(|| {
            ctx.signatures.method_signature_by_base_name_and_arg_types(
                current_type,
                name,
                &arg_types,
            )
        })
        .or_else(|| {
            ctx.signatures
                .method_signature_by_base_name_and_arity(current_type, name, args.len())
        })
    {
        return Some(ContextFallbackMethodSignature {
            rust_name,
            arg_shapes: signature
                .params
                .iter()
                .map(|param| param.shape.clone())
                .collect(),
            receiver: signature.receiver,
            arg_shapes_are_authoritative: false,
        });
    }
    if let Some(method) =
        unique_static_method_matching_known_arg_types(&matching_methods, &arg_types)
    {
        let rust_name = method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
        if let Some(signature) = ctx
            .signatures
            .method_signature_by_rust_name(current_type, &rust_name)
        {
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: signature
                    .params
                    .iter()
                    .map(|param| param.shape.clone())
                    .collect(),
                receiver: signature.receiver,
                arg_shapes_are_authoritative: false,
            });
        }
        return Some(ContextFallbackMethodSignature {
            rust_name,
            arg_shapes: source_context_param_shapes(method, ctx),
            receiver: ReceiverPlan::Shared,
            arg_shapes_are_authoritative: false,
        });
    }
    if matching_methods.len() == 1 {
        let method = matching_methods[0];
        return Some(context_fallback_signature_for_method(
            current_type,
            method,
            ctx,
        ));
    }
    None
}

fn unique_static_method_matching_known_arg_types<'a>(
    methods: &[&'a MethodDecl],
    arg_types: &[JavaType],
) -> Option<&'a MethodDecl> {
    let mut matches = methods
        .iter()
        .copied()
        .filter_map(|method| {
            method_params_known_arg_type_score(&method.params, arg_types)
                .map(|score| (method, score))
        })
        .collect::<Vec<_>>();
    matches.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    match matches.as_slice() {
        [(method, _)] => Some(*method),
        [(method, best), (_, next), ..] if best > next => Some(*method),
        _ => None,
    }
}

fn method_params_known_arg_type_score(params: &[Param], arg_types: &[JavaType]) -> Option<usize> {
    if params.len() != arg_types.len() {
        return None;
    }
    params
        .iter()
        .zip(arg_types)
        .try_fold(0usize, |score, (param, arg_ty)| {
            known_arg_type_param_match_score(arg_ty, &param.ty).map(|next| score + next)
        })
}

fn known_arg_type_param_match_score(arg_ty: &JavaType, param_ty: &JavaType) -> Option<usize> {
    if matches!(arg_ty, JavaType::Unknown(_)) {
        return Some(0);
    }
    if same_concrete_java_type(arg_ty, param_ty) {
        return Some(2);
    }
    if primitive_widening_arg_matches_param(arg_ty, param_ty) {
        return Some(1);
    }
    None
}

fn primitive_widening_arg_matches_param(arg_ty: &JavaType, param_ty: &JavaType) -> bool {
    let (JavaType::Primitive(arg), JavaType::Primitive(param)) = (arg_ty, param_ty) else {
        return false;
    };
    matches!(
        (arg, param),
        (PrimitiveType::Byte, PrimitiveType::Short)
            | (PrimitiveType::Byte, PrimitiveType::Int)
            | (PrimitiveType::Byte, PrimitiveType::Long)
            | (PrimitiveType::Byte, PrimitiveType::Float)
            | (PrimitiveType::Byte, PrimitiveType::Double)
            | (PrimitiveType::Short, PrimitiveType::Int)
            | (PrimitiveType::Short, PrimitiveType::Long)
            | (PrimitiveType::Short, PrimitiveType::Float)
            | (PrimitiveType::Short, PrimitiveType::Double)
            | (PrimitiveType::Char, PrimitiveType::Int)
            | (PrimitiveType::Char, PrimitiveType::Long)
            | (PrimitiveType::Char, PrimitiveType::Float)
            | (PrimitiveType::Char, PrimitiveType::Double)
            | (PrimitiveType::Int, PrimitiveType::Long)
            | (PrimitiveType::Int, PrimitiveType::Float)
            | (PrimitiveType::Int, PrimitiveType::Double)
            | (PrimitiveType::Long, PrimitiveType::Float)
            | (PrimitiveType::Long, PrimitiveType::Double)
            | (PrimitiveType::Float, PrimitiveType::Double)
    )
}

fn retarget_receiverless_current_static_call<'a>(
    target: CallTargetPlan<'a>,
    fallback_signature: Option<&ContextFallbackMethodSignature>,
) -> CallTargetPlan<'a> {
    let Some(signature) = fallback_signature else {
        return target;
    };
    if signature.receiver != ReceiverPlan::None {
        return target;
    }
    match target {
        CallTargetPlan::FallbackFree { .. } | CallTargetPlan::FallbackSelf { .. } => {
            CallTargetPlan::StaticOwner {
                owner: "Self".to_string(),
                rust_name: signature.rust_name.clone(),
            }
        }
        target => target,
    }
}

fn retarget_receiverless_resolved_static_owner_call<'a>(
    target: CallTargetPlan<'a>,
    receiver: Option<&Expr>,
    resolved: Option<&ResolvedCall>,
    ctx: LowerCtx,
) -> CallTargetPlan<'a> {
    if receiver.is_some() {
        return target;
    }
    let Some(resolved) = resolved else {
        return target;
    };
    if resolved.kind != CallKind::Static {
        return target;
    }
    let Some(current_type) = ctx.current_type else {
        return target;
    };
    if crate::stdlib::canonical_external_receiver_type_key(current_type)
        == crate::stdlib::canonical_external_receiver_type_key(&resolved.declaring_type)
    {
        return target;
    }
    if find_context_type_by_key(ctx.program, &resolved.declaring_type).is_none()
        && find_context_type_by_key(ctx.support_local_type_program, &resolved.declaring_type)
            .is_none()
        && find_context_type_by_key(ctx.source_summary_type_program, &resolved.declaring_type)
            .is_none()
    {
        return target;
    }
    let rust_name = match target {
        CallTargetPlan::StaticSelf { rust_name } => rust_name.to_string(),
        CallTargetPlan::StaticOwner { owner, rust_name } if owner == "Self" => rust_name,
        target => return target,
    };
    CallTargetPlan::StaticOwner {
        owner: crate::call_plan::rust_static_owner_type_name_with_facts(
            &resolved.declaring_type,
            ctx.facts,
        ),
        rust_name,
    }
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
    if let Some(receiver) = receiver
        && fallback_receiver_is_static_owner_expr(receiver, env, ctx)
    {
        return None;
    }
    let shape_ctx = ExprShapeContext {
        signatures: Some(ctx.signatures),
        representations: Some(ctx.representations),
        facts: Some(ctx.facts),
    };
    let receiver_uses_dyn_trait_slots = receiver
        .map(|receiver| receiver_expr_uses_dyn_trait_slots(receiver, env, ctx))
        .unwrap_or(false);
    let arg_types = args
        .iter()
        .map(|arg| infer_expr_shape_with_context(arg, env, shape_ctx).java_ty)
        .collect::<Vec<_>>();
    let snake_name = crate::parser::to_snake_case(name);
    for owner in fallback_receiver_type_name_candidates(receiver, name, args.len(), env, ctx) {
        let ty = if receiver.is_none() {
            find_context_type_by_key(ctx.program, &owner)
                .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
                .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))
        } else {
            find_context_type_by_key(ctx.support_local_type_program, &owner)
                .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))
        };
        let Some(ty) = ty else {
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
            let owner_is_emitted = static_context_owner_is_emitted(&owner, ctx)
                || static_context_owner_is_emitted(&ty.name, ctx);
            return Some(ContextFallbackMethodSignature {
                rust_name,
                arg_shapes: if owner_is_emitted {
                    effective_signature
                        .params
                        .iter()
                        .map(|param| param.shape.clone())
                        .collect()
                } else {
                    source_context_param_shapes(method, ctx)
                        .into_iter()
                        .zip(effective_signature.params.iter())
                        .map(|(_, solved)| {
                            non_emitted_static_context_arg_shape_from_param_plan(solved)
                        })
                        .collect()
                },
                receiver: effective_signature.receiver,
                arg_shapes_are_authoritative: true,
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
                receiver: ReceiverPlan::Shared,
                arg_shapes_are_authoritative: true,
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
    if matches!(
        receiver_shape.dispatch,
        DispatchShape::DynTrait { .. } | DispatchShape::BoxedDynTrait { .. }
    ) {
        return true;
    }
    let effective_facts = crate::effective_facts::EffectiveFacts::new(ctx.facts);
    let optional_element_ty = receiver_shape.optional_element_type();
    let represented_ty = optional_element_ty
        .as_ref()
        .unwrap_or(&receiver_shape.java_ty);
    matches!(
        effective_facts.object_representation_override_strategy_for(represented_ty),
        Some(HierarchyStrategy::DynTrait { .. })
    )
}

fn find_context_type_by_key<'a>(program: &'a Program, type_key: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_context_type_by_key_in_type(ty, type_key))
}

fn context_type_key_is_summary_only(ctx: LowerCtx, type_key: &str) -> bool {
    find_context_type_by_key(ctx.source_summary_type_program, type_key).is_some()
        && find_context_type_by_key(ctx.program, type_key).is_none()
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
            let rust_receiver_name =
                crate::slot_render::rust_name_for_java_named_type(&receiver_name);
            let nested_receiver_candidates =
                nested_type_owner_candidates_for_receiver(&receiver_name, ctx);
            push_unique_receiver_type_name(&mut candidates, receiver_name);
            push_unique_receiver_type_name(&mut candidates, rust_receiver_name);
            for nested_receiver in nested_receiver_candidates {
                let rust_nested_receiver =
                    crate::slot_render::rust_name_for_java_named_type(&nested_receiver);
                push_unique_receiver_type_name(&mut candidates, nested_receiver);
                push_unique_receiver_type_name(&mut candidates, rust_nested_receiver);
            }
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

fn nested_type_owner_candidates_for_receiver(receiver_name: &str, ctx: LowerCtx) -> Vec<String> {
    let Some(current_type) = ctx.current_type else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for program in [
        ctx.program,
        ctx.support_local_type_program,
        ctx.source_summary_type_program,
    ] {
        if let Some(current) = find_context_type_by_key(program, current_type) {
            collect_nested_type_owner_candidates(current, receiver_name, &mut candidates);
        }
    }
    candidates
}

fn collect_nested_type_owner_candidates(ty: &TypeDecl, receiver_name: &str, out: &mut Vec<String>) {
    for member in &ty.members {
        let Member::Type(nested) = member else {
            continue;
        };
        if type_simple_name_matches_receiver_alias(&nested.name, receiver_name) {
            push_unique_receiver_type_name(out, nested.name.clone());
        }
        collect_nested_type_owner_candidates(nested, receiver_name, out);
    }
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
        receiver_expr = adapt_optional_instance_receiver_with_slot_plan(
            receiver,
            receiver_expr,
            env,
            ctx,
            false,
            false,
        );
    }
    let index_expr = lower_expr_for_slot(&args[0], env, index_slot(), ctx);
    let get_expr = sequence_get_expect_projection(receiver_expr, index_expr);
    let actual = borrowed_sequence_element_shape_with_facts(element_ty, ctx.facts);
    if let Some(adapted) =
        lower_borrowed_sequence_get_subtype_to_concrete_slot(&get_expr, &actual, slot, ctx.facts)
    {
        return Some(adapted);
    }
    Some(apply_expr_to_slot_via_plan(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn borrowed_sequence_element_shape_with_facts(
    element_ty: JavaType,
    facts: &FactSet,
) -> RustSlotShape {
    let mut shape = crate::shape_projection::borrowed_sequence_element_shape(element_ty.clone());
    if let JavaType::Named { name, args } = &element_ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        crate::shape_projection::apply_required_dispatch(
            &mut shape,
            DispatchShape::BoxedDynTrait {
                trait_name: name.clone(),
            },
        );
        shape.ownership = OwnershipShape::SharedBorrow;
    }
    shape
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
    Some(subtype_owned_constructor_from_borrowed_projection(
        &target_ty,
        get_expr.clone(),
    ))
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
    let access_expr = lower_array_access_expr(array, index, env, ctx, refinements);
    let actual = indexed_read_shape_for_access_with_facts(&array_shape, ctx.facts)?;
    if let Some(target) = slot_target_shape(slot)
        && two_dimensional_array_expr_uses_nested_slice(array, env, ctx)
        && target.ownership == OwnershipShape::Owned
        && actual.ownership == OwnershipShape::SharedBorrow
        && same_concrete_java_type(&actual.java_ty, &target.java_ty)
        && crate::types::sequence_element_type(&actual.java_ty).is_some()
    {
        return Some(to_owned_sequence_projection(
            access_expr,
            &CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: actual.java_ty.clone(),
            },
        ));
    }
    if let Some(target) = slot_target_shape(slot)
        && target
            .optional_element_type()
            .is_some_and(|target_element_ty| target_element_ty == actual.java_ty)
        && matches!(target.ownership, OwnershipShape::SharedBorrow)
        && matches!(target.view, ViewShape::Native)
    {
        let actual_access = RustSlotShape::conservative(actual.java_ty.clone());
        return Some(planned_optional_value_for_target(
            access_expr,
            actual_access,
            target,
            ctx.facts,
        ));
    }
    if indexed_access_needs_borrowed_place(&actual)
        && indexed_access_target_slot_accepts_borrowed_place(slot)
    {
        return Some(apply_expr_to_slot_via_plan(
            force_shared_borrow_projection(access_expr),
            actual,
            slot,
            ctx.facts,
        ));
    }
    Some(apply_expr_to_slot_via_plan(
        access_expr,
        actual,
        slot,
        ctx.facts,
    ))
}

fn indexed_access_needs_borrowed_place(actual: &RustSlotShape) -> bool {
    matches!(actual.ownership, OwnershipShape::SharedBorrow)
        && !matches!(actual.java_ty, JavaType::Primitive(_) | JavaType::Void)
        && matches!(actual.view, ViewShape::Native | ViewShape::ErasedObjectView)
}

fn indexed_access_target_slot_accepts_borrowed_place(slot: &Slot) -> bool {
    slot_target_shape(slot)
        .is_some_and(|target| matches!(target.ownership, OwnershipShape::SharedBorrow))
        || matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment)
}

fn lower_map_get_call_expr_for_slot(
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
    if name != "get" || args.len() != 1 {
        return None;
    }
    let receiver = receiver.as_deref()?;
    let receiver_ty = types::infer_expr_type(receiver, env.type_env());
    let (_, value_ty, _) = map_type_args_from_java_type(&receiver_ty)?;
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
    let key = lower_map_key_arg_borrow_projection(key, &args[0], env, ctx);
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    })?;
    if matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment) {
        let get_expr = map_get_projection(lowered_receiver, key);
        return Some(apply_java_display_plan(
            get_expr,
            JavaDisplayPlan::OptionalBorrowedValue,
        ));
    }
    let actual = crate::shape_projection::borrowed_sequence_element_shape(value_ty);
    let target_shape = slot_target_shape(slot)?;
    if target_shape
        .optional_element_type()
        .is_some_and(|element_ty| element_ty == actual.java_ty)
    {
        let get_expr = map_get_projection(lowered_receiver, key);
        let actual = LoweredType::optional_element_borrow_shape(actual);
        let plan = plan_lowered_slot_conversion(&actual, slot.clone(), ctx.facts);
        return Some(apply_lowered_slot_expr_conversion(
            crate::coercion::LoweredExpr {
                expr: get_expr,
                actual,
            },
            &plan,
        ));
    }
    if target_shape.java_ty != actual.java_ty {
        return None;
    }
    let get_expr = map_get_expect_projection(lowered_receiver, key);
    Some(apply_expr_to_slot_via_plan(
        get_expr, actual, slot, ctx.facts,
    ))
}

fn lower_map_get_to_array_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let Expr::Call {
        receiver: Some(to_array_receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if !matches!(name.as_str(), "toArray" | "to_array") || args.len() > 1 {
        return None;
    }
    let Expr::Call {
        receiver: Some(map_receiver),
        name: get_name,
        args: get_args,
        resolved: get_resolved,
    } = to_array_receiver.as_ref()
    else {
        return None;
    };
    if get_name != "get" || get_args.len() != 1 {
        return None;
    }
    let target = slot_target_shape(slot)?;
    if !shape_is_owned(target) {
        return None;
    }
    let target_element_ty = crate::types::sequence_element_type(&target.java_ty)?;
    let map_receiver_ty = types::infer_expr_type(map_receiver, env.type_env());
    let (_, value_ty, _) = map_type_args_from_java_type(&map_receiver_ty)?;
    let value_element_ty = crate::types::sequence_element_type(&value_ty)?;
    if target_element_ty != value_element_ty {
        return None;
    }

    let map_get_plan = plan_call_expr_with_lower_context(
        Some(map_receiver),
        get_name,
        get_args,
        get_resolved.as_ref(),
        ctx,
        env,
    );
    let CallExprPlan::Target(
        target_plan @ CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapGet,
            ..
        },
    ) = map_get_plan
    else {
        return None;
    };
    let key_args = lower_args_for_call_target(
        &target_plan,
        Some(map_receiver),
        get_args,
        env,
        plan_call_arguments_with_facts_and_args(
            Some(map_receiver),
            get_name,
            get_resolved.as_ref(),
            get_args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        get_resolved.as_ref(),
        ctx,
        None,
    );
    let key = key_args.into_iter().next()?;
    let key = lower_map_key_arg_borrow_projection(key, &get_args[0], env, ctx);
    let lowered_receiver = lower_expr_with_refinements(map_receiver, env, ctx, refinements);
    let get_expr = map_get_projection(lowered_receiver, key);
    let value_shape = RustSlotShape::conservative(value_ty);
    let actual = LoweredType::optional_element_borrow_shape(borrowed_sequence_element_shape(
        value_shape.java_ty.clone(),
    ));
    let recovery_plan =
        plan_lowered_slot_conversion(&actual, Slot::Argument(value_shape.clone()), ctx.facts);
    let recovered = apply_lowered_slot_expr_conversion(
        crate::coercion::LoweredExpr {
            expr: get_expr,
            actual,
        },
        &recovery_plan,
    );
    Some(apply_expr_to_slot_via_plan(
        recovered,
        value_shape,
        slot,
        ctx.facts,
    ))
}

fn lower_map_get_collection_mutation_call(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let map_get_expr = receiver?;
    let Expr::Call {
        receiver: Some(map_receiver),
        name: get_name,
        args: get_args,
        resolved: get_resolved,
    } = map_get_expr
    else {
        return None;
    };
    if get_name != "get" || get_args.len() != 1 {
        return None;
    }

    let map_receiver_ty = types::infer_expr_type(map_receiver, env.type_env());
    let (_, value_ty, map_receiver_is_optional) = map_type_args_from_java_type(&map_receiver_ty)?;
    if crate::stdlib::java_collection_kind_for_type(&value_ty).is_none() {
        return None;
    }

    let outer_plan =
        plan_call_expr_with_lower_context(Some(map_get_expr), name, args, resolved, ctx, env);
    let CallExprPlan::Target(CallTargetPlan::NativeCollectionMethod {
        method:
            method @ (NativeCollectionMethod::ListPush
            | NativeCollectionMethod::SetInsert
            | NativeCollectionMethod::Extend),
        ..
    }) = outer_plan
    else {
        return None;
    };

    let map_get_plan = plan_call_expr_with_lower_context(
        Some(map_receiver),
        get_name,
        get_args,
        get_resolved.as_ref(),
        ctx,
        env,
    );
    let CallExprPlan::Target(
        target @ CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapGet,
            ..
        },
    ) = map_get_plan
    else {
        return None;
    };
    let key_args = lower_args_for_call_target(
        &target,
        Some(map_receiver),
        get_args,
        env,
        plan_call_arguments_with_facts_and_args(
            Some(map_receiver),
            get_name,
            get_resolved.as_ref(),
            get_args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        get_resolved.as_ref(),
        ctx,
        None,
    );
    let key = key_args.into_iter().next()?;
    let key = lower_map_key_arg_borrow_projection(key, &get_args[0], env, ctx);

    let mut lowered_map_receiver = lower_expr_with_refinements(map_receiver, env, ctx, refinements);
    if map_receiver_is_optional {
        lowered_map_receiver = adapt_optional_instance_receiver_with_slot_plan(
            map_receiver,
            lowered_map_receiver,
            env,
            ctx,
            true,
            false,
        );
    }
    let lowered_value_receiver = map_get_mut_expect_projection(lowered_map_receiver, key);

    let value_args = lower_args_for_call_target(
        &CallTargetPlan::NativeCollectionMethod {
            receiver: map_get_expr,
            method,
        },
        Some(map_get_expr),
        args,
        env,
        plan_call_arguments_with_facts_and_args(
            Some(map_get_expr),
            name,
            resolved,
            args,
            ctx.signatures,
            ctx.facts,
            env.type_env(),
        ),
        resolved,
        ctx,
        None,
    );
    Some(apply_call_target_with_lower_context(
        &CallTargetPlan::NativeCollectionMethod {
            receiver: map_get_expr,
            method,
        },
        Some(lowered_value_receiver),
        value_args,
        ctx,
    ))
}

fn lower_map_key_arg_borrow_projection(
    lowered: RustExpr,
    arg: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> RustExpr {
    let local_name = if let Expr::Var(name) = arg {
        Some(name.as_str())
    } else {
        None
    };
    let actual = infer_expr_shape_with_context(
        arg,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    normalize_map_key_borrow_projection(lowered, local_name, Some(&actual))
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

fn lower_map_key_set_expr_for_set_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    if !args.is_empty() || !matches!(name.as_str(), "keySet" | "key_set") {
        return None;
    }
    let target = slot_target_shape(slot)?;
    let target_ty = crate::types::optional_element_type(&target.java_ty)
        .unwrap_or_else(|| target.java_ty.clone());
    if crate::stdlib::java_collection_kind_for_type(&target_ty)
        != Some(crate::stdlib::JavaCollectionKind::Set)
    {
        return None;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    if crate::stdlib::java_collection_kind_for_type(&receiver_ty)
        .is_some_and(|kind| kind != crate::stdlib::JavaCollectionKind::Map)
    {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    Some(owned_map_keys_set_projection(lowered_receiver))
}

fn lower_map_values_expr_for_set_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
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
    if !args.is_empty() || name != "values" {
        return None;
    }
    let target = slot_target_shape(slot)?;
    let target_ty = crate::types::optional_element_type(&target.java_ty)
        .unwrap_or_else(|| target.java_ty.clone());
    if crate::stdlib::java_collection_kind_for_type(&target_ty)
        != Some(crate::stdlib::JavaCollectionKind::Set)
    {
        return None;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    if crate::stdlib::java_collection_kind_for_type(&receiver_ty)
        .is_some_and(|kind| kind != crate::stdlib::JavaCollectionKind::Map)
    {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    Some(crate::coercion::owned_map_values_set_projection(
        lowered_receiver,
    ))
}

fn lower_native_collection_constructor_from_map_view(
    ty: &JavaType,
    args: &[Expr],
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<RustExpr> {
    let target_kind = crate::stdlib::native_collection_constructor_kind_for_type(ty)?;
    let [
        Expr::Call {
            receiver: Some(receiver),
            name,
            args: call_args,
            ..
        },
    ] = args
    else {
        return None;
    };
    if !call_args.is_empty() {
        return None;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, env.type_env());
    if crate::stdlib::java_collection_kind_for_type(&receiver_ty)
        .is_some_and(|kind| kind != crate::stdlib::JavaCollectionKind::Map)
    {
        return None;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    match (name.as_str(), target_kind) {
        ("keySet" | "key_set", crate::stdlib::JavaCollectionKind::List) => Some(
            crate::coercion::owned_map_keys_vec_projection(lowered_receiver),
        ),
        ("keySet" | "key_set", crate::stdlib::JavaCollectionKind::Set) => Some(
            crate::coercion::owned_map_keys_set_projection(lowered_receiver),
        ),
        ("values", crate::stdlib::JavaCollectionKind::List) => Some(
            crate::coercion::owned_map_values_vec_projection(lowered_receiver),
        ),
        ("values", crate::stdlib::JavaCollectionKind::Set) => Some(
            crate::coercion::owned_map_values_set_projection(lowered_receiver),
        ),
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
        lowered_expr = propagate_result_projection(lowered_expr);
    }
    Some(crate::coercion::LoweredExpr {
        expr: lowered_expr,
        actual,
    })
}

fn lower_source_receiver_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<crate::coercion::LoweredExpr> {
    if slot_targets_erased_object(slot) {
        return None;
    }
    let Expr::Call { .. } = expr else {
        return None;
    };
    let method_shape = method_call_return_shape_from_receiver(expr, env, ctx);
    let actual = method_shape
        .as_ref()
        .filter(|shape| shape.ownership != OwnershipShape::SharedBorrow)
        .cloned()
        .map(|shape| LoweredType::Slot(Box::new(shape)))
        .or_else(|| {
            method_shape
                .map(|shape| optional_borrow_slot_lowered_type(LoweredType::Slot(Box::new(shape))))
        })
        .or_else(|| source_trivial_accessor_getter_lowered_type(expr, env, ctx))?;
    let expr = lower_expr_with_refinements(expr, env, ctx, refinements);
    Some(crate::coercion::LoweredExpr { expr, actual })
}

fn lower_java_object_to_string_call_expr_for_slot(
    expr: &Expr,
    env: &ShapeEnv,
    slot: &Slot,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> Option<crate::coercion::LoweredExpr> {
    if slot_targets_erased_object(slot) {
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
    if !matches!(name.as_str(), "toString" | "to_string") || !args.is_empty() {
        return None;
    }
    let receiver_name = source_receiver_type_name(receiver, env, ctx)?;
    let receiver_ty = find_context_type_by_key(ctx.program, &receiver_name)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &receiver_name));
    if let Some(receiver_ty) = receiver_ty {
        if find_declared_method_by_name_and_arity(receiver_ty, name, 0).is_some()
            || find_declared_method_by_name_and_arity(receiver_ty, "toString", 0).is_some()
            || find_declared_method_by_name_and_arity(receiver_ty, "to_string", 0).is_some()
        {
            return None;
        }
    } else {
        find_context_type_by_key(ctx.source_summary_type_program, &receiver_name)?;
    }
    let lowered_receiver = lower_expr_with_refinements(receiver, env, ctx, refinements);
    Some(crate::coercion::LoweredExpr {
        expr: apply_java_display_plan(lowered_receiver, JavaDisplayPlan::ErasedObject),
        actual: LoweredType::Slot(Box::new(value_shape(JavaType::String))),
    })
}

fn source_trivial_accessor_getter_lowered_type(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<LoweredType> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    let receiver_expr = receiver.as_deref()?;
    let receiver_name = source_receiver_type_name(receiver_expr, env, ctx)?;
    let rust_name = resolved
        .as_ref()
        .map(|resolved| resolved.rust_name.clone())
        .unwrap_or_else(|| crate::parser::to_snake_case(name));
    let mut declaring_names = Vec::new();
    if let Some(resolved) = resolved {
        declaring_names.push(resolved.declaring_type.as_str());
    }
    declaring_names.push(receiver_name.as_str());
    declaring_names.sort_unstable();
    declaring_names.dedup();
    for declaring_name in declaring_names {
        if let Some(field_name) =
            source_method_returned_field_name(declaring_name, name, args.len(), ctx)
            && let Some(field_shape) =
                signature_field_shape_for_type_aliases(ctx.signatures, declaring_name, &field_name)
        {
            return Some(source_field_getter_lowered_type(field_shape));
        }
    }
    if let Some(shape) = ctx
        .signatures
        .method_return_shape_by_rust_name(&receiver_name, &rust_name)
    {
        return Some(optional_borrow_slot_lowered_type(LoweredType::Slot(
            Box::new(shape.clone()),
        )));
    }
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
    if receiver
        .as_ref()
        .is_none_or(|receiver| is_this_receiver_expr(receiver))
    {
        return None;
    }
    let field_name = accessor_member_name(&field_name);
    let field_shape =
        signature_field_shape_for_type_aliases(ctx.signatures, &receiver_name, field_name)?;
    Some(source_field_getter_lowered_type(field_shape))
}

fn source_receiver_type_name(receiver: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> Option<String> {
    if let Expr::Var(name) = receiver
        && let Some(local_ty) = env.local_type(name)
    {
        let receiver_ty =
            crate::types::optional_element_type(local_ty).unwrap_or_else(|| local_ty.clone());
        if let JavaType::Named { name, .. } | JavaType::Unknown(name) = receiver_ty {
            return Some(name);
        }
    }
    let receiver_shape = infer_expr_shape_with_context(
        receiver,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    let receiver_ty = crate::types::optional_element_type(&receiver_shape.java_ty)
        .unwrap_or(receiver_shape.java_ty);
    match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn source_method_returned_field_name(
    receiver_name: &str,
    method_name: &str,
    arg_count: usize,
    ctx: LowerCtx,
) -> Option<String> {
    let ty = find_context_type_by_key(ctx.program, receiver_name)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, receiver_name))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, receiver_name))?;
    let method = find_declared_method_by_name_and_arity(ty, method_name, arg_count)?;
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(expr))] = body.statements.as_slice() else {
        return None;
    };
    returned_field_name_from_expr(expr).map(str::to_string)
}

fn source_null_assertion_call_expr_stmt_is_noop(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return false;
    };
    if args.is_empty()
        || !args
            .iter()
            .all(|arg| null_assertion_arg_is_non_null_slot(arg, env, ctx))
    {
        return false;
    }
    let Some(method) =
        source_call_method_decl(receiver.as_deref(), name, args.len(), resolved, ctx)
    else {
        return false;
    };
    method_is_strict_object_null_assertion(method)
}

fn source_call_method_decl<'a>(
    receiver: Option<&Expr>,
    method_name: &str,
    arg_count: usize,
    resolved: &'a Option<ResolvedCall>,
    ctx: LowerCtx<'a>,
) -> Option<&'a MethodDecl> {
    if let Some(resolved) = resolved {
        return find_context_type_by_key(ctx.program, &resolved.declaring_type)
            .or_else(|| {
                find_context_type_by_key(ctx.support_local_type_program, &resolved.declaring_type)
            })
            .or_else(|| {
                find_context_type_by_key(ctx.source_summary_type_program, &resolved.declaring_type)
            })
            .and_then(|ty| {
                find_declared_method_by_name_and_arity(ty, &resolved.rust_name, arg_count)
            });
    }
    let owner = receiver
        .and_then(static_owner_expr_candidates_for_source_call)
        .or_else(|| ctx.current_type.map(str::to_string))?;
    find_context_type_by_key(ctx.program, &owner)
        .or_else(|| find_context_type_by_key(ctx.support_local_type_program, &owner))
        .or_else(|| find_context_type_by_key(ctx.source_summary_type_program, &owner))
        .and_then(|ty| find_declared_method_by_name_and_arity(ty, method_name, arg_count))
}

fn static_owner_expr_candidates_for_source_call(receiver: &Expr) -> Option<String> {
    match receiver {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(owner, field) => static_owner_expr_candidates_for_source_call(owner)
            .map(|owner| format!("{owner}.{field}")),
        _ => None,
    }
}

fn null_assertion_arg_is_non_null_slot(arg: &Expr, env: &ShapeEnv, ctx: LowerCtx) -> bool {
    if !matches!(arg, Expr::Var(_) | Expr::This) {
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
    shape.optional_element_type().is_none()
}

fn method_is_strict_object_null_assertion(method: &MethodDecl) -> bool {
    let [param] = method.params.as_slice() else {
        return false;
    };
    if !java_type_is_object_erasure(&param.ty) {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    let [first, rest @ ..] = body.statements.as_slice() else {
        return false;
    };
    if !stmt_is_null_throw_guard_for_param(first, &param.name) {
        return false;
    }
    rest.iter()
        .all(|stmt| stmt_is_void_or_identity_return(stmt, &param.name))
}

fn java_type_is_object_erasure(ty: &JavaType) -> bool {
    matches!(
        ty,
        JavaType::Named { name, .. } | JavaType::Unknown(name)
            if crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Object")
    )
}

fn stmt_is_null_throw_guard_for_param(stmt: &Stmt, param_name: &str) -> bool {
    let Stmt::If {
        condition,
        then_block,
        else_block,
    } = stmt
    else {
        return false;
    };
    else_block.is_none()
        && expr_is_param_null_test(condition, param_name)
        && block_contains_only_throw_or_throw_block(then_block)
}

fn expr_is_param_null_test(expr: &Expr, param_name: &str) -> bool {
    let Expr::Binary { op, left, right } = expr else {
        return false;
    };
    *op == BinaryOp::Eq
        && ((matches!(left.as_ref(), Expr::Var(name) if name == param_name)
            && matches!(right.as_ref(), Expr::Literal(Literal::Null)))
            || (matches!(right.as_ref(), Expr::Var(name) if name == param_name)
                && matches!(left.as_ref(), Expr::Literal(Literal::Null))))
}

fn block_contains_only_throw_or_throw_block(block: &Block) -> bool {
    !block.statements.is_empty()
        && block.statements.iter().all(|stmt| match stmt {
            Stmt::Throw(_) => true,
            Stmt::Block(block) => block_contains_only_throw_or_throw_block(block),
            _ => false,
        })
}

fn stmt_is_void_or_identity_return(stmt: &Stmt, param_name: &str) -> bool {
    match stmt {
        Stmt::Return(None) => true,
        Stmt::Return(Some(Expr::Var(name))) => name == param_name,
        _ => false,
    }
}

fn returned_field_name_from_expr(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name.as_str()),
        Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => {
            Some(field.as_str())
        }
        _ => None,
    }
}

fn source_field_getter_lowered_type(field_shape: &RustSlotShape) -> LoweredType {
    if let Some(element_ty) = field_shape.optional_element_type() {
        let mut option_shape = field_shape.clone();
        option_shape.ownership = OwnershipShape::Owned;
        option_shape.view = ViewShape::Native;
        return optional_element_borrow_lowered_type(
            option_shape,
            element_ty,
            Some(OwnershipShape::SharedBorrow),
        )
        .unwrap_or_else(|| LoweredType::Slot(Box::new(field_shape.clone())));
    }
    let mut actual = field_shape.clone();
    actual.ownership = OwnershipShape::SharedBorrow;
    LoweredType::Slot(Box::new(actual))
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
        external_call_lowered_type_from_static_owner_plan_with_external_index(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            ctx.facts,
            ctx.external_index,
            env.type_env(),
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
        && !lowered_type_is_optional_when_inferred_is_concrete(&actual, &inferred_actual)
        && !lowered_type_has_concrete_java_type_when_inferred_is_unknown(&actual, &inferred_actual)
        && !lowered_type_slot_has_more_generic_information(&actual, &inferred_actual)
    {
        actual = LoweredType::Slot(Box::new(inferred_actual.clone()));
    }
    if matches!(actual, LoweredType::Slot(_))
        && let Some(source_actual) = source_method_return_shape_for_call(expr, env, ctx)
        && source_actual.optional_element_type().is_some()
    {
        actual = LoweredType::Slot(Box::new(source_actual));
    }
    actual = optional_borrow_slot_lowered_type(actual);
    if receiver.is_some()
        && args.is_empty()
        && crate::java_naming::java_getter_method_name(name)
        && !slot_targets_erased_object(slot)
        && let Some(target) = slot_target_shape(slot)
        && target.optional_element_type().is_some()
        && source_method_return_shape_for_call(expr, env, ctx).is_none()
    {
        actual = LoweredType::Slot(Box::new(target.clone()));
    }
    let plan = plan_call_expr_with_lower_context(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        ctx,
        env,
    );
    let context_fallback_signature = source_context_receiver_method_fallback_signature(
        receiver.as_deref(),
        name,
        args,
        env,
        ctx,
    )
    .or_else(|| {
        source_context_static_method_fallback_signature(receiver.as_deref(), name, args, env, ctx)
    })
    .or_else(|| {
        source_context_current_static_method_fallback_signature(
            receiver.as_deref(),
            name,
            args,
            env,
            ctx,
        )
    });
    let fallback_rust_name = context_fallback_signature
        .as_ref()
        .map(|signature| signature.rust_name.clone());
    let plan = apply_fallback_receiver_rust_name(plan, fallback_rust_name.clone());
    let plan = apply_fallback_self_rust_name(plan, fallback_rust_name.clone());
    let plan = apply_fallback_static_owner_rust_name(plan, fallback_rust_name);
    let target = match plan {
        CallExprPlan::Target(target) => target,
        CallExprPlan::TrivialAccessorGetter { receiver, .. }
            if !receiver
                .as_ref()
                .is_none_or(|receiver| is_this_receiver_expr(receiver)) =>
        {
            let rust_name = resolved
                .as_ref()
                .map(|resolved| resolved.rust_name.as_str())
                .unwrap_or(name);
            non_self_trivial_accessor_target(receiver, rust_name)?
        }
        CallExprPlan::TrivialAccessorGetter { .. } => return None,
    };
    let target = retarget_static_owner_call_for_instance_field_receiver(
        target,
        receiver.as_deref(),
        env,
        ctx,
    );
    let target = retarget_java_logging_facade_call(target, receiver.as_deref(), name);
    let target =
        retarget_receiverless_current_static_call(target, context_fallback_signature.as_ref());
    let target = retarget_receiverless_resolved_static_owner_call(
        target,
        receiver.as_deref(),
        resolved.as_ref(),
        ctx,
    );
    let arg_plan = plan_call_arguments_with_facts_and_args(
        receiver.as_deref(),
        name,
        resolved.as_ref(),
        args,
        ctx.signatures,
        ctx.facts,
        env.type_env(),
    );
    let arg_plan = apply_context_fallback_signature_arg_shapes(
        arg_plan,
        context_fallback_signature.as_ref(),
        ctx.facts,
    );
    let lowered_args = lower_args_for_call_target_with_temporaries(
        &target,
        receiver.as_deref(),
        args,
        env,
        arg_plan,
        resolved.as_ref(),
        ctx,
        None,
    );
    let lowered_receiver = target.receiver_expr().map(|receiver| {
        let lowered = lower_expr_with_refinements(receiver, env, ctx, refinements);
        adapt_call_target_receiver(&target, receiver, lowered, env, ctx, refinements)
    });
    let mut lowered_expr =
        apply_call_target_with_lower_context(&target, lowered_receiver, lowered_args.args, ctx);
    lowered_expr = wrap_call_expr_with_argument_temporaries(lowered_expr, lowered_args.temporaries);
    if external_call_returns_rust_result_with_lower_context(
        receiver.as_deref(),
        name,
        args.len(),
        ctx,
        env,
        ctx.current_type,
    ) || slot_expr_needs_result_propagation(expr, env, ctx)
    {
        lowered_expr = propagate_result_projection(lowered_expr);
    }
    actual = external_call_actual_for_matching_generic_slot(actual, slot);
    Some(crate::coercion::LoweredExpr {
        expr: lowered_expr,
        actual,
    })
}

fn lowered_type_is_optional_when_inferred_is_concrete(
    actual: &LoweredType,
    inferred: &RustSlotShape,
) -> bool {
    matches!(
        actual,
        LoweredType::Slot(shape)
            if shape.optional_element_type().is_some()
                && inferred.optional_element_type().is_none()
    )
}

fn optional_borrow_slot_lowered_type(actual: LoweredType) -> LoweredType {
    let LoweredType::Slot(shape) = actual else {
        return actual;
    };
    let Some(element_ty) = shape.optional_element_type() else {
        return LoweredType::Slot(shape);
    };
    if shape.ownership != OwnershipShape::SharedBorrow {
        return LoweredType::Slot(shape);
    }
    let mut option_shape = (*shape).clone();
    option_shape.ownership = OwnershipShape::Owned;
    option_shape.view = ViewShape::Native;
    optional_element_borrow_lowered_type(
        option_shape,
        element_ty,
        Some(OwnershipShape::SharedBorrow),
    )
    .unwrap_or_else(|| LoweredType::Slot(shape))
}

fn source_method_return_shape_for_call(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
) -> Option<RustSlotShape> {
    method_call_return_shape_from_receiver(expr, env, ctx)
        .or_else(|| receiverless_current_type_call_return_shape(expr, ctx))
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
        let lower_optional_branch =
            |branch: &Expr, branch_refinements: &ExprRefinements| -> RustExpr {
                if matches!(branch, Expr::Literal(Literal::Null)) {
                    RustExpr::Var("None".to_string())
                } else {
                    lower_present_expr_for_optional_target(
                        branch,
                        env,
                        target,
                        element_ty.clone(),
                        ctx,
                        branch_refinements,
                    )
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
        return lower_present_expr_for_optional_target(
            &Expr::Ternary {
                condition: Box::new(condition.clone()),
                then_expr: Box::new(then_expr.clone()),
                else_expr: Box::new(else_expr.clone()),
            },
            env,
            target,
            element_ty,
            ctx,
            refinements,
        );
    }
    if !matches!(
        (then_expr, else_expr),
        (Expr::Literal(Literal::Null), _) | (_, Expr::Literal(Literal::Null))
    ) && let Some(target) = slot_target_shape(&slot)
        && let Some(element_ty) = target.optional_element_type()
    {
        let then_optional = expr_type_is_optional(then_expr, env);
        let else_optional = expr_type_is_optional(else_expr, env);
        if then_optional != else_optional {
            let mut element_shape = target.clone();
            element_shape.java_ty = element_ty;
            let lower_branch = |branch: &Expr,
                                branch_refinements: &ExprRefinements,
                                branch_optional: bool|
             -> RustExpr {
                if branch_optional {
                    lower_expr_for_slot_with_refinements(
                        branch,
                        env,
                        slot.clone(),
                        ctx,
                        branch_refinements,
                    )
                } else {
                    lower_present_expr_for_optional_target(
                        branch,
                        env,
                        target,
                        element_shape.java_ty.clone(),
                        ctx,
                        branch_refinements,
                    )
                }
            };
            return ternary_expr(
                lower_expr_for_slot(condition, env, condition_slot(), ctx),
                lower_branch(then_expr, &then_refinements, then_optional),
                lower_branch(else_expr, &else_refinements, else_optional),
            );
        }
    }
    ternary_expr(
        lower_expr_for_slot(condition, env, condition_slot(), ctx),
        lower_expr_for_slot_with_refinements(then_expr, env, slot.clone(), ctx, &then_refinements),
        lower_expr_for_slot_with_refinements(else_expr, env, slot, ctx, &else_refinements),
    )
}

fn lower_present_expr_for_optional_target(
    expr: &Expr,
    env: &ShapeEnv,
    target: &RustSlotShape,
    element_ty: JavaType,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if let Expr::Ternary {
        condition,
        then_expr,
        else_expr,
    } = expr
        && !matches!(
            (then_expr.as_ref(), else_expr.as_ref()),
            (Expr::Literal(Literal::Null), _) | (_, Expr::Literal(Literal::Null))
        )
        && !expr_type_is_optional(then_expr, env)
        && !expr_type_is_optional(else_expr, env)
    {
        let (then_refinements, else_refinements) = branch_refinements(condition, refinements);
        let mut actual_element_shape =
            crate::conversion_plan::optional_element_slot_shape(target, element_ty.clone());
        if matches!(then_expr.as_ref(), Expr::ArrayAccess(_, _))
            || matches!(else_expr.as_ref(), Expr::ArrayAccess(_, _))
        {
            actual_element_shape.ownership = OwnershipShape::SharedBorrow;
        }
        let construction =
            optional_slot_construction_plan_for_actual(&actual_element_shape, target, ctx.facts);
        let branch_slot = Slot::Assignment(construction.element_shape.clone());
        let branch_value = ternary_expr(
            lower_expr_for_slot(condition, env, condition_slot(), ctx),
            lower_expr_for_slot_with_refinements(
                then_expr,
                env,
                branch_slot.clone(),
                ctx,
                &then_refinements,
            ),
            lower_expr_for_slot_with_refinements(
                else_expr,
                env,
                branch_slot,
                ctx,
                &else_refinements,
            ),
        );
        return planned_optional_value_for_target(
            branch_value,
            construction.element_shape,
            target,
            ctx.facts,
        );
    }
    let mut actual_element_shape = actual_shape_for_slot_expr(
        expr,
        env,
        ExprShapeContext {
            signatures: Some(ctx.signatures),
            representations: Some(ctx.representations),
            facts: Some(ctx.facts),
        },
    );
    if matches!(actual_element_shape.java_ty, JavaType::Unknown(_))
        || actual_element_shape.optional_element_type().is_some()
    {
        actual_element_shape =
            crate::conversion_plan::optional_element_slot_shape(target, element_ty.clone());
    }
    let actual_is_indexed_borrow_projection = matches!(expr, Expr::ArrayAccess(_, _))
        && actual_element_shape.ownership == OwnershipShape::SharedBorrow;
    let target_element_shape =
        crate::conversion_plan::optional_element_slot_shape(target, element_ty.clone());
    if !actual_is_indexed_borrow_projection
        && matches!(
            target_element_shape.ownership,
            OwnershipShape::Owned | OwnershipShape::CopyValue
        )
    {
        actual_element_shape = target_element_shape;
    }
    if actual_is_indexed_borrow_projection {
        actual_element_shape = RustSlotShape::conservative(actual_element_shape.java_ty.clone());
    }
    let construction =
        optional_slot_construction_plan_for_actual(&actual_element_shape, target, ctx.facts);
    let element_lowering_slot = if actual_is_indexed_borrow_projection {
        Slot::Value
    } else {
        Slot::Assignment(construction.element_shape.clone())
    };
    let lowered_expr =
        lower_expr_for_slot_with_refinements(expr, env, element_lowering_slot, ctx, refinements);
    construct_optional_value_for_target(lowered_expr, construction)
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
        unwrap_optional_shared_value(lowered)
    };
    if matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment) {
        return Some(apply_java_display_plan(
            unwrapped,
            JavaDisplayPlan::ConcatValue,
        ));
    }
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
        .map(|part| lower_string_concat_part_with_refinements(part, env, ctx, refinements))
        .collect::<Vec<_>>();
    Some(render_string_concat(&plan, args))
}

fn lower_string_concat_part_with_refinements(
    part: &Expr,
    env: &ShapeEnv,
    ctx: LowerCtx,
    refinements: &ExprRefinements,
) -> RustExpr {
    if matches!(
        part,
        Expr::Binary {
            op: BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
            ..
        }
    ) {
        let lowered =
            lower_expr_for_slot_with_refinements(part, env, Slot::Value, ctx, refinements);
        let actual = actual_shape_for_slot_expr_with_slot(
            part,
            env,
            ExprShapeContext {
                signatures: Some(ctx.signatures),
                representations: Some(ctx.representations),
                facts: Some(ctx.facts),
            },
            &Slot::StringConcatSegment,
        );
        let plan = plan_slot_conversion(actual, Slot::StringConcatSegment, ctx.facts);
        return apply_slot_expr_conversion(lowered, &plan);
    }
    lower_expr_for_slot_with_refinements(part, env, Slot::StringConcatSegment, ctx, refinements)
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
        let later_mutated_names = statement_suffix_mutated_names(&block.statements[index + 1..]);
        let current_stmt_read_counts = statement_read_counts(stmt);
        let stmt_ctx = LowerCtx {
            later_read_names: Some(&later_read_names),
            later_mutated_names: Some(&later_mutated_names),
            current_stmt_read_counts: Some(&current_stmt_read_counts),
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
