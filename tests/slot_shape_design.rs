use std::collections::{HashMap, HashSet};

use new_java2rust::access_lowering::{lower_bare_name, lower_field_access};
use new_java2rust::array_lowering::render_array_expr;
use new_java2rust::array_plan::{ArrayExprPlan, plan_array_expr};
use new_java2rust::call_lowering::{
    apply_call_target, apply_class_literal_token, apply_new_target, apply_new_target_with_facts,
};
use new_java2rust::call_plan::{
    ArgumentLoweringPlan, CallExceptionPropagationPlan, CallExprPlan, CallStatementPlan,
    CallTargetPlan, NativeCollectionMethod, NativeCollectionNewTarget,
    NativeCollectionStaticMethod, NativeJavaInstanceMethod, NativeJavaStaticMethod,
    TrivialAccessorCallPlan, external_call_lowered_type, external_constructor_arg_shapes,
    native_collection_arg_shapes, native_collection_arg_shapes_with_facts,
    native_collection_lowered_type, plan_call_arguments, plan_call_arguments_with_facts,
    plan_call_arguments_with_facts_and_args, plan_call_exception_propagation, plan_call_expr,
    plan_call_statement, plan_call_target, plan_call_target_with_facts, plan_constructor_arguments,
    plan_new_target, plan_new_target_with_facts, plan_trivial_accessor_call,
    resolved_call_arg_shapes, resolved_constructor_arg_shapes,
};
use new_java2rust::coercion::Coercion;
use new_java2rust::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, DelegatedConstructorTrailingStmt,
    plan_constructor_delegation, plan_constructor_field_values,
};
use new_java2rust::conversion_lowering::{apply_cast_conversion, apply_instanceof};
use new_java2rust::conversion_plan::{
    AssignmentStatementPlan, CastAndInstanceOfPlanningBoundary, InstanceOfCheck,
    JavaCastSemanticsPolicy, LoweredType, actual_shape_for_slot_expr_with_slot, argument_slot,
    assignment_statement_plan_for_target, cast_and_instanceof_planning_boundary,
    plan_cast_conversion, plan_instanceof,
};
use new_java2rust::dependency_mode::DependencyMode;
use new_java2rust::diagnostics::Severity;
use new_java2rust::effective_facts::EffectiveFacts;
use new_java2rust::exception_lowering::{
    CatchRenderArm, exception_cause_or_self_if_instance_expr, exception_expr_from_new,
    exception_expr_from_new_with_cause_hint, exception_result_type, fallback_exception_expr,
    java_exception_support_item, new_exception_expr, new_exception_expr_with_cause, return_stmt,
    throw_stmt, try_catch_stmt, try_finally_scope_stmt, try_with_resources_scope_stmt,
};
use new_java2rust::exception_plan::{
    JavaExceptionSupportPlan, MethodExceptionPlan, ThrowExprPlan, TryBlockCompletion, TryStmtPlan,
    catch_match_class_names, exception_class_name, plan_java_exception_support,
    plan_method_exceptions, plan_program_exceptions, plan_throw_expr, plan_try_stmt,
    try_resource_names,
};
use new_java2rust::expr_render::{
    binary_expr, indexed_expr, literal_expr, raw_expr, super_expr, ternary_expr, this_expr,
};
use new_java2rust::expr_shape::{
    ExprShapeContext, ShapeEnv, infer_assignment_target_shape,
    infer_assignment_target_shape_with_context, infer_expr_shape, infer_expr_shape_with_context,
    learn_stmt_shapes, shape_env_for_constructor, shape_env_for_method, shape_env_for_type,
};
use new_java2rust::external_facts::parse_external_facts;
use new_java2rust::external_packs::parse_external_pack_manifest;
use new_java2rust::facts::{
    AccessorInlinePolicy, CaptureOrigin, CaptureOwnerKind, CaptureSource, ClosedHierarchyFact,
    ControlFlowFactKind, DispatchRequirementFact, DispatchRequirementKind,
    ExternalApiArgumentEffectFact, ExternalApiEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalConstructorFact, ExternalCrateConstructorMappingFact,
    ExternalFieldFact, FactProvenance, FactSet, FieldKey as FactFieldKey, FieldNullabilityFact,
    FieldNullabilityProvenance, HierarchyEdgeFact, HierarchyOverride, HierarchyStrategy,
    MethodEffectFact, MethodKey as FactMethodKey, NullabilityState, ObjectSafetyViolation,
    OwnedRecoveryCapability, OwnedRecoveryCapabilityFact, ReceiverMutabilityFact, RecursionKind,
    TrivialAccessorFact, TrivialAccessorKind, TypeCapabilityOverride,
};
use new_java2rust::item_assembly::{
    constructor_default_field_value, constructor_function_shell, constructor_return_body,
    delegating_constructor_return_body, impl_item_for_type, method_function_shell,
    method_function_shell_with_facts, method_function_shell_with_facts_and_exception_plan,
    raw_item, struct_item_for_type,
};
use new_java2rust::java_ir::{
    Block, ConstructorInvocationKind, Expr, JavaType, LambdaBody, LambdaParam, Literal, Member,
    Modifier, PrimitiveType, Stmt, WildcardBound,
};
use new_java2rust::literal_lowering::literal_to_rust;
use new_java2rust::loop_lowering::{
    assemble_for_loop, assignment_condition_loop, continue_with_updates,
};
use new_java2rust::loop_plan::{
    ForBodyStmtPlan, PostLoopRefinementPlan, WhileStmtPlan, plan_for_body_stmt, plan_while_stmt,
    post_loop_refinement_for_while,
};
use new_java2rust::lower::{lower_program_with_facts, lower_program_with_plans};
use new_java2rust::lowering_diagnostics::collect_lowering_diagnostic_plans;
use new_java2rust::method_reference_lowering::render_method_reference;
use new_java2rust::method_reference_plan::{MethodReferencePlan, plan_method_reference};
use new_java2rust::nullability_plan::{TernaryNullRefinementPlan, plan_ternary_null_refinement};
use new_java2rust::object_model::rust_enum_variant_name_from_parsed_constant;
use new_java2rust::operator_lowering::{
    EqualityComparisonPlan, NullComparisonMethod, NumericBinaryPromotionPlan, binary_op_to_rust,
    lower_equality_comparison_expr, lower_equality_comparison_expr_for_op,
    lower_null_comparison_expr, lower_numeric_binary_expr, lower_unary_expr,
    null_comparison_method, plan_equality_comparison, plan_equality_comparison_with_facts,
    plan_numeric_binary_promotion, stream_unary_prefix,
};
use new_java2rust::parser::{parse_expr, parse_java_source, parse_type};
use new_java2rust::representation::{
    BoxedDynObjectRepresentationInvariant, BoxedDynObjectRepresentationPolicy,
    ClosedEnumFactPolicy, ClosedEnumProof, ClosedEnumProofPolicy, HierarchyEdgeDiagnosticOutcome,
    ObjectRepresentation, ObjectRepresentationRequest, ObjectSafetyFactPolicy, ObjectSafetyPolicy,
    RepresentationDispatchPolicy, RepresentationHierarchyFactPolicy, RepresentationLookupFallback,
    RepresentationProvenance, TypeRepresentation, anonymous_class_diagnostic_policy,
    boxed_dyn_object_representation_policy, closed_enum_fact_policy, closed_enum_inference_policy,
    closed_enum_proof_policy, extend_representation_facts_for_source_summary_superclass_boundaries,
    hierarchy_edge_diagnostic_policy, hierarchy_override_application_policy,
    hierarchy_override_validation_policy, object_hierarchy_context, object_representation_policy,
    object_safety_fact_policy, plan_representations, representation_dispatch_policy,
    representation_hierarchy_fact_policy, source_summary_superclass_boundary_policy,
};
use new_java2rust::representation_lowering::{
    RepresentationTypeEmission, TraitImplMethodCompatibilityError, TraitImplMethodFallbackPolicy,
    TraitImplMethodMessagePolicy, raw_type_item_for_with_facts, trait_impl_emissions_for_type,
    trait_impl_emissions_for_type_with_facts, trait_impl_method_message_policy, type_emission_for,
    type_emission_for_with_facts,
};
use new_java2rust::resolve::{resolve_program, resolve_program_with_source_program_and_facts};
use new_java2rust::rewrite::{
    AssignmentConditionNormalizedShape, AssignmentInConditionPlan, ControlFlowRewritePlan,
    MethodControlFlowPlan, NullTestKind, TailRecursiveLoopPlan, TailRecursiveNormalizedShape,
    control_flow_rewrite_diagnostics,
};
use new_java2rust::rust_ir::{RustExpr, RustItem, RustReceiver, RustStmt, RustType};
use new_java2rust::semantic_analysis::analyze_program;
use new_java2rust::shape_projection::{
    DispatchProjectionPolicy, DispatchRequirementProjectionPolicy,
    DispatchRequirementProjectionRole, EscapingRoleDispatchPolicy, SequenceElementProjectionPolicy,
    ShapeProjectionCapabilityPolicy, borrowed_string_content_shape, dispatch_projection_policy,
    dispatch_requirement_projection_policy, escaping_role_dispatch_policy,
    indexed_assignment_shape, indexed_assignment_shape_with_effective_facts,
    indexed_assignment_shape_with_facts, indexed_element_projection_policy, indexed_read_shape,
    indexed_read_shape_with_effective_facts, indexed_read_shape_with_facts,
    iteration_element_projection_policy, iteration_element_shape, owned_string_value_shape,
    param_shape_for_java_type, project_java_value_shape, return_shape_for_java_type,
    sequence_element_projection_policy, shape_projection_capability_policy,
    shape_projection_capability_policy_with_effective_facts, storage_shape_for_java_type,
    value_shape_for_java_type,
};
use new_java2rust::signature::{
    BindingBoundaryPolicy, ConstructorKey, ConstructorParamBoundaryPolicy, FieldBoundaryPolicy,
    FieldKey, LocalPlan, MergedSignatureConstraints, MethodConstraintSet, MethodKey,
    MethodParamBoundaryPolicy, MethodReturnBoundaryPolicy, ReceiverConstraint, ReceiverPlan,
    SignatureBoundaryFallback, SignatureBoundaryKind, SignatureBoundaryKindMerge,
    SignatureBoundaryKindPriorityPolicy, SignatureBoundaryRepresentationFactPolicy,
    SignatureBoundaryRepresentationPolicy, SignatureBoundaryRole, SignatureBoundaryRolePolicy,
    SignatureConstraint, SignatureConstraintAxis, SignatureConstraintConflictDiagnostic,
    SignatureConstraintPropagationKind, SignatureConstraintPropagationPolicy,
    SignatureConstraintProvenance, SignatureConstraintState, SignatureDiagnostic,
    SignatureDispatchRequirementPriorityPolicy, SignatureFlagMerge,
    SignatureMemberApiBoundaryPolicy, SignatureNativeBoundaryPolicy,
    SignatureOwnedRecoveryCapabilityPolicy, SignatureOwnedRecoveryPolicy,
    SignatureOwnershipRequirementPriorityPolicy, SignaturePlan, SignatureRequirementMerge,
    SignatureShapeRequest, SignatureSlotBaselinePolicy, SignatureSlotRole,
    SignatureViewRequirementPriorityPolicy, apply_signature_constraints, binding_boundary_policy,
    capability_diagnostics_for_signature_constraints, collect_signature_constraints,
    conservative_boundary_constraints, conservative_boundary_constraints_with_effective_facts,
    constrained_receiver_plan, constructor_param_boundary_policy,
    constructor_param_shape_constraints, field_boundary_policy, field_nullability_shape_policy,
    field_shape_constraints, field_shape_constraints_with_facts,
    field_shape_constraints_with_usage, local_shape_constraints, merge_dispatch_requirement_policy,
    merge_ownership_requirement_policy, merge_signature_boundary_kind_policy,
    merge_signature_flag_requirement_policy, merge_storage_nullability_requirement_policy,
    merge_view_requirement_policy, method_param_boundary_policy, method_param_shape_constraints,
    method_return_boundary_policy, method_return_shape_constraints, receiver_constraints,
    receiver_constraints_with_facts, receiver_constraints_with_usage,
    signature_boundary_constraints, signature_boundary_constraints_with_effective_facts,
    signature_boundary_kind_priority_policy, signature_boundary_policy,
    signature_boundary_policy_with_effective_facts, signature_boundary_representation_fact_policy,
    signature_boundary_representation_fact_policy_with_effective_facts,
    signature_boundary_representation_policy, signature_boundary_role_policies,
    signature_boundary_role_policy, signature_capability_policy,
    signature_capability_policy_with_effective_facts, signature_constraint_conflict_diagnostic,
    signature_constraint_provenance, signature_constraint_provenance_label,
    signature_constraint_state_conflict_diagnostics,
    signature_constraints_for_borrowed_content_policy,
    signature_dispatch_requirement_priority_policy, signature_fixpoint_iteration_budget,
    signature_fixpoint_saturation_policy, signature_member_api_boundary_policy,
    signature_native_boundary_policy, signature_native_boundary_policy_with_effective_facts,
    signature_owned_recovery_capability_policy,
    signature_owned_recovery_capability_policy_with_effective_facts,
    signature_owned_recovery_policy, signature_owned_recovery_policy_from_capability,
    signature_owned_recovery_policy_with_effective_facts,
    signature_ownership_requirement_priority_policy, signature_saturation_report_for_constraints,
    signature_shape_constraint_propagation_policy, signature_slot_baseline_policy,
    signature_slot_baseline_policy_with_effective_facts,
    signature_slot_constraint_propagation_policy, signature_slot_dispatch_requirement_policy,
    signature_slot_dispatch_requirement_policy_with_effective_facts,
    signature_view_requirement_priority_policy, solve_signature_constraints_to_fixpoint,
    solve_signature_constraints_to_fixpoint_result, solve_signature_shape,
    solve_signature_shape_with_facts, solve_signatures, solve_signatures_with_facts,
    source_binding_nullability_constraints, source_method_return_nullability_constraints,
};
use new_java2rust::slot_policy::{
    BorrowedContentCapabilityPolicy, BorrowedContentPolicy, BorrowedContentTypePolicy,
    apply_borrowed_content_policy, borrowed_content_capability_policy,
    borrowed_content_capability_policy_with_effective_facts, borrowed_content_type_policy,
    borrowed_content_type_policy_with_effective_facts, borrowed_return_content_policy_from_param,
    borrowed_return_content_policy_from_param_with_effective_facts,
    borrowed_return_shape_from_param, borrowed_return_shape_from_param_with_effective_facts,
    borrowed_return_shape_from_param_with_facts, constructor_param_borrowed_content_policy,
    constructor_param_borrowed_content_policy_with_effective_facts,
    method_param_borrowed_content_policy,
    method_param_borrowed_content_policy_with_effective_facts,
    method_param_borrowed_content_policy_with_facts,
};
use new_java2rust::slot_render::{
    NativeJavaRustTypePolicy, NativeNamedJavaRustTypePolicy, NativeSequenceContainer,
    SlotContentCapabilityPolicy, SlotDefaultPolicy, SlotDefaultValuePolicy,
    SlotNoDefaultMessagePolicy, SlotRustTypePolicy, SlotValueRustTypePolicy, default_expr_for_slot,
    default_expr_for_slot_with_facts, native_java_rust_type_policy,
    native_java_rust_type_policy_with_effective_facts, native_named_java_rust_type_policy,
    rust_type_for_java_type_with_facts, rust_type_for_slot, rust_type_for_slot_with_facts,
    slot_content_capability_policy, slot_content_capability_policy_with_effective_facts,
    slot_default_policy, slot_default_value_policy, slot_no_default_message_policy,
    slot_rust_type_policy, slot_rust_type_policy_with_effective_facts, slot_value_rust_type_policy,
};
use new_java2rust::slot_shape::{
    DispatchShape, MutabilityShape, OwnershipShape, RustSlotShape, TypeCapabilities, ViewShape,
};
use new_java2rust::statement_render::{
    assignment_stmt, break_stmt, continue_stmt, expr_stmt, if_stmt, local_let_stmt,
    local_let_stmt_with_facts, raw_stmt, render_block_stmt, render_enhanced_for, while_stmt,
};
use new_java2rust::stdlib::{
    class_literal_token_plan, is_borrowed_string_view_method, is_content_read_method,
    is_owned_string_result_method, java_class_support_item, java_platform_facade_support_item,
    program_uses_java_platform_facades,
};
use new_java2rust::stream::{
    FilterPlan, FlatMapPlan, StreamOp, StreamSourceIteration, build_stream_plan,
    stream_collect_actual,
};
use new_java2rust::stream_lowering::{
    StreamLambdaValue, StreamRenderContext, emit_stream_lambda_body,
    emit_stream_lambda_body_with_context, render_stream_plan,
};
use new_java2rust::stream_shape::{
    StreamSourceCapabilityPolicy, infer_stream_lambda_body_shape, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape, infer_stream_source_element_shape_with_effective_facts,
    infer_stream_source_element_shape_with_facts, stream_source_capability_policy,
    stream_source_capability_policy_with_effective_facts,
};
use new_java2rust::switch_lowering::{
    SwitchMatchRenderCase, SwitchRenderCase, render_simple_return_match_switch, render_switch,
    switch_case_condition,
};
use new_java2rust::switch_plan::{
    SWITCH_LABEL, SWITCH_MATCHED, SWITCH_VALUE, SwitchCaseLabelPlan, plan_simple_return_switch,
    plan_simple_return_switch_case, plan_switch, plan_switch_case, rewrite_switch_breaks,
};
use new_java2rust::types::{TypeEnv, list_java_type, optional_java_type, stream_element_type};
use new_java2rust::unsupported::{
    UnsupportedConstructorLowering, UnsupportedDiagnosticPolicy, UnsupportedExpressionLowering,
    UnsupportedLoweringMessagePolicy, UnsupportedMemberLowering, UnsupportedStatementLowering,
    plan_unsupported_expression, plan_unsupported_member, plan_unsupported_statement,
    unsupported_constructor_message_policy, unsupported_expression_expr,
    unsupported_expression_message_policy, unsupported_member_diagnostic_policy,
    unsupported_statement_message_policy, unsupported_statement_stmt,
};
use new_java2rust::usage::{
    BindingUse, BindingUseFacts, CallArgumentBoundary, CallArgumentBoundaryPolicy,
    CallResultBoundaryPolicy, ConstructorInvocationBoundaryPolicy, FieldUse,
    MethodApiBoundaryPolicy, MethodUsageFacts, analyze_usage, analyze_usage_with_facts,
    call_argument_boundary_policy, call_argument_use_boundary_policy, call_result_boundary_policy,
    constructor_invocation_boundary_policy, method_api_boundary_policy,
};
use new_java2rust::{
    emit, translate_source, translate_source_with_context_sources, translate_source_with_facts,
};

#[test]
fn slot_shape_keeps_string_ownership_and_view_as_separate_axes() {
    let mut shape = RustSlotShape::conservative(JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(rust_type_for_slot(&shape), RustType::String);

    shape.ownership = OwnershipShape::SharedBorrow;
    shape.view = ViewShape::Str;
    assert_eq!(
        rust_type_for_slot(&shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string()))
        }
    );
}

#[test]
fn slot_shape_projects_arrays_and_primitives_without_type_specific_ownership_variants() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(int_shape.ownership, OwnershipShape::CopyValue);
    assert_eq!(int_shape.view, ViewShape::Native);
    assert_eq!(rust_type_for_slot(&int_shape), RustType::I32);

    let mut array_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    array_shape.ownership = OwnershipShape::SharedBorrow;
    array_shape.view = ViewShape::Slice;
    assert_eq!(
        rust_type_for_slot(&array_shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Slice(Box::new(RustType::String)))
        }
    );
}

#[test]
fn slot_shape_owns_storage_defaults() {
    let storage = RustSlotShape::for_storage(JavaType::String);
    assert_eq!(storage.ownership, OwnershipShape::Owned);
    assert_eq!(storage.view, ViewShape::Native);
    assert_eq!(storage.mutability, MutabilityShape::Mutable);

    let primitive_storage = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Boolean));
    assert_eq!(primitive_storage.ownership, OwnershipShape::CopyValue);
    assert_eq!(primitive_storage.mutability, MutabilityShape::Mutable);
}

#[test]
fn type_capabilities_drive_copy_string_and_sequence_facts() {
    let string_caps = TypeCapabilities::conservative(&JavaType::String);
    assert!(string_caps.string_like);
    assert!(string_caps.clone);
    assert!(!string_caps.copy);

    let int_caps = TypeCapabilities::conservative(&JavaType::Primitive(PrimitiveType::Int));
    assert!(int_caps.copy);
    assert!(FactSet::default().is_copy_type(&JavaType::Primitive(PrimitiveType::Int)));
    assert!(
        FactSet::default()
            .can_recover_owned_from_borrowed(&JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        FactSet::default().owned_recovery_capability_for(&JavaType::Primitive(PrimitiveType::Int)),
        OwnedRecoveryCapability::Copy
    );
    assert_eq!(
        FactSet::default().owned_recovery_capability_for(&JavaType::String),
        OwnedRecoveryCapability::ExplicitClone {
            java_ty: JavaType::String,
        }
    );

    let list_ty = list_java_type(JavaType::String);
    let list_caps = TypeCapabilities::conservative(&list_ty);
    assert_eq!(list_caps.sequence_like, Some(JavaType::String));
    assert_eq!(
        FactSet::default().sequence_element_type_for(&list_ty),
        Some(JavaType::String)
    );
    assert_eq!(
        RustSlotShape::conservative(list_ty)
            .capabilities()
            .sequence_like,
        Some(JavaType::String)
    );

    let mut facts = FactSet::default();
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    assert_eq!(
        facts.owned_recovery_capability_for(&external_id),
        OwnedRecoveryCapability::ExplicitClone {
            java_ty: external_id.clone(),
        }
    );
    assert_eq!(
        facts.owned_recovery_capability_for(&external_handle),
        OwnedRecoveryCapability::NonCloneable
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(
            facts.owned_recovery_capability_for(&external_id),
        ),
        SignatureOwnedRecoveryPolicy::ProvenByExplicitClone
    );
}

#[test]
fn type_identity_preserves_generic_erasure_and_wildcard_bounds() {
    let list_of_string = list_java_type(JavaType::String);
    let list_shape = RustSlotShape::conservative(list_of_string.clone());

    assert_eq!(list_shape.type_identity.declared_java_ty, list_of_string);
    assert_eq!(
        list_shape.type_identity.generic_args,
        vec![JavaType::String]
    );
    assert_eq!(
        list_shape.type_identity.erasure,
        Some(JavaType::Named {
            name: "List".to_string(),
            args: Vec::new(),
        })
    );

    let bounded = JavaType::Wildcard(Some(WildcardBound::Extends(Box::new(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    }))));
    let wildcard_shape = RustSlotShape::conservative(bounded.clone());

    assert_eq!(wildcard_shape.type_identity.declared_java_ty, bounded);
    assert_eq!(
        wildcard_shape.type_identity.runtime_bounds,
        vec![JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        wildcard_shape.type_identity.erasure,
        Some(JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        })
    );
}

#[test]
fn facts_own_type_capability_overrides_with_provenance() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: Some(true),
            string_like: Some(false),
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Comparable".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
            final_type: Some(true),
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "ExternalBase".to_string(),
        closed: true,
        provenance: FactProvenance::UserOverride,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalBase".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExternalBaseValue".to_string(),
            variants: vec!["ExternalId".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_base = JavaType::Named {
        name: "ExternalBase".to_string(),
        args: Vec::new(),
    };
    let capabilities = facts.type_capabilities_for(&external_id);

    assert!(capabilities.copy);
    assert!(capabilities.clone);
    assert!(!capabilities.string_like);
    assert!(facts.is_copy_type(&external_id));
    assert!(facts.is_clone_type(&external_id));
    assert!(!facts.is_string_like_type(&external_id));
    assert!(facts.can_recover_owned_from_borrowed(&external_id));
    assert_eq!(facts.object_safe_trait_fact_for(&external_id), Some(false));
    assert_eq!(
        facts.object_safe_trait_fact_for_name("ExternalId"),
        Some(false)
    );
    assert!(facts.has_object_representation_facts(&external_id));
    assert!(facts.has_object_representation_facts(&external_base));
    assert!(facts.is_final_type(&external_id));
    assert!(capabilities.final_type);
    assert_eq!(
        facts.extends_type_for(&external_id),
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        facts.implemented_types_for(&external_id),
        vec![JavaType::Named {
            name: "Comparable".to_string(),
            args: Vec::new(),
        }]
    );
    let hierarchy_edges = facts.hierarchy_edges_for_type(&external_id);
    assert!(
        hierarchy_edges
            .iter()
            .any(|edge| { edge.super_name == "Object" && edge.sub_name == "ExternalId" })
    );
    assert!(
        hierarchy_edges
            .iter()
            .any(|edge| { edge.super_name == "Comparable" && edge.sub_name == "ExternalId" })
    );
    assert_eq!(
        representation_hierarchy_fact_policy(&external_id, &facts),
        RepresentationHierarchyFactPolicy {
            java_ty: external_id.clone(),
            extends: Some("Object".to_string()),
            implements: vec!["Comparable".to_string()],
            edges: vec![
                HierarchyEdgeFact {
                    super_name: "Object".to_string(),
                    sub_name: "ExternalId".to_string(),
                },
                HierarchyEdgeFact {
                    super_name: "Comparable".to_string(),
                    sub_name: "ExternalId".to_string(),
                },
            ],
        }
    );
    assert_eq!(
        facts.closed_hierarchy_fact_for_name("ExternalBase"),
        Some(true)
    );
    assert_eq!(
        facts.closed_hierarchy_variants_for("ExternalBase", &["ExternalId".to_string()]),
        Some(vec!["ExternalId".to_string()])
    );
    let mut extra_facts = FactSet::default();
    extra_facts
        .closed_hierarchy_facts
        .push(ClosedHierarchyFact {
            type_name: "InjectedBase".to_string(),
            closed: true,
            provenance: FactProvenance::CorpusAnalysis,
        });
    facts.extend_from(extra_facts);
    assert_eq!(
        facts.closed_hierarchy_fact_for_name("InjectedBase"),
        Some(true)
    );
    assert_eq!(
        capabilities.implements,
        vec![JavaType::Named {
            name: "Comparable".to_string(),
            args: Vec::new(),
        }]
    );
    assert_eq!(
        capabilities.extends,
        Some(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        facts
            .type_capability_overrides_for(&external_id)
            .next()
            .unwrap()
            .provenance,
        FactProvenance::UserOverride
    );
    assert_eq!(facts.sequence_element_type_for(&external_id), None);
}

#[test]
fn facts_saturate_reachable_hierarchy_edges_through_finite_domain() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Sub".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "Base".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::CorpusAnalysis,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Base".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Named".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::CorpusAnalysis,
        });
    let sub_ty = JavaType::Named {
        name: "Sub".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        facts.hierarchy_edges_for_type(&sub_ty),
        vec![new_java2rust::facts::HierarchyEdgeFact {
            super_name: "Base".to_string(),
            sub_name: "Sub".to_string(),
        }]
    );
    let domain = facts.hierarchy_reachability_domain("Sub");
    assert_eq!(domain.declared_slots, 4);
    assert_eq!(domain.value_lattice_height, 4);
    assert_eq!(domain.iteration_budget(), 17);

    let reachability = facts.reachable_hierarchy_edges_for_type_with_stats(&sub_ty);
    assert_eq!(reachability.stats.inserted_facts, 3);
    assert_eq!(reachability.stats.processed_items, 3);
    assert_eq!(reachability.stats.max_queue_len, 2);
    assert!(reachability.diagnostics.is_empty());
    let reachable = reachability.edges;
    assert!(
        reachable
            .iter()
            .any(|edge| edge.sub_name == "Sub" && edge.super_name == "Base"),
        "{reachable:?}"
    );
    assert!(
        reachable
            .iter()
            .any(|edge| edge.sub_name == "Sub" && edge.super_name == "Object"),
        "{reachable:?}"
    );
    assert!(
        reachable
            .iter()
            .any(|edge| edge.sub_name == "Sub" && edge.super_name == "Named"),
        "{reachable:?}"
    );
}

#[test]
fn facts_saturate_recursive_hierarchy_edges_without_unbounded_expansion() {
    let mut facts = FactSet::default();
    for (type_name, super_name) in [("A", "B"), ("B", "A")] {
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: type_name.to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: Some(JavaType::Named {
                    name: super_name.to_string(),
                    args: Vec::new(),
                }),
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::CorpusAnalysis,
            });
    }
    let a_ty = JavaType::Named {
        name: "A".to_string(),
        args: Vec::new(),
    };
    let domain = facts.hierarchy_reachability_domain("A");
    assert_eq!(domain.declared_slots, 2);
    assert_eq!(domain.value_lattice_height, 2);
    assert_eq!(domain.iteration_budget(), 5);

    let reachability = facts.reachable_hierarchy_edges_for_type_with_stats(&a_ty);
    assert_eq!(reachability.stats.inserted_facts, 1);
    assert_eq!(reachability.stats.processed_items, 1);
    assert!(reachability.diagnostics.is_empty());
    let reachable = reachability.edges;

    assert_eq!(
        reachable,
        vec![new_java2rust::facts::HierarchyEdgeFact {
            super_name: "B".to_string(),
            sub_name: "A".to_string(),
        }]
    );
}

#[test]
fn facts_report_type_capability_override_conflicts() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Comparable".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            }),
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Serializable".to_string(),
                args: Vec::new(),
            }],
            extends: Some(JavaType::Named {
                name: "BaseId".to_string(),
                args: Vec::new(),
            }),
            final_type: Some(false),
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let diagnostics = facts.diagnostics();

    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "type_capability_override_conflict"
                && diagnostic.message.contains("copy=true")
                && diagnostic.message.contains("source-proven copy=false")
        ),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "type_capability_override_conflict"
                && diagnostic.message.contains("final_type=false")
                && diagnostic.message.contains("source-proven final_type=true")
        ),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "type_capability_override_conflict"
                && diagnostic.message.contains("implements facts")
        ),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "type_capability_override_conflict"
                && diagnostic.message.contains("extends")
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn facts_allow_non_conflicting_type_capability_overrides() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(false),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    assert!(facts.diagnostics().is_empty());
}

#[test]
fn lowering_reports_fact_override_conflicts_as_diagnostics() {
    let parsed = parse_java_source(
        r#"
class ExternalUse {
    ExternalId id(ExternalId value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(false),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let lowered = lower_program_with_facts(
        &resolved.program,
        new_java2rust::diagnostics::DiagnosticBag::new(),
        &facts,
    );

    assert!(
        lowered
            .diagnostics
            .has_code("type_capability_override_conflict"),
        "{:?}",
        lowered.diagnostics
    );
}

#[test]
fn facts_do_not_render_rust_for_capability_overrides() {
    let source = include_str!("../src/facts.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("RustType"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("rust_type_for_slot"));
}

#[test]
fn types_own_java_collection_and_optional_type_helpers() {
    let source = include_str!("../src/types.rs");
    let slot_shape_source = include_str!("../src/slot_shape.rs");

    assert!(source.contains("pub fn sequence_element_type"));
    assert!(source.contains("pub fn optional_element_type"));
    assert!(source.contains("pub fn list_java_type"));
    assert!(source.contains("pub fn optional_java_type"));
    assert!(!slot_shape_source.contains("pub fn list_java_type"));
    assert!(!slot_shape_source.contains("pub fn optional_java_type"));
}

#[test]
fn slot_shape_exposes_collection_and_optional_shape_queries() {
    let string_shape = RustSlotShape::conservative(JavaType::String);
    let list_shape = RustSlotShape::conservative(list_java_type(JavaType::String));
    let optional_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));

    assert_eq!(list_shape.sequence_element_type(), Some(JavaType::String));
    assert!(list_shape.is_sequence_of(&string_shape));
    assert_eq!(
        optional_shape.optional_element_type(),
        Some(JavaType::String)
    );
    assert!(optional_shape.is_optional_of(&string_shape));
}

#[test]
fn slot_render_owns_conservative_java_to_rust_type_projection() {
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Primitive(PrimitiveType::Int),
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Direct { ty: RustType::I32 }
    );
    assert_eq!(
        native_named_java_rust_type_policy("Integer"),
        NativeNamedJavaRustTypePolicy {
            name: "Integer".to_string(),
            policy: NativeJavaRustTypePolicy::Direct { ty: RustType::I32 },
        }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            },
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Direct { ty: RustType::I32 }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &optional_java_type(JavaType::Primitive(PrimitiveType::Int)),
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Optional {
            element_ty: JavaType::Primitive(PrimitiveType::Int),
        }
    );
    assert_eq!(
        native_java_rust_type_policy(&list_java_type(JavaType::String), &FactSet::default(),),
        NativeJavaRustTypePolicy::Sequence {
            container: NativeSequenceContainer::Vec,
            element_ty: JavaType::String,
        }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::String],
            },
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Sequence {
            container: NativeSequenceContainer::HashSet,
            element_ty: JavaType::String,
        }
    );
    let facts = FactSet::default();
    let effective_facts = EffectiveFacts::new(&facts);
    assert_eq!(
        native_java_rust_type_policy_with_effective_facts(
            &list_java_type(JavaType::String),
            &effective_facts,
        ),
        native_java_rust_type_policy(&list_java_type(JavaType::String), &facts)
    );
    assert_eq!(
        native_named_java_rust_type_policy("ExternalId"),
        NativeNamedJavaRustTypePolicy {
            name: "ExternalId".to_string(),
            policy: NativeJavaRustTypePolicy::Named {
                name: "ExternalId".to_string(),
            },
        }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Named {
                name: "ExternalId".to_string(),
                args: Vec::new(),
            },
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Named {
            name: "ExternalId".to_string(),
        }
    );
    assert_eq!(
        parse_type("com.intel.genomicsdb.model.GenomicsDBVidMapProto.VidMappingPB.Builder"),
        JavaType::Named {
            name: "GenomicsDBVidMapProto.VidMappingPB.Builder".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        native_named_java_rust_type_policy("ExportConfiguration.Builder"),
        NativeNamedJavaRustTypePolicy {
            name: "ExportConfiguration.Builder".to_string(),
            policy: NativeJavaRustTypePolicy::Named {
                name: "ExportConfigurationBuilder".to_string(),
            },
        }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Named {
                name: "VidMappingPB.Builder".to_string(),
                args: Vec::new(),
            },
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Named {
            name: "VidMappingPBBuilder".to_string(),
        }
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::Direct {
            ty: RustType::Raw("Box<dyn std::any::Any>".to_string()),
        }
    );
    assert_eq!(
        native_java_rust_type_policy(&JavaType::Wildcard(None), &FactSet::default()),
        NativeJavaRustTypePolicy::WildcardObject
    );
    assert_eq!(
        native_java_rust_type_policy(
            &JavaType::Unknown("Object".to_string()),
            &FactSet::default(),
        ),
        NativeJavaRustTypePolicy::UnknownRaw {
            raw: "Object".to_string(),
        }
    );

    assert_eq!(
        slot_rust_type_policy(
            &RustSlotShape::conservative(list_java_type(JavaType::String)),
            &FactSet::default()
        ),
        SlotRustTypePolicy::OwnedOrCopy {
            ty: RustType::Vec(Box::new(RustType::String)),
        }
    );
    assert_eq!(
        slot_value_rust_type_policy(
            &RustSlotShape::conservative(list_java_type(JavaType::String)),
            &FactSet::default()
        ),
        SlotValueRustTypePolicy::NativeDispatch {
            java_ty: list_java_type(JavaType::String),
        }
    );
    assert_eq!(
        rust_type_for_slot(&RustSlotShape::conservative(list_java_type(
            JavaType::String
        ))),
        RustType::Vec(Box::new(RustType::String))
    );

    assert_eq!(
        rust_type_for_slot(&RustSlotShape::conservative(optional_java_type(
            JavaType::Primitive(PrimitiveType::Int)
        ))),
        RustType::Option(Box::new(RustType::I32))
    );

    let mut dyn_trait_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_trait_shape.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    dyn_trait_shape.ownership = OwnershipShape::SharedBorrow;
    dyn_trait_shape.view = ViewShape::DynTraitView;
    assert_eq!(
        slot_value_rust_type_policy(&dyn_trait_shape, &FactSet::default()),
        SlotValueRustTypePolicy::DynTraitView {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        slot_rust_type_policy(&dyn_trait_shape, &FactSet::default()),
        SlotRustTypePolicy::SharedBorrow {
            ty: RustType::Raw("dyn Animal".to_string()),
        }
    );
    assert_eq!(
        rust_type_for_slot(&dyn_trait_shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("dyn Animal".to_string()))
        }
    );

    let mut boxed_trait_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_trait_shape.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        slot_value_rust_type_policy(&boxed_trait_shape, &FactSet::default()),
        SlotValueRustTypePolicy::BoxedDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        slot_rust_type_policy(&boxed_trait_shape, &FactSet::default()),
        SlotRustTypePolicy::OwnedOrCopy {
            ty: RustType::Raw("Box<dyn Animal>".to_string()),
        }
    );
    assert_eq!(
        rust_type_for_slot(&boxed_trait_shape),
        RustType::Raw("Box<dyn Animal>".to_string())
    );

    let mut mutable_string_shape = RustSlotShape::conservative(JavaType::String);
    mutable_string_shape.ownership = OwnershipShape::MutableBorrow;
    assert_eq!(
        slot_value_rust_type_policy(&mutable_string_shape, &FactSet::default()),
        SlotValueRustTypePolicy::NativeDispatch {
            java_ty: JavaType::String,
        }
    );
    assert_eq!(
        slot_rust_type_policy(&mutable_string_shape, &FactSet::default()),
        SlotRustTypePolicy::MutableBorrow {
            ty: RustType::String,
        }
    );
    assert_eq!(
        slot_rust_type_policy_with_effective_facts(&mutable_string_shape, &effective_facts),
        slot_rust_type_policy(&mutable_string_shape, &facts)
    );

    let source = include_str!("../src/slot_render.rs");
    assert!(source.contains("pub enum NativeJavaRustTypePolicy"));
    assert!(source.contains("pub fn native_java_rust_type_policy"));
    assert!(source.contains("pub fn native_java_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub struct NativeNamedJavaRustTypePolicy"));
    assert!(source.contains("pub fn native_named_java_rust_type_policy"));
    assert!(source.contains("pub fn render_native_java_rust_type_policy"));
    assert!(source.contains("pub fn render_native_java_rust_type_policy_with_effective_facts"));
    assert!(!source.contains("\nfn native_named_java_rust_type_policy("));
    assert!(source.contains("pub enum SlotValueRustTypePolicy"));
    assert!(source.contains("pub fn slot_value_rust_type_policy"));
    assert!(source.contains("pub fn slot_value_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub fn render_slot_value_rust_type_policy"));
    assert!(source.contains("pub fn render_slot_value_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub enum SlotRustTypePolicy"));
    assert!(source.contains("pub fn slot_rust_type_policy"));
    assert!(source.contains("pub fn slot_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub fn render_slot_rust_type_policy"));
    assert!(source.contains("EffectiveFacts::new(facts)"));
    assert_eq!(source.matches("facts.is_string_like_type").count(), 1);
    assert_eq!(source.matches("facts.sequence_element_type_for").count(), 1);
}

#[test]
fn slot_render_projects_erased_object_facts_inside_native_generic_containers() {
    let erased_handle = JavaType::Named {
        name: "ExternalReader".to_string(),
        args: Vec::new(),
    };
    let final_payload = JavaType::Named {
        name: "ExternalOwner.Payload".to_string(),
        args: Vec::new(),
    };
    let concrete_facade = JavaType::Named {
        name: "IntervalList".to_string(),
        args: Vec::new(),
    };
    let map_ty = JavaType::Named {
        name: "Map".to_string(),
        args: vec![JavaType::String, erased_handle.clone()],
    };
    let final_payload_map_ty = JavaType::Named {
        name: "Map".to_string(),
        args: vec![JavaType::String, final_payload.clone()],
    };
    let final_payload_list_ty = list_java_type(final_payload.clone());
    let facade_list_ty = list_java_type(concrete_facade.clone());
    let list_ty = list_java_type(erased_handle.clone());
    let optional_ty = optional_java_type(erased_handle.clone());
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalReader".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "IntervalList".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalOwner.Payload".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "IntervalList".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: Some(false),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalOwner.Payload".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: Some(false),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    assert_eq!(
        native_java_rust_type_policy(&erased_handle, &facts),
        NativeJavaRustTypePolicy::Direct {
            ty: RustType::Raw("Box<dyn std::any::Any>".to_string()),
        }
    );
    assert_eq!(
        native_java_rust_type_policy(&concrete_facade, &facts),
        NativeJavaRustTypePolicy::Named {
            name: "IntervalList".to_string(),
        }
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&map_ty, &facts),
        RustType::Raw("std::collections::HashMap<String, Box<dyn std::any::Any>>".to_string())
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&final_payload_map_ty, &facts),
        RustType::Raw("std::collections::HashMap<String, ExternalOwnerPayload>".to_string())
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&list_ty, &facts),
        RustType::Vec(Box::new(RustType::Raw(
            "Box<dyn std::any::Any>".to_string()
        )))
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&final_payload_list_ty, &facts),
        RustType::Vec(Box::new(RustType::Named(
            "ExternalOwnerPayload".to_string()
        )))
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&optional_ty, &facts),
        RustType::Option(Box::new(RustType::Raw(
            "Box<dyn std::any::Any>".to_string()
        )))
    );
    assert_eq!(
        rust_type_for_java_type_with_facts(&facade_list_ty, &facts),
        RustType::Vec(Box::new(RustType::Named("IntervalList".to_string())))
    );

    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    assert!(effective_facts_source.contains("object_representation_override_strategy_for"));
    assert!(slot_render_source.contains("object_representation_override_strategy_for"));
    assert!(!slot_render_source.contains("representation_for("));
}

#[test]
fn native_map_get_into_final_facade_local_keeps_value_shape() {
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalOwner.Payload".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalOwner.Payload".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: Some(false),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let source = r#"
import java.util.Map;

class NativeMapFinalFacadeValueUse {
    String label(Map<String, ExternalOwner.Payload> payloads, String name) {
        ExternalOwner.Payload payload = payloads.get(name);
        return payload.getName();
    }
}
"#;
    let parsed = parse_java_source(source);
    let resolved = resolve_program_with_source_program_and_facts(
        &parsed.program,
        parsed.diagnostics,
        &parsed.program,
        &facts,
    );
    let representations = plan_representations(&resolved.program, &facts);
    let usage = analyze_usage_with_facts(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = &resolved.program.types[0];
    let method = match &ty.members[0] {
        Member::Method(method) => method,
        _ => panic!("expected method"),
    };
    let local_shape = signatures.required_local_shape(
        signatures.method_local_shapes_or_empty(ty, method),
        "payload",
    );
    assert_eq!(local_shape.dispatch, DispatchShape::Concrete);
    assert_eq!(local_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(local_shape.view, ViewShape::Native);

    let translation = translate_source_with_facts(source, &facts);

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "let payload: &ExternalOwnerPayload = payloads.get(&name).expect(\"Java null receiver\");"
        ) && translation.rust.contains("return payload.get_name();")
            && !translation
                .rust
                .contains("let payload: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
}

#[test]
fn slot_render_owns_default_expressions_for_planned_views() {
    let mut string_view = RustSlotShape::conservative(JavaType::String);
    string_view.ownership = OwnershipShape::SharedBorrow;
    string_view.view = ViewShape::Str;
    assert_eq!(
        slot_default_policy(&string_view, &FactSet::default()),
        SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::EmptyStringView,
        }
    );
    assert_eq!(
        slot_default_value_policy(&string_view, &FactSet::default()),
        SlotDefaultValuePolicy::EmptyStringView
    );
    assert_eq!(
        default_expr_for_slot(&string_view),
        RustExpr::Literal("\"\"".to_string())
    );

    let mut slice_view = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    slice_view.ownership = OwnershipShape::SharedBorrow;
    slice_view.view = ViewShape::Slice;
    assert_eq!(
        slot_default_policy(&slice_view, &FactSet::default()),
        SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::EmptySliceView,
        }
    );
    assert_eq!(
        slot_default_value_policy(&slice_view, &FactSet::default()),
        SlotDefaultValuePolicy::EmptySliceView
    );
    assert_eq!(
        default_expr_for_slot(&slice_view),
        RustExpr::Raw("&[]".to_string())
    );

    assert_eq!(
        slot_default_policy(
            &RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Boolean)),
            &FactSet::default()
        ),
        SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::PrimitiveBooleanZero,
        }
    );
    assert_eq!(
        slot_default_policy(
            &RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int)),
            &FactSet::default()
        ),
        SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::PrimitiveNumericZero,
        }
    );
    assert_eq!(
        slot_default_policy(
            &RustSlotShape::conservative(JavaType::String),
            &FactSet::default()
        ),
        SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::EmptyString,
        }
    );
    assert_eq!(
        slot_default_value_policy(
            &RustSlotShape::conservative(list_java_type(JavaType::String)),
            &FactSet::default()
        ),
        SlotDefaultValuePolicy::EmptySequence
    );
    assert_eq!(
        default_expr_for_slot(&RustSlotShape::conservative(JavaType::String)),
        RustExpr::Raw("String::new()".to_string())
    );

    let mut enum_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    enum_shape.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    assert_eq!(
        slot_default_policy(&enum_shape, &FactSet::default()),
        SlotDefaultPolicy::NoDefaultForClosedEnum {
            enum_name: "AnimalValue".to_string(),
        }
    );
    let enum_no_default =
        slot_no_default_message_policy(&slot_default_policy(&enum_shape, &FactSet::default()));
    assert_eq!(
        enum_no_default,
        SlotNoDefaultMessagePolicy {
            message: "no default value is available for closed enum slot AnimalValue".to_string(),
        }
    );
    assert_eq!(
        enum_no_default.compile_error_expr(),
        RustExpr::Raw(
            "compile_error!(\"no default value is available for closed enum slot AnimalValue\")"
                .to_string()
        )
    );
    assert_eq!(
        default_expr_for_slot(&enum_shape),
        RustExpr::Raw(
            "compile_error!(\"no default value is available for closed enum slot AnimalValue\")"
                .to_string()
        )
    );

    let mut dyn_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_shape.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        slot_default_policy(&dyn_shape, &FactSet::default()),
        SlotDefaultPolicy::NoDefaultForDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        default_expr_for_slot(&dyn_shape),
        RustExpr::Raw(
            "compile_error!(\"no default value is available for dyn trait slot Animal\")"
                .to_string()
        )
    );

    let mut boxed_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_shape.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        slot_default_policy(&boxed_shape, &FactSet::default()),
        SlotDefaultPolicy::NoDefaultForBoxedDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        default_expr_for_slot(&boxed_shape),
        RustExpr::Raw(
            "compile_error!(\"no default value is available for boxed dyn trait slot Animal\")"
                .to_string()
        )
    );

    let mut erased_shape = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    erased_shape.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        slot_default_policy(&erased_shape, &FactSet::default()),
        SlotDefaultPolicy::NoDefaultForErasedObject
    );
    assert_eq!(
        default_expr_for_slot(&erased_shape),
        RustExpr::Raw(
            "compile_error!(\"no default value is available for erased object slot\")".to_string()
        )
    );

    let source = include_str!("../src/slot_render.rs");
    assert!(source.contains("pub enum SlotDefaultPolicy"));
    assert!(source.contains("pub enum SlotDefaultValuePolicy"));
    assert!(source.contains("pub struct SlotContentCapabilityPolicy"));
    assert!(source.contains("pub fn slot_content_capability_policy"));
    assert!(source.contains("pub struct SlotNoDefaultMessagePolicy"));
    assert!(source.contains("pub fn slot_no_default_message_policy"));
    assert!(source.contains("pub fn slot_default_policy"));
    assert!(source.contains("pub fn slot_default_value_policy"));
    assert!(source.contains("pub fn render_slot_default_policy"));
    assert!(source.contains("pub fn render_slot_default_value_policy"));
}

#[test]
fn shape_projection_policy_owns_dispatch_axis_effects() {
    assert_eq!(
        dispatch_projection_policy(DispatchShape::Concrete),
        DispatchProjectionPolicy {
            dispatch: DispatchShape::Concrete,
            ownership: None,
            view: None,
        }
    );
    assert_eq!(
        dispatch_projection_policy(DispatchShape::DynTrait {
            trait_name: "Animal".to_string(),
        }),
        DispatchProjectionPolicy {
            dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            ownership: Some(OwnershipShape::SharedBorrow),
            view: Some(ViewShape::DynTraitView),
        }
    );
    assert_eq!(
        dispatch_projection_policy(DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string(),
        }),
        DispatchProjectionPolicy {
            dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            ownership: Some(OwnershipShape::Owned),
            view: Some(ViewShape::DynTraitView),
        }
    );
    assert_eq!(
        dispatch_projection_policy(DispatchShape::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
        }),
        DispatchProjectionPolicy {
            dispatch: DispatchShape::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
            },
            ownership: None,
            view: Some(ViewShape::EnumView),
        }
    );
    assert_eq!(
        dispatch_projection_policy(DispatchShape::ErasedObject),
        DispatchProjectionPolicy {
            dispatch: DispatchShape::ErasedObject,
            ownership: Some(OwnershipShape::Owned),
            view: Some(ViewShape::ErasedObjectView),
        }
    );
}

#[test]
fn slot_render_uses_type_capability_facts_for_string_views() {
    let external_label = JavaType::Named {
        name: "ExternalLabel".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalLabel".to_string(),
            copy: None,
            clone: None,
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let effective_facts = EffectiveFacts::new(&facts);
    let mut shape = RustSlotShape::conservative(external_label);
    shape.ownership = OwnershipShape::SharedBorrow;
    shape.view = ViewShape::Str;

    assert_eq!(
        rust_type_for_slot(&shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Named("ExternalLabel".to_string())),
        }
    );
    assert_eq!(
        rust_type_for_slot_with_facts(&shape, &facts),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string())),
        }
    );
    assert_eq!(
        default_expr_for_slot_with_facts(&shape, &facts),
        RustExpr::Literal("\"\"".to_string())
    );
    assert_eq!(
        slot_content_capability_policy(&shape.java_ty, &facts),
        SlotContentCapabilityPolicy {
            java_ty: shape.java_ty.clone(),
            string_content: true,
            sequence_element: None,
        }
    );
    assert_eq!(
        slot_content_capability_policy_with_effective_facts(&shape.java_ty, &effective_facts),
        slot_content_capability_policy(&shape.java_ty, &facts)
    );
}

#[test]
fn slot_render_uses_type_capability_facts_for_sequence_views_and_storage() {
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let effective_facts = EffectiveFacts::new(&facts);

    let mut slice_shape = RustSlotShape::conservative(external_ids.clone());
    slice_shape.ownership = OwnershipShape::SharedBorrow;
    slice_shape.view = ViewShape::Slice;

    assert_eq!(
        rust_type_for_slot(&slice_shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Named("ExternalIds".to_string())),
        }
    );
    assert_eq!(
        rust_type_for_slot_with_facts(&slice_shape, &facts),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Slice(Box::new(RustType::Named(
                "ExternalId".to_string()
            )))),
        }
    );

    let storage_shape = RustSlotShape::for_storage(external_ids);
    assert_eq!(
        rust_type_for_slot(&storage_shape),
        RustType::Named("ExternalIds".to_string())
    );
    assert_eq!(
        rust_type_for_slot_with_facts(&storage_shape, &facts),
        RustType::Vec(Box::new(RustType::Named("ExternalId".to_string())))
    );
    assert_eq!(
        default_expr_for_slot(&storage_shape),
        RustExpr::Raw("Default::default()".to_string())
    );
    assert_eq!(
        default_expr_for_slot_with_facts(&storage_shape, &facts),
        RustExpr::Raw("Vec::new()".to_string())
    );
    assert_eq!(
        slot_content_capability_policy(&storage_shape.java_ty, &facts),
        SlotContentCapabilityPolicy {
            java_ty: storage_shape.java_ty.clone(),
            string_content: false,
            sequence_element: Some(JavaType::Named {
                name: "ExternalId".to_string(),
                args: Vec::new(),
            }),
        }
    );
    assert_eq!(
        slot_content_capability_policy_with_effective_facts(
            &storage_shape.java_ty,
            &effective_facts,
        ),
        slot_content_capability_policy(&storage_shape.java_ty, &facts)
    );
}

#[test]
fn representation_plan_keeps_interface_dispatch_as_fallback_policy() {
    let parsed = parse_java_source(
        r#"
interface Animal {
    String name();
}

class Dog implements Animal {
    public String name() { return "dog"; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: true,
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new()
        }),
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new()
        }),
        DispatchShape::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(
        representation_dispatch_policy(ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }),
        RepresentationDispatchPolicy {
            representation: ObjectRepresentation::DynTrait {
                trait_name: "Animal".to_string(),
            },
            dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
        }
    );
    assert_eq!(
        representation_dispatch_policy(ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Literal".to_string()],
        })
        .dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string(),
        }
    );
    assert_eq!(
        representation_dispatch_policy(ObjectRepresentation::ErasedObject).dispatch,
        DispatchShape::ErasedObject
    );
    assert_eq!(
        representation_dispatch_policy(ObjectRepresentation::ConcreteStruct).dispatch,
        DispatchShape::Concrete
    );
    let param_shape = param_shape_for_java_type(
        &representations,
        JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        },
        false,
    );
    assert_eq!(
        param_shape.dispatch,
        DispatchShape::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(param_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(param_shape.view, ViewShape::DynTraitView);
    assert_eq!(
        rust_type_for_slot(&param_shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("dyn Animal".to_string()))
        }
    );

    let return_shape = return_shape_for_java_type(
        &representations,
        &JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        return_shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(return_shape.ownership, OwnershipShape::Owned);
    assert_eq!(return_shape.view, ViewShape::DynTraitView);
    assert_eq!(
        rust_type_for_slot(&return_shape),
        RustType::Raw("Box<dyn Animal>".to_string())
    );
}

#[test]
fn representation_lookup_policy_makes_conservative_fallbacks_explicit() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    let planned_interface = representations.representation_lookup_policy(&JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    let missing_named = representations.representation_lookup_policy(&JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    });
    let unknown_object =
        representations.representation_lookup_policy(&JavaType::Unknown("Object".to_string()));
    let wildcard_object = representations.representation_lookup_policy(&JavaType::Wildcard(None));
    let primitive =
        representations.representation_lookup_policy(&JavaType::Primitive(PrimitiveType::Int));

    assert_eq!(
        planned_interface.fallback,
        RepresentationLookupFallback::PlannedType
    );
    assert_eq!(
        planned_interface.representation,
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        missing_named.fallback,
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        missing_named.representation,
        ObjectRepresentation::ConcreteStruct
    );
    assert_eq!(
        unknown_object.fallback,
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(
        unknown_object.representation,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        wildcard_object.fallback,
        RepresentationLookupFallback::ConservativeErasedObject
    );
    assert_eq!(
        wildcard_object.representation,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        primitive.fallback,
        RepresentationLookupFallback::ConservativeConcrete
    );
    assert_eq!(
        primitive.representation,
        ObjectRepresentation::ConcreteStruct
    );
}

#[test]
fn shape_projection_uses_boxed_dyn_for_stored_interface_slots() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.closed_hierarchy_facts.push(ClosedHierarchyFact {
        type_name: "Expr".to_string(),
        closed: true,
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);

    let storage_shape = storage_shape_for_java_type(
        &representations,
        JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        },
    );

    assert_eq!(
        storage_shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(storage_shape.ownership, OwnershipShape::Owned);
    assert_eq!(storage_shape.view, ViewShape::DynTraitView);
    assert_eq!(
        rust_type_for_slot(&storage_shape),
        RustType::Raw("Box<dyn Animal>".to_string())
    );
}

#[test]
fn signature_plan_uses_boxed_dyn_for_interface_fields() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Kennel {
    Animal resident;
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);

    let field_shape = signatures.required_field_shape("Kennel", "resident");

    assert_eq!(
        field_shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(field_shape.ownership, OwnershipShape::Owned);
    assert_eq!(field_shape.view, ViewShape::DynTraitView);
}

#[test]
fn resolve_records_parsed_interface_object_safety_facts() {
    let parsed = parse_java_source(
        r#"
interface Named {
    String name();
}

interface Factory {
    Factory next();
}

interface Combiner {
    void merge(Combiner other);
}

interface PredicateLike {
    PredicateLike and(PredicateLike other);
    PredicateLike or(PredicateLike other);
    PredicateLike negate();
}

interface TransformerLike {
    TransformerLike andThen(TransformerLike after);
    TransformerLike compose(TransformerLike before);
}

interface HasConstant {
    int VERSION = 1;
}

interface HasStaticFactory {
    static HasStaticFactory of(String name) { return null; }
    String name();
}

interface HasNestedHelper {
    String name();

    final class Helper {
        Helper(String value) {}
    }

    interface NestedFactory {
        NestedFactory next();
    }
}

interface GenericFactory {
    List<GenericFactory> nextAll();
}

interface GenericCombiner {
    void mergeAll(List<? extends GenericCombiner> others);
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let named_ty = JavaType::Named {
        name: "Named".to_string(),
        args: Vec::new(),
    };
    let factory_ty = JavaType::Named {
        name: "Factory".to_string(),
        args: Vec::new(),
    };
    let combiner_ty = JavaType::Named {
        name: "Combiner".to_string(),
        args: Vec::new(),
    };
    let predicate_like_ty = JavaType::Named {
        name: "PredicateLike".to_string(),
        args: Vec::new(),
    };
    let transformer_like_ty = JavaType::Named {
        name: "TransformerLike".to_string(),
        args: Vec::new(),
    };
    let has_constant_ty = JavaType::Named {
        name: "HasConstant".to_string(),
        args: Vec::new(),
    };
    let has_static_factory_ty = JavaType::Named {
        name: "HasStaticFactory".to_string(),
        args: Vec::new(),
    };
    let has_nested_helper_ty = JavaType::Named {
        name: "HasNestedHelper".to_string(),
        args: Vec::new(),
    };
    let nested_factory_ty = JavaType::Named {
        name: "NestedFactory".to_string(),
        args: Vec::new(),
    };
    let generic_factory_ty = JavaType::Named {
        name: "GenericFactory".to_string(),
        args: Vec::new(),
    };
    let generic_combiner_ty = JavaType::Named {
        name: "GenericCombiner".to_string(),
        args: Vec::new(),
    };

    assert!(
        resolved
            .facts
            .type_capabilities_for(&named_ty)
            .object_safe_trait
    );
    assert!(
        !resolved
            .facts
            .type_capabilities_for(&factory_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&combiner_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&predicate_like_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&transformer_like_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&has_constant_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&has_static_factory_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capabilities_for(&has_nested_helper_ty)
            .object_safe_trait
    );
    assert!(
        !resolved
            .facts
            .type_capabilities_for(&nested_factory_ty)
            .object_safe_trait
    );
    assert!(
        !resolved
            .facts
            .type_capabilities_for(&generic_factory_ty)
            .object_safe_trait
    );
    assert!(
        !resolved
            .facts
            .type_capabilities_for(&generic_combiner_ty)
            .object_safe_trait
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&named_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&factory_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(false))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&combiner_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&predicate_like_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&transformer_like_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&has_constant_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&has_static_factory_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&has_nested_helper_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(true))
    );
    let factory_safety = resolved
        .facts
        .object_safety_fact_for_name("Factory")
        .unwrap();
    assert_eq!(
        factory_safety.violations,
        vec![ObjectSafetyViolation::SelfReferentialReturn {
            method_name: "next".to_string()
        }]
    );
    let combiner_safety = resolved
        .facts
        .object_safety_fact_for_name("Combiner")
        .unwrap();
    assert!(combiner_safety.violations.is_empty());
    let predicate_like_safety = resolved
        .facts
        .object_safety_fact_for_name("PredicateLike")
        .unwrap();
    assert!(predicate_like_safety.violations.is_empty());
    let transformer_like_safety = resolved
        .facts
        .object_safety_fact_for_name("TransformerLike")
        .unwrap();
    assert!(transformer_like_safety.violations.is_empty());
    let has_constant_safety = resolved
        .facts
        .object_safety_fact_for_name("HasConstant")
        .unwrap();
    assert!(has_constant_safety.violations.is_empty());
    let has_static_factory_safety = resolved
        .facts
        .object_safety_fact_for_name("HasStaticFactory")
        .unwrap();
    assert!(has_static_factory_safety.violations.is_empty());
    let has_nested_helper_safety = resolved
        .facts
        .object_safety_fact_for_name("HasNestedHelper")
        .unwrap();
    assert!(has_nested_helper_safety.violations.is_empty());
    let nested_factory_safety = resolved
        .facts
        .object_safety_fact_for_name("NestedFactory")
        .unwrap();
    assert_eq!(
        nested_factory_safety.violations,
        vec![ObjectSafetyViolation::SelfReferentialReturn {
            method_name: "next".to_string(),
        }]
    );
    let generic_factory_safety = resolved
        .facts
        .object_safety_fact_for_name("GenericFactory")
        .unwrap();
    assert_eq!(
        generic_factory_safety.violations,
        vec![ObjectSafetyViolation::NestedSelfReferentialReturn {
            method_name: "nextAll".to_string(),
        }]
    );
    let generic_combiner_safety = resolved
        .facts
        .object_safety_fact_for_name("GenericCombiner")
        .unwrap();
    assert_eq!(
        generic_combiner_safety.violations,
        vec![ObjectSafetyViolation::NestedSelfReferentialParameter {
            method_name: "mergeAll".to_string(),
            param_name: "others".to_string(),
        }]
    );
}

#[test]
fn resolve_records_parsed_hierarchy_capability_facts() {
    let parsed = parse_java_source(
        r#"
interface Named {}
class Base {}
final class User extends Base implements Named {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let user_ty = JavaType::Named {
        name: "User".to_string(),
        args: Vec::new(),
    };
    let capabilities = resolved.facts.type_capabilities_for(&user_ty);

    assert_eq!(
        capabilities.extends,
        Some(JavaType::Named {
            name: "Base".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        capabilities.implements,
        vec![JavaType::Named {
            name: "Named".to_string(),
            args: Vec::new(),
        }]
    );
    assert!(capabilities.final_type);
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&user_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.final_type == Some(true)
                && fact.extends
                    == Some(JavaType::Named {
                        name: "Base".to_string(),
                        args: Vec::new(),
                    })
                && fact.implements
                    == vec![JavaType::Named {
                        name: "Named".to_string(),
                        args: Vec::new(),
                    }])
    );
}

#[test]
fn resolve_records_parsed_closed_hierarchy_facts_for_sealed_permits() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    assert_eq!(resolved.program.types[0].permits, vec!["Add", "Num"]);
    assert!(
        resolved
            .facts
            .closed_hierarchy_facts
            .iter()
            .any(|fact| fact.type_name == "Expr"
                && fact.closed
                && fact.provenance == FactProvenance::ParsedSource)
    );
    assert!(
        resolved
            .facts
            .sealed_permits_facts
            .iter()
            .any(|fact| fact.type_name == "Expr"
                && fact.permitted_subtypes == vec!["Add".to_string(), "Num".to_string()]
                && fact.provenance == FactProvenance::ParsedSource)
    );
}

#[test]
fn representation_plan_uses_parsed_object_safety_facts_for_interface_fallback() {
    let parsed = parse_java_source(
        r#"
interface Factory {
    Factory next();
}

class DefaultFactory implements Factory {
    public Factory next() { return this; }
}

interface Combiner {
    void merge(Combiner other);
}

class DefaultCombiner implements Combiner {
    public void merge(Combiner other) {}
}

interface PredicateLike {
    PredicateLike and(PredicateLike other);
    PredicateLike or(PredicateLike other);
    PredicateLike negate();
}

interface TransformerLike {
    TransformerLike andThen(TransformerLike after);
    TransformerLike compose(TransformerLike before);
}

interface HasConstant {
    int VERSION = 1;
}

interface HasStaticFactory {
    static HasStaticFactory of(String name) { return null; }
    String name();
}

interface HasNestedHelper {
    String name();

    final class Helper {
        Helper(String value) {}
    }

    interface NestedFactory {
        NestedFactory next();
    }
}

interface GenericFactory {
    List<GenericFactory> nextAll();
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let factory_ty = JavaType::Named {
        name: "Factory".to_string(),
        args: Vec::new(),
    };
    let combiner_ty = JavaType::Named {
        name: "Combiner".to_string(),
        args: Vec::new(),
    };
    let predicate_like_ty = JavaType::Named {
        name: "PredicateLike".to_string(),
        args: Vec::new(),
    };
    let transformer_like_ty = JavaType::Named {
        name: "TransformerLike".to_string(),
        args: Vec::new(),
    };
    let has_constant_ty = JavaType::Named {
        name: "HasConstant".to_string(),
        args: Vec::new(),
    };
    let has_static_factory_ty = JavaType::Named {
        name: "HasStaticFactory".to_string(),
        args: Vec::new(),
    };
    let has_nested_helper_ty = JavaType::Named {
        name: "HasNestedHelper".to_string(),
        args: Vec::new(),
    };
    let nested_factory_ty = JavaType::Named {
        name: "NestedFactory".to_string(),
        args: Vec::new(),
    };
    let generic_factory_ty = JavaType::Named {
        name: "GenericFactory".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        representations.representation_for(&factory_ty),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&factory_ty),
        DispatchShape::ErasedObject
    );
    assert_eq!(
        representations.representation_for(&combiner_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "Combiner".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&combiner_ty),
        DispatchShape::DynTrait {
            trait_name: "Combiner".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&predicate_like_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "PredicateLike".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&predicate_like_ty),
        DispatchShape::DynTrait {
            trait_name: "PredicateLike".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&transformer_like_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "TransformerLike".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&transformer_like_ty),
        DispatchShape::DynTrait {
            trait_name: "TransformerLike".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&has_constant_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "HasConstant".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&has_constant_ty),
        DispatchShape::DynTrait {
            trait_name: "HasConstant".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&has_static_factory_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "HasStaticFactory".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&has_static_factory_ty),
        DispatchShape::DynTrait {
            trait_name: "HasStaticFactory".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&has_nested_helper_ty),
        ObjectRepresentation::DynTrait {
            trait_name: "HasNestedHelper".to_string(),
        }
    );
    assert_eq!(
        representations.dispatch_for(&has_nested_helper_ty),
        DispatchShape::DynTrait {
            trait_name: "HasNestedHelper".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&nested_factory_ty),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.representation_for(&generic_factory_ty),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&generic_factory_ty),
        DispatchShape::ErasedObject
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic.message.contains("interface Factory")
                    && diagnostic
                        .message
                        .contains("method next returns the interface type")
            ),
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic.message.contains("interface GenericFactory")
                    && diagnostic
                        .message
                        .contains("method nextAll returns a type containing the interface type")
            ),
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(
                |diagnostic| !diagnostic.message.contains("interface HasConstant")
                    && !diagnostic.message.contains("interface HasStaticFactory")
                    && !diagnostic.message.contains("interface HasNestedHelper")
                    && !diagnostic.message.contains("interface PredicateLike")
                    && !diagnostic.message.contains("interface TransformerLike")
                    && !diagnostic.message.contains("interface Combiner")
            ),
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported_interface_impl"),
    );
}

#[test]
fn object_representation_policy_owns_dyn_trait_erased_fallback_matrix() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Source".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(true),
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Sink".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });

    let safe_default = object_representation_policy(
        ObjectRepresentationRequest::DefaultInterface {
            type_name: "Source".to_string(),
        },
        &facts,
    );
    let assumed_default = object_representation_policy(
        ObjectRepresentationRequest::DefaultInterface {
            type_name: "Unproven".to_string(),
        },
        &facts,
    );
    let unsafe_default = object_representation_policy(
        ObjectRepresentationRequest::DefaultInterface {
            type_name: "Sink".to_string(),
        },
        &facts,
    );
    let unsafe_dyn_override = object_representation_policy(
        ObjectRepresentationRequest::DynTraitOverride {
            type_name: "Sink".to_string(),
            trait_name: "SinkTrait".to_string(),
        },
        &facts,
    );
    let erased_override = object_representation_policy(
        ObjectRepresentationRequest::ErasedObjectOverride {
            type_name: "Sink".to_string(),
        },
        &facts,
    );

    assert_eq!(
        object_safety_fact_policy("Source", &facts),
        ObjectSafetyFactPolicy {
            type_name: "Source".to_string(),
            object_safe_trait: Some(true),
            violation: None,
        }
    );
    assert_eq!(
        object_safety_fact_policy("Sink", &facts),
        ObjectSafetyFactPolicy {
            type_name: "Sink".to_string(),
            object_safe_trait: Some(false),
            violation: Some(ObjectSafetyViolation::ExplicitNonObjectSafeFact {
                provenance: FactProvenance::UserOverride,
            }),
        }
    );
    assert_eq!(
        object_safety_fact_policy("Unproven", &facts),
        ObjectSafetyFactPolicy {
            type_name: "Unproven".to_string(),
            object_safe_trait: None,
            violation: None,
        }
    );

    assert_eq!(
        safe_default.representation,
        ObjectRepresentation::DynTrait {
            trait_name: "Source".to_string()
        }
    );
    assert_eq!(
        safe_default.object_safety,
        ObjectSafetyPolicy::ProvenObjectSafe
    );
    assert!(safe_default.diagnostics.is_empty());
    assert_eq!(
        assumed_default.representation,
        ObjectRepresentation::DynTrait {
            trait_name: "Unproven".to_string()
        }
    );
    assert_eq!(
        assumed_default.object_safety,
        ObjectSafetyPolicy::AssumedObjectSafe
    );
    assert!(assumed_default.diagnostics.is_empty());
    assert_eq!(
        unsafe_default.representation,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        unsafe_default.object_safety,
        ObjectSafetyPolicy::RejectedNonObjectSafe
    );
    assert!(unsafe_default.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "non_object_safe_trait_fallback"
            && diagnostic.message.contains("interface Sink")
            && diagnostic
                .message
                .contains("explicit non-object-safe trait fact from UserOverride")
    }));
    assert_eq!(
        unsafe_dyn_override.representation,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        unsafe_dyn_override.object_safety,
        ObjectSafetyPolicy::RejectedNonObjectSafe
    );
    assert!(unsafe_dyn_override.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("dyn trait representation for Sink")
    }));
    assert_eq!(
        erased_override.representation,
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        erased_override.object_safety,
        ObjectSafetyPolicy::NotRequired
    );
    assert!(erased_override.diagnostics.is_empty());

    let parsed = parse_java_source(
        r#"
interface Sink {}
class Impl implements Sink {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut plan_facts = resolved.facts.clone();
    plan_facts.extend_from(facts.clone());
    let plan = plan_representations(&resolved.program, &plan_facts);
    let dyn_override = HierarchyOverride {
        type_name: "Sink".to_string(),
        strategy: HierarchyStrategy::DynTrait {
            trait_name: "SinkTrait".to_string(),
        },
        provenance: FactProvenance::UserOverride,
    };
    let concrete_override = HierarchyOverride {
        type_name: "Impl".to_string(),
        strategy: HierarchyStrategy::ConcreteStruct,
        provenance: FactProvenance::CorpusAnalysis,
    };
    let missing_override = HierarchyOverride {
        type_name: "Missing".to_string(),
        strategy: HierarchyStrategy::ConcreteStruct,
        provenance: FactProvenance::UserOverride,
    };
    let context = object_hierarchy_context(&plan);
    let dyn_policy = hierarchy_override_application_policy(&dyn_override, &context, &plan_facts);
    let concrete_policy =
        hierarchy_override_application_policy(&concrete_override, &context, &plan_facts);
    let missing_policy =
        hierarchy_override_application_policy(&missing_override, &context, &plan_facts);

    assert_eq!(
        dyn_policy.representation,
        Some(ObjectRepresentation::ErasedObject)
    );
    assert_eq!(dyn_policy.provenance, FactProvenance::UserOverride);
    assert_eq!(dyn_policy.direct_subtypes, vec!["Impl".to_string()]);
    assert!(dyn_policy.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("dyn trait representation for Sink")
    }));
    assert_eq!(
        concrete_policy.representation,
        Some(ObjectRepresentation::ConcreteStruct)
    );
    assert_eq!(concrete_policy.provenance, FactProvenance::CorpusAnalysis);
    assert!(concrete_policy.diagnostics.is_empty());
    assert_eq!(missing_policy.representation, None);
    assert!(missing_policy.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("hierarchy override references unknown Java type Missing")
    }));
}

#[test]
fn object_representation_policy_is_the_dyn_fallback_entrypoint() {
    let representation_source = include_str!("../src/representation.rs");
    let object_model_source = include_str!("../src/object_model.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let facts_source = include_str!("../src/facts.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");

    assert!(effective_facts_source.contains("pub struct EffectiveFacts"));
    assert!(effective_facts_source.contains("pub fn object_safe_trait_fact_for_name"));
    assert!(effective_facts_source.contains("pub fn closed_hierarchy_variants_for"));
    assert!(effective_facts_source.contains("pub fn dispatch_requirements_for"));
    assert!(object_model_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(object_model_source.contains("pub struct ObjectModelInput"));
    assert!(object_model_source.contains("pub program: &'a Program"));
    assert!(object_model_source.contains("pub facts: &'a FactSet"));
    assert!(object_model_source.contains("pub effective_facts: &'a EffectiveFacts<'a>"));
    assert!(object_model_source.contains("pub usage: Option<&'a UsageFacts>"));
    assert!(object_model_source.contains("pub fn plan_object_model_from_input"));
    assert!(object_model_source.contains("plan_object_model_with_effective_facts"));
    assert!(representation_source.contains("ObjectModelInput"));
    assert!(representation_source.contains("plan_object_model_from_input"));
    assert!(representation_source.contains("usage: None"));
    assert!(object_model_source.contains("pub strategies: BTreeMap<String, ObjectStrategyPlan>"));
    assert!(object_model_source.contains("pub fn strategy_for(&self, root: &str)"));
    assert!(object_model_source.contains("pub provenance: Vec<FactProvenance>"));
    assert!(object_model_source.contains("pub fn primary_provenance(&self)"));
    assert!(object_model_source.contains("fn add_provenance(&mut self"));
    assert!(object_model_source.contains("pub enum ObjectRepresentationRequest"));
    assert!(facts_source.contains("pub enum DispatchRequirementKind"));
    assert!(facts_source.contains("pub struct DispatchRequirementFact"));
    assert!(object_model_source.contains("pub type DispatchRequirement"));
    assert!(object_model_source.contains("dispatch_requirements_for"));
    assert!(object_model_source.contains("pub struct RepresentationLookupPolicy"));
    assert!(object_model_source.contains("pub enum RepresentationLookupFallback"));
    assert!(object_model_source.contains("pub fn object_representation_lookup_policy"));
    assert!(!representation_source.contains("pub struct RepresentationLookupPolicy"));
    assert!(!representation_source.contains("pub enum RepresentationLookupFallback"));
    assert!(representation_source.contains("object_representation_lookup_policy"));
    assert!(representation_source.contains("pub struct RepresentationDispatchPolicy"));
    assert!(representation_source.contains("pub fn representation_dispatch_policy"));
    assert!(representation_source.contains("representation_lookup_policy"));
    assert!(object_model_source.contains("ConservativeConcrete"));
    assert!(object_model_source.contains("ConservativeErasedObject"));
    assert!(object_model_source.contains("pub enum ObjectSafetyPolicy"));
    assert!(object_model_source.contains("pub struct ObjectSafetyFactPolicy"));
    assert!(object_model_source.contains("pub fn object_safety_fact_policy"));
    assert!(object_model_source.contains("object_safety_policy_for_request"));
    assert_eq!(
        object_model_source
            .matches("object_safe_trait_fact_for_name")
            .count(),
        1
    );
    assert_eq!(
        object_model_source
            .matches("object_safety_violation_for_name")
            .count(),
        1
    );
    assert!(object_model_source.contains("ErasedObjectOverride"));
    assert!(object_model_source.contains("object_representation_request_requires_object_safety"));
    assert!(object_model_source.contains("has_explicit_hierarchy_override"));
    assert!(object_model_source.contains("pub struct HierarchyOverrideValidationPolicy"));
    assert!(object_model_source.contains("pub fn hierarchy_override_validation_policy"));
    assert!(object_model_source.contains("pub struct HierarchyOverrideApplicationPolicy"));
    assert!(object_model_source.contains("pub fn hierarchy_override_application_policy"));
    assert!(object_model_source.contains("pub struct HierarchyOverrideRepresentationPolicy"));
    assert!(object_model_source.contains("hierarchy_override_representation_policy"));
    assert!(representation_source.contains("pub struct RepresentationHierarchyFactPolicy"));
    assert!(representation_source.contains("pub fn representation_hierarchy_fact_policy"));
    assert!(
        representation_source.contains("representation_hierarchy_fact_policy_with_effective_facts")
    );
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdge"));
    assert!(object_model_source.contains("pub enum ObjectHierarchyEdgeKind"));
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdgeRepresentationPolicy"));
    assert!(object_model_source.contains("pub fn object_hierarchy_edge_representation_policy"));
    assert!(object_model_source.contains("hierarchy_edges: Vec<ObjectHierarchyEdge>"));
    assert!(representation_source.contains("for edge in object_plan.hierarchy_edges"));
    assert!(representation_source.contains("object_hierarchy_edge_representation_policy"));
    assert!(!representation_source.contains("fn collect_hierarchy_edges("));
    assert!(!representation_source.contains("ObjectRepresentation::ClosedEnum { .. }"));
    assert!(effective_facts_source.contains("pub fn extends_type_for"));
    assert!(effective_facts_source.contains("pub fn implemented_types_for"));
    assert!(effective_facts_source.contains("pub fn hierarchy_edges_for_type"));
    assert_eq!(
        representation_source
            .matches("hierarchy_edges_for_type")
            .count(),
        1
    );
    assert!(object_model_source.contains("pub struct ClosedEnumFactPolicy"));
    assert!(object_model_source.contains("pub fn closed_enum_fact_policy"));
    assert!(object_model_source.contains("pub struct ClosedEnumInferencePolicy"));
    assert!(object_model_source.contains("pub fn closed_enum_inference_policy"));
    assert!(object_model_source.contains("pub enum ClosedEnumProof"));
    assert!(object_model_source.contains("pub fn closed_enum_proof_policy"));
    assert!(object_model_source.contains("MissingPermittedSubtypes"));
    assert!(facts_source.contains("pub struct SealedPermitsFact"));
    assert!(effective_facts_source.contains("sealed_permitted_subtypes_for"));
    assert!(!representation_source.contains("fn validate_hierarchy_override"));
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdgeDiagnosticInput"));
    assert!(object_model_source.contains("pub struct HierarchyEdgeDiagnosticPolicy"));
    assert!(object_model_source.contains("pub enum HierarchyEdgeDiagnosticOutcome"));
    assert!(object_model_source.contains("pub fn object_hierarchy_edge_diagnostic_policy"));
    assert!(representation_source.contains("ObjectHierarchyEdgeDiagnosticInput"));
    assert!(representation_source.contains("pub fn hierarchy_edge_diagnostic_policy"));
    assert!(representation_source.contains("object_hierarchy_edge_diagnostic_policy"));
    assert!(!representation_source.contains("pub struct HierarchyEdgeDiagnosticPolicy"));
    assert!(!representation_source.contains("pub enum HierarchyEdgeDiagnosticOutcome"));
    assert!(!representation_source.contains("render_hierarchy_edge_diagnostic_outcome"));
    assert!(representation_source.contains("pub struct AnonymousClassDiagnosticPolicy"));
    assert!(representation_source.contains("pub fn anonymous_class_diagnostic_policy"));
    assert!(object_model_source.contains("pub fn object_representation_policy"));
    assert_eq!(
        boxed_dyn_object_representation_policy(),
        BoxedDynObjectRepresentationPolicy {
            invariant: BoxedDynObjectRepresentationInvariant::SlotProjectionOnly,
            object_representation_has_boxed_dyn_variant: false,
            representation_dispatch_maps_boxed_dyn: false,
            slot_projection_creates_owned_boxed_dyn_storage: true,
        }
    );
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Storage,
            DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        )
        .projected_dispatch,
        Some(DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string(),
        })
    );
    assert!(!facts_source.contains("HierarchyStrategy::BoxedDynTrait"));
    assert!(!facts_source.contains("BoxedDynTrait {"));
    assert!(!object_model_source.contains("ObjectRepresentation::BoxedDynTrait"));
    assert!(!object_model_source.contains("ObjectRepresentationRequest::BoxedDynTraitOverride"));
    assert!(shape_projection_source.contains("DispatchShape::BoxedDynTrait"));
    assert!(!representation_source.contains("interface_representation_from_facts"));
    assert!(!representation_source.contains("dyn_trait_representation_from_facts"));
    assert!(!representation_source.contains("boxed_dyn_trait_representation_from_facts"));
}

#[test]
fn representation_plan_uses_object_safety_facts_for_interface_fallback() {
    let parsed = parse_java_source(
        r#"
interface Sink {
    void accept(String value);
}

class LoggingSink implements Sink {
    public void accept(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Sink".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });

    let representations = plan_representations(&resolved.program, &facts);
    let sink_ty = JavaType::Named {
        name: "Sink".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        representations.representation_for(&sink_ty),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&sink_ty),
        DispatchShape::ErasedObject
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic.message.contains("interface Sink")
            ),
    );
}

#[test]
fn representation_plan_projects_unknown_java_types_as_erased_objects() {
    let parsed = parse_java_source(
        r#"
class Known {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let unknown = JavaType::Unknown("Object".to_string());
    let named_object = JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        representations.representation_for(&unknown),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&unknown),
        DispatchShape::ErasedObject
    );
    assert_eq!(
        representations.representation_for(&named_object),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&named_object),
        DispatchShape::ErasedObject
    );

    let shape = value_shape_for_java_type(&representations, unknown);

    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);

    let named_shape = value_shape_for_java_type(&representations, named_object);

    assert_eq!(named_shape.ownership, OwnershipShape::Owned);
    assert_eq!(named_shape.view, ViewShape::ErasedObjectView);
    assert_eq!(named_shape.dispatch, DispatchShape::ErasedObject);
}

#[test]
fn shape_projection_projects_unknown_java_types_as_erased_without_plan() {
    let shape = project_java_value_shape(None, JavaType::Unknown("Object".to_string()));

    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);

    let wildcard_shape = project_java_value_shape(None, JavaType::Wildcard(None));

    assert_eq!(wildcard_shape.ownership, OwnershipShape::Owned);
    assert_eq!(wildcard_shape.view, ViewShape::ErasedObjectView);
    assert_eq!(wildcard_shape.dispatch, DispatchShape::ErasedObject);

    let named_object_shape = project_java_value_shape(
        None,
        JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        },
    );

    assert_eq!(named_object_shape.ownership, OwnershipShape::Owned);
    assert_eq!(named_object_shape.view, ViewShape::ErasedObjectView);
    assert_eq!(named_object_shape.dispatch, DispatchShape::ErasedObject);
}

#[test]
fn representation_plan_treats_explicit_erased_object_as_supported_hierarchy_fallback() {
    let parsed = parse_java_source(
        r#"
interface Sink {}
class LoggingSink implements Sink {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Sink".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Sink".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Sink".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported_interface_impl"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "non_object_safe_trait_fallback"),
        "{diagnostics:?}"
    );
}

#[test]
fn representation_plan_rejects_non_object_safe_dyn_trait_override() {
    let parsed = parse_java_source(
        r#"
interface Sink {}
class LoggingSink implements Sink {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Sink".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(false),
            provenance: FactProvenance::UserOverride,
        });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Sink".to_string(),
        strategy: HierarchyStrategy::DynTrait {
            trait_name: "Sink".to_string(),
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Sink".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic
                        .message
                        .contains("dyn trait representation for Sink")
            ),
    );
}

#[test]
fn representation_plan_records_hierarchy_edges_before_lowering() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Poodle extends Dog {}
class Marker {}
class GhostDog implements Marker {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.direct_subtypes_of("Animal"),
        ["Dog".to_string()].as_slice()
    );
    assert_eq!(
        representations.direct_subtypes_of("Dog"),
        ["Poodle".to_string()].as_slice()
    );
}

#[test]
fn representation_plan_uses_capability_facts_for_hierarchy_edges() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            }],
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Animal".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.direct_subtypes_of("Animal"),
        ["Dog".to_string()].as_slice()
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        }
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(|diagnostic| diagnostic.code != "representation_override_conflict")
    );
}

#[test]
fn hierarchy_edge_diagnostic_policy_owns_unsupported_hierarchy_diagnostics() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Poodle extends Dog {}
class Marker {}
class GhostDog implements Marker {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let poodle = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Poodle")
        .unwrap();
    let ghost_dog = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "GhostDog")
        .unwrap();

    let inheritance_policy = hierarchy_edge_diagnostic_policy(poodle, &representations);
    let interface_policy = hierarchy_edge_diagnostic_policy(ghost_dog, &representations);

    assert_eq!(inheritance_policy.type_name, "Poodle");
    assert_eq!(
        inheritance_policy.outcomes,
        vec![HierarchyEdgeDiagnosticOutcome::UnsupportedInheritance]
    );
    assert!(inheritance_policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "unsupported_inheritance" && diagnostic.message.contains("Poodle")
    }));
    assert_eq!(interface_policy.type_name, "GhostDog");
    assert_eq!(
        interface_policy.outcomes,
        vec![HierarchyEdgeDiagnosticOutcome::UnsupportedInterfaceImpl {
            interfaces: vec!["Marker".to_string()],
        }]
    );
    assert!(interface_policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "unsupported_interface_impl"
            && diagnostic.message.contains("GhostDog")
            && diagnostic.message.contains("Marker")
    }));
}

#[test]
fn representation_diagnostics_use_injected_extends_capability_edges() {
    let parsed = parse_java_source(
        r#"
class Base {}
class User {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "User".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "Base".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "unsupported_inheritance" && diagnostic.message.contains("User")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn representation_diagnostics_use_injected_implements_capability_edges() {
    let parsed = parse_java_source(
        r#"
class ConcreteMarker {}
class User {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "User".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "ConcreteMarker".to_string(),
                args: Vec::new(),
            }],
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "unsupported_interface_impl"
                && diagnostic.message.contains("User")
                && diagnostic.message.contains("ConcreteMarker")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn representation_plan_owns_object_model_unsupported_diagnostics() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Poodle extends Dog {}
class Marker {}
class GhostDog implements Marker {}
"#,
    );
    assert!(!parsed.diagnostics.has_code("unsupported_interface"));
    assert!(!parsed.diagnostics.has_code("unsupported_interface_impl"));
    assert!(!parsed.diagnostics.has_code("unsupported_inheritance"));

    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported_interface")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_interface_impl")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_inheritance")
    );
}

#[test]
fn representation_policy_erases_source_summary_superclass_boundaries() {
    let parsed = parse_java_source(
        r#"
class VariantLikeTool extends SummaryWalker {
    int apply(int value) {
        return value + 1;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut source_type_modes = HashMap::new();
    source_type_modes.insert("VariantLikeTool".to_string(), DependencyMode::PrimarySource);
    source_type_modes.insert("SummaryWalker".to_string(), DependencyMode::SourceSummary);

    let policy = source_summary_superclass_boundary_policy(
        [&resolved.program].into_iter(),
        &source_type_modes,
        &resolved.facts,
    );
    assert_eq!(policy.boundary_types, vec!["SummaryWalker".to_string()]);
    assert_eq!(policy.facts.len(), 1);
    assert_eq!(policy.facts[0].type_name, "SummaryWalker");
    assert_eq!(policy.facts[0].strategy, HierarchyStrategy::ErasedObject);
    assert_eq!(policy.facts[0].provenance, FactProvenance::CorpusAnalysis);

    let mut facts = resolved.facts.clone();
    extend_representation_facts_for_source_summary_superclass_boundaries(
        [&resolved.program].into_iter(),
        &source_type_modes,
        &mut facts,
    );
    let representations = plan_representations(&resolved.program, &facts);
    assert_eq!(
        representations
            .type_representation("SummaryWalker")
            .unwrap()
            .representation,
        ObjectRepresentation::ErasedObject
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported_inheritance")
    );

    let mut cotranslated_modes = HashMap::new();
    cotranslated_modes.insert("VariantLikeTool".to_string(), DependencyMode::PrimarySource);
    cotranslated_modes.insert(
        "SummaryWalker".to_string(),
        DependencyMode::CoTranslateSource,
    );
    let cotranslated_policy = source_summary_superclass_boundary_policy(
        [&resolved.program].into_iter(),
        &cotranslated_modes,
        &resolved.facts,
    );
    assert!(cotranslated_policy.boundary_types.is_empty());
    assert!(cotranslated_policy.facts.is_empty());
}

#[test]
fn representation_policy_matches_generic_source_summary_superclass_aliases() {
    let parsed = parse_java_source(
        r#"
class Payload {}
class VariantLikeTool extends FrameworkBase<Payload> {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut source_type_modes = HashMap::new();
    source_type_modes.insert("FrameworkBase".to_string(), DependencyMode::SourceSummary);

    let policy = source_summary_superclass_boundary_policy(
        [&resolved.program].into_iter(),
        &source_type_modes,
        &resolved.facts,
    );

    assert_eq!(policy.boundary_types, vec!["FrameworkBase".to_string()]);
    assert_eq!(policy.facts[0].type_name, "FrameworkBase");
    assert_eq!(policy.facts[0].strategy, HierarchyStrategy::ErasedObject);
}

#[test]
fn unsupported_lowering_policy_owns_compile_error_placeholders() {
    let statement_policy =
        unsupported_statement_message_policy(UnsupportedStatementLowering::LocalTypeDeclaration);
    assert_eq!(
        statement_policy,
        UnsupportedLoweringMessagePolicy {
            message: "unsupported Java statement: local type declaration".to_string(),
        }
    );
    assert_eq!(
        statement_policy.compile_error_stmt(),
        RustStmt::Raw(
            "compile_error!(\"unsupported Java statement: local type declaration\");".to_string()
        )
    );
    assert_eq!(
        unsupported_constructor_message_policy(
            UnsupportedConstructorLowering::SuperRequiresInheritanceLowering
        ),
        UnsupportedLoweringMessagePolicy {
            message: "unsupported Java constructor: super(...) requires inheritance lowering"
                .to_string(),
        }
    );
    assert_eq!(
        unsupported_expression_message_policy(UnsupportedExpressionLowering::MethodReference),
        UnsupportedLoweringMessagePolicy {
            message: "unsupported Java expression: method reference".to_string(),
        }
    );
    assert_eq!(
        unsupported_member_diagnostic_policy(UnsupportedMemberLowering::ParserUnsupported(
            "record component".to_string()
        )),
        UnsupportedDiagnosticPolicy {
            code: "unsupported_member",
            message: "record component".to_string(),
        }
    );
    assert_eq!(
        unsupported_statement_stmt(UnsupportedStatementLowering::LocalTypeDeclaration),
        RustStmt::Raw(
            "compile_error!(\"unsupported Java statement: local type declaration\");".to_string()
        )
    );
    assert_eq!(
        unsupported_expression_expr(UnsupportedExpressionLowering::MethodReference),
        RustExpr::Raw(
            "compile_error!(\"unsupported Java expression: method reference\")".to_string()
        )
    );
    assert_eq!(
        plan_unsupported_expression(&parse_expr("String::valueOf")),
        Some(UnsupportedExpressionLowering::MethodReference)
    );
    assert_eq!(
        plan_unsupported_expression(&parse_expr("String.class")),
        None
    );
    assert_eq!(plan_unsupported_expression(&parse_expr("value")), None);
    assert_eq!(
        plan_unsupported_statement(&Stmt::Try {
            resources: Vec::new(),
            body: Default::default(),
            catches: Vec::new(),
            finally_block: None,
        }),
        Some(UnsupportedStatementLowering::TryCatchFinally)
    );
    assert_eq!(
        plan_unsupported_statement(&Stmt::ConstructorInvocation {
            kind: ConstructorInvocationKind::This,
            args: Vec::new(),
            resolved: None,
        }),
        Some(UnsupportedStatementLowering::ConstructorDelegation)
    );
    assert_eq!(
        plan_unsupported_statement(&Stmt::Unsupported("raw parser node".to_string())),
        Some(UnsupportedStatementLowering::ParserUnsupported(
            "raw parser node".to_string()
        ))
    );
    assert_eq!(plan_unsupported_statement(&Stmt::Break(None)), None);
    assert_eq!(
        plan_unsupported_member(&Member::Unsupported {
            reason: "record component".to_string(),
            span: Default::default(),
        }),
        Some(UnsupportedMemberLowering::ParserUnsupported(
            "record component".to_string()
        ))
    );
    assert_eq!(
        plan_unsupported_member(&Member::Type(new_java2rust::java_ir::TypeDecl {
            kind: new_java2rust::java_ir::TypeKind::Class,
            name: "Nested".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            span: Default::default(),
        })),
        None
    );

    let source = include_str!("../src/unsupported.rs");
    let lower_source = include_str!("../src/lower.rs");
    assert!(source.contains("pub struct UnsupportedDiagnosticPolicy"));
    assert!(source.contains("pub struct UnsupportedLoweringMessagePolicy"));
    assert!(source.contains("pub fn plan_unsupported_expression"));
    assert!(source.contains("pub fn plan_unsupported_member"));
    assert!(source.contains("pub fn plan_unsupported_statement"));
    assert!(source.contains("pub fn unsupported_member_diagnostic_policy"));
    assert!(source.contains("pub fn unsupported_constructor_message_policy"));
    assert!(source.contains("pub fn unsupported_statement_message_policy"));
    assert!(source.contains("pub fn unsupported_expression_message_policy"));
    assert!(lower_source.contains("plan_unsupported_expression"));
    assert!(lower_source.contains("plan_unsupported_member"));
    assert!(lower_source.contains("plan_unsupported_statement"));
    assert!(lower_source.contains("unsupported_member_diagnostic_policy"));
    assert!(!lower_source.contains("UnsupportedExpressionLowering::Lambda"));
    assert!(!lower_source.contains("UnsupportedExpressionLowering::MethodReference"));
    assert!(!lower_source.contains("UnsupportedMemberLowering::ParserUnsupported"));
    assert!(!lower_source.contains("Member::Unsupported"));
    assert!(!lower_source.contains("\"unsupported_member\""));
    assert!(!lower_source.contains("UnsupportedStatementLowering::TryCatchFinally"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::LocalTypeDeclaration"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ConstructorDelegation"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ParserUnsupported"));
}

#[test]
fn class_literal_tokens_are_planned_before_lowering() {
    let plan = class_literal_token_plan(&JavaType::String, &FactSet::default());
    assert_eq!(
        plan,
        new_java2rust::stdlib::ClassLiteralTokenPlan {
            java_name: "String".to_string(),
            rust_type: "String".to_string(),
        }
    );
    assert_eq!(
        apply_class_literal_token(&JavaType::String, &FactSet::default()),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaClass::<String>::literal".to_string())),
            args: vec![RustExpr::Literal("\"String\"".to_string())],
        }
    );
    let RustItem::Raw(support) = java_class_support_item() else {
        panic!("expected raw JavaClass support item");
    };
    assert!(support.contains("pub struct JavaClass<T>"));
    assert_eq!(
        rust_type_for_slot(&RustSlotShape::conservative(JavaType::Named {
            name: "Class".to_string(),
            args: vec![JavaType::String],
        })),
        RustType::Raw("JavaClass<String>".to_string())
    );
}

#[test]
fn class_for_name_uses_java_class_token_support() {
    let translation = translate_source(
        r#"
class ClassForNameToken {
    Class<?> load() {
        return Class.forName("demo.Plugin");
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct JavaClass<T>")
            && translation.rust.contains("JavaClass::<()>::for_name(")
            && translation.rust.contains("demo.Plugin")
            && !translation
                .rust
                .contains("unsupported Java expression: class literal"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_platform_facade_support_covers_path_uri_and_file_boundaries() {
    let source = r#"
import java.nio.file.Path;
import java.net.URI;
import java.io.File;
import org.broadinstitute.hellbender.utils.io.IOUtils;

class PlatformBoundaryUse {
    URI uri(Path path) {
        return path.toAbsolutePath().toUri();
    }

    Path fromFile(File file) {
        return file.toPath();
    }

    Path fromString(String value) {
        return IOUtils.getPath(value);
    }
}
"#;
    let parsed = parse_java_source(source);
    assert!(program_uses_java_platform_facades(&parsed.program));

    let RustItem::Raw(support) = java_platform_facade_support_item() else {
        panic!("expected raw Java platform facade support item");
    };
    assert!(support.contains("pub struct Path"));
    assert!(support.contains("pub struct URI"));
    assert!(support.contains("pub struct File"));
    assert!(support.contains("pub struct IOUtils"));
    assert!(support.contains("pub mod logger"));
    assert!(support.contains("pub fn get_path<P: JavaPathArg>(path: P) -> Path"));
    assert!(support.contains("pub fn to_uri(&self) -> URI"));
    assert!(support.contains("pub fn to_path(&self) -> Path"));
    assert!(support.contains("pub fn resolve_sibling<P: JavaPathArg>(&self, sibling: P) -> Path"));
    assert!(support.contains("impl JavaPathArg for Option<Path>"));

    let translation = translate_source(source);
    assert!(translation.rust.contains("pub struct Path"));
    assert!(translation.rust.contains("pub struct URI"));
    assert!(translation.rust.contains("pub struct File"));
    assert!(translation.rust.contains("pub struct IOUtils"));
    assert!(
        translation
            .rust
            .contains("path.to_absolute_path().to_uri()")
    );
    assert!(translation.rust.contains("file.to_path()"));
    assert!(translation.rust.contains("IOUtils::get_path(value)"));
}

#[test]
fn java_logging_facade_supports_inherited_logger_calls() {
    let source = r#"
class LoggingBoundaryUse {
    void run(String sample) {
        logger.info("started " + sample);
        logger.warn("slow path");
    }
}
"#;
    let parsed = parse_java_source(source);
    assert!(program_uses_java_platform_facades(&parsed.program));

    let translation = translate_source(source);
    assert!(translation.rust.contains("pub mod logger"));
    assert!(
        translation.rust.contains("logger::info("),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("logger::warn("),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("use of unresolved module"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_logging_facade_supports_static_logger_field_calls() {
    let source = r#"
class StaticLoggerBoundaryUse {
    private static final Logger logger = LogManager.getLogger(StaticLoggerBoundaryUse.class);

    void run(String sample) {
        logger.info("started " + sample);
        logger.debug("done");
    }
}
"#;

    let translation = translate_source(source);
    assert!(translation.rust.contains("pub mod logger"));
    assert!(
        translation.rust.contains("logger::info("),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("logger::debug("),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Self::logger"),
        "{}",
        translation.rust
    );
}

#[test]
fn representation_plan_owns_anonymous_class_diagnostics() {
    let parsed = parse_java_source(
        r#"
class Factory {
    interface Value {
        int get();
    }

    Value make() {
        return new Value() {
            public int get() { return 1; }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let policy = anonymous_class_diagnostic_policy(&resolved.program);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        policy
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class")
    );
}

#[test]
fn representation_plan_treats_erased_external_anonymous_class_as_boundary_value() {
    let parsed = parse_java_source(
        r#"
class Factory {
    Object make() {
        return new AbstractIterator<String>() {
            protected String computeNext() { return null; }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    let without_external_fact = plan_representations(&resolved.program, &resolved.facts);
    assert!(
        without_external_fact
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class"),
        "anonymous classes without an explicit erased boundary still need representation lowering"
    );

    let mut facts = resolved.facts.clone();
    facts.extend_from(new_java2rust::external_packs::guava_immutable_collect_pack_facts());
    let with_external_fact = plan_representations(&resolved.program, &facts);
    assert_eq!(
        with_external_fact.representation_for(&JavaType::Named {
            name: "AbstractIterator".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert!(
        !with_external_fact
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class"),
        "anonymous implementations of explicitly erased external objects should remain boundary values"
    );
}

#[test]
fn representation_plan_treats_tribble_feature_reader_anonymous_class_as_boundary_value() {
    let parsed = parse_java_source(
        r#"
class Factory {
    Object make() {
        return new FeatureReader<String>() {
            public String next() { return null; }
            public void close() {}
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    let without_external_fact = plan_representations(&resolved.program, &resolved.facts);
    assert!(
        without_external_fact
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class"),
        "anonymous FeatureReader implementations need representation lowering unless the Tribble boundary marks them erased"
    );

    let mut facts = resolved.facts.clone();
    facts.extend_from(new_java2rust::external_packs::htsjdk_tribble_feature_pack_facts());
    let with_external_fact = plan_representations(&resolved.program, &facts);
    assert_eq!(
        with_external_fact.representation_for(&JavaType::Named {
            name: "FeatureReader".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert!(
        !with_external_fact
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class"),
        "anonymous implementations of Tribble FeatureReader should remain erased external boundary values"
    );
}

#[test]
fn representation_plan_applies_user_closed_enum_hierarchy_override() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string(),
        }
    );
    assert_eq!(
        representations.direct_subtypes_of("Expr"),
        ["Add".to_string(), "Num".to_string()].as_slice()
    );
    assert_eq!(
        representations
            .type_representation("Expr")
            .unwrap()
            .provenance,
        RepresentationProvenance::UserOverride
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(|diagnostic| diagnostic.code != "representation_override_conflict")
    );
}

#[test]
fn representation_lowering_owns_non_struct_type_emission() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &analyze_usage(&resolved.program),
        &facts,
    );
    let expr_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Expr")
        .unwrap();
    let add_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Add")
        .unwrap();

    assert_eq!(
        type_emission_for(&representations, expr_ty, &signatures),
        RepresentationTypeEmission::RawRustItem(
            "pub enum ExprValue {\n    Add(Add),\n    Num(Num),\n}".to_string()
        )
    );
    assert_eq!(
        raw_type_item_for_with_facts(&representations, expr_ty, &signatures, &facts),
        Some("pub enum ExprValue {\n    Add(Add),\n    Num(Num),\n}".to_string())
    );
    assert_eq!(
        type_emission_for(&representations, add_ty, &signatures),
        RepresentationTypeEmission::LowerAsConcreteStruct
    );
    assert_eq!(
        raw_type_item_for_with_facts(&representations, add_ty, &signatures, &facts),
        None
    );
    let enum_parsed = parse_java_source(
        r#"
class Holder {
    enum Type {
        SNP,
        INDEL
    }
}
"#,
    );
    let enum_resolved = resolve_program(&enum_parsed.program, enum_parsed.diagnostics);
    let enum_representations = plan_representations(&enum_resolved.program, &FactSet::default());
    let enum_signatures = solve_signatures_with_facts(
        &enum_resolved.program,
        &enum_representations,
        &analyze_usage(&enum_resolved.program),
        &FactSet::default(),
    );
    let holder_ty = &enum_resolved.program.types[0];
    let enum_ty = holder_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Type(ty) if ty.name == "Type" => Some(ty),
            _ => None,
        })
        .unwrap();
    assert_eq!(rust_enum_variant_name_from_parsed_constant("s_n_p"), "SNP");
    assert!(matches!(
        enum_representations
            .type_representation("Type")
            .map(|representation| &representation.representation),
        Some(ObjectRepresentation::JavaEnum { enum_name, variants })
            if enum_name == "Type" && variants == &vec!["SNP".to_string(), "INDEL".to_string()]
    ));
    assert_eq!(
        type_emission_for(&enum_representations, enum_ty, &enum_signatures),
        RepresentationTypeEmission::RawRustItem(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum Type {\n    SNP,\n    INDEL,\n}".to_string()
        )
    );
}

#[test]
fn representation_lowering_emits_dyn_trait_method_signatures_from_signature_plan() {
    let parsed = parse_java_source(
        r#"
interface Named {
    String name();
    int score(String prefix);
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let named_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Named")
        .unwrap();

    assert_eq!(
        type_emission_for(&representations, named_ty, &signatures),
        RepresentationTypeEmission::RawRustItem(
            "pub trait Named {\n    fn name(&self) -> String;\n    fn score(&self, prefix: String) -> i32;\n}"
                .to_string()
        )
    );
}

#[test]
fn representation_lowering_renders_trait_signatures_with_facts() {
    let parsed = parse_java_source(
        r#"
interface IdSource {
    ExternalIds ids(ExternalIds values);
}
class ExternalIds {}
class ExternalId {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(JavaType::Named {
                name: "ExternalId".to_string(),
                args: Vec::new(),
            }),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let representations = plan_representations(&resolved.program, &facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let source_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "IdSource")
        .unwrap();

    assert_eq!(
        type_emission_for_with_facts(&representations, source_ty, &signatures, &facts),
        RepresentationTypeEmission::RawRustItem(
            "pub trait IdSource {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId>;\n}"
                .to_string()
        )
    );
}

#[test]
fn representation_lowering_emits_dyn_trait_impls_for_implementing_classes() {
    let parsed = parse_java_source(
        r#"
interface Named {
    String name();
}

class Dog implements Named {
    public String name() { return "dog"; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let dog_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Dog")
        .unwrap();

    assert_eq!(
        trait_impl_emissions_for_type(&representations, &resolved.program, dog_ty, &signatures),
        vec![
            "impl Named for Dog {\n    fn name(&self) -> String { Dog::name(self) }\n}".to_string()
        ]
    );
}

#[test]
fn representation_lowering_renders_trait_impls_with_facts() {
    let parsed = parse_java_source(
        r#"
interface IdSource {
    ExternalIds ids(ExternalIds values);
}
class ExternalIds {}
class ExternalId {}
class Source implements IdSource {
    public ExternalIds ids(ExternalIds values) { return values; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(JavaType::Named {
                name: "ExternalId".to_string(),
                args: Vec::new(),
            }),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let representations = plan_representations(&resolved.program, &facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let source_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Source")
        .unwrap();

    assert_eq!(
        trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            source_ty,
            &signatures,
            &facts,
        ),
        vec![
            "impl IdSource for Source {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { Source::ids(self, values) }\n}"
                .to_string()
        ]
    );
}

#[test]
fn representation_lowering_reports_incompatible_dyn_trait_impl_methods() {
    let parsed = parse_java_source(
        r#"
interface Named {
    String name();
    int score(String prefix);
    int arity(String left, String right);
}

class BrokenNamed implements Named {
    public int name() { return 1; }
    public int score(int prefix) { return prefix; }
    public int arity(String left) { return 1; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let broken_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "BrokenNamed")
        .unwrap();
    let named_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Named")
        .unwrap();
    let name_method = named_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "name" => Some(method),
            _ => None,
        })
        .unwrap();

    let impls =
        trait_impl_emissions_for_type(&representations, &resolved.program, broken_ty, &signatures);

    let message_policy = trait_impl_method_message_policy(
        broken_ty,
        name_method,
        TraitImplMethodFallbackPolicy::IncompatibleImplementation {
            error: TraitImplMethodCompatibilityError::ReturnTypeMismatch,
        },
    );
    assert_eq!(
        message_policy,
        TraitImplMethodMessagePolicy {
            message:
                "incompatible Java interface method implementation: BrokenNamed.name return type does not match interface"
                    .to_string(),
        }
    );
    assert_eq!(
        message_policy.compile_error_body("fn name(&self) -> String;"),
        "fn name(&self) -> String { compile_error!(\"incompatible Java interface method implementation: BrokenNamed.name return type does not match interface\"); }"
    );

    assert_eq!(impls.len(), 1);
    assert!(impls[0].contains("impl Named for BrokenNamed"));
    assert!(
        impls[0].contains(
            "compile_error!(\"incompatible Java interface method implementation: BrokenNamed.name return type does not match interface\")"
        ),
        "{}",
        impls[0]
    );
    assert!(
        impls[0].contains(
            "compile_error!(\"incompatible Java interface method implementation: BrokenNamed.score parameter types do not match interface\")"
        ),
        "{}",
        impls[0]
    );
    assert!(
        impls[0].contains(
            "compile_error!(\"incompatible Java interface method implementation: BrokenNamed.arity parameter count does not match interface\")"
        ),
        "{}",
        impls[0]
    );
}

#[test]
fn representation_lowering_prefers_compatible_dyn_trait_impl_overloads() {
    let parsed = parse_java_source(
        r#"
interface Named {
    int score(String prefix);
}

class OverloadedNamed implements Named {
    public int score(int prefix) { return prefix; }
    public int score(String prefix) { return 1; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let overloaded_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "OverloadedNamed")
        .unwrap();

    let impls = trait_impl_emissions_for_type(
        &representations,
        &resolved.program,
        overloaded_ty,
        &signatures,
    );

    assert_eq!(impls.len(), 1);
    assert!(impls[0].contains("impl Named for OverloadedNamed"));
    assert!(!impls[0].contains("compile_error!"), "{}", impls[0]);
    assert!(
        impls[0].contains(
            "fn score(&self, prefix: String) -> i32 { OverloadedNamed::score_string(self, prefix) }"
        ),
        "{}",
        impls[0]
    );
}

#[test]
fn representation_lowering_owns_dyn_trait_impl_compatibility_policy() {
    let source = include_str!("../src/representation_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("trait_impl_method_compatibility_error"));
    assert!(source.contains("pub enum TraitImplMethodFallbackPolicy"));
    assert!(source.contains("pub enum TraitImplMethodCompatibilityError"));
    assert!(source.contains("pub struct TraitImplMethodMessagePolicy"));
    assert!(source.contains("pub fn trait_impl_method_message_policy"));
    assert!(source.contains("implementing_method_candidates"));
    assert!(source.contains("incompatible Java interface method implementation"));
    assert!(source.contains("parameter count does not match interface"));
    assert!(source.contains("type_emission_for_with_facts"));
    assert!(source.contains("type_emission_for_program_with_facts_and_exceptions"));
    assert!(source.contains("raw_type_item_for_with_facts"));
    assert!(source.contains("raw_type_item_for_program_with_facts_and_exceptions"));
    assert!(source.contains("trait_method_receiver_plan"));
    assert!(source.contains("trait_impl_emissions_for_type_with_facts"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&"));
    assert!(lower_source.contains("raw_type_item_for_program_with_facts_and_exceptions"));
    assert!(!lower_source.contains("type_emission_for_with_facts"));
    assert!(!lower_source.contains("RepresentationTypeEmission"));
    assert!(!lower_source.contains("RawRustItem"));
    assert!(lower_source.contains("trait_impl_emissions_for_type_with_facts"));
    assert!(!lower_source.contains("incompatible Java interface method implementation"));
}

#[test]
fn representation_plan_does_not_render_rust_items() {
    let source = include_str!("../src/representation.rs");

    assert!(!source.contains("RepresentationTypeEmission"));
    assert!(!source.contains("RawRustItem"));
    assert!(!source.contains("pub trait"));
    assert!(!source.contains("Box<dyn std::any::Any>"));
    assert!(source.contains("ObjectRepresentation::ErasedObject"));
}

#[test]
fn hierarchy_override_validation_policy_owns_conflict_diagnostics() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
class Other {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let override_fact = HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec![
                "Add".to_string(),
                "Other".to_string(),
                "Missing".to_string(),
            ],
        },
        provenance: FactProvenance::UserOverride,
    };
    let unknown_target = HierarchyOverride {
        type_name: "MissingExpr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "MissingExprValue".to_string(),
            variants: Vec::new(),
        },
        provenance: FactProvenance::UserOverride,
    };

    let context = object_hierarchy_context(&representations);
    let policy = hierarchy_override_validation_policy(&override_fact, &context);
    let unknown_policy = hierarchy_override_validation_policy(&unknown_target, &context);

    assert_eq!(policy.type_name, "Expr");
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && diagnostic.message.contains("includes Other")
            && diagnostic.message.contains("subtype")
    }));
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && diagnostic.message.contains("unknown variant Missing")
    }));
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && (diagnostic.message.contains("omits source subtype Num")
                || diagnostic
                    .message
                    .contains("omits terminal source subtype Num"))
    }));
    assert!(unknown_policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && diagnostic.message.contains("unknown Java type MissingExpr")
    }));
}

#[test]
fn representation_plan_reports_closed_enum_override_conflicts() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Missing".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "representation_override_conflict"
                && diagnostic.message.contains("unknown variant Missing")
        ),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "representation_override_conflict"
                && (diagnostic.message.contains("omits source subtype Num")
                    || diagnostic
                        .message
                        .contains("omits terminal source subtype Num"))
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn representation_plan_does_not_apply_unknown_hierarchy_override_targets() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "MissingExpr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "MissingExprValue".to_string(),
            variants: Vec::new(),
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "representation_override_conflict"
                && diagnostic.message.contains("unknown Java type MissingExpr")
        ),
        "{diagnostics:?}"
    );
    assert!(
        representations.type_representation("MissingExpr").is_none(),
        "{representations:?}"
    );
}

#[test]
fn signature_plan_owns_local_shapes_before_lowering() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Use {
    boolean check(Add add) {
        Expr expr = (Expr) add;
        return expr instanceof Add;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let use_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = use_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "check" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(use_ty, method).unwrap();
    let expr_shape = signatures
        .local_shape(&method_signature.locals, "expr")
        .expect("expr local should have a solved shape");

    assert_eq!(
        expr_shape.java_ty,
        JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        expr_shape.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert!(
        signatures
            .local_shape(&method_signature.locals, "missing")
            .is_none()
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(
        rust.contains("let expr: ExprValue = ExprValue::Add(add);"),
        "{rust}"
    );
    assert!(
        rust.contains("return matches!(expr, ExprValue::Add(_));"),
        "{rust}"
    );
}

#[test]
fn signature_plan_owns_local_binding_mutability_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Use {
    int calc() {
        int stable = 1;
        int changed = stable;
        changed = changed + 1;
        return stable + changed;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "calc" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let local_mutability = |name: &str| {
        signatures
            .local_binding_mutability(&method_signature.locals, name)
            .unwrap()
    };

    assert!(!local_mutability("stable"));
    assert!(local_mutability("changed"));
    assert!(
        signatures
            .local_binding_mutability(&method_signature.locals, "missing")
            .is_none()
    );
    let mutated = std::collections::HashSet::from(["changed".to_string()]);
    assert_eq!(local_shape_constraints("stable", &mutated), Vec::new());
    assert_eq!(
        local_shape_constraints("changed", &mutated),
        vec![SignatureConstraint::MutableBinding]
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &FactSet::default(),
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("let stable: i32 = 1;"), "{rust}");
    assert!(rust.contains("let mut changed: i32 = stable;"), "{rust}");
}

#[test]
fn signature_plan_marks_final_native_map_receiver_mutation_local_mutable() {
    let parsed = parse_java_source(
        r#"
import java.util.SortedMap;
import java.util.TreeMap;

class Use {
    SortedMap<String, Integer> build(String key, int value) {
        final SortedMap<String, Integer> sampleToReaderMap = new TreeMap<>();
        run(() -> {
            sampleToReaderMap.put(key, value);
        });
        return sampleToReaderMap;
    }
    void run(Runnable runnable) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = plan_representations(&resolved.program, &semantic.facts);
    let signatures = solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "build" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();

    assert_eq!(
        signatures.local_binding_mutability(&method_signature.locals, "sample_to_reader_map"),
        Some(true)
    );
}

#[test]
fn signature_plan_marks_arrays_sort_target_local_mutable() {
    let parsed = parse_java_source(
        r#"
import java.util.Arrays;

class Use {
    int firstSorted() {
        int[] values = {2, 1};
        Arrays.sort(values);
        return values[0];
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "firstSorted" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();

    assert_eq!(
        signatures.local_binding_mutability(&method_signature.locals, "values"),
        Some(true)
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &FactSet::default(),
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("let mut values"), "{rust}");
    assert!(rust.contains("values.sort();"), "{rust}");
}

#[test]
fn signature_plan_marks_assignment_in_condition_locals_mutable_and_optional() {
    let parsed = parse_java_source(
        r#"
class Use {
    int drain(Cursor cursor) {
        int count = 0;
        Row row;
        while ((row = cursor.next()) != null) {
            count = count + row.size();
        }
        return count;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "drain" => Some(method),
            _ => None,
        })
        .unwrap();
    let body = method.body.as_ref().unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let row = method_signature
        .locals
        .iter()
        .find(|local| local.name == "row")
        .unwrap();

    assert_eq!(
        source_binding_nullability_constraints(body, "row"),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );
    assert!(row.binding_mutable);
    assert!(matches!(
        &row.shape.java_ty,
        JavaType::Named { name, args } if name == "Optional" && args.len() == 1
    ));
}

#[test]
fn signature_plan_refines_assignment_null_loop_return_to_non_optional() {
    let parsed = parse_java_source(
        r#"
class Use {
    Row firstMissing(Cursor cursor) {
        Row row;
        while ((row = cursor.read()) == null) {
            cursor.advance();
        }
        return row;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "firstMissing" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let row = method_signature
        .locals
        .iter()
        .find(|local| local.name == "row")
        .unwrap();

    assert!(row.binding_mutable);
    assert!(matches!(
        &row.shape.java_ty,
        JavaType::Named { name, args } if name == "Optional" && args.len() == 1
    ));
    assert_eq!(
        source_method_return_nullability_constraints(method),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::DefinitelyNonNull
        )]
    );
    assert_eq!(
        method_signature.return_shape.as_ref().unwrap().java_ty,
        JavaType::Named {
            name: "Row".to_string(),
            args: Vec::new(),
        }
    );
    assert!(!matches!(
        rust_type_for_slot(method_signature.return_shape.as_ref().unwrap()),
        RustType::Option(_)
    ));
}

#[test]
fn signature_plan_marks_null_checked_reference_locals_optional() {
    let parsed = parse_java_source(
        r#"
class Use {
    void check(Importer importer) {
        Value value = importer.read();
        if (value == null) {
            return;
        }
        value.touch();
    }
}

class Value {
    void touch() {}
}

class Importer {
    Value read() { return new Value(); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "check" => Some(method),
            _ => None,
        })
        .unwrap();
    let body = method.body.as_ref().unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let value = method_signature
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert_eq!(
        source_binding_nullability_constraints(body, "value"),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );
    assert_eq!(
        value.shape.java_ty,
        optional_java_type(JavaType::Named {
            name: "Value".to_string(),
            args: Vec::new(),
        })
    );
    assert!(matches!(
        rust_type_for_slot(&value.shape),
        RustType::Option(_)
    ));
}

#[test]
fn signature_mutated_locals_are_not_escape_boundaries() {
    let parsed = parse_java_source(
        r#"
class Use {
    void update(Animal animal) {
        Animal current = animal;
        current = animal;
    }
}

interface Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "Use".to_string(),
            rust_name: "update".to_string(),
        })
        .unwrap();
    let current = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "current")
        .unwrap();

    assert_eq!(
        current.constraints,
        vec![SignatureConstraint::MutableBinding]
    );
}

#[test]
fn signature_captured_method_bindings_are_escape_boundaries() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
interface Runnable { void run(); }

class CaptureFlow {
    void use(Animal animal) {
        Animal current = animal;
        Runnable task = () -> current.toString() + animal.toString();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "CaptureFlow".to_string(),
            rust_name: "use".to_string(),
        })
        .unwrap();
    let animal_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "animal")
        .unwrap();
    let current = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "current")
        .unwrap();

    assert!(
        method_constraints.params[animal_index]
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{method_constraints:?}"
    );
    assert!(
        current
            .constraints
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{current:?}"
    );
}

#[test]
fn signature_captured_constructor_bindings_are_escape_boundaries() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
interface Runnable { void run(); }

class CaptureFlow {
    CaptureFlow(Animal animal) {
        Animal current = animal;
        Runnable task = () -> current.toString() + animal.toString();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let constructor_constraints = constraints
        .constructors
        .get(&ConstructorKey {
            type_name: "CaptureFlow".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();
    let animal_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "animal")
        .unwrap();
    let current = constructor_constraints
        .locals
        .iter()
        .find(|local| local.name == "current")
        .unwrap();

    assert!(
        constructor_constraints.params[animal_index]
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{constructor_constraints:?}"
    );
    assert!(
        current
            .constraints
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{current:?}"
    );
}

#[test]
fn signature_capture_boundaries_come_from_resolved_capture_facts() {
    let signature_source = include_str!("../src/signature.rs");
    let usage_source = include_str!("../src/usage.rs");

    assert!(signature_source.contains("captured_binding_names_for_method"));
    assert!(signature_source.contains("captured_binding_names_for_constructor"));
    assert!(signature_source.contains("CaptureOrigin::Method"));
    assert!(signature_source.contains("CaptureOrigin::Constructor"));
    assert!(signature_source.contains("apply_capture_constraints"));
    assert!(!usage_source.contains("Captured"));
    assert!(!signature_source.contains("BindingUse::Captured"));
}

#[test]
fn signature_plan_uses_usage_facts_for_local_escape_constraints() {
    let parsed = parse_java_source(
        r#"
class LocalEscape {
    String field;

    private void use(Helper helper) {
        String value = "name";
        helper.consume(value);
        this.field = value;
    }
}

class Helper {
    void consume(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_key = MethodKey {
        type_name: "LocalEscape".to_string(),
        rust_name: method.rust_name.clone().unwrap(),
    };
    let value_constraints = constraints
        .methods
        .get(&method_key)
        .unwrap()
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert!(
        value_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{value_constraints:?}"
    );
    assert!(
        value_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{value_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let locals = signatures.method_local_shapes_or_empty(ty, method);
    let value_shape = signatures.required_local_shape(locals, "value");

    assert_eq!(value_shape.ownership, OwnershipShape::Owned);
    assert_eq!(value_shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_local_alias_escape_constraints() {
    let parsed = parse_java_source(
        r#"
class AliasEscape {
    private void use(String input, Helper helper) {
        String first = input;
        String second = first;
        helper.consume(second);
    }
}

class Helper {
    void consume(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "AliasEscape".to_string(),
        rust_name: "use".to_string(),
    };
    let method_constraints = solved_constraints.methods.get(&method_key).unwrap();
    let input_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations >= 2, "{iterations}");
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{method_constraints:?}"
    );
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{method_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.method_signature(ty, method).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_constructor_alias_escape_constraints() {
    let parsed = parse_java_source(
        r#"
class ConstructorAliasEscape {
    ConstructorAliasEscape(String input, Helper helper) {
        String first = input;
        String second = first;
        helper.consume(second);
    }
}

class Helper {
    void consume(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ConstructorAliasEscape".to_string(),
        rust_name: "new".to_string(),
    };
    let constructor_constraints = solved_constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let input_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations >= 2, "{iterations}");
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{constructor_constraints:?}"
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{constructor_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let signature = signatures.constructor_signature(ty, ctor).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_method_field_storage_constraints() {
    let parsed = parse_java_source(
        r#"
class FieldStorageFlow {
    int[] values;

    private void use(int[] input) {
        int[] first = input;
        int[] second = first;
        this.values = second;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "FieldStorageFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let method_constraints = solved_constraints.methods.get(&method_key).unwrap();
    let input_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations >= 2, "{iterations}");
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{method_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.method_signature(ty, method).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_constructor_field_storage_constraints() {
    let parsed = parse_java_source(
        r#"
class ConstructorFieldStorageFlow {
    int[] values;

    ConstructorFieldStorageFlow(int[] input) {
        int[] first = input;
        int[] second = first;
        this.values = second;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ConstructorFieldStorageFlow".to_string(),
        rust_name: "new".to_string(),
    };
    let constructor_constraints = solved_constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let input_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations > 0, "{iterations}");
    assert!(iterations <= budget, "{} > {}", iterations, budget);
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot)),
        "{constructor_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let signature = signatures.constructor_signature(ty, ctor).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_return_constraints_through_aliases() {
    let parsed = parse_java_source(
        r#"
class ReturnFlow {
    public int[] expose(int[] input) {
        int[] first = input;
        int[] second = first;
        return second;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "ReturnFlow".to_string(),
        rust_name: "expose".to_string(),
    };
    let method_constraints = solved_constraints.methods.get(&method_key).unwrap();
    let input_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations >= 2, "{iterations}");
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{method_constraints:?}"
    );
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{method_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.method_signature(ty, method).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_new_argument_unknown_boundaries_through_aliases() {
    let parsed = parse_java_source(
        r#"
class NewBoundaryFlow {
    private void use(int[] input) {
        int[] first = input;
        int[] second = first;
        new Helper(second);
    }
}

class Helper {
    Helper(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let usage = &semantic.usage;
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "NewBoundaryFlow".to_string(),
            method_name: "use".to_string(),
        })
        .unwrap();
    let second = method_usage.bindings.get("second").unwrap();

    assert!(
        second.uses.iter().any(|use_site| matches!(
            use_site,
            BindingUse::PassedToCall {
                name,
                arg_index: 0,
                boundary: CallArgumentBoundary::Unknown,
            } if name == "new"
        )),
        "{second:?}"
    );

    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "NewBoundaryFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let method_constraints = solved_constraints.methods.get(&method_key).unwrap();
    let input_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations > 0, "{iterations}");
    assert!(iterations <= budget, "{} > {}", iterations, budget);
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{method_constraints:?}"
    );
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{method_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.method_signature(ty, method).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_constructor_internal_call_param_constraints() {
    let parsed = parse_java_source(
        r#"
class ConstructorInternalCallFlow {
    private ConstructorInternalCallFlow(int[] input, Helper helper) {
        int[] alias = input;
        use(alias, helper);
    }

    private void use(int[] values, Helper helper) {
        helper.consume(values);
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ConstructorInternalCallFlow".to_string(),
        rust_name: "new".to_string(),
    };
    let constructor_constraints = solved_constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let input_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(iterations >= 2, "{iterations}");
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{constructor_constraints:?}"
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{constructor_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let signature = signatures.constructor_signature(ty, ctor).unwrap();
    let input = signature
        .params
        .iter()
        .find(|param| param.name == "input")
        .unwrap();

    assert_eq!(input.shape.ownership, OwnershipShape::Owned);
    assert_eq!(input.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_method_argument_constraints_to_internal_callee() {
    let parsed = parse_java_source(
        r#"
class MethodArgumentConstraintFlow {
    int[] cached;

    private void caller(int[] input) {
        this.cached = input;
        use(input);
    }

    private void use(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let callee_key = MethodKey {
        type_name: "MethodArgumentConstraintFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let callee_constraints = solved_constraints.methods.get(&callee_key).unwrap();
    let values_index = callee_constraints
        .param_names
        .iter()
        .position(|name| name == "values")
        .unwrap();

    assert!(
        callee_constraints.params[values_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{callee_constraints:?}"
    );
    assert!(
        callee_constraints.params[values_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{callee_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let callee = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let values = signatures.required_method_signature(ty, callee).params[0]
        .shape
        .clone();

    assert_eq!(values.ownership, OwnershipShape::Owned);
    assert_eq!(values.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_constructor_argument_constraints_to_internal_callee() {
    let parsed = parse_java_source(
        r#"
class ConstructorArgumentConstraintFlow {
    int[] cached;

    ConstructorArgumentConstraintFlow(int[] input) {
        this.cached = input;
        use(input);
    }

    private void use(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let callee_key = MethodKey {
        type_name: "ConstructorArgumentConstraintFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let callee_constraints = solved_constraints.methods.get(&callee_key).unwrap();
    let values_index = callee_constraints
        .param_names
        .iter()
        .position(|name| name == "values")
        .unwrap();

    assert!(
        callee_constraints.params[values_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{callee_constraints:?}"
    );
    assert!(
        callee_constraints.params[values_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{callee_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let callee = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let values = signatures.required_method_signature(ty, callee).params[0]
        .shape
        .clone();

    assert_eq!(values.ownership, OwnershipShape::Owned);
    assert_eq!(values.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_does_not_push_public_method_boundary_into_internal_callee() {
    let parsed = parse_java_source(
        r#"
class PublicMethodBoundaryFlow {
    public int expose(String value) {
        return use(value);
    }

    private int use(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let public_key = MethodKey {
        type_name: "PublicMethodBoundaryFlow".to_string(),
        rust_name: "expose".to_string(),
    };
    assert!(constraints.methods.get(&public_key).unwrap().public_api);

    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let callee_key = MethodKey {
        type_name: "PublicMethodBoundaryFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let callee_constraints = solved_constraints.methods.get(&callee_key).unwrap();
    let value_index = callee_constraints
        .param_names
        .iter()
        .position(|name| name == "value")
        .unwrap();

    assert!(
        !callee_constraints.params[value_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{callee_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let public_method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();
    let helper = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let public_param = &signatures
        .required_method_signature(ty, public_method)
        .params[0]
        .shape;
    let helper_param = &signatures.required_method_signature(ty, helper).params[0].shape;

    assert_eq!(public_param.ownership, OwnershipShape::Owned);
    assert_eq!(public_param.view, ViewShape::Native);
    assert_eq!(helper_param.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(helper_param.view, ViewShape::Str);
}

#[test]
fn signature_fixpoint_does_not_push_public_constructor_boundary_into_internal_callee() {
    let parsed = parse_java_source(
        r#"
class PublicConstructorBoundaryFlow {
    public PublicConstructorBoundaryFlow(String value) {
        use(value);
    }

    private int use(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let constructor_key = ConstructorKey {
        type_name: "PublicConstructorBoundaryFlow".to_string(),
        rust_name: "new".to_string(),
    };
    assert!(
        constraints
            .constructors
            .get(&constructor_key)
            .unwrap()
            .public_api
    );

    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let callee_key = MethodKey {
        type_name: "PublicConstructorBoundaryFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let callee_constraints = solved_constraints.methods.get(&callee_key).unwrap();
    let value_index = callee_constraints
        .param_names
        .iter()
        .position(|name| name == "value")
        .unwrap();

    assert!(
        !callee_constraints.params[value_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{callee_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let helper = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let public_param = &signatures.required_constructor_signature(ty, ctor).params[0].shape;
    let helper_param = &signatures.required_method_signature(ty, helper).params[0].shape;

    assert_eq!(public_param.ownership, OwnershipShape::Owned);
    assert_eq!(public_param.view, ViewShape::Native);
    assert_eq!(helper_param.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(helper_param.view, ViewShape::Str);
}

#[test]
fn signature_fixpoint_propagates_internal_call_result_constraints_to_local() {
    let parsed = parse_java_source(
        r#"
class InternalCallResultLocalFlow {
    private int[] expose(int[] input, Helper helper) {
        int[] value = make(input, helper);
        return value;
    }

    private int[] make(int[] input, Helper helper) {
        helper.consume(input);
        return input;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "InternalCallResultLocalFlow".to_string(),
        rust_name: "expose".to_string(),
    };
    let method_constraints = solved_constraints.methods.get(&method_key).unwrap();
    let local_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{local_constraints:?}"
    );
    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{local_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.required_method_signature(ty, method);
    let value = signature
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert_eq!(value.shape.ownership, OwnershipShape::Owned);
    assert_eq!(value.shape.view, ViewShape::Native);
    assert_eq!(
        signature.return_shape.as_ref().unwrap().ownership,
        OwnershipShape::Owned
    );
    assert_eq!(
        signature.return_shape.as_ref().unwrap().view,
        ViewShape::Native
    );
}

#[test]
fn signature_fixpoint_propagates_constructor_internal_call_result_constraints_to_local() {
    let parsed = parse_java_source(
        r#"
class ConstructorInternalCallResultLocalFlow {
    private ConstructorInternalCallResultLocalFlow(int[] input, Helper helper) {
        int[] value = make(input, helper);
    }

    private int[] make(int[] input, Helper helper) {
        helper.consume(input);
        return input;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ConstructorInternalCallResultLocalFlow".to_string(),
        rust_name: "new".to_string(),
    };
    let constructor_constraints = solved_constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let local_constraints = constructor_constraints
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{local_constraints:?}"
    );
    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{local_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let signature = signatures.required_constructor_signature(ty, ctor);
    let value = signature
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert_eq!(value.shape.ownership, OwnershipShape::Owned);
    assert_eq!(value.shape.view, ViewShape::Native);
}

#[test]
fn signature_fixpoint_propagates_internal_call_result_constraints_to_field() {
    let parsed = parse_java_source(
        r#"
class InternalCallResultFieldFlow {
    int[] cached;

    private void expose(int[] input, Helper helper) {
        this.cached = make(input, helper);
    }

    private int[] make(int[] input, Helper helper) {
        helper.consume(input);
        return input;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let field_constraints = solved_constraints
        .fields
        .get(&FieldKey {
            type_name: "InternalCallResultFieldFlow".to_string(),
            field_name: "cached".to_string(),
        })
        .unwrap();

    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
        )),
        "{field_constraints:?}"
    );
    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredView(ViewShape::Native)
        )),
        "{field_constraints:?}"
    );
}

#[test]
fn signature_fixpoint_propagates_constructor_internal_call_result_constraints_to_field() {
    let parsed = parse_java_source(
        r#"
class ConstructorInternalCallResultFieldFlow {
    int[] cached;

    private ConstructorInternalCallResultFieldFlow(int[] input, Helper helper) {
        this.cached = make(input, helper);
    }

    private int[] make(int[] input, Helper helper) {
        helper.consume(input);
        return input;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let field_constraints = solved_constraints
        .fields
        .get(&FieldKey {
            type_name: "ConstructorInternalCallResultFieldFlow".to_string(),
            field_name: "cached".to_string(),
        })
        .unwrap();

    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
        )),
        "{field_constraints:?}"
    );
    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredView(ViewShape::Native)
        )),
        "{field_constraints:?}"
    );
}

#[test]
fn usage_records_constructor_delegation_as_resolved_internal_edge() {
    let parsed = parse_java_source(
        r#"
class ConstructorDelegationUsageFlow {
    private ConstructorDelegationUsageFlow(int[] input) {
        this(input, 0);
    }

    private ConstructorDelegationUsageFlow(int[] input, int depth) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constructor_usage = usage
        .constructors
        .get(&new_java2rust::usage::UsageConstructorKey {
            type_name: "ConstructorDelegationUsageFlow".to_string(),
            rust_name: "new_int_array".to_string(),
        })
        .unwrap();
    let call_site = constructor_usage.constructor_call_sites.first().unwrap();

    assert_eq!(call_site.name, "constructor");
    assert_eq!(call_site.arg_count, 2);
    assert_eq!(call_site.boundary, CallArgumentBoundary::InternalSameType);
    assert_eq!(
        call_site.resolved.as_ref().unwrap().declaring_type,
        "ConstructorDelegationUsageFlow"
    );
    assert_eq!(
        call_site.resolved.as_ref().unwrap().rust_name,
        "new_int_array_int"
    );
    assert!(
        constructor_usage
            .bindings
            .get("input")
            .unwrap()
            .uses
            .iter()
            .any(|usage| matches!(
                usage,
                BindingUse::PassedToCall {
                    name,
                    arg_index: 0,
                    boundary: CallArgumentBoundary::InternalSameType,
                } if name == "constructor"
            ))
    );
}

#[test]
fn signature_fixpoint_propagates_constructor_delegation_constraints() {
    let parsed = parse_java_source(
        r#"
class ConstructorDelegationConstraintFlow {
    private ConstructorDelegationConstraintFlow(int[] input, Helper helper) {
        this(input, helper, 0);
    }

    private ConstructorDelegationConstraintFlow(int[] input, Helper helper, int depth) {
        helper.consume(input);
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    assert_eq!(
        signature_fixpoint_saturation_policy(&constraints).iteration_budget(),
        budget
    );
    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ConstructorDelegationConstraintFlow".to_string(),
        rust_name: "new_int_array_helper".to_string(),
    };
    let constructor_constraints = result
        .constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let input_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.iterations <= budget,
        "{} > {}",
        result.iterations,
        budget
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{constructor_constraints:?}"
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{constructor_constraints:?}"
    );
}

#[test]
fn signature_fixpoint_propagates_chained_constructor_delegation_constraints() {
    let parsed = parse_java_source(
        r#"
class ChainedConstructorDelegationConstraintFlow {
    private ChainedConstructorDelegationConstraintFlow(int[] input, Helper helper) {
        this(input, helper, 0);
    }

    private ChainedConstructorDelegationConstraintFlow(int[] input, Helper helper, int depth) {
        this(input, helper, depth, 0);
    }

    private ChainedConstructorDelegationConstraintFlow(int[] input, Helper helper, int depth, int salt) {
        helper.consume(input);
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);
    let constructor_key = ConstructorKey {
        type_name: "ChainedConstructorDelegationConstraintFlow".to_string(),
        rust_name: "new_int_array_helper".to_string(),
    };
    let constructor_constraints = result
        .constraints
        .constructors
        .get(&constructor_key)
        .unwrap();
    let input_index = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.iterations <= budget,
        "{} > {}",
        result.iterations,
        budget
    );
    assert!(
        result.pass_plan.stats.planned_components > 0,
        "{:?}",
        result.pass_plan.stats
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{constructor_constraints:?}"
    );
    assert!(
        constructor_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{constructor_constraints:?}"
    );
}

#[test]
fn signature_dependency_graph_contains_boundary_slot_nodes() {
    let parsed = parse_java_source(
        r#"
class SignatureGraphSlots {
    int[] cached;

    SignatureGraphSlots(int[] input) {
        int[] alias = input;
        this.cached = alias;
    }

    private int[] expose(int[] input) {
        int[] alias = input;
        return alias;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let graph = new_java2rust::signature::build_signature_dependency_graph(&constraints, &usage);
    let method = MethodKey {
        type_name: "SignatureGraphSlots".to_string(),
        rust_name: "expose".to_string(),
    };
    let constructor = ConstructorKey {
        type_name: "SignatureGraphSlots".to_string(),
        rust_name: "new".to_string(),
    };

    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::MethodParam {
            method: method.clone(),
            name: "input".to_string(),
            index: 0,
        }
    ));
    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::MethodReturn {
            method: method.clone(),
        }
    ));
    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::MethodLocal {
            method: method.clone(),
            name: "alias".to_string(),
        }
    ));
    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::MethodReceiver {
            method: method.clone(),
        }
    ));
    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::ConstructorParam {
            constructor: constructor.clone(),
            name: "input".to_string(),
            index: 0,
        }
    ));
    assert!(graph.nodes.contains(
        &new_java2rust::signature::SignatureDependencyNode::ConstructorLocal {
            constructor,
            name: "alias".to_string(),
        }
    ));
    assert!(
        graph
            .nodes
            .contains(&new_java2rust::signature::SignatureDependencyNode::Field {
                field: FieldKey {
                    type_name: "SignatureGraphSlots".to_string(),
                    field_name: "cached".to_string(),
                },
            })
    );
}

#[test]
fn signature_dependency_graph_records_alias_field_return_and_call_edges() {
    let parsed = parse_java_source(
        r#"
class SignatureGraphEdges {
    int[] cached;

    private int[] caller(int[] input, Helper helper) {
        int[] alias = input;
        this.cached = alias;
        int[] made = make(alias, helper);
        helper.consume(made);
        return this.cached;
    }

    private int[] make(int[] values, Helper helper) {
        helper.consume(values);
        return values;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let graph = new_java2rust::signature::build_signature_dependency_graph(&constraints, &usage);

    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::LocalAlias
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::FieldStorage
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::MethodReturn
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::CallArgument
            && matches!(
                &edge.to,
                new_java2rust::signature::SignatureDependencyNode::MethodParam { method, name, .. }
                    if method.type_name == "SignatureGraphEdges"
                        && method.rust_name == "make"
                        && name == "values"
            )
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::CallResult
            && matches!(
                &edge.from,
                new_java2rust::signature::SignatureDependencyNode::MethodReturn { method }
                    if method.type_name == "SignatureGraphEdges"
                        && method.rust_name == "make"
            )
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::UnknownBoundary
            && matches!(
                &edge.to,
                new_java2rust::signature::SignatureDependencyNode::UnknownBoundaryArg { .. }
            )
    }));
}

#[test]
fn signature_dependency_graph_records_constructor_delegation_argument_edges() {
    let parsed = parse_java_source(
        r#"
class SignatureGraphCtorDelegation {
    SignatureGraphCtorDelegation(int[] input) {
        this(input, 0);
    }

    SignatureGraphCtorDelegation(int[] input, int depth) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let graph = new_java2rust::signature::build_signature_dependency_graph(&constraints, &usage);

    assert!(graph.edges.iter().any(|edge| {
        edge.kind
            == new_java2rust::signature::SignatureDependencyEdgeKind::ConstructorDelegationArgument
            && matches!(
                &edge.to,
                new_java2rust::signature::SignatureDependencyNode::ConstructorParam {
                    constructor,
                    name,
                    index: 0,
                } if constructor.type_name == "SignatureGraphCtorDelegation"
                    && constructor.rust_name == "new_int_array_int"
                    && name == "input"
            )
    }));
}

#[test]
fn signature_dependency_graph_records_captured_value_nodes_from_facts() {
    let parsed = parse_java_source(
        r#"
interface Mapper {
    String map(String value);
}

class CaptureGraph {
    Mapper make(String base) {
        return value -> base + value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let graph = new_java2rust::signature::build_signature_dependency_graph_with_facts(
        &constraints,
        &usage,
        &resolved.facts,
    );

    assert!(graph.nodes.iter().any(|node| {
        matches!(
            node,
            new_java2rust::signature::SignatureDependencyNode::CapturedValue { name, .. }
                if name == "base"
        )
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::signature::SignatureDependencyEdgeKind::LocalAlias
            && matches!(
                &edge.to,
                new_java2rust::signature::SignatureDependencyNode::CapturedValue { name, .. }
                    if name == "base"
            )
    }));
}

#[test]
fn signature_solver_pass_plan_orders_all_dependency_components() {
    let parsed = parse_java_source(
        r#"
class SignaturePassPlan {
    int[] cached;

    private int[] caller(int[] input, Helper helper) {
        int[] alias = input;
        this.cached = alias;
        return make(alias, helper);
    }

    private int[] make(int[] values, Helper helper) {
        helper.consume(values);
        return values;
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let graph = new_java2rust::signature::build_signature_dependency_graph(&constraints, &usage);
    let sccs = new_java2rust::signature::analyze_signature_dependency_sccs(&graph);
    let pass_plan = new_java2rust::signature::plan_signature_solver_passes(&sccs);

    assert_eq!(
        pass_plan.stats.planned_components,
        sccs.stats.component_count
    );
    assert_eq!(pass_plan.component_order.len(), sccs.stats.component_count);
    assert_eq!(
        pass_plan
            .component_order
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        sccs.stats.component_count
    );
}

#[test]
fn signature_fixpoint_saturates_recursive_constructor_delegation_within_budget() {
    let parsed = parse_java_source(
        r#"
class RecursiveConstructorDelegationFlow {
    private RecursiveConstructorDelegationFlow(int[] input) {
        this(input, 0);
    }

    private RecursiveConstructorDelegationFlow(int[] input, int depth) {
        this(input);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.iterations <= budget,
        "{} > {}",
        result.iterations,
        budget
    );
    assert!(
        result.dependency_sccs.stats.node_count > 0,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert!(
        result.dependency_sccs.stats.cyclic_component_count >= 1,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert_eq!(
        result.pass_plan.stats.planned_components,
        result.dependency_sccs.stats.component_count
    );
    assert_eq!(
        result.pass_plan.stats.cyclic_components,
        result.dependency_sccs.stats.cyclic_component_count
    );
}

#[test]
fn signature_fixpoint_propagates_unknown_call_result_constraints_to_return_slot() {
    let parsed = parse_java_source(
        r#"
class UnknownResultReturnFlow {
    private ExternalName expose(Helper helper) {
        ExternalName value = helper.fetch();
        ExternalName alias = value;
        return alias;
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalName".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_key = MethodKey {
        type_name: "UnknownResultReturnFlow".to_string(),
        rust_name: "expose".to_string(),
    };

    assert!(
        constraints
            .methods
            .get(&method_key)
            .unwrap()
            .return_constraints
            .is_empty()
    );

    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);
    let method_constraints = result.constraints.methods.get(&method_key).unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        method_constraints
            .return_constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{method_constraints:?}"
    );
    assert!(
        method_constraints
            .return_constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{method_constraints:?}"
    );
}

#[test]
fn signature_constraints_apply_unknown_new_result_boundary_to_direct_return_slot() {
    let parsed = parse_java_source(
        r#"
class UnknownNewResultReturnFlow {
    private ExternalName create() {
        return new ExternalName();
    }
}

class ExternalName {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "create" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_return_shape_constraints(ty, method, &usage, &facts),
        conservative_boundary_constraints(&external_name_type(), &facts)
    );

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "UnknownNewResultReturnFlow".to_string(),
            rust_name: "create".to_string(),
        })
        .unwrap();

    assert_eq!(
        method_constraints.return_constraints,
        conservative_boundary_constraints(&external_name_type(), &facts)
    );
}

#[test]
fn signature_constraints_apply_unknown_new_result_boundary_through_ternary_return_slot() {
    let parsed = parse_java_source(
        r#"
class UnknownNewTernaryReturnFlow {
    private ExternalName choose(boolean ready) {
        return ready ? new ExternalName() : new ExternalName();
    }
}

class ExternalName {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "choose" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_return_shape_constraints(ty, method, &usage, &facts),
        conservative_boundary_constraints(&external_name_type(), &facts)
    );
}

#[test]
fn signature_fixpoint_saturates_recursive_call_cycles_within_budget() {
    let parsed = parse_java_source(
        r#"
class RecursiveFlow {
    private int[] recur(int[] input, Helper helper) {
        if (input.length == 0) {
            helper.consume(input);
            return input;
        }
        return recur(input, helper);
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);
    let method_key = MethodKey {
        type_name: "RecursiveFlow".to_string(),
        rust_name: "recur".to_string(),
    };
    let method_constraints = result.constraints.methods.get(&method_key).unwrap();
    let input_index = method_constraints
        .param_names
        .iter()
        .position(|name| name == "input")
        .unwrap();

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.iterations <= budget,
        "{} > {}",
        result.iterations,
        budget
    );
    assert!(
        result.dependency_sccs.stats.node_count > 0,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert!(
        result.dependency_sccs.stats.cyclic_component_count >= 1,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert_eq!(
        result.pass_plan.stats.planned_components,
        result.dependency_sccs.stats.component_count
    );
    assert_eq!(
        result.pass_plan.stats.cyclic_components,
        result.dependency_sccs.stats.cyclic_component_count
    );
    assert!(
        result.dependency_sccs.stats.max_component_size >= 2,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{method_constraints:?}"
    );
    assert!(
        method_constraints.params[input_index]
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
            )),
        "{method_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    assert!(
        signatures.diagnostics().is_empty(),
        "{:?}",
        signatures.diagnostics()
    );
    assert!(signatures.solve_iterations() <= budget);
}

#[test]
fn signature_fixpoint_saturates_mutual_recursive_call_cycles_within_budget() {
    let parsed = parse_java_source(
        r#"
class MutualRecursiveFlow {
    private int[] first(int[] input, Helper helper) {
        return second(input, helper);
    }

    private int[] second(int[] input, Helper helper) {
        if (input.length == 0) {
            helper.consume(input);
            return input;
        }
        return first(input, helper);
    }
}

class Helper {
    void consume(int[] values) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let budget = signature_fixpoint_iteration_budget(&constraints);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, &usage);

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result.iterations <= budget,
        "{} > {}",
        result.iterations,
        budget
    );
    assert!(
        result.dependency_sccs.stats.node_count > 0,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert!(
        result.dependency_sccs.stats.cyclic_component_count >= 1,
        "{:?}",
        result.dependency_sccs.stats
    );
    assert_eq!(
        result.pass_plan.stats.planned_components,
        result.dependency_sccs.stats.component_count
    );
    assert_eq!(
        result.pass_plan.stats.cyclic_components,
        result.dependency_sccs.stats.cyclic_component_count
    );
    assert!(
        result.dependency_sccs.stats.max_component_size >= 2,
        "{:?}",
        result.dependency_sccs.stats
    );

    for rust_name in ["first", "second"] {
        let method_key = MethodKey {
            type_name: "MutualRecursiveFlow".to_string(),
            rust_name: rust_name.to_string(),
        };
        let method_constraints = result.constraints.methods.get(&method_key).unwrap();
        let input_index = method_constraints
            .param_names
            .iter()
            .position(|name| name == "input")
            .unwrap();

        assert!(
            method_constraints.params[input_index]
                .iter()
                .any(|constraint| matches!(
                    constraint,
                    SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
                )),
            "{method_constraints:?}"
        );
        assert!(
            method_constraints.params[input_index]
                .iter()
                .any(|constraint| matches!(
                    constraint,
                    SignatureConstraint::RequiredView(ViewShape::Native)
                )),
            "{method_constraints:?}"
        );
    }

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    assert!(
        signatures.diagnostics().is_empty(),
        "{:?}",
        signatures.diagnostics()
    );
    assert!(signatures.solve_iterations() <= budget);
}

#[test]
fn constructor_field_initializers_use_planned_field_shapes() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {
    Add() {}
}
class Holder {
    Expr expr = (Expr) new Add();
    Holder() {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "Maker".to_string(),
            method_name: "make".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Named {
            name: "Exception".to_string(),
            args: Vec::new(),
        }],
        provenance: FactProvenance::ParsedSource,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let expr_java_ty = JavaType::Named {
        name: "Expr".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        storage_shape_for_java_type(&representations, expr_java_ty).dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert_eq!(
        signatures.field_shape("Holder", "expr").unwrap().dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("expr: ExprValue"), "{rust}");
    assert!(rust.contains("expr: ExprValue::Add(Add::new())"), "{rust}");
}

#[test]
fn constructor_plan_owns_field_value_source_selection() {
    let parsed = parse_java_source(
        r#"
class Holder {
    String assigned;
    String initialized = "fallback";
    int defaulted;

    Holder(String input) {
        this.assigned = input;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let plan = plan_constructor_field_values(ty, ctor.body.as_ref(), &signatures);

    assert_eq!(plan.len(), 3);
    assert!(matches!(
        &plan[0],
        ConstructorFieldValuePlan::Assigned {
            field_name,
            shape,
            ..
        } if field_name == "assigned" && shape.java_ty == JavaType::String
    ));
    assert!(matches!(
        &plan[1],
        ConstructorFieldValuePlan::FieldInitializer {
            field_name,
            shape,
            ..
        } if field_name == "initialized" && shape.java_ty == JavaType::String
    ));
    assert!(matches!(
        &plan[2],
        ConstructorFieldValuePlan::Default { field_name, shape }
            if field_name == "defaulted"
                && shape.java_ty == JavaType::Primitive(PrimitiveType::Int)
    ));
}

#[test]
fn constructor_delegation_plan_owns_this_and_super_policy_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Delegation {
    int value;
    Delegation() {
        this(1);
    }
    Delegation(String label) {
        this(1);
        this.value = 2;
        audit();
    }
    Delegation(int value) {}
    void audit() {}
}

class Child extends Delegation {
    Child() {
        super();
    }
}

class ExternalProblem extends UserException {
    ExternalProblem(String message) {
        super(message);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let delegation = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Delegation")
        .unwrap();
    let default_ctor = delegation
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) if ctor.params.is_empty() => Some(ctor),
            _ => None,
        })
        .unwrap();
    let plan = plan_constructor_delegation(default_ctor.body.as_ref().unwrap()).unwrap();
    assert!(matches!(
        plan,
        ConstructorDelegationPlan::SameClassCall { resolved, .. }
            if resolved.rust_name == "new_int"
    ));
    let trailing_ctor = delegation
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor)
                if ctor.params.len() == 1 && ctor.params[0].name == "label" =>
            {
                Some(ctor)
            }
            _ => None,
        })
        .unwrap();
    let plan = plan_constructor_delegation(trailing_ctor.body.as_ref().unwrap()).unwrap();
    assert!(matches!(
        plan,
        ConstructorDelegationPlan::SameClassCallWithTrailing {
            resolved,
            trailing,
            ..
        } if resolved.rust_name == "new_int"
            && matches!(
                trailing.as_slice(),
                [
                    DelegatedConstructorTrailingStmt::FieldAssignment { field_name, .. },
                    DelegatedConstructorTrailingStmt::InstanceCall { resolved, args }
                ] if field_name == "value" && resolved.rust_name == "audit" && args.is_empty()
            )
    ));
    assert_eq!(
        constructor_invocation_boundary_policy(ConstructorInvocationKind::This),
        ConstructorInvocationBoundaryPolicy {
            kind: ConstructorInvocationKind::This,
            boundary: CallArgumentBoundary::InternalSameType,
        }
    );

    let child = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Child")
        .unwrap();
    let child_ctor = child
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let plan = plan_constructor_delegation(child_ctor.body.as_ref().unwrap()).unwrap();
    assert!(matches!(
        plan,
        ConstructorDelegationPlan::Unsupported(
            new_java2rust::unsupported::UnsupportedConstructorLowering::SuperRequiresInheritanceLowering
        )
    ));
    assert_eq!(
        constructor_invocation_boundary_policy(ConstructorInvocationKind::Super),
        ConstructorInvocationBoundaryPolicy {
            kind: ConstructorInvocationKind::Super,
            boundary: CallArgumentBoundary::Unknown,
        }
    );
    let external_problem = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "ExternalProblem")
        .unwrap();
    let external_problem_ctor = external_problem
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        new_java2rust::constructor_plan::plan_constructor_delegation_for_type(
            Some(external_problem),
            external_problem_ctor.body.as_ref().unwrap()
        ),
        Some(ConstructorDelegationPlan::ExternalExceptionSuperclass {
            superclass,
            ..
        }) if superclass == "UserException"
    ));
}

#[test]
fn shape_projection_does_not_render_rust_types() {
    let source = include_str!("../src/shape_projection.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustType"));
    assert!(!source.contains("slot_render"));
    assert!(!source.contains("rust_type_for_slot"));
}

#[test]
fn slot_shape_does_not_own_indexed_projection_rules() {
    let source = include_str!("../src/slot_shape.rs");

    assert!(!source.contains("indexed_read_shape"));
    assert!(!source.contains("indexed_assignment_shape"));
}

#[test]
fn slot_shape_does_not_own_lowered_expression_actual_wrappers() {
    let source = include_str!("../src/slot_shape.rs");

    assert!(!source.contains("LoweredType"));
    assert!(!source.contains("OptionalElementBorrow"));
    assert!(!source.contains("VecElementBorrow"));
}

#[test]
fn shape_projection_owns_java_value_shape_projection() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);
    let expr_ty = JavaType::Named {
        name: "Expr".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        project_java_value_shape(Some(&representations), expr_ty).dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert_eq!(
        escaping_role_dispatch_policy(DispatchShape::DynTrait {
            trait_name: "Expr".to_string(),
        }),
        EscapingRoleDispatchPolicy {
            source_dispatch: DispatchShape::DynTrait {
                trait_name: "Expr".to_string(),
            },
            projected_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Expr".to_string(),
            },
        }
    );
    assert_eq!(
        escaping_role_dispatch_policy(DispatchShape::Concrete),
        EscapingRoleDispatchPolicy {
            source_dispatch: DispatchShape::Concrete,
            projected_dispatch: DispatchShape::Concrete,
        }
    );
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Storage,
            DispatchShape::DynTrait {
                trait_name: "Expr".to_string(),
            },
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
        ),
        DispatchRequirementProjectionPolicy {
            role: DispatchRequirementProjectionRole::Storage,
            source_dispatch: DispatchShape::DynTrait {
                trait_name: "Expr".to_string(),
            },
            requirement: DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
            projected_dispatch: Some(DispatchShape::BoxedDynTrait {
                trait_name: "Expr".to_string(),
            }),
        }
    );
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Param,
            DispatchShape::DynTrait {
                trait_name: "Expr".to_string(),
            },
            DispatchRequirementKind::NeedsBorrowedTraitView,
        )
        .projected_dispatch,
        Some(DispatchShape::DynTrait {
            trait_name: "Expr".to_string(),
        })
    );
    assert_eq!(
        dispatch_requirement_projection_policy(
            DispatchRequirementProjectionRole::Return,
            DispatchShape::DynTrait {
                trait_name: "Expr".to_string(),
            },
            DispatchRequirementKind::RejectTraitObject,
        )
        .projected_dispatch,
        Some(DispatchShape::ErasedObject)
    );
    let shape_projection_source = include_str!("../src/shape_projection.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    assert!(shape_projection_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(shape_projection_source.contains("pub struct DispatchProjectionPolicy"));
    assert!(shape_projection_source.contains("pub fn dispatch_projection_policy"));
    assert!(shape_projection_source.contains("pub struct EscapingRoleDispatchPolicy"));
    assert!(shape_projection_source.contains("pub fn escaping_role_dispatch_policy"));
    assert!(shape_projection_source.contains("pub enum DispatchRequirementProjectionRole"));
    assert!(shape_projection_source.contains("pub struct DispatchRequirementProjectionPolicy"));
    assert!(shape_projection_source.contains("pub fn dispatch_requirement_projection_policy"));
    assert!(!shape_projection_source.contains("fn owned_dispatch_for_escaping_role"));
    assert!(shape_projection_source.contains("pub struct ShapeProjectionCapabilityPolicy"));
    assert!(shape_projection_source.contains("pub fn shape_projection_capability_policy"));
    assert!(
        shape_projection_source
            .contains("pub fn shape_projection_capability_policy_with_effective_facts")
    );
    assert!(shape_projection_source.contains("pub struct IndexedElementProjectionPolicy"));
    assert!(shape_projection_source.contains("pub fn indexed_element_projection_policy"));
    assert!(
        shape_projection_source
            .contains("pub fn indexed_element_projection_policy_with_effective_facts")
    );
    assert!(shape_projection_source.contains("pub fn indexed_read_shape_with_effective_facts"));
    assert!(
        shape_projection_source.contains("pub fn indexed_assignment_shape_with_effective_facts")
    );
    assert!(shape_projection_source.contains("pub fn iteration_element_projection_policy"));
    assert!(
        shape_projection_source
            .contains("pub fn iteration_element_projection_policy_with_effective_facts")
    );
    assert!(shape_projection_source.contains("pub struct SequenceElementProjectionPolicy"));
    assert!(shape_projection_source.contains("pub fn sequence_element_projection_policy"));
    assert!(
        shape_projection_source
            .contains("pub fn sequence_element_projection_policy_with_effective_facts")
    );
    assert!(!shape_projection_source.contains("fn sequence_element_type_with_facts"));
    assert!(shape_projection_source.contains("indexed_assignment_shape_with_facts"));
    assert!(effective_facts_source.contains("pub fn is_copy_type"));
    assert!(effective_facts_source.contains("pub fn sequence_element_type_for"));
    assert_eq!(
        shape_projection_source
            .matches("facts.is_copy_type")
            .count(),
        1
    );
    assert_eq!(
        shape_projection_source
            .matches("facts.sequence_element_type_for")
            .count(),
        1
    );
    assert!(shape_projection_source.contains(
        "sequence_element_projection_policy_with_effective_facts(&array_shape.java_ty, facts)"
    ));
    assert!(!shape_projection_source.contains(
        "let element_ty = sequence_element_projection_policy(&array_shape.java_ty, facts)"
    ));
    assert!(!include_str!("../src/expr_shape.rs").contains("pub fn project_java_value_shape"));
}

#[test]
fn call_plan_owns_call_and_new_target_selection_before_argument_lowering() {
    let receiver = Expr::Var("helper".to_string());
    let resolved_static = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Owner".to_string(),
        rust_name: "make".to_string(),
        return_type: JavaType::Primitive(PrimitiveType::Int),
        kind: new_java2rust::java_ir::CallKind::Static,
        arg_coercions: Vec::new(),
    };
    assert!(matches!(
        plan_call_target(None, "make", Some(&resolved_static)),
        CallTargetPlan::StaticSelf { rust_name: "make" }
    ));
    assert!(matches!(
        plan_call_target(
            Some(&Expr::FieldAccess(
                Box::new(Expr::Var("coordinates".to_string())),
                "genomics_d_b_column".to_string()
            )),
            "newBuilder",
            Some(&resolved_static)
        ),
        CallTargetPlan::StaticOwner {
            owner,
            rust_name
        } if owner == "Owner" && rust_name == "make"
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&Expr::FieldAccess(
                Box::new(Expr::Var("coordinates".to_string())),
                "genomics_d_b_column".to_string()
            )),
            "newBuilder",
            None,
            &FactSet::default(),
            &new_java2rust::types::TypeEnv::default(),
        ),
        CallTargetPlan::StaticOwner {
            owner,
            rust_name
        } if owner == "GenomicsDBColumn" && rust_name == "new_builder"
    ));
    let resolved_instance = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Helper".to_string(),
        rust_name: "read_name".to_string(),
        return_type: JavaType::String,
        kind: new_java2rust::java_ir::CallKind::Instance,
        arg_coercions: Vec::new(),
    };
    let target = plan_call_target(Some(&receiver), "readName", Some(&resolved_instance));
    assert!(matches!(
        target,
        CallTargetPlan::InstanceReceiver {
            rust_name: "read_name",
            ..
        }
    ));
    assert_eq!(
        apply_call_target(
            &target,
            Some(RustExpr::Var("helper".to_string())),
            vec![RustExpr::Literal("1".to_string())],
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("helper".to_string())),
            method: "read_name".to_string(),
            args: vec![RustExpr::Literal("1".to_string())],
        }
    );
    assert_eq!(target.receiver_expr(), Some(&receiver));

    assert!(matches!(
        plan_call_target(Some(&receiver), "readName", None),
        CallTargetPlan::FallbackReceiver {
            rust_name,
            ..
        } if rust_name == "read_name"
    ));
    assert_eq!(
        plan_call_target(Some(&receiver), "readName", None).receiver_expr(),
        Some(&receiver)
    );
    assert_eq!(
        plan_call_target(None, "readName", None).receiver_expr(),
        None
    );

    let super_target = plan_call_target_with_facts(
        Some(&Expr::Super),
        "readName",
        Some(&resolved_instance),
        &FactSet::default(),
        &new_java2rust::types::TypeEnv::default(),
    );
    assert!(matches!(
        super_target,
        CallTargetPlan::SuperBoundary {
            ref rust_name,
            return_type: JavaType::String,
            propagates_result: false,
        } if rust_name == "read_name"
    ));
    assert_eq!(super_target.receiver_expr(), None);
    assert_eq!(
        apply_call_target(&super_target, None, Vec::new()),
        RustExpr::Raw("Default::default()".to_string())
    );

    let hdf5_receiver = Expr::Var("hdf5".to_string());
    let hdf5_facts = new_java2rust::external_packs::hdf5_pack_facts();
    let mut hdf5_types = TypeEnv::new();
    hdf5_types.insert_local(
        "hdf5",
        JavaType::Named {
            name: "HDF5File".to_string(),
            args: Vec::new(),
        },
    );
    hdf5_types.insert_local(
        "library",
        JavaType::Named {
            name: "HDF5Library".to_string(),
            args: Vec::new(),
        },
    );
    let crate_target = plan_call_target_with_facts(
        Some(&hdf5_receiver),
        "writeDoubleArray",
        None,
        &hdf5_facts,
        &hdf5_types,
    );
    assert_eq!(crate_target.receiver_expr(), Some(&hdf5_receiver));
    assert!(matches!(
        crate_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "hdf5_pure_rust" && rust_name == "write_double_array"
    ));
    assert_eq!(
        apply_call_target(
            &crate_target,
            Some(RustExpr::Var("hdf5".to_string())),
            vec![
                RustExpr::Literal("\"/metrics\"".to_string()),
                RustExpr::Var("values".to_string()),
            ],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw(
                "hdf5_pure_rust::write_double_array".to_string()
            )),
            args: vec![
                RustExpr::Var("hdf5".to_string()),
                RustExpr::Literal("\"/metrics\"".to_string()),
                RustExpr::Var("values".to_string()),
            ],
        }
    );
    let hdf5_flush_target = plan_call_target_with_facts(
        Some(&hdf5_receiver),
        "flush",
        None,
        &hdf5_facts,
        &hdf5_types,
    );
    assert_eq!(hdf5_flush_target.receiver_expr(), Some(&hdf5_receiver));
    assert!(matches!(
        hdf5_flush_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "hdf5_pure_rust" && rust_name == "flush"
    ));
    assert_eq!(
        apply_call_target(
            &hdf5_flush_target,
            Some(RustExpr::Var("hdf5".to_string())),
            Vec::new(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("hdf5_pure_rust::flush".to_string())),
            args: vec![RustExpr::Var("hdf5".to_string())],
        }
    );
    let hdf5_library_receiver = Expr::Var("library".to_string());
    let hdf5_load_target = plan_call_target_with_facts(
        Some(&hdf5_library_receiver),
        "load",
        None,
        &hdf5_facts,
        &hdf5_types,
    );
    assert_eq!(
        hdf5_load_target.receiver_expr(),
        Some(&hdf5_library_receiver)
    );
    assert!(matches!(
        hdf5_load_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "hdf5_pure_rust" && rust_name == "load"
    ));
    assert_eq!(
        apply_call_target(
            &hdf5_load_target,
            Some(RustExpr::Var("library".to_string())),
            Vec::new(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("hdf5_pure_rust::load".to_string())),
            args: vec![RustExpr::Var("library".to_string())],
        }
    );
    let mut override_facts = hdf5_facts.clone();
    override_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "writeDoubleArray".to_string(),
            receiver_type: Some("HDF5File".to_string()),
            arg_count: None,
            role: ExternalApiRole::Mutator,
            receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
            provenance: FactProvenance::UserOverride,
        });
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&hdf5_receiver),
            "writeDoubleArray",
            None,
            &override_facts,
            &hdf5_types,
        ),
        CallTargetPlan::FallbackReceiver { rust_name, .. } if rust_name == "write_double_array"
    ));

    let parsed = parse_java_source("class Add {}");
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let new_target = plan_new_target(
        &JavaType::Named {
            name: "Add".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
    );
    assert_eq!(
        apply_new_target(&new_target, Vec::new()),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("Add::new".to_string())),
            args: Vec::new(),
        }
    );
    let mut collection_types = TypeEnv::new();
    collection_types.insert_local("values", list_java_type(JavaType::String));
    collection_types.insert_local(
        "names",
        JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::String],
        },
    );
    collection_types.insert_local(
        "counts",
        JavaType::Named {
            name: "Map".to_string(),
            args: vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int)],
        },
    );
    let values = Expr::Var("values".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&values),
            "add",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::ListPush,
            ..
        }
    ));
    assert_eq!(
        apply_call_target(
            &plan_call_target_with_facts(
                Some(&values),
                "size",
                None,
                &FactSet::default(),
                &collection_types,
            ),
            Some(RustExpr::Var("values".to_string())),
            Vec::new(),
        ),
        RustExpr::Raw("values.len() as i32".to_string())
    );
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&values),
            "isEmpty",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::IsEmpty,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&values),
            "addAll",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::Extend,
            ..
        }
    ));
    let names = Expr::Var("names".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&names),
            "add",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::SetInsert,
            ..
        }
    ));
    let counts = Expr::Var("counts".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "put",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapInsert,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "get",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapGet,
            ..
        }
    ));
    assert!(matches!(
        native_collection_lowered_type(
            &counts,
            NativeCollectionMethod::MapGet,
            &collection_types,
        ),
        Some(LoweredType::OptionalElementBorrow {
            element_shape,
            ..
        }) if element_shape.java_ty == JavaType::Primitive(PrimitiveType::Int)
    ));
    assert!(matches!(
        native_collection_lowered_type(
            &counts,
            NativeCollectionMethod::MapInsert,
            &collection_types,
        ),
        Some(LoweredType::Slot(shape))
            if shape.optional_element_type() == Some(JavaType::Primitive(PrimitiveType::Int))
    ));
    let Some(map_insert_shapes) =
        native_collection_arg_shapes(Some(&counts), "put", 2, &collection_types)
    else {
        panic!("HashMap insert arguments should be shaped from Java map generics");
    };
    assert_eq!(map_insert_shapes[0].java_ty, JavaType::String);
    assert_eq!(map_insert_shapes[0].ownership, OwnershipShape::Owned);
    assert_eq!(
        map_insert_shapes[1].java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
    assert_eq!(map_insert_shapes[1].ownership, OwnershipShape::CopyValue);
    let external_reader = JavaType::Named {
        name: "ExternalReader".to_string(),
        args: Vec::new(),
    };
    let mut erased_map_types = TypeEnv::new();
    erased_map_types.insert_local(
        "readers",
        JavaType::Named {
            name: "Map".to_string(),
            args: vec![JavaType::String, external_reader.clone()],
        },
    );
    let mut erased_facts = FactSet::default();
    erased_facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalReader".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    let readers = Expr::Var("readers".to_string());
    let erased_map_insert_shapes = native_collection_arg_shapes_with_facts(
        Some(&readers),
        "put",
        2,
        &erased_map_types,
        &erased_facts,
    )
    .expect("erased-object HashMap insert arguments should be shaped from Java map generics");
    assert_eq!(erased_map_insert_shapes[1].java_ty, external_reader);
    assert_eq!(
        erased_map_insert_shapes[1].dispatch,
        DispatchShape::ErasedObject
    );
    assert_eq!(
        erased_map_insert_shapes[1].view,
        ViewShape::ErasedObjectView
    );
    let final_product = JavaType::Named {
        name: "ExternalProduct".to_string(),
        args: Vec::new(),
    };
    let mut final_facade_collection_types = TypeEnv::new();
    final_facade_collection_types.insert_local(
        "products",
        JavaType::Named {
            name: "List".to_string(),
            args: vec![final_product.clone()],
        },
    );
    let mut final_facade_facts = FactSet::default();
    final_facade_facts
        .hierarchy_overrides
        .push(HierarchyOverride {
            type_name: "ExternalProduct".to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    final_facade_facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalProduct".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let products = Expr::Var("products".to_string());
    let final_list_push_shapes = native_collection_arg_shapes_with_facts(
        Some(&products),
        "add",
        1,
        &final_facade_collection_types,
        &final_facade_facts,
    )
    .expect("final erased facade Vec push should use the rendered concrete element slot");
    assert_eq!(final_list_push_shapes[0].java_ty, final_product);
    assert_eq!(final_list_push_shapes[0].dispatch, DispatchShape::Concrete);
    assert_eq!(final_list_push_shapes[0].view, ViewShape::Native);
    let Some(map_get_shapes) =
        native_collection_arg_shapes(Some(&counts), "get", 1, &collection_types)
    else {
        panic!("HashMap get key argument should be shaped from Java map generics");
    };
    assert_eq!(map_get_shapes[0].java_ty, JavaType::String);
    assert_eq!(map_get_shapes[0].ownership, OwnershipShape::SharedBorrow);
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "containsKey",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapContainsKey,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "contains_key",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapContainsKey,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "key_set",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapKeys,
            ..
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeCollectionMethod {
                receiver: &counts,
                method: NativeCollectionMethod::MapKeys,
            },
            Some(RustExpr::Var("counts".to_string())),
            Vec::new(),
        ),
        RustExpr::Raw("counts.keys().cloned().collect::<Vec<_>>()".to_string())
    );
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "entrySet",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapIter,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&counts),
            "entry_set",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionMethod {
            method: NativeCollectionMethod::MapIter,
            ..
        }
    ));
    let arrays = Expr::Var("Arrays".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&arrays),
            "asList",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeCollectionStatic {
            method: NativeCollectionStaticMethod::ArraysAsList
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeCollectionStatic {
                method: NativeCollectionStaticMethod::ArraysAsList,
            },
            None,
            vec![
                RustExpr::Literal("\"a\".to_string()".to_string()),
                RustExpr::Literal("\"b\".to_string()".to_string()),
            ],
        ),
        RustExpr::Raw("vec![\"a\".to_string(), \"b\".to_string()]".to_string())
    );
    let string_value = Expr::Var("text".to_string());
    collection_types.insert_local("text", JavaType::String);
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&string_value),
            "length",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringLen,
            ..
        }
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&string_value),
            "contains",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringContains,
            ..
        }
    ));
    let optional_string_value = Expr::Var("maybe_text".to_string());
    collection_types.insert_local("maybe_text", optional_java_type(JavaType::String));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&optional_string_value),
            "endsWith",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringEndsWith,
            ..
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaInstance {
                receiver: &string_value,
                method: NativeJavaInstanceMethod::StringToLowercase,
            },
            Some(RustExpr::Var("text".to_string())),
            Vec::new(),
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("text".to_string())),
            method: "to_lowercase".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaInstance {
                receiver: &string_value,
                method: NativeJavaInstanceMethod::StringEndsWith,
            },
            Some(RustExpr::Var("text".to_string())),
            vec![RustExpr::Raw("suffix.as_str()".to_string())],
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("text".to_string())),
            method: "ends_with".to_string(),
            args: vec![RustExpr::Raw("suffix.as_str()".to_string())],
        }
    );
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaInstance {
                receiver: &string_value,
                method: NativeJavaInstanceMethod::Equals,
            },
            Some(RustExpr::Var("left".to_string())),
            vec![RustExpr::Var("right".to_string())],
        ),
        RustExpr::Binary {
            op: "==".to_string(),
            left: Box::new(RustExpr::Var("left".to_string())),
            right: Box::new(RustExpr::Var("right".to_string())),
        }
    );
    let resolved_equals = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Value".to_string(),
        rust_name: "equals".to_string(),
        kind: new_java2rust::java_ir::CallKind::Instance,
        return_type: JavaType::Primitive(PrimitiveType::Boolean),
        arg_coercions: Vec::new(),
    };
    let value_receiver = Expr::Var("left".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&value_receiver),
            "equals",
            Some(&resolved_equals),
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::InstanceReceiver {
            rust_name: "equals",
            ..
        }
    ));
    let string_owner = Expr::Var("String".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&string_owner),
            "format",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::StringFormat
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaStatic {
                method: NativeJavaStaticMethod::StringFormat,
            },
            None,
            vec![
                RustExpr::Literal("\"%s:%d\".to_string()".to_string()),
                RustExpr::Var("name".to_string()),
                RustExpr::Var("count".to_string()),
            ],
        ),
        RustExpr::Raw("format!(\"{}:{}\", name, count)".to_string())
    );
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaStatic {
                method: NativeJavaStaticMethod::StringFormat,
            },
            None,
            vec![
                RustExpr::Literal("\"signal %3.0f at %6.1f\".to_string()".to_string()),
                RustExpr::Var("strength".to_string()),
                RustExpr::Var("angle".to_string()),
            ],
        ),
        RustExpr::Raw("format!(\"signal {:3.0} at {:6.1}\", strength, angle)".to_string())
    );
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&string_owner),
            "join",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::StringJoin
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaStatic {
                method: NativeJavaStaticMethod::StringJoin,
            },
            None,
            vec![
                RustExpr::Literal("\" and \".to_string()".to_string()),
                RustExpr::Var("missing".to_string()),
            ],
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("missing".to_string())),
            method: "join".to_string(),
            args: vec![RustExpr::Literal("\" and \"".to_string())],
        }
    );
    let objects_owner = Expr::Var("Objects".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&objects_owner),
            "equals",
            None,
            &FactSet::default(),
            &collection_types,
        ),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::ObjectsEquals
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaStatic {
                method: NativeJavaStaticMethod::MathMax,
            },
            None,
            vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string())
            ],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("std::cmp::max".to_string())),
            args: vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string())
            ],
        }
    );
    let list_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "ArrayList".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &FactSet::default(),
        0,
    );
    assert!(matches!(
        list_new_target.native_collection,
        Some(NativeCollectionNewTarget::Empty {
            kind: new_java2rust::stdlib::JavaCollectionKind::List
        })
    ));
    assert_eq!(
        apply_new_target(&list_new_target, Vec::new()),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("Vec::new".to_string())),
            args: Vec::new(),
        }
    );
    let capacity_arg = Expr::Call {
        receiver: Some(Box::new(Expr::Var("names".to_string()))),
        name: "size".to_string(),
        args: Vec::new(),
        resolved: None,
    };
    let mut capacity_types = new_java2rust::types::TypeEnv::new();
    capacity_types.insert_local(
        "names",
        JavaType::Named {
            name: "Optional".to_string(),
            args: vec![JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::String],
            }],
        },
    );
    let list_capacity_target = new_java2rust::call_plan::plan_new_target_for_args_with_facts(
        &JavaType::Named {
            name: "ArrayList".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &FactSet::default(),
        &[capacity_arg],
        &capacity_types,
    );
    assert!(matches!(
        list_capacity_target.native_collection,
        Some(NativeCollectionNewTarget::WithCapacity {
            kind: new_java2rust::stdlib::JavaCollectionKind::List
        })
    ));
    assert_eq!(
        apply_new_target(
            &list_capacity_target,
            vec![RustExpr::Raw("names.as_ref().unwrap().len()".to_string())]
        ),
        RustExpr::Raw("Vec::with_capacity(names.as_ref().unwrap().len() as usize)".to_string())
    );

    let mut external_constructor_facts = FactSet::default();
    external_constructor_facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            receiver_type: Some("ExternalConfig".to_string()),
            method_name: "new".to_string(),
            parameter_index: 1,
            parameter_type: Some(JavaType::Named {
                name: "Function".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::MaybeNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let constructor_shapes = external_constructor_arg_shapes(
        &JavaType::Named {
            name: "ExternalConfig".to_string(),
            args: Vec::new(),
        },
        "new",
        3,
        &external_constructor_facts,
    )
    .expect("one constructor argument fact should shape all constructor slots");
    assert!(constructor_shapes[0].optional_element_type().is_none());
    assert_eq!(
        constructor_shapes[1].optional_element_type(),
        Some(JavaType::Named {
            name: "Function".to_string(),
            args: Vec::new(),
        })
    );
    assert!(constructor_shapes[2].optional_element_type().is_none());

    let map_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "HashMap".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &FactSet::default(),
        0,
    );
    assert!(matches!(
        map_new_target.native_collection,
        Some(NativeCollectionNewTarget::Empty {
            kind: new_java2rust::stdlib::JavaCollectionKind::Map
        })
    ));
    assert_eq!(
        apply_new_target(&map_new_target, Vec::new()),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("std::collections::HashMap::new".to_string())),
            args: Vec::new(),
        }
    );
    let interval_list_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "IntervalList".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &new_java2rust::external_packs::htsjdk_samtools_util_pack_facts(),
        1,
    );
    assert_eq!(interval_list_target.native_collection, None);
    assert_eq!(interval_list_target.rust_name, "new");
    assert_eq!(
        apply_new_target_with_facts(
            &interval_list_target,
            vec![RustExpr::Var("dictionary".to_string())],
            &new_java2rust::external_packs::htsjdk_samtools_util_pack_facts()
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("IntervalList::new".to_string())),
            args: vec![RustExpr::Var("dictionary".to_string())],
        }
    );
    let hdf5_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "HDF5File".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &hdf5_facts,
        1,
    );
    assert_eq!(
        hdf5_new_target.external_crate.as_ref().map(|target| (
            target.crate_name.as_str(),
            target.type_name.as_str(),
            target.rust_name.as_str()
        )),
        Some(("hdf5_pure_rust", "HDF5File", "new"))
    );
    assert_eq!(
        apply_new_target_with_facts(
            &hdf5_new_target,
            vec![RustExpr::Var("file".to_string())],
            &hdf5_facts
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("hdf5_pure_rust::HDF5File::new".to_string())),
            args: vec![RustExpr::Var("file".to_string())],
        }
    );
    let mut constructor_mapping_override_facts = hdf5_facts.clone();
    constructor_mapping_override_facts
        .external_crate_constructor_mapping_facts
        .push(ExternalCrateConstructorMappingFact {
            type_name: "HDF5File".to_string(),
            arg_count: 1,
            rust_type_name: Some("FileHandle".to_string()),
            rust_name: "open_read_only".to_string(),
            provenance: FactProvenance::UserOverride,
        });
    let hdf5_mapping_override_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "HDF5File".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &constructor_mapping_override_facts,
        1,
    );
    assert_eq!(
        hdf5_mapping_override_new_target
            .external_crate
            .as_ref()
            .map(|target| (
                target.crate_name.as_str(),
                target.type_name.as_str(),
                target.rust_name.as_str()
            )),
        Some(("hdf5_pure_rust", "FileHandle", "open_read_only")),
        "user crate-constructor mapping facts should override built-in HDF5 facade symbols"
    );
    assert_eq!(
        apply_new_target_with_facts(
            &hdf5_mapping_override_new_target,
            vec![RustExpr::Var("file".to_string())],
            &constructor_mapping_override_facts
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw(
                "hdf5_pure_rust::FileHandle::open_read_only".to_string()
            )),
            args: vec![RustExpr::Var("file".to_string())],
        }
    );
    let mut constructor_override_facts = hdf5_facts.clone();
    constructor_override_facts.external_constructor_facts.push(
        new_java2rust::facts::ExternalConstructorFact {
            type_name: "HDF5File".to_string(),
            arg_types: Vec::new(),
            provenance: FactProvenance::UserOverride,
        },
    );
    let hdf5_override_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "HDF5File".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &constructor_override_facts,
        1,
    );
    assert_eq!(hdf5_override_new_target.external_crate, None);

    let sqlite_receiver = Expr::Var("config".to_string());
    let sqlite_facts = new_java2rust::external_packs::sqlite_pack_facts();
    let mut sqlite_types = TypeEnv::new();
    sqlite_types.insert_local(
        "config",
        JavaType::Named {
            name: "SQLiteConfig".to_string(),
            args: Vec::new(),
        },
    );
    sqlite_types.insert_local(
        "statement",
        JavaType::Named {
            name: "SQLiteStatement".to_string(),
            args: Vec::new(),
        },
    );
    sqlite_types.insert_local(
        "connection",
        JavaType::Named {
            name: "SQLiteConnection".to_string(),
            args: Vec::new(),
        },
    );
    sqlite_types.insert_local(
        "results",
        JavaType::Named {
            name: "SQLiteResultSet".to_string(),
            args: Vec::new(),
        },
    );
    let sqlite_connect_target = plan_call_target_with_facts(
        Some(&sqlite_receiver),
        "createConnection",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert_eq!(
        sqlite_connect_target.receiver_expr(),
        Some(&sqlite_receiver)
    );
    assert!(matches!(
        sqlite_connect_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "create_connection"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_connect_target,
            Some(RustExpr::Var("config".to_string())),
            vec![RustExpr::Var("path".to_string())],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::create_connection".to_string())),
            args: vec![
                RustExpr::Var("config".to_string()),
                RustExpr::Var("path".to_string()),
            ],
        }
    );
    let sqlite_statement_receiver = Expr::Var("statement".to_string());
    let sqlite_query_target = plan_call_target_with_facts(
        Some(&sqlite_statement_receiver),
        "executeQuery",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_query_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "execute_query"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_query_target,
            Some(RustExpr::Var("statement".to_string())),
            Vec::new(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::execute_query".to_string())),
            args: vec![RustExpr::Var("statement".to_string())],
        }
    );
    let sqlite_clear_parameters_target = plan_call_target_with_facts(
        Some(&sqlite_statement_receiver),
        "clearParameters",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_clear_parameters_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "clear_parameters"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_clear_parameters_target,
            Some(RustExpr::Var("statement".to_string())),
            Vec::new(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::clear_parameters".to_string())),
            args: vec![RustExpr::Var("statement".to_string())],
        }
    );
    let sqlite_statement_close_target = plan_call_target_with_facts(
        Some(&sqlite_statement_receiver),
        "close",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_statement_close_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "close"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_statement_close_target,
            Some(RustExpr::Var("statement".to_string())),
            Vec::new(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::close".to_string())),
            args: vec![RustExpr::Var("statement".to_string())],
        }
    );
    let sqlite_connection_receiver = Expr::Var("connection".to_string());
    let sqlite_set_auto_commit_target = plan_call_target_with_facts(
        Some(&sqlite_connection_receiver),
        "setAutoCommit",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_set_auto_commit_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "set_auto_commit"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_set_auto_commit_target,
            Some(RustExpr::Var("connection".to_string())),
            vec![RustExpr::Literal("false".to_string())],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::set_auto_commit".to_string())),
            args: vec![
                RustExpr::Var("connection".to_string()),
                RustExpr::Literal("false".to_string()),
            ],
        }
    );
    let sqlite_result_receiver = Expr::Var("results".to_string());
    let sqlite_get_string_target = plan_call_target_with_facts(
        Some(&sqlite_result_receiver),
        "getString",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_get_string_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "get_string"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_get_string_target,
            Some(RustExpr::Var("results".to_string())),
            vec![RustExpr::Literal("\"sample\"".to_string())],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::get_string".to_string())),
            args: vec![
                RustExpr::Var("results".to_string()),
                RustExpr::Literal("\"sample\"".to_string()),
            ],
        }
    );
    let sqlite_get_long_target = plan_call_target_with_facts(
        Some(&sqlite_result_receiver),
        "getLong",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_get_long_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "get_long"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_get_long_target,
            Some(RustExpr::Var("results".to_string())),
            vec![RustExpr::Literal("1".to_string())],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::get_long".to_string())),
            args: vec![
                RustExpr::Var("results".to_string()),
                RustExpr::Literal("1".to_string()),
            ],
        }
    );
    let sqlite_get_boolean_target = plan_call_target_with_facts(
        Some(&sqlite_result_receiver),
        "getBoolean",
        None,
        &sqlite_facts,
        &sqlite_types,
    );
    assert!(matches!(
        sqlite_get_boolean_target,
        CallTargetPlan::ExternalCrateReceiver {
            ref crate_name,
            ref rust_name,
            ..
        } if crate_name == "sqlite" && rust_name == "get_boolean"
    ));
    assert_eq!(
        apply_call_target(
            &sqlite_get_boolean_target,
            Some(RustExpr::Var("results".to_string())),
            vec![RustExpr::Literal("\"active\"".to_string())],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::get_boolean".to_string())),
            args: vec![
                RustExpr::Var("results".to_string()),
                RustExpr::Literal("\"active\"".to_string()),
            ],
        }
    );
    let sqlite_new_target = plan_new_target_with_facts(
        &JavaType::Named {
            name: "SQLiteConfig".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
        &sqlite_facts,
        0,
    );
    assert_eq!(
        sqlite_new_target.external_crate.as_ref().map(|target| (
            target.crate_name.as_str(),
            target.type_name.as_str(),
            target.rust_name.as_str()
        )),
        Some(("sqlite", "SQLiteConfig", "new"))
    );
    assert_eq!(
        apply_new_target_with_facts(&sqlite_new_target, Vec::new(), &sqlite_facts),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("sqlite::SQLiteConfig::new".to_string())),
            args: Vec::new(),
        }
    );
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let external_new_target = plan_new_target(&external_ids, None, &representations);
    assert_eq!(
        apply_new_target_with_facts(&external_new_target, Vec::new(), &facts),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("Vec<ExternalId>::new".to_string())),
            args: Vec::new(),
        }
    );
    let object_new_target = plan_new_target(
        &JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        },
        None,
        &representations,
    );
    assert_eq!(
        apply_new_target_with_facts(&object_new_target, Vec::new(), &FactSet::default()),
        RustExpr::Raw("Box::new(()) as Box<dyn std::any::Any>".to_string())
    );

    let parsed = parse_java_source(
        r#"
class Uses {
    Uses(String name) {}
    String echo(String value) { return value; }
    String call(String input) { return echo(input); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let echo = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "echo" => Some(method),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let call = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Uses".to_string(),
        rust_name: echo.rust_name.clone().unwrap(),
        return_type: JavaType::String,
        kind: new_java2rust::java_ir::CallKind::Instance,
        arg_coercions: Vec::new(),
    };
    let constructor = new_java2rust::java_ir::ResolvedConstructor {
        declaring_type: "Uses".to_string(),
        rust_name: ctor.rust_name.clone().unwrap(),
        constructed_type: JavaType::Named {
            name: "Uses".to_string(),
            args: Vec::new(),
        },
        params: ctor.params.clone(),
        arg_coercions: Vec::new(),
    };

    let method_arg_shapes = signatures
        .method_param_plans(ty, echo)
        .unwrap()
        .iter()
        .map(|param| param.shape.clone())
        .collect::<Vec<_>>();
    let constructor_arg_shapes = signatures
        .constructor_param_plans(ty, ctor)
        .unwrap()
        .iter()
        .map(|param| param.shape.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        resolved_call_arg_shapes(&call, 1, &signatures),
        method_arg_shapes
    );
    assert_eq!(
        plan_call_arguments(Some(&call), 1, &signatures),
        ArgumentLoweringPlan::Shaped(method_arg_shapes)
    );
    assert_eq!(
        plan_call_arguments(None, 2, &signatures),
        ArgumentLoweringPlan::Unshaped { arg_count: 2 }
    );
    let mut sqlite_types = TypeEnv::new();
    sqlite_types.insert_local(
        "statement",
        JavaType::Named {
            name: "SQLiteStatement".to_string(),
            args: Vec::new(),
        },
    );
    let sqlite_facts = new_java2rust::external_packs::sqlite_pack_facts();
    let sqlite_bind_args = plan_call_arguments_with_facts(
        Some(&Expr::Var("statement".to_string())),
        "bindString",
        None,
        2,
        &signatures,
        &sqlite_facts,
        &sqlite_types,
    );
    let ArgumentLoweringPlan::Shaped(sqlite_bind_shapes) = sqlite_bind_args else {
        panic!("SQLite bindString arguments should be shaped by external argument-effect facts");
    };
    assert_eq!(
        sqlite_bind_shapes[0].java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
    assert_eq!(sqlite_bind_shapes[0].ownership, OwnershipShape::CopyValue);
    assert_eq!(sqlite_bind_shapes[1].java_ty, JavaType::String);
    assert_eq!(
        sqlite_bind_shapes[1].ownership,
        OwnershipShape::SharedBorrow
    );
    assert_eq!(sqlite_bind_shapes[1].view, ViewShape::Str);
    assert_eq!(
        plan_call_arguments_with_facts(
            Some(&Expr::Var("statement".to_string())),
            "prepareStatement",
            None,
            1,
            &signatures,
            &sqlite_facts,
            &sqlite_types,
        ),
        ArgumentLoweringPlan::Unshaped { arg_count: 1 },
        "external argument facts should shape calls only when the whole argument vector is modeled"
    );

    assert_eq!(
        resolved_constructor_arg_shapes(&constructor, 1, &signatures),
        constructor_arg_shapes
    );
    assert_eq!(
        plan_constructor_arguments(Some(&constructor), 1, &signatures),
        ArgumentLoweringPlan::Shaped(constructor_arg_shapes)
    );
    assert_eq!(
        plan_constructor_arguments(None, 0, &signatures),
        ArgumentLoweringPlan::Unshaped { arg_count: 0 }
    );

    let lower_source = include_str!("../src/lower.rs");
    let call_plan_source = include_str!("../src/call_plan.rs");
    assert!(call_plan_source.contains("pub enum ArgumentLoweringPlan"));
    assert!(call_plan_source.contains("pub fn plan_call_arguments"));
    assert!(call_plan_source.contains("pub fn plan_call_arguments_with_facts"));
    assert!(call_plan_source.contains("pub fn native_collection_arg_shapes"));
    assert!(call_plan_source.contains("pub fn plan_constructor_arguments"));
    assert!(call_plan_source.contains("pub fn receiver_expr"));
    assert!(!lower_source.contains("resolved_call_arg_shapes"));
    assert!(!lower_source.contains("resolved_constructor_arg_shapes"));
    assert!(!lower_source.contains("lower_args_for_shapes"));
    assert!(!lower_source.contains("CallTargetPlan::InstanceReceiver"));
    assert!(!lower_source.contains("CallTargetPlan::FallbackReceiver"));
    assert!(lower_source.contains("lower_args_with_plan"));
    assert!(lower_source.contains(".receiver_expr()"));
    assert!(lower_source.contains("fn lower_enhanced_for_stmt"));
    assert!(lower_source.contains("loop_env.insert_local_shape"));
}

#[test]
fn static_external_owner_argument_shapes_use_external_effect_facts() {
    let signatures = SignaturePlan::default();
    let mut facts = FactSet::default();
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "readValue".to_string(),
            receiver_type: Some("external.lib.StaticUtility".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });

    let args = vec![Expr::Var("path".to_string())];

    let mut imported_type_owner = TypeEnv::new();
    imported_type_owner.insert_static_type_alias("StaticUtility", "external.lib.StaticUtility");
    let alias_plan = plan_call_arguments_with_facts_and_args(
        Some(&Expr::Var("StaticUtility".to_string())),
        "readValue",
        None,
        &args,
        &signatures,
        &facts,
        &imported_type_owner,
    );
    let ArgumentLoweringPlan::Shaped(alias_shapes) = alias_plan else {
        panic!("imported static owner calls should use external argument facts");
    };
    assert_eq!(alias_shapes[0].java_ty, JavaType::String);
    assert_eq!(alias_shapes[0].ownership, OwnershipShape::SharedBorrow);
    assert_eq!(alias_shapes[0].view, ViewShape::Str);

    let mut static_member_import = TypeEnv::new();
    static_member_import.insert_static_member_import("readValue", "external.lib.StaticUtility");
    let member_plan = plan_call_arguments_with_facts_and_args(
        None,
        "readValue",
        None,
        &args,
        &signatures,
        &facts,
        &static_member_import,
    );
    assert_eq!(
        member_plan,
        ArgumentLoweringPlan::Shaped(alias_shapes.clone()),
        "explicit static member imports should recover the owner for argument facts"
    );

    let qualified_receiver = Expr::FieldAccess(
        Box::new(Expr::FieldAccess(
            Box::new(Expr::Var("external".to_string())),
            "lib".to_string(),
        )),
        "StaticUtility".to_string(),
    );
    let qualified_plan = plan_call_arguments_with_facts_and_args(
        Some(&qualified_receiver),
        "readValue",
        None,
        &args,
        &signatures,
        &facts,
        &TypeEnv::new(),
    );
    assert_eq!(
        qualified_plan,
        ArgumentLoweringPlan::Shaped(alias_shapes),
        "package-qualified static owner calls should recover the Java owner for argument facts"
    );
}

#[test]
fn static_import_lowering_borrows_arguments_from_external_effect_facts() {
    let mut facts = FactSet::default();
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "readValue".to_string(),
            receiver_type: Some("external.lib.StaticUtility".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });
    let translation = translate_source_with_facts(
        r#"
import static external.lib.StaticUtility.readValue;

class StaticImportBorrowedArg {
    Object read(String path) {
        return readValue(path);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("StaticUtility::read_value(path.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_import_support_uses_external_return_facts_without_instance_duplicate() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readValue".to_string(),
        receiver_type: Some("external.lib.StaticUtility".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "readValue".to_string(),
            receiver_type: Some("external.lib.StaticUtility".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });

    let translation = translate_source_with_facts(
        r#"
import static external.lib.StaticUtility.readValue;

class StaticImportTypedReturn {
    String read(String path) {
        return readValue(path);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("StaticUtility::read_value(path.as_str())"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn read_value(_arg0: &str) -> String"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn read_value<R: Default"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn read_value<T0>(&self"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_external_facade_uses_nullable_argument_fact_without_call_site_type_hack() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "read".to_string(),
        receiver_type: Some("external.lib.ReaderFactory".to_string()),
        arg_count: Some(6),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "read".to_string(),
            receiver_type: Some("external.lib.ReaderFactory".to_string()),
            parameter_index: 1,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::MaybeNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::UserOverride,
        });

    let translation = translate_source_with_facts(
        r#"
import external.lib.ReaderFactory;

class NullableExternalArgument {
    String read(String path, Object codec, Object wrapper) {
        return ReaderFactory.read(path, null, codec, false, wrapper, wrapper);
    }
}
"#,
        &facts,
    );

    assert!(
        translation.rust.contains("ReaderFactory::read(path, None"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(", None, codec, false"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("None::<"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub fn read<T0, T2, T3, T4, T5>(_arg0: T0, _arg1: Option<String>, _arg2: T2, _arg3: T3, _arg4: T4, _arg5: T5) -> String"
        ),
        "{}",
        translation.rust
    );
}

#[test]
fn external_static_validation_facade_uses_side_effect_fact_return() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "validate".to_string(),
        receiver_type: Some("org.example.validation.Utils".to_string()),
        arg_count: Some(2),
        role: ExternalApiRole::SideEffect,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "validate".to_string(),
            receiver_type: Some("org.example.validation.Utils".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::Primitive(PrimitiveType::Boolean)),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
import org.example.validation.Utils;

class ExternalValidationUse {
    void check(boolean ok, Object message) {
        Utils.validate(ok, message);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("Utils::validate(ok, message);"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn validate<T1>(_arg0: bool, _arg1: T1) -> () { () }"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn validate<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_platform_files_write_string_facade_uses_known_path_return() {
    let translation = translate_source(
        r#"
import java.nio.file.Files;
import java.nio.file.Path;

class JavaPlatformFileWrite {
    void write(Path path, String contents) {
        Files.writeString(path, contents);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("Files::write_string(path, contents);"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn write_string<T0, T1>(_arg0: T0, _arg1: T1) -> Path"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn write_string<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn branch_scoped_same_name_locals_keep_declared_type_specific_shapes() {
    let translation = translate_source(
        r#"
class SameNameBranchLocalTypes {
    String read() {
        return "header";
    }

    int count() {
        return 1;
    }

    void sinkString(String value) {
    }

    void sinkCount(int value) {
    }

    void run(boolean flag) {
        if (flag) {
            String header = read();
            sinkString(header);
        } else {
            int header = count();
            sinkCount(header);
        }
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("let header: String = self.read();"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("let header: i32 = self.count();"),
        "{}",
        translation.rust
    );
}

#[test]
fn static_import_support_reuses_fqn_return_facts_for_simple_owner_facade() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "isGenomicsDBArray".to_string(),
        receiver_type: Some("org.genomicsdb.GenomicsDBUtils".to_string()),
        arg_count: Some(2),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Boolean)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "isGenomicsDBArray".to_string(),
            receiver_type: Some("org.genomicsdb.GenomicsDBUtils".to_string()),
            parameter_index: 0,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            method_name: "isGenomicsDBArray".to_string(),
            receiver_type: Some("org.genomicsdb.GenomicsDBUtils".to_string()),
            parameter_index: 1,
            parameter_type: Some(JavaType::String),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Borrowed,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
import static org.genomicsdb.GenomicsDBUtils.isGenomicsDBArray;

class StaticImportFqnFactReturn {
    boolean exists(String workspace, String arrayName) {
        return isGenomicsDBArray(workspace, arrayName);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains(
            "GenomicsDBUtils::is_genomics_d_b_array(workspace.as_str(), array_name.as_str())"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn is_genomics_d_b_array(_arg0: &str, _arg1: &str) -> bool"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("pub fn is_genomics_d_b_array<R: Default"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_constructor_support_uses_arity_names_for_overloaded_facades() {
    let mut facts = FactSet::default();
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "ExternalBox".to_string(),
            arg_types: vec![JavaType::String],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "ExternalBox".to_string(),
            arg_types: vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int)],
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
class ExternalConstructorCaller {
    ExternalBox one(String label) {
        return new ExternalBox(label);
    }

    ExternalBox two(String label, int count) {
        return new ExternalBox(label, count);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("ExternalBox::new_1(label)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("ExternalBox::new_2(label, count)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn new_1<T0>(_arg0: T0) -> Self"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn new_2<T0, T1>(_arg0: T0, _arg1: T1) -> Self"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("ExternalBox::new(label"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_method_support_uses_arity_names_for_overloaded_facades() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "query".to_string(),
        receiver_type: Some("ExternalCursor".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalIterator".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "query".to_string(),
        receiver_type: Some("ExternalCursor".to_string()),
        arg_count: Some(3),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalRangeIterator".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(
        r#"
class ExternalMethodCaller {
    ExternalIterator single(ExternalCursor cursor, String contig) {
        return cursor.query(contig);
    }

    ExternalRangeIterator range(ExternalCursor cursor, String contig, int start, int end) {
        return cursor.query(contig, start, end);
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("cursor.query_1(contig)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("cursor.query_3(contig, start, end)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn query_1<T0>(&self, _arg0: T0) -> ExternalIterator"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub fn query_3<T0, T1, T2>(&self, _arg0: T0, _arg1: T1, _arg2: T2) -> ExternalRangeIterator"
        ),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub fn query<T0, T1, T2>(&self"),
        "{}",
        translation.rust
    );
}

#[test]
fn domain_collection_facade_constructor_is_not_native_collection_constructor() {
    let facts = new_java2rust::external_packs::htsjdk_samtools_util_pack_facts();
    let translation = translate_source_with_facts(
        r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.util.IntervalList;

class DomainCollectionConstructorUse {
    IntervalList make(SAMSequenceDictionary dictionary) {
        IntervalList intervals = new IntervalList(dictionary);
        intervals.add(new Interval("chr1", 1, 2));
        intervals.write("out.interval_list");
        return intervals;
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("IntervalList::new(dictionary)"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("intervals.add("),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("intervals.write("),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("intervals.push("),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn new<T0>(_arg0: T0) -> Self"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn add<T0>(&self, _arg0: T0)"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn write<T0>(&self, _arg0: T0)"),
        "{}",
        translation.rust
    );
}

#[test]
fn pack_enabled_final_domain_facade_local_stays_concrete_not_erased() {
    let facts = new_java2rust::external_packs::htsjdk_samtools_util_pack_facts();
    let translation = translate_source_with_facts(
        r#"
import htsjdk.samtools.SAMSequenceDictionary;
import htsjdk.samtools.util.IntervalList;

class PackEnabledDomainFacadeUse {
    IntervalList make(SAMSequenceDictionary dictionary) {
        IntervalList intervals = new IntervalList(dictionary);
        intervals.add(new Interval("chr1", 1, 2));
        return intervals;
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("let mut intervals: IntervalList = IntervalList::new(dictionary);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("let mut intervals: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("intervals.add("),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains(
            "Box::new(Interval::new_3(\"chr1\".to_string(), 1, 2)) as Box<dyn std::any::Any>"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("intervals.add(Interval::new_3(\"chr1\".to_string(), 1, 2));"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_enum_constant_facades_do_not_emit_recursive_storage_fields() {
    let facts = new_java2rust::external_packs::htsjdk_samtools_util_pack_facts();
    let translation = translate_source_with_facts(
        r#"
import htsjdk.samtools.SamPairUtil.PairOrientation;

class ExternalEnumConstantUse {
    PairOrientation value() {
        return PairOrientation.FR;
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct PairOrientation;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub struct PairOrientation {"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub f_r: PairOrientation"),
        "{}",
        translation.rust
    );
}

#[test]
fn stream_lambda_constructor_rendering_uses_external_constructor_arity_names() {
    let mut facts = FactSet::default();
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "SimpleInterval".to_string(),
            arg_types: vec![JavaType::Named {
                name: "Locatable".to_string(),
                args: Vec::new(),
            }],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "SimpleInterval".to_string(),
            arg_types: vec![
                JavaType::String,
                JavaType::Primitive(PrimitiveType::Int),
                JavaType::Primitive(PrimitiveType::Int),
            ],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let env = ShapeEnv::new();
    let ctx = StreamRenderContext::new_with_facts(&env, &facts);

    let rendered = emit_stream_lambda_body_with_context(
        &parse_expr("new SimpleInterval(contig, start, end)"),
        "partition",
        StreamLambdaValue::BorrowedItem,
        ctx,
    );

    assert_eq!(
        rendered,
        Some("SimpleInterval::new_3(contig, start, end)".to_string())
    );
}

#[test]
fn external_opaque_facades_support_equality_and_display_fallbacks() {
    let mut facts = FactSet::default();
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "ExternalToken".to_string(),
            arg_types: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
class ExternalFacadeDisplayEquality {
    boolean same(ExternalToken left, ExternalToken right) {
        return left == right;
    }

    String label() {
        return new ExternalToken().toString();
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("impl PartialEq for ExternalToken"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("impl Eq for ExternalToken"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("impl std::hash::Hash for ExternalToken"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("impl std::fmt::Display for ExternalToken"),
        "{}",
        translation.rust
    );
}

#[test]
fn call_plan_recovers_normalized_static_utility_owner_receivers() {
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&Expr::Var("i_o_utils".to_string())),
            "getPath",
            None,
            &FactSet::default(),
            &new_java2rust::types::TypeEnv::default(),
        ),
        CallTargetPlan::StaticOwner {
            owner,
            rust_name
        } if owner == "IOUtils" && rust_name == "get_path"
    ));

    let mut shadowed_types = TypeEnv::new();
    shadowed_types.insert_local("i_o_utils", JavaType::Unknown("LocalIOUtils".to_string()));
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&Expr::Var("i_o_utils".to_string())),
            "getPath",
            None,
            &FactSet::default(),
            &shadowed_types,
        ),
        CallTargetPlan::FallbackReceiver { .. }
    ));

    let mut imported_types = TypeEnv::new();
    imported_types.insert_static_type_alias("external_utility", "external.lib.ExternalUtility");
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&Expr::Var("external_utility".to_string())),
            "readValue",
            None,
            &FactSet::default(),
            &imported_types,
        ),
        CallTargetPlan::StaticOwner {
            owner,
            rust_name
        } if owner == "ExternalUtility" && rust_name == "read_value"
    ));

    imported_types.insert_local(
        "external_utility",
        JavaType::Unknown("LocalExternalUtility".to_string()),
    );
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&Expr::Var("external_utility".to_string())),
            "readValue",
            None,
            &FactSet::default(),
            &imported_types,
        ),
        CallTargetPlan::FallbackReceiver { .. }
    ));

    let mut static_import_types = TypeEnv::new();
    static_import_types.insert_static_member_import("readValue", "external.lib.StaticUtility");
    assert!(matches!(
        plan_call_target_with_facts(
            None,
            "readValue",
            None,
            &FactSet::default(),
            &static_import_types,
        ),
        CallTargetPlan::StaticOwner {
            owner,
            rust_name
        } if owner == "StaticUtility" && rust_name == "read_value"
    ));
    assert!(matches!(
        plan_call_target_with_facts(
            None,
            "localValue",
            None,
            &FactSet::default(),
            &static_import_types,
        ),
        CallTargetPlan::FallbackFree { rust_name } if rust_name == "local_value"
    ));
}

#[test]
fn java_string_join_static_call_renders_collection_join() {
    let string_owner = Expr::Var("String".to_string());
    assert!(matches!(
        plan_call_target_with_facts(
            Some(&string_owner),
            "join",
            None,
            &FactSet::default(),
            &TypeEnv::new(),
        ),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::StringJoin
        }
    ));
    assert_eq!(
        apply_call_target(
            &CallTargetPlan::NativeJavaStatic {
                method: NativeJavaStaticMethod::StringJoin,
            },
            None,
            vec![
                RustExpr::Literal("\" and \".to_string()".to_string()),
                RustExpr::Var("missing".to_string()),
            ],
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("missing".to_string())),
            method: "join".to_string(),
            args: vec![RustExpr::Literal("\" and \"".to_string())],
        }
    );
}

#[test]
fn unresolved_external_setter_arguments_infer_slots_from_argument_types() {
    let mut setter_types = TypeEnv::new();
    setter_types.insert_local(
        "builder",
        JavaType::Named {
            name: "ExternalBuilder".to_string(),
            args: Vec::new(),
        },
    );
    setter_types.insert_local("name", JavaType::String);
    let setter_args = vec![Expr::Var("name".to_string())];
    let signatures = SignaturePlan::default();
    let setter_plan = plan_call_arguments_with_facts_and_args(
        Some(&Expr::Var("builder".to_string())),
        "set_array_name",
        None,
        &setter_args,
        &signatures,
        &FactSet::default(),
        &setter_types,
    );
    let ArgumentLoweringPlan::Shaped(setter_shapes) = setter_plan else {
        panic!("unresolved external setter calls should infer target slots from argument types");
    };
    assert_eq!(setter_shapes.len(), 1);
    assert_eq!(setter_shapes[0].java_ty, JavaType::String);
    assert_eq!(setter_shapes[0].ownership, OwnershipShape::Owned);
    assert_eq!(
        plan_call_arguments_with_facts(
            Some(&Expr::Var("builder".to_string())),
            "set_array_name",
            None,
            1,
            &signatures,
            &FactSet::default(),
            &setter_types,
        ),
        ArgumentLoweringPlan::Unshaped { arg_count: 1 },
        "arity-only planning should not guess setter argument slots without argument types"
    );
}

#[test]
fn crate_mapped_external_method_calls_render_through_facade_target() {
    let translation = translate_source(
        r#"
class Hdf5FacadeCall {
    void write(HDF5File hdf5, double[] values) {
        hdf5.writeDoubleArray("/metrics", values);
    }

    void load(HDF5Library library) {
        library.load();
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("hdf5_pure_rust::write_double_array"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("hdf5_pure_rust::load"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("hdf5.write_double_array(\"/metrics\""),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("library.load()"),
        "{}",
        translation.rust
    );
}

#[test]
fn sqlite_crate_mapped_external_method_calls_render_through_facade_target() {
    let translation = translate_source(
        r#"
class SqliteFacadeCall {
    Object connect(SQLiteConfig config, String path) {
        SQLiteConnection connection = config.createConnection(path);
        SQLiteStatement statement = connection.prepareStatement("select name from samples");
        statement.bindString(1, path);
        SQLiteResultSet results = statement.executeQuery();
        if (results.next()) {
            results.getLong(1);
            results.getBoolean("active");
            return results.getString("name");
        }
        statement.clearParameters();
        statement.close();
        results.close();
        return null;
    }
}
"#,
    );

    assert!(
        translation.rust.contains("sqlite::create_connection"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::prepare_statement"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::bind_string"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::execute_query"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::clear_parameters"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::get_string"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::get_long"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("sqlite::get_boolean"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("config.create_connection(path)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("statement.execute_query()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("statement.clear_parameters()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("results.get_string(\"name\")"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("results.get_long(1)"),
        "{}",
        translation.rust
    );
}

#[test]
fn crate_mapped_external_constructors_render_through_facade_target() {
    let translation = translate_source(
        r#"
class Hdf5FacadeConstructor {
    HDF5File open(File file) {
        return new HDF5File(file);
    }
}
"#,
    );

    assert!(
        translation.rust.contains("hdf5_pure_rust::HDF5File::new"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("return HDF5File::new(file)"),
        "{}",
        translation.rust
    );
}

#[test]
fn sqlite_crate_mapped_external_constructors_render_through_facade_target() {
    let translation = translate_source(
        r#"
class SqliteFacadeConstructor {
    SQLiteConfig configure() {
        return new SQLiteConfig();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("sqlite::SQLiteConfig::new"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("return SQLiteConfig::new()"),
        "{}",
        translation.rust
    );
}

#[test]
fn parsed_crate_mapping_facade_names_are_facts_not_java_snake_case() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.io Reader rust-crate-mapping mapped target=dep_io
        external-pack-api dep.io Reader readNext mapped getter read-only definitely-non-null return=Record rust=fetch_record
        external-pack-constructor dep.io Reader 0 mapped rust-type=ReaderHandle rust=open_reader
        "#,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "manifest parse should be clean: {:?}",
        parsed.diagnostics
    );

    let translation = translate_source_with_facts(
        r#"
class ParsedFacadeNames {
    Object load(Reader reader) {
        return reader.readNext();
    }

    Reader open() {
        return new Reader();
    }
}
"#,
        &parsed.facts,
    );

    assert!(
        translation.rust.contains("dep_io::fetch_record(reader)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("dep_io::read_next"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("dep_io::ReaderHandle::open_reader"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("dep_io::Reader::new"),
        "{}",
        translation.rust
    );
}

#[test]
fn call_plan_owns_trivial_accessor_call_selection_before_lowering() {
    let receiver = Expr::Var("other".to_string());
    let value = Expr::Literal(Literal::Int(7));
    let resolved_getter = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Bean".to_string(),
        rust_name: "get_value".to_string(),
        return_type: JavaType::Primitive(PrimitiveType::Int),
        kind: new_java2rust::java_ir::CallKind::Instance,
        arg_coercions: Vec::new(),
    };
    let resolved_setter = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Bean".to_string(),
        rust_name: "set_value".to_string(),
        return_type: JavaType::Void,
        kind: new_java2rust::java_ir::CallKind::Instance,
        arg_coercions: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts.trivial_accessor_facts.push(TrivialAccessorFact {
        method: FactMethodKey {
            type_name: "Bean".to_string(),
            method_name: "getValue".to_string(),
        },
        kind: TrivialAccessorKind::Getter {
            field: FactFieldKey {
                type_name: "Bean".to_string(),
                field_name: "value".to_string(),
            },
        },
        inline_policy: AccessorInlinePolicy::InlineCandidate,
        provenance: FactProvenance::ParsedSource,
    });
    facts.trivial_accessor_facts.push(TrivialAccessorFact {
        method: FactMethodKey {
            type_name: "Bean".to_string(),
            method_name: "setValue".to_string(),
        },
        kind: TrivialAccessorKind::Setter {
            field: FactFieldKey {
                type_name: "Bean".to_string(),
                field_name: "value".to_string(),
            },
            param_name: "value".to_string(),
            fluent: false,
        },
        inline_policy: AccessorInlinePolicy::InlineCandidate,
        provenance: FactProvenance::ParsedSource,
    });

    assert!(matches!(
        plan_trivial_accessor_call(
            Some(&receiver),
            "getValue",
            &[],
            Some(&resolved_getter),
            &facts,
        ),
        Some(TrivialAccessorCallPlan::Getter {
            field_name,
            ..
        }) if field_name == "value"
    ));
    assert!(matches!(
        plan_trivial_accessor_call(None, "setValue", &[value], Some(&resolved_setter), &facts),
        Some(TrivialAccessorCallPlan::Setter {
            field_name,
            ..
        }) if field_name == "value"
    ));
    assert!(
        plan_trivial_accessor_call(
            Some(&receiver),
            "getCount",
            &[],
            Some(&resolved_getter),
            &facts
        )
        .is_none()
    );

    assert!(matches!(
        plan_call_expr(
            Some(&receiver),
            "getValue",
            &[],
            Some(&resolved_getter),
            &facts,
            &TypeEnv::new(),
        ),
        CallExprPlan::TrivialAccessorGetter {
            field_name,
            ..
        } if field_name == "value"
    ));
    assert!(matches!(
        plan_call_expr(
            Some(&receiver),
            "getCount",
            &[],
            Some(&resolved_getter),
            &facts,
            &TypeEnv::new(),
        ),
        CallExprPlan::Target(CallTargetPlan::InstanceReceiver {
            rust_name: "get_value",
            ..
        })
    ));
    assert!(matches!(
        plan_call_statement(
            &Expr::Call {
                receiver: None,
                name: "setValue".to_string(),
                args: vec![Expr::Literal(Literal::Int(7))],
                resolved: Some(resolved_setter),
            },
            &facts,
        ),
        Some(CallStatementPlan::TrivialAccessorSetter {
            field_name,
            ..
        }) if field_name == "value"
    ));
    let for_each_receiver = Expr::Var("names".to_string());
    let for_each_body = Expr::Call {
        receiver: None,
        name: "emit".to_string(),
        args: vec![Expr::Var("name".to_string())],
        resolved: None,
    };
    assert!(matches!(
        plan_call_statement(
            &Expr::Call {
                receiver: Some(Box::new(for_each_receiver)),
                name: "forEach".to_string(),
                args: vec![Expr::Lambda {
                    params: vec![LambdaParam {
                        name: "name".to_string(),
                        ty: None,
                    }],
                    body: LambdaBody::Expr(Box::new(for_each_body)),
                }],
                resolved: None,
            },
            &facts,
        ),
        Some(CallStatementPlan::IterableForEach {
            param_names,
            ..
        }) if param_names == vec!["name"]
    ));
    let two_param_for_each_body = LambdaBody::Block(Block {
        statements: vec![Stmt::Expr(Expr::Call {
            receiver: Some(Box::new(Expr::Var("dest".to_string()))),
            name: "put".to_string(),
            args: vec![Expr::Var("name".to_string()), Expr::Var("item".to_string())],
            resolved: None,
        })],
    });
    assert!(matches!(
        plan_call_statement(
            &Expr::Call {
                receiver: Some(Box::new(Expr::Var("source".to_string()))),
                name: "forEach".to_string(),
                args: vec![Expr::Lambda {
                    params: vec![
                        LambdaParam {
                            name: "name".to_string(),
                            ty: None,
                        },
                        LambdaParam {
                            name: "item".to_string(),
                            ty: None,
                        },
                    ],
                    body: two_param_for_each_body,
                }],
                resolved: None,
            },
            &facts,
        ),
        Some(CallStatementPlan::IterableForEach {
            param_names,
            body: LambdaBody::Block(_),
            ..
        }) if param_names == vec!["name", "item"]
    ));
    assert!(matches!(
        plan_call_statement(
            &Expr::Call {
                receiver: Some(Box::new(Expr::Var("Arrays".to_string()))),
                name: "sort".to_string(),
                args: vec![Expr::Var("values".to_string())],
                resolved: None,
            },
            &facts,
        ),
        Some(CallStatementPlan::NativeArraysSort {
            target: Expr::Var(name),
        }) if name == "values"
    ));

    let mut preserve_api_facts = facts.clone();
    preserve_api_facts.trivial_accessor_facts[0].inline_policy = AccessorInlinePolicy::PreserveApi;
    assert!(
        plan_trivial_accessor_call(
            Some(&receiver),
            "getValue",
            &[],
            Some(&resolved_getter),
            &preserve_api_facts,
        )
        .is_none()
    );
    assert!(matches!(
        plan_call_expr(
            Some(&receiver),
            "getValue",
            &[],
            Some(&resolved_getter),
            &preserve_api_facts,
            &TypeEnv::new(),
        ),
        CallExprPlan::Target(CallTargetPlan::InstanceReceiver {
            rust_name: "get_value",
            ..
        })
    ));
}

#[test]
fn arrays_sort_statement_lowers_to_mutating_rust_sort() {
    let source = r#"
import java.util.Arrays;

class ArraysSortUse {
    void sortValues(int[] values) {
        Arrays.sort(values);
    }
}
"#;

    let translation = translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("values.sort();"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Arrays::sort"),
        "{}",
        translation.rust
    );
}

#[test]
fn throwing_method_result_ok_type_uses_planned_return_shape() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {
    Add() {}
}
class Maker {
    Expr make() throws Exception {
        return (Expr) new Add();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "Maker".to_string(),
            method_name: "make".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Named {
            name: "Exception".to_string(),
            args: Vec::new(),
        }],
        provenance: FactProvenance::ParsedSource,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let maker = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Maker")
        .unwrap();
    let method = maker
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        rust_type_for_slot(&signatures.method_return_shape(maker, method).unwrap()),
        RustType::Named("ExprValue".to_string())
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(
        rust.contains("pub fn make(&self) -> Result<ExprValue, JavaException>"),
        "{rust}"
    );
    assert!(
        rust.contains("return Ok(ExprValue::Add(Add::new()));"),
        "{rust}"
    );
}

#[test]
fn expression_shape_context_uses_representation_for_cast_dispatch() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);
    let mut env = ShapeEnv::new();
    env.insert_local(
        "add",
        JavaType::Named {
            name: "Add".to_string(),
            args: Vec::new(),
        },
    );

    let shape = infer_expr_shape_with_context(
        &parse_expr("(Expr) add"),
        &env,
        ExprShapeContext {
            signatures: None,
            representations: Some(&representations),
            facts: None,
        },
    );

    assert_eq!(
        shape.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert_eq!(
        rust_type_for_slot(&shape),
        RustType::Named("ExprValue".to_string())
    );
}

#[test]
fn cast_lowering_uses_representation_shaped_coercion_for_closed_enums() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Use {
    Animal up(Dog dog) {
        return (Animal) dog;
    }

    Dog down(Animal animal) {
        return (Dog) animal;
    }

    boolean isDog(Animal animal) {
        return animal instanceof Dog;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Animal".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("pub enum AnimalValue"), "{rust}");
    assert!(
        rust.contains("pub fn up(&self, dog: Dog) -> AnimalValue"),
        "{rust}"
    );
    assert!(rust.contains("return AnimalValue::Dog(dog);"), "{rust}");
    assert!(
        rust.contains("pub fn down(&self, animal: AnimalValue) -> Dog"),
        "{rust}"
    );
    assert!(
        rust.contains(
            "return match animal { AnimalValue::Dog(value) => value, _ => panic!(\"invalid Java cast\") };"
        ),
        "{rust}"
    );
    assert!(
        rust.contains("pub fn is_dog(&self, animal: AnimalValue) -> bool"),
        "{rust}"
    );
    assert!(
        rust.contains("return matches!(animal, AnimalValue::Dog(_));"),
        "{rust}"
    );
}

#[test]
fn coercion_plan_owns_invalid_cast_runtime_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let lower_source = include_str!("../src/lower.rs");
    let types_source = include_str!("../src/types.rs");

    assert!(conversion_source.contains("pub enum SyntaxSlotCoercionPolicy"));
    assert!(!source.contains("pub enum SyntaxSlotCoercionPolicy"));
    assert!(conversion_source.contains("pub enum Slot"));
    assert!(conversion_source.contains("pub fn assignment_slot"));
    assert!(conversion_source.contains("pub fn argument_slot"));
    assert!(conversion_source.contains("pub fn condition_slot"));
    assert!(conversion_source.contains("pub fn index_slot"));
    assert!(conversion_source.contains("pub fn assignment_slot_for_target"));
    assert!(conversion_source.contains("pub struct AssignmentValuePlan"));
    assert!(conversion_source.contains("pub fn assignment_value_plan_for_shape"));
    assert!(conversion_source.contains("pub fn assignment_value_plan_for_target"));
    assert!(conversion_source.contains("pub enum AssignmentStatementPlan"));
    assert!(conversion_source.contains("pub fn assignment_statement_plan_for_target"));
    assert!(conversion_source.contains("pub fn return_slot"));
    assert!(conversion_source.contains("pub enum ReturnValuePlan"));
    assert!(conversion_source.contains("pub fn return_value_plan"));
    assert!(!source.contains("pub enum Slot"));
    assert!(!source.contains("pub fn assignment_slot"));
    assert!(!source.contains("pub fn argument_slot"));
    assert!(!source.contains("pub fn condition_slot"));
    assert!(!source.contains("pub fn index_slot"));
    assert!(!source.contains("pub fn assignment_slot_for_target"));
    assert!(!source.contains("pub struct AssignmentValuePlan"));
    assert!(!source.contains("pub fn assignment_value_plan_for_shape"));
    assert!(!source.contains("pub fn assignment_value_plan_for_target"));
    assert!(!source.contains("pub enum AssignmentStatementPlan"));
    assert!(!source.contains("pub fn assignment_statement_plan_for_target"));
    assert!(!source.contains("pub fn return_slot"));
    assert!(!source.contains("pub enum ReturnValuePlan"));
    assert!(!source.contains("pub fn return_value_plan"));
    assert!(lower_source.contains("return_value_plan"));
    assert!(!lower_source.contains("return_slot"));
    assert!(lower_source.contains("assignment_value_plan_for_shape"));
    assert!(lower_source.contains("assignment_value_plan_for_target"));
    assert!(lower_source.contains("assignment_statement_plan_for_target"));
    assert!(!lower_source.contains("assignment_slot("));
    assert!(!lower_source.contains("assignment_slot_for_target"));
    assert!(conversion_source.contains("pub struct SyntaxCoercionPlan"));
    assert!(!source.contains("pub struct SyntaxCoercionPlan"));
    assert!(conversion_source.contains("pub fn syntax_coercion_plan"));
    assert!(conversion_source.contains("pub fn syntax_coercion_plan_with_facts"));
    assert!(!source.contains("pub fn syntax_coercion_plan"));
    assert!(!source.contains("pub fn syntax_coercion_plan_with_facts"));
    assert!(source.contains("pub fn apply_syntax_coercion_plan"));
    assert!(source.contains("impl SyntaxCoercionPlan"));
    assert!(source.contains("pub fn plan_coercion_with_facts"));
    assert!(source.contains("crate::conversion_plan::syntax_coercion_plan_with_facts"));
    assert!(
        conversion_source.contains("SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts")
    );
    assert!(source.contains("apply_syntax_coercion_plan(expr, plan)"));
    assert!(conversion_source.contains("pub fn syntax_slot_coercion_policy"));
    assert!(!source.contains("pub fn syntax_slot_coercion_policy"));
    assert!(conversion_source.contains("pub enum DispatchCoercionPolicy"));
    assert!(conversion_source.contains("pub fn dispatch_coercion_policy"));
    assert!(conversion_source.contains("pub fn dispatch_coercion_policy_with_facts"));
    assert!(conversion_source.contains("pub enum BorrowedDynToOwnedBoxedDynRecoveryPolicy"));
    assert!(conversion_source.contains("pub enum DynCloneBoxAdapterAvailability"));
    assert!(conversion_source.contains("pub enum DynCloneBoxAdapterRequirement"));
    assert!(conversion_source.contains("pub enum DynCloneBoxAdapterGenerationPolicy"));
    assert!(conversion_source.contains("pub enum DynCloneBoxAdapterDeferralReason"));
    assert!(conversion_source.contains("pub fn dyn_clone_box_adapter_requirement"));
    assert!(conversion_source.contains("dyn_clone_box_adapter_requirement_with_availability"));
    assert!(conversion_source.contains("pub fn dyn_clone_box_adapter_generation_policy"));
    assert!(
        conversion_source.contains("dyn_clone_box_adapter_generation_policy_with_availability")
    );
    assert!(!source.contains("pub enum DispatchCoercionPolicy"));
    assert!(!source.contains("pub fn dispatch_coercion_policy"));
    assert!(!source.contains("pub fn dispatch_coercion_policy_with_facts"));
    assert!(!source.contains("pub enum BorrowedDynToOwnedBoxedDynRecoveryPolicy"));
    assert!(!source.contains("pub enum DynCloneBoxAdapterRequirement"));
    assert!(!source.contains("pub enum DynCloneBoxAdapterGenerationPolicy"));
    assert!(conversion_source.contains("CloneBoxAdapterRequired"));
    assert!(source.contains("dyn_clone_box_adapter_generation_policy"));
    assert!(source.contains("pub struct UnsupportedCoercionMessagePolicy"));
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    assert!(design.contains("## Boxed Dyn Clone-Box Requirements"));
    assert!(design.contains("## Boxed Dyn Clone-Box Generation Policy"));
    assert!(design.contains("ProvenAdapter"));
    assert!(design.contains("RequiresGeneratedAdapter"));
    assert!(design.contains("UnsupportedNonCloneable"));
    assert!(design.contains("DeferredUnsupported"));
    assert!(conversion_source.contains("pub struct CoercionContentPolicy"));
    assert!(conversion_source.contains("pub fn coercion_content_policy"));
    assert!(conversion_source.contains("pub struct CoercionShapeContentPolicy"));
    assert!(conversion_source.contains("pub fn coercion_shape_content_policy"));
    assert!(!source.contains("pub struct CoercionContentPolicy"));
    assert!(!source.contains("pub fn coercion_content_policy"));
    assert!(!source.contains("pub struct CoercionShapeContentPolicy"));
    assert!(!source.contains("pub fn coercion_shape_content_policy"));
    assert!(conversion_source.contains("pub struct OptionalElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn optional_element_target_match_policy"));
    assert!(!source.contains("pub struct OptionalElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn optional_element_target_match_policy"));
    assert!(conversion_source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(!source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(conversion_source.contains("pub fn borrowed_element_slot_adapter_policy"));
    assert!(!source.contains("pub fn borrowed_element_slot_adapter_policy"));
    assert!(source.contains("fn apply_borrowed_element_slot_adapter"));
    assert!(conversion_source.contains("pub enum SlotConversionPlan"));
    assert!(!source.contains("pub enum SlotConversionPlan"));
    assert!(conversion_source.contains("pub fn slot_conversion_plan"));
    assert!(conversion_source.contains("pub fn slot_conversion_plan_with_facts"));
    assert!(conversion_source.contains("pub fn lowered_slot_conversion_plan"));
    assert!(conversion_source.contains("pub fn lowered_slot_conversion_plan_with_facts"));
    assert!(!source.contains("pub fn slot_conversion_plan"));
    assert!(!source.contains("pub fn slot_conversion_plan_with_facts"));
    assert!(!source.contains("pub fn lowered_slot_conversion_plan"));
    assert!(!source.contains("pub fn lowered_slot_conversion_plan_with_facts"));
    assert!(source.contains("pub fn apply_slot_conversion_plan"));
    assert!(source.contains("SlotConversionPlan::Shape"));
    assert!(source.contains("SlotConversionPlan::Syntax"));
    assert!(source.contains("SlotConversionPlan::BorrowedElement"));
    assert!(source.contains("crate::conversion_plan::lowered_slot_conversion_plan_with_facts"));
    assert!(source.contains("apply_slot_conversion_plan(lowered.expr, &plan)"));
    assert!(conversion_source.contains("pub struct ShapeCoercionPlan"));
    assert!(!source.contains("pub struct ShapeCoercionPlan"));
    assert!(conversion_source.contains("pub fn shape_coercion_plan"));
    assert!(conversion_source.contains("pub fn shape_coercion_plan_with_facts"));
    assert!(!source.contains("pub fn shape_coercion_plan"));
    assert!(!source.contains("pub fn shape_coercion_plan_with_facts"));
    assert!(source.contains("pub fn apply_shape_coercion_plan"));
    assert!(source.contains("impl ShapeCoercionPlan"));
    assert!(source.contains("pub fn plan_shape_coercion_with_facts"));
    assert!(source.contains("crate::conversion_plan::shape_coercion_plan_with_facts"));
    assert!(source.contains("apply_shape_coercion_plan(expr, &plan)"));
    assert!(conversion_source.contains("pub enum OptionalShapeCoercionPolicy"));
    assert!(conversion_source.contains("pub fn optional_shape_coercion_policy"));
    assert!(conversion_source.contains("pub fn optional_shape_coercion_policy_with_facts"));
    assert!(conversion_source.contains("element_plan: Box<ShapeCoercionPlan>"));
    assert!(conversion_source.contains("fn optional_element_slot_shape"));
    assert!(!source.contains("pub enum OptionalShapeCoercionPolicy"));
    assert!(!source.contains("pub fn optional_shape_coercion_policy"));
    assert!(!source.contains("pub fn optional_shape_coercion_policy_with_facts"));
    assert!(source.contains("pub fn non_identity_coercions"));
    assert!(!source.contains("element_coercions: Vec<Coercion>"));
    assert!(conversion_source.contains("pub struct VecElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn vec_element_target_match_policy"));
    assert!(!source.contains("pub struct VecElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn vec_element_target_match_policy"));
    assert!(source.contains("dispatch_coercion_fallback_message_policy"));
    assert!(source.contains("unsupported_dispatch_coercion"));
    assert!(conversion_source.contains("pub enum ViewCoercionPolicy"));
    assert!(conversion_source.contains("pub fn view_coercion_policy"));
    assert!(conversion_source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub enum ViewCoercionPolicy"));
    assert!(!source.contains("pub fn view_coercion_policy"));
    assert!(!source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub enum InvalidCastPolicy"));
    assert!(conversion_source.contains("pub enum JavaCastSemanticsPolicy"));
    assert!(conversion_source.contains("pub fn java_cast_semantics_policy"));
    assert!(conversion_source.contains("pub enum JavaTypeConversionKind"));
    assert!(conversion_source.contains("pub struct JavaTypeConversionPlan"));
    assert!(conversion_source.contains("pub fn java_type_conversion_plan_from_evidence"));
    assert!(conversion_source.contains("pub fn java_cast_type_conversion_plan"));
    assert!(types_source.contains("pub fn is_numeric_primitive_type"));
    assert!(types_source.contains("pub fn contains_wildcard_type"));
    assert!(types_source.contains("pub fn is_raw_generic_counterpart"));
    assert!(types_source.contains("pub fn is_unchecked_generic_counterpart"));
    assert!(types_source.contains("pub fn is_optional_or_result_type"));
    assert!(!source.contains("pub enum InvalidCastPolicy"));
    assert!(!source.contains("pub enum JavaCastSemanticsPolicy"));
    assert!(!source.contains("pub fn java_cast_semantics_policy"));
    assert!(!source.contains("pub enum JavaTypeConversionKind"));
    assert!(!source.contains("pub struct JavaTypeConversionPlan"));
    assert!(!source.contains("pub fn java_type_conversion_plan_from_evidence"));
    assert!(!source.contains("pub fn java_cast_type_conversion_plan"));
    assert!(conversion_source.contains("pub struct RustNominalTypeNamePolicy"));
    assert!(conversion_source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(conversion_source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(conversion_source.contains("pub fn runtime_type_target_policy"));
    assert!(!source.contains("pub struct RustNominalTypeNamePolicy"));
    assert!(!source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(!source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(!source.contains("pub fn runtime_type_target_policy"));
    assert!(conversion_source.contains("pub struct PrimitiveWideningPolicy"));
    assert!(conversion_source.contains("pub fn primitive_widening_policy"));
    assert!(conversion_source.contains("pub struct PrimitiveRankPolicy"));
    assert!(conversion_source.contains("pub fn primitive_rank_policy"));
    assert!(!source.contains("pub struct PrimitiveWideningPolicy"));
    assert!(!source.contains("pub fn primitive_widening_policy"));
    assert!(!source.contains("pub struct PrimitiveRankPolicy"));
    assert!(!source.contains("pub fn primitive_rank_policy"));
    assert!(source.contains("pub struct RustPrimitiveTypeNamePolicy"));
    assert!(source.contains("pub fn rust_primitive_type_name_policy"));
    assert!(!source.contains("fn rust_nominal_type_name("));
    assert!(!source.contains("fn option_target_matches("));
    assert!(!source.contains("fn vec_target_matches("));
    assert!(!source.contains("fn apply_borrowed_option_element_adapter"));
    assert!(!source.contains("fn apply_borrowed_vec_element_adapter"));
    assert!(!source.contains("fn primitive_widening("));
    assert!(!source.contains("fn primitive_rank("));
    assert!(!source.contains("fn rust_primitive_name("));
    assert!(source.contains("MissingRuntimeTarget"));
    assert!(source.contains("unsupported_dispatch_missing_runtime_target"));
    assert!(source.contains("invalid_cast: InvalidCastPolicy"));
    assert!(source.contains("DowncastBoxedDynTrait"));
    assert!(source.contains("DowncastErasedObject"));
    assert!(source.contains("apply_boxed_dyn_trait_downcast"));
    assert!(source.contains("render_invalid_cast_policy"));
    assert!(conversion_source.contains("semantics: JavaCastSemanticsPolicy"));
    assert!(conversion_source.contains("PrimitiveNumericCast"));
    assert!(conversion_source.contains("CheckedReferenceDowncast"));
    assert!(conversion_source.contains("UncheckedGenericCast"));
    assert!(conversion_source.contains("RawOrWildcardCast"));
    assert!(conversion_source.contains("OptionalResultCast"));
    assert!(conversion_source.contains("DiagnosticOnlyFallback"));
}

#[test]
fn conversion_plan_owns_optional_null_clear_assignment_policy() {
    let mut env = ShapeEnv::new();
    env.insert_local_shape(
        "cached",
        RustSlotShape::conservative(optional_java_type(JavaType::String)),
    );

    assert_eq!(
        assignment_statement_plan_for_target(
            &parse_expr("cached"),
            &Expr::Literal(Literal::Null),
            &env,
            ExprShapeContext::default(),
        ),
        AssignmentStatementPlan::ClearOptionalTarget
    );
    assert_eq!(
        assignment_statement_plan_for_target(
            &parse_expr("cached"),
            &parse_expr("\"value\""),
            &env,
            ExprShapeContext::default(),
        ),
        AssignmentStatementPlan::AssignValue
    );
}

#[test]
fn coercion_plan_owns_instanceof_fallback_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let facts_source = include_str!("../src/facts.rs");

    assert!(source.contains("pub struct CoercionDiagnostic"));
    assert!(conversion_source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(conversion_source.contains("pub enum InstanceOfTargetPolicy"));
    assert!(!source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(!source.contains("pub enum InstanceOfTargetPolicy"));
    assert!(source.contains("ErasedObjectToDynTrait"));
    assert!(source.contains("ErasedObjectToBoxedDynTrait"));
    assert!(conversion_source.contains("pub enum InstanceOfCheck"));
    assert!(!source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("BoxedDynTraitType"));
    assert!(conversion_source.contains("policy: InstanceOfFallbackPolicy"));
    assert!(conversion_source.contains("pub fn instanceof_target_policy"));
    assert!(conversion_source.contains("pub fn plan_instanceof_check"));
    assert!(conversion_source.contains("pub fn plan_instanceof_check_with_facts"));
    assert!(conversion_source.contains("pub fn hierarchy_instanceof_policy"));
    assert!(facts_source.contains("pub struct HierarchySubtypePolicy"));
    assert!(facts_source.contains("pub fn hierarchy_subtype_policy"));
    assert!(!source.contains("pub fn plan_instanceof_check"));
    assert!(!source.contains("pub fn instanceof_target_policy"));
    assert!(!source.contains("hierarchy_subtype_policy"));
    assert!(source.contains("unsupported_instanceof_shape"));
    assert!(source.contains("unsupported_instanceof_primitive_target"));
    assert!(source.contains("unsupported_instanceof_raw_or_wildcard_target"));
    assert!(source.contains("unsupported_erased_object_instanceof_dyn_trait"));
    assert!(source.contains("unsupported_erased_object_instanceof_boxed_dyn_trait"));
    assert!(source.contains("unsupported_instanceof_missing_runtime_target"));
    assert!(source.contains("instanceof_fallback_message_policy"));
    assert!(source.contains("render_instanceof_fallback_policy"));
}

#[test]
fn coercion_plan_owns_clone_recovery_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let external_packs_source = include_str!("../src/external_packs.rs");
    let facts_source = include_str!("../src/facts.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(conversion_source.contains("pub enum CloneRecoveryPolicy"));
    assert!(conversion_source.contains("pub enum OwnershipCoercionPolicy"));
    assert!(conversion_source.contains("pub enum OwnershipCoercionFallbackPolicy"));
    assert!(!source.contains("pub enum CloneRecoveryPolicy"));
    assert!(!source.contains("pub enum OwnershipCoercionPolicy"));
    assert!(!source.contains("pub enum OwnershipCoercionFallbackPolicy"));
    assert!(!source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_source.contains("pub fn ownership_coercion_policy"));
    assert!(conversion_source.contains("ownership_coercion_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub struct CoercionOwnedRecoveryCapabilityPolicy"));
    assert!(conversion_source.contains("pub fn coercion_owned_recovery_capability_policy"));
    assert!(
        conversion_source
            .contains("coercion_owned_recovery_capability_policy_with_effective_facts")
    );
    assert!(facts_source.contains("pub struct OwnedRecoveryCapabilityFact"));
    assert!(facts_source.contains("owned_recovery_capability_facts"));
    assert!(facts_source.contains("CloneBoxAdapter"));
    assert!(facts_source.contains("FactoryCopy"));
    assert!(facts_source.contains("ImmutableSharedHandle"));
    assert!(facts_source.contains("FacadeOwnedExternalHandle"));
    assert!(facts_source.contains("FfiOwnedHandle"));
    assert!(external_packs_source.contains("external-pack-owned-recovery"));
    assert!(external_packs_source.contains("push_external_pack_owned_recovery_fact"));
    assert!(external_packs_source.contains("parse_external_pack_owned_recovery_capability"));
    assert!(external_packs_source.contains("owned_recovery_capability_facts"));
    assert!(!source.contains("pub fn ownership_coercion_policy"));
    assert!(!source.contains("pub fn ownership_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub struct CoercionOwnedRecoveryCapabilityPolicy"));
    assert!(!source.contains("pub fn coercion_owned_recovery_capability_policy"));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedCopyValue"));
    assert!(source.contains("Coercion::CopyBorrowedValue"));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedWithClone"));
    assert!(conversion_source.contains("BorrowedToOwnedNonCloneable"));
    assert!(conversion_source.contains("BorrowedToFacadeOwnedExternalHandle"));
    assert!(conversion_source.contains("BorrowedToFfiOwnedHandle"));
    assert!(source.contains("ownership_coercion_fallback_message_policy"));
    assert!(conversion_source.contains("policy: CloneRecoveryPolicy"));
    assert!(conversion_source.contains("ProvenCloneCapability"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_for"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_fact_for"));
    assert_eq!(
        conversion_source
            .matches("facts.owned_recovery_capability_for")
            .count(),
        1
    );
    assert!(conversion_source.contains("ownership_recovery_policy_from_capability"));
    assert!(source.contains("clone_audit_required"));
    assert!(source.contains("unsupported_ownership_coercion"));
    assert!(source.contains("unsupported_borrowed_to_owned_non_cloneable"));
    assert!(source.contains("render_clone_recovery_policy"));
}

#[test]
fn coercion_plan_owns_borrowed_element_recovery_fallback_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(conversion_source.contains("pub enum BorrowedElementRecoveryPolicy"));
    assert!(!source.contains("pub enum BorrowedElementRecoveryPolicy"));
    assert!(conversion_source.contains("pub enum BorrowedElementCarrier"));
    assert!(conversion_source.contains("pub struct BorrowedElementAdapterPolicy"));
    assert!(!source.contains("pub enum BorrowedElementCarrier"));
    assert!(!source.contains("pub struct BorrowedElementAdapterPolicy"));
    assert!(conversion_source.contains("pub fn borrowed_element_adapter_policy"));
    assert!(!source.contains("pub fn borrowed_element_adapter_policy"));
    assert!(source.contains("fn apply_borrowed_element_adapter"));
    assert!(conversion_source.contains("pub fn borrowed_element_recovery_policy"));
    assert!(conversion_source.contains("borrowed_element_recovery_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub enum BorrowedElementRecoveryFallbackPolicy"));
    assert!(conversion_source.contains("CompileError"));
    assert!(conversion_source.contains("NonCloneableElement"));
    assert!(conversion_source.contains("policy: CloneRecoveryPolicy"));
    assert!(conversion_source.contains("borrowed_element_recovery_policy_from_capability"));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy"));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy_with_effective_facts"));
    assert!(!source.contains("pub enum BorrowedElementRecoveryFallbackPolicy"));
    assert!(source.contains("borrowed_element_clone_audit_required"));
    assert!(source.contains("borrowed_element_recovery_fallback_message_policy"));
    assert!(source.contains("unsupported_borrowed_element_recovery"));
    assert!(source.contains("unsupported_borrowed_element_non_cloneable"));
    assert!(source.contains("render_borrowed_element_recovery_fallback_policy"));
    assert!(!source.contains("fn shape_has_string_content"));
    assert!(conversion_source.contains("coercion_content_policy_with_effective_facts"));
    assert!(conversion_source.contains("coercion_shape_content_policy_with_effective_facts"));
    assert!(!source.contains("pub fn coercion_content_policy_with_effective_facts"));
    assert!(!source.contains("pub fn coercion_shape_content_policy_with_effective_facts"));
    assert!(conversion_source.contains("view_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(effective_facts_source.contains("pub fn is_string_like_type"));
    assert!(conversion_source.contains("coercion_owned_recovery_capability_policy"));
    assert!(conversion_source.contains("pub fn borrowed_element_recovery_policy("));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy("));
    assert!(conversion_source.contains("borrowed_element_recovery_policy(element_shape, facts)"));
    assert_eq!(
        conversion_source
            .matches("facts.owned_recovery_capability_for")
            .count(),
        1
    );
    assert!(!source.contains(".copy"));
    assert!(!source.contains(".clone {"));
    assert!(!source.contains(".string_like"));
}

#[test]
fn representation_plan_infers_closed_enum_for_final_known_subtypes() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let pre_inference_expr = TypeRepresentation {
        java_name: "Expr".to_string(),
        representation: ObjectRepresentation::DynTrait {
            trait_name: "Expr".to_string(),
        },
        provenance: RepresentationProvenance::ParsedSource,
        direct_subtypes: vec!["Add".to_string(), "Num".to_string()],
    };
    let pre_inference_add = TypeRepresentation {
        java_name: "Add".to_string(),
        representation: ObjectRepresentation::ConcreteStruct,
        provenance: RepresentationProvenance::ParsedSource,
        direct_subtypes: Vec::new(),
    };
    let expr_policy = closed_enum_inference_policy(
        "Expr",
        &pre_inference_expr.representation,
        &pre_inference_expr.direct_subtypes,
        &resolved.facts,
    );
    let add_policy = closed_enum_inference_policy(
        "Add",
        &pre_inference_add.representation,
        &pre_inference_add.direct_subtypes,
        &resolved.facts,
    );
    let expr_fact_policy =
        closed_enum_fact_policy("Expr", &pre_inference_expr.direct_subtypes, &resolved.facts);
    let add_fact_policy =
        closed_enum_fact_policy("Add", &pre_inference_add.direct_subtypes, &resolved.facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(
        expr_fact_policy,
        ClosedEnumFactPolicy {
            type_name: "Expr".to_string(),
            variants: Some(vec!["Add".to_string(), "Num".to_string()]),
        }
    );
    assert_eq!(
        add_fact_policy,
        ClosedEnumFactPolicy {
            type_name: "Add".to_string(),
            variants: None,
        }
    );
    assert_eq!(
        expr_policy.representation,
        Some(ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        })
    );
    assert_eq!(expr_policy.provenance, Some(FactProvenance::CorpusAnalysis));
    assert_eq!(add_policy.representation, None);
    assert_eq!(add_policy.provenance, None);
    assert_eq!(
        representations
            .type_representation("Expr")
            .unwrap()
            .provenance,
        RepresentationProvenance::CorpusAnalysis
    );
}

#[test]
fn representation_plan_does_not_infer_closed_enum_without_closed_world_fact() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
final class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::DynTrait {
            trait_name: "Expr".to_string(),
        }
    );
    assert_eq!(
        closed_enum_proof_policy(
            "Expr",
            &["Add".to_string(), "Num".to_string()],
            &resolved.facts
        ),
        ClosedEnumProofPolicy {
            type_name: "Expr".to_string(),
            proof: ClosedEnumProof::OpenWorld,
        }
    );
}

#[test]
fn representation_plan_rejects_closed_enum_when_sealed_permit_is_missing() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        closed_enum_proof_policy("Expr", &["Add".to_string()], &resolved.facts),
        ClosedEnumProofPolicy {
            type_name: "Expr".to_string(),
            proof: ClosedEnumProof::MissingPermittedSubtypes {
                missing: vec!["Num".to_string()],
            },
        }
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::DynTrait {
            trait_name: "Expr".to_string(),
        }
    );
}

#[test]
fn closed_enum_proof_accepts_external_closed_facts_without_permits() {
    let source = r#"
interface Expr {}
final class Add implements Expr {}
final class Num implements Expr {}
"#;
    let parsed = parse_java_source(source);
    let mut resolved = resolve_program(&parsed.program, parsed.diagnostics);
    resolved
        .facts
        .closed_hierarchy_facts
        .push(ClosedHierarchyFact {
            type_name: "Expr".to_string(),
            closed: true,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    assert_eq!(
        closed_enum_proof_policy(
            "Expr",
            &["Add".to_string(), "Num".to_string()],
            &resolved.facts
        ),
        ClosedEnumProofPolicy {
            type_name: "Expr".to_string(),
            proof: ClosedEnumProof::Proven {
                variants: vec!["Add".to_string(), "Num".to_string()],
                provenance: FactProvenance::ExternalLibraryPack,
            },
        }
    );
}

#[test]
fn closed_enum_proof_rejects_non_final_direct_variants() {
    let parsed = parse_java_source(
        r#"
sealed interface Expr permits Add, Num {}
class Add implements Expr {}
final class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);

    assert_eq!(
        closed_enum_proof_policy(
            "Expr",
            &["Add".to_string(), "Num".to_string()],
            &resolved.facts
        ),
        ClosedEnumProofPolicy {
            type_name: "Expr".to_string(),
            proof: ClosedEnumProof::NonFinalVariants {
                variants: vec!["Add".to_string()],
            },
        }
    );
}

#[test]
fn translate_source_with_facts_ingests_external_closed_hierarchy_facts() {
    let source = r#"
interface Expr {}
final class Add implements Expr {}
final class Num implements Expr {}
"#;
    let mut external_facts = FactSet::default();
    external_facts
        .closed_hierarchy_facts
        .push(ClosedHierarchyFact {
            type_name: "Expr".to_string(),
            closed: true,
            provenance: FactProvenance::UserOverride,
        });

    let conservative = translate_source(source);
    let with_facts = translate_source_with_facts(source, &external_facts);

    assert!(!conservative.rust.contains("pub enum ExprValue"));
    assert!(
        with_facts.rust.contains("pub enum ExprValue"),
        "{}",
        with_facts.rust
    );
}

#[test]
fn erased_object_external_handle_lowers_store_pass_and_return_through_same_carrier() {
    let source = r#"
class Dog {}
class Helper {
    void consume(Object object) {}
}
class Holder {
    Object value;
    Holder(Object value) { this.value = value; }
    void store(Dog dog) { this.value = dog; }
    Object get() { return this.value; }
    void pass(Helper helper) { helper.consume(this.value); }
}
"#;
    let mut external_facts = FactSet::default();
    external_facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Object".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &external_facts);

    assert!(
        translation.rust.contains("value: Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.value = Box::new(dog) as Box<dyn std::any::Any>;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn get(&self) -> Box<dyn std::any::Any>")
            || translation
                .rust
                .contains("fn get(&self) -> Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("helper.consume(self.value);")
            || translation.rust.contains("helper.consume(&self.value);"),
        "{}",
        translation.rust
    );
}

#[test]
fn borrowed_self_field_owned_recovery_applies_to_mutable_receiver_assignment() {
    let source = r#"
class Holder {
    String workspace;
    String incrementalImportWorkspace;
    void promote() {
        this.workspace = this.incrementalImportWorkspace;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("self.workspace = Some(self.incremental_import_workspace.clone()")
            && !translation
                .rust
                .contains("self.workspace = self.incremental_import_workspace;"),
        "{}",
        translation.rust
    );
}

#[test]
fn borrowed_self_field_owned_recovery_applies_to_unshaped_external_arguments() {
    let source = r#"
class Holder {
    String workspace;
    void pass() {
        ExternalUtil.consume(this.workspace);
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("external_util::consume(self.workspace.clone()")
            && !translation
                .rust
                .contains("external_util::consume(self.workspace);"),
        "{}",
        translation.rust
    );
}

#[test]
fn source_static_string_constant_push_to_vec_string_uses_to_string_adapter() {
    let source = r#"
import java.util.ArrayList;
import java.util.List;

class Holder {
    static final String NAME = "name";
    List<String> missing() {
        List<String> values = new ArrayList<>();
        values.add(NAME);
        return values;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("values.push(Self::NAME.to_string())")
            && !translation.rust.contains("values.push(Self::NAME.clone())"),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_local_recovery_applies_to_unshaped_external_arguments() {
    let source = r#"
class Holder {
    String pass(String workspaceDir) {
        ExternalUtil.consume(workspaceDir);
        return workspaceDir;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains("external_util::consume(workspace_dir.clone()")
            && translation.rust.contains("return workspace_dir;")
            && !translation
                .rust
                .contains("external_util::consume(workspace_dir);"),
        "{}",
        translation.rust
    );
}

#[test]
fn usage_counts_static_call_arguments_after_fluent_builder_consumption() {
    let parsed = parse_java_source(
        r#"
class Holder {
    String pass(String workspace) {
        Builder.newBuilder().setWorkspace(workspace);
        if (ExternalUtil.exists(workspace)) {
            return workspace;
        }
        return "";
    }
}

class Builder {
    static Builder newBuilder() {
        return null;
    }
    Builder setWorkspace(String workspace) {
        return this;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "Holder".to_string(),
            method_name: "pass".to_string(),
        })
        .unwrap();
    let binding = method.bindings.get("workspace").unwrap();

    assert!(
        binding.uses.len() >= 3
            && binding
                .uses
                .iter()
                .any(|usage| matches!(usage, BindingUse::PassedToCall { name, .. } if name == "setWorkspace"))
            && binding
                .uses
                .iter()
                .any(|usage| matches!(usage, BindingUse::PassedToCall { name, .. } if name == "exists"))
            && binding.uses.iter().any(|usage| matches!(usage, BindingUse::Returned)),
        "{:?}",
        binding.uses
    );
}

#[test]
fn reused_local_recovery_applies_inside_fluent_external_builder_chains() {
    let source = r#"
class Holder {
    String pass(String workspace) {
        Builder builder = ExternalBuilder.newBuilder().setWorkspace(workspace);
        if (ExternalUtil.exists(workspace)) {
            return workspace;
        }
        return "";
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains(".set_workspace(workspace.clone()")
            && translation.rust.contains("external_util::exists(workspace")
            && translation.rust.contains("return workspace;")
            && !translation.rust.contains(".set_workspace(workspace)"),
        "{}",
        translation.rust
    );
}

#[test]
fn reused_local_recovery_uses_overload_specific_usage_facts() {
    let source = r#"
class Holder {
    String make(String workspace) {
        Builder builder = ExternalBuilder.newBuilder().setWorkspace(workspace);
        if (ExternalUtil.exists(workspace)) {
            return workspace;
        }
        return "";
    }
    String make(String workspace, String ignored) {
        ExternalUtil.consume(ignored);
        return workspace;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation
            .rust
            .contains(".set_workspace(workspace.clone()")
            && translation.rust.contains("pub fn make_string_string")
            && !translation.rust.contains(".set_workspace(workspace)"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_self_field_return_recovers_owned_option_without_moving_field() {
    let source = r#"
class Holder {
    String workspace;
    String take() {
        return this.workspace;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation.rust.contains("return self.workspace.clone();")
            && !translation.rust.contains("return self.workspace;"),
        "{}",
        translation.rust
    );
}

#[test]
fn optional_self_field_read_into_owned_local_borrows_then_clones_element() {
    let source = r#"
class Cursor {}
class Holder {
    Cursor query;
    Cursor take() {
        Cursor tmp = query;
        query = null;
        return tmp;
    }
}
"#;

    let translation = translate_source(source);

    assert!(
        translation.rust.contains(
            "let tmp: Cursor = self.query.as_ref().expect(\"Java null receiver\").clone()"
        ) && translation.rust.contains("self.query.take();")
            && !translation
                .rust
                .contains("let tmp: Cursor = self.query.unwrap();"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_facts_parser_ingests_closed_world_and_representation_overrides() {
    let parsed = parse_external_facts(
        r#"
# one closed-world fact and two hierarchy overrides
closed Expr
closed-enum Expr ExprValue Add Num
boxed-dyn-trait Sink SinkTrait
dispatch-requirement RuntimeValue needs-subtype-dispatch
dispatch-requirement RuntimeValue needs-downcast
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(
        parsed.facts.closed_hierarchy_fact_for_name("Expr"),
        Some(true)
    );
    assert!(
        parsed
            .facts
            .hierarchy_overrides
            .iter()
            .any(|override_fact| {
                override_fact.type_name == "Expr"
                    && matches!(
                        &override_fact.strategy,
                        HierarchyStrategy::ClosedEnum { enum_name, variants }
                            if enum_name == "ExprValue"
                                && variants == &vec!["Add".to_string(), "Num".to_string()]
                    )
            })
    );
    assert!(
        parsed
            .facts
            .hierarchy_overrides
            .iter()
            .any(|override_fact| {
                override_fact.type_name == "Sink"
                    && matches!(
                        &override_fact.strategy,
                        HierarchyStrategy::DynTrait { trait_name } if trait_name == "SinkTrait"
                    )
            })
    );
    assert!(parsed.facts.dispatch_requirement_facts.iter().any(|fact| {
        fact.type_name == "Sink"
            && fact.requirement == DispatchRequirementKind::NeedsOwnedPolymorphicStorage
    }));
    assert!(parsed.facts.dispatch_requirement_facts.iter().any(|fact| {
        fact.type_name == "RuntimeValue"
            && fact.requirement == DispatchRequirementKind::NeedsSubtypeDispatch
            && fact.provenance == FactProvenance::UserOverride
    }));
    assert!(parsed.facts.dispatch_requirement_facts.iter().any(|fact| {
        fact.type_name == "RuntimeValue"
            && fact.requirement == DispatchRequirementKind::NeedsDowncast
            && fact.provenance == FactProvenance::UserOverride
    }));
}

#[test]
fn external_closed_world_capability_facts_infer_closed_enum_representation() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add {}
class Num {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let external = parse_external_facts(
        r#"
closed Expr
implements Add Expr
implements Num Expr
final Add
final Num
"#,
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.direct_subtypes_of("Expr"),
        ["Add".to_string(), "Num".to_string()].as_slice()
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(
        representations
            .type_representation("Expr")
            .unwrap()
            .provenance,
        RepresentationProvenance::CorpusAnalysis
    );
}

#[test]
fn external_facts_parser_ingests_type_capability_overrides() {
    let parsed = parse_external_facts(
        r#"
copy ExternalId
clone ExternalHandle false
string-like ExternalLabel true
sequence-like ExternalIds ExternalId
extends Dog Animal
implements Dog Pet Named
final Dog
object-safe-trait Pet false
object-safety-violation Pet self-param accept value
object-safety-violation Pack nested-self-return pack
object-safety-violation Pack nested-self-param unpack values
"#,
    );
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let external_label = JavaType::Named {
        name: "ExternalLabel".to_string(),
        args: Vec::new(),
    };
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let dog = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };
    let pet = JavaType::Named {
        name: "Pet".to_string(),
        args: Vec::new(),
    };

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert!(parsed.facts.is_copy_type(&external_id));
    assert!(!parsed.facts.is_clone_type(&external_handle));
    assert!(parsed.facts.is_string_like_type(&external_label));
    assert_eq!(
        parsed.facts.sequence_element_type_for(&external_ids),
        Some(external_id)
    );
    assert_eq!(
        parsed.facts.extends_type_for(&dog),
        Some(JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        parsed.facts.implemented_types_for(&dog),
        vec![
            JavaType::Named {
                name: "Pet".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "Named".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert!(parsed.facts.is_final_type(&dog));
    assert_eq!(parsed.facts.object_safe_trait_fact_for(&pet), Some(false));
    assert_eq!(
        parsed.facts.object_safety_violation_for_name("Pet"),
        Some(ObjectSafetyViolation::SelfReferentialParameter {
            method_name: "accept".to_string(),
            param_name: "value".to_string(),
        })
    );
    assert_eq!(
        parsed
            .facts
            .object_safety_fact_for_name("Pet")
            .unwrap()
            .violations,
        vec![ObjectSafetyViolation::SelfReferentialParameter {
            method_name: "accept".to_string(),
            param_name: "value".to_string(),
        }]
    );
    assert_eq!(
        parsed
            .facts
            .object_safety_fact_for_name("Pack")
            .unwrap()
            .violations,
        vec![
            ObjectSafetyViolation::NestedSelfReferentialReturn {
                method_name: "pack".to_string(),
            },
            ObjectSafetyViolation::NestedSelfReferentialParameter {
                method_name: "unpack".to_string(),
                param_name: "values".to_string(),
            },
        ]
    );
}

#[test]
fn external_facts_parser_ingests_source_summary_field_type_facts() {
    let parsed = parse_external_facts(
        r#"
external-constructor SummaryBuilder String int
field GATKConfig.CONFIG_FILE_VARIABLE_FILE_NAME String
field ReferenceContext UNINITIALIZED_REFERENCE_ID int
field ReadLikelihoodCalculationEngine.DEFAULT_EXPECTED_ERROR_RATE_PER_BASE double
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let constructor = parsed
        .facts
        .external_constructor_facts
        .iter()
        .find(|fact| fact.type_name == "SummaryBuilder")
        .expect("source-summary constructor should be parsed as a constructor fact");
    assert_eq!(constructor.provenance, FactProvenance::UserOverride);
    assert_eq!(
        constructor.arg_types,
        vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int),]
    );
    let config = parsed
        .facts
        .external_field_for("GATKConfig", "CONFIG_FILE_VARIABLE_FILE_NAME")
        .expect("GATKConfig constant should be parsed as a field fact");
    assert_eq!(config.provenance, FactProvenance::UserOverride);
    assert_eq!(config.field_type, Some(JavaType::String));
    assert!(
        parsed
            .facts
            .external_field_for("g_a_t_k_config", "CONFIG_FILE_VARIABLE_FILE_NAME")
            .is_some(),
        "field facts should be addressable by resolver-canonical receiver spelling"
    );

    let reference = parsed
        .facts
        .external_field_for("ReferenceContext", "UNINITIALIZED_REFERENCE_ID")
        .expect("ReferenceContext constant should be parsed as a field fact");
    assert_eq!(
        reference.field_type,
        Some(JavaType::Primitive(PrimitiveType::Int))
    );

    let error_rate = parsed
        .facts
        .external_field_for(
            "ReadLikelihoodCalculationEngine",
            "DEFAULT_EXPECTED_ERROR_RATE_PER_BASE",
        )
        .expect("ReadLikelihoodCalculationEngine constant should be parsed as a field fact");
    assert_eq!(
        error_rate.field_type,
        Some(JavaType::Primitive(PrimitiveType::Double))
    );
}

#[test]
fn external_facts_parser_ingests_external_api_effect_overrides() {
    let parsed = parse_external_facts(
        r#"
external-api getValue * getter read-only
external-api setValue ExternalCounter setter mutates-receiver unknown
external-api format String getter read-only definitely-non-null
external-api read NativeHandle getter read-only maybe-null args=1 return=Vec<u8> throws=NativeError,IOException,NativeError
external-api qualifiedRead dep.variant.NativeHandle getter read-only definitely-non-null args=1 return=String
external-argument read NativeHandle 0 definitely-non-null read-only borrowed type=String
external-argument qualifiedRead dep.variant.NativeHandle 0 definitely-non-null read-only borrowed type=String
external-crate-api NativeHandle read 1 read_record
external-crate-constructor NativeHandle 1 NativeHandleFacade open
external-constructor dep.variant.NativeHandle String
external-crate-api dep.variant.NativeHandle rawRead 0 raw_read_record
external-crate-constructor dep.variant.NativeHandle 1 NativeHandleFacade open_qualified
unsupported-api native NativeHandle rawRead requires runtime codec
unsupported-api native dep.variant.NativeHandle rawDecode requires qualified runtime codec
unsupported-constructor native NativeHandle 2 requires native open flags
unsupported-constructor native dep.variant.NativeHandle 2 requires qualified native open flags
unsupported-field native NativeHandle RAW_HANDLE exposes native pointer
unsupported-field native dep.variant.NativeHandle RAW_HANDLE exposes qualified native pointer
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let getter = parsed.facts.external_api_effect_for("getValue").unwrap();
    assert_eq!(getter.method_name, "getValue");
    assert_eq!(getter.receiver_type, None);
    assert_eq!(getter.role, ExternalApiRole::Getter);
    assert_eq!(getter.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(getter.return_nullability, NullabilityState::Unknown);
    assert_eq!(getter.provenance, FactProvenance::UserOverride);

    let setter = parsed
        .facts
        .external_api_effect_for_receiver(Some("ExternalCounter"), "setValue")
        .unwrap();
    assert_eq!(setter.receiver_type.as_deref(), Some("ExternalCounter"));
    assert_eq!(setter.role, ExternalApiRole::Setter);
    assert_eq!(
        setter.receiver_mutability,
        ReceiverMutabilityFact::MutatesReceiver
    );
    assert_eq!(setter.return_nullability, NullabilityState::Unknown);
    assert_eq!(setter.provenance, FactProvenance::UserOverride);

    let format = parsed
        .facts
        .external_api_effect_for_receiver(Some("String"), "format")
        .unwrap();
    assert_eq!(
        format.return_nullability,
        NullabilityState::DefinitelyNonNull
    );

    let read = parsed
        .facts
        .external_api_effect_for_receiver_with_arity(Some("NativeHandle"), "read", Some(1))
        .unwrap();
    assert_eq!(read.arg_count, Some(1));
    assert_eq!(
        read.return_type,
        Some(JavaType::Named {
            name: "Vec".to_string(),
            args: vec![JavaType::Named {
                name: "u8".to_string(),
                args: Vec::new(),
            }],
        })
    );
    assert_eq!(
        read.thrown,
        vec![
            JavaType::Named {
                name: "NativeError".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "IOException".to_string(),
                args: Vec::new(),
            },
        ]
    );
    let read_argument = parsed
        .facts
        .external_api_argument_effect_for_receiver(Some("NativeHandle"), "read", 0)
        .expect("external-argument should parse as an argument effect fact");
    assert_eq!(read_argument.parameter_type, Some(JavaType::String));
    assert_eq!(
        read_argument.parameter_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        read_argument.argument_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        read_argument.alias_policy,
        ExternalArgumentAliasPolicy::Borrowed
    );
    assert_eq!(read_argument.provenance, FactProvenance::UserOverride);

    let read_mapping = parsed
        .facts
        .external_crate_api_mapping_for_receiver("NativeHandle", "read", Some(1))
        .expect("external-crate-api should parse as an API mapping fact");
    assert_eq!(read_mapping.rust_name, "read_record");
    assert_eq!(read_mapping.arg_count, Some(1));
    assert_eq!(read_mapping.provenance, FactProvenance::UserOverride);

    let qualified_read = parsed
        .facts
        .external_api_effect_for_receiver_with_arity(
            Some("dep.variant.native_handle"),
            "qualifiedRead",
            Some(1),
        )
        .expect("external-api lookup should normalize qualified receiver spellings");
    assert_eq!(qualified_read.return_type, Some(JavaType::String));
    assert_eq!(qualified_read.provenance, FactProvenance::UserOverride);

    let qualified_read_argument = parsed
        .facts
        .external_api_argument_effect_for_receiver(
            Some("dep.variant.native_handle"),
            "qualifiedRead",
            0,
        )
        .expect("external-argument lookup should normalize qualified receiver spellings");
    assert_eq!(
        qualified_read_argument.parameter_type,
        Some(JavaType::String)
    );
    assert_eq!(
        qualified_read_argument.alias_policy,
        ExternalArgumentAliasPolicy::Borrowed
    );
    assert_eq!(
        qualified_read_argument.provenance,
        FactProvenance::UserOverride
    );

    let constructor_mapping = parsed
        .facts
        .external_crate_constructor_mapping_for("NativeHandle", 1)
        .expect("external-crate-constructor should parse as a constructor mapping fact");
    assert_eq!(
        constructor_mapping.rust_type_name.as_deref(),
        Some("NativeHandleFacade")
    );
    assert_eq!(constructor_mapping.rust_name, "open");
    assert_eq!(constructor_mapping.provenance, FactProvenance::UserOverride);

    let qualified_constructor = parsed
        .facts
        .external_constructor_for("dep.variant.native_handle", &[JavaType::String])
        .expect("external-constructor lookup should normalize qualified receiver spellings");
    assert_eq!(
        qualified_constructor.type_name,
        "dep.variant.NativeHandle".to_string()
    );
    assert_eq!(
        qualified_constructor.provenance,
        FactProvenance::UserOverride
    );

    let qualified_read_mapping = parsed
        .facts
        .external_crate_api_mapping_for_receiver("dep.variant.native_handle", "rawRead", Some(0))
        .expect("external-crate-api lookup should normalize qualified receiver spellings");
    assert_eq!(qualified_read_mapping.rust_name, "raw_read_record");
    assert_eq!(
        qualified_read_mapping.provenance,
        FactProvenance::UserOverride
    );

    let qualified_constructor_mapping = parsed
        .facts
        .external_crate_constructor_mapping_for("dep.variant.native_handle", 1)
        .expect("external-crate-constructor lookup should normalize qualified type spellings");
    assert_eq!(qualified_constructor_mapping.rust_name, "open_qualified");
    assert_eq!(
        qualified_constructor_mapping.provenance,
        FactProvenance::UserOverride
    );

    let unsupported_api = parsed
        .facts
        .unsupported_external_api_for_receiver("NativeHandle", "rawRead")
        .expect("unsupported-api should parse as an unsupported API fact");
    assert_eq!(unsupported_api.pack_name, "native");
    assert_eq!(unsupported_api.reason, "requires runtime codec");
    assert_eq!(unsupported_api.provenance, FactProvenance::UserOverride);

    let qualified_unsupported_api = parsed
        .facts
        .unsupported_external_api_for_receiver("dep.variant.native_handle", "rawDecode")
        .expect("unsupported-api lookup should normalize qualified receiver spellings");
    assert_eq!(
        qualified_unsupported_api.reason,
        "requires qualified runtime codec"
    );
    assert_eq!(
        qualified_unsupported_api.provenance,
        FactProvenance::UserOverride
    );

    let unsupported_constructor = parsed
        .facts
        .unsupported_external_constructor_for("NativeHandle", 2)
        .expect("unsupported-constructor should parse as an unsupported constructor fact");
    assert_eq!(unsupported_constructor.pack_name, "native");
    assert_eq!(unsupported_constructor.reason, "requires native open flags");
    assert_eq!(
        unsupported_constructor.provenance,
        FactProvenance::UserOverride
    );

    let qualified_unsupported_constructor = parsed
        .facts
        .unsupported_external_constructor_for("dep.variant.native_handle", 2)
        .expect("unsupported-constructor lookup should normalize qualified type spellings");
    assert_eq!(
        qualified_unsupported_constructor.reason,
        "requires qualified native open flags"
    );
    assert_eq!(
        qualified_unsupported_constructor.provenance,
        FactProvenance::UserOverride
    );

    let unsupported_field = parsed
        .facts
        .unsupported_external_field_for("NativeHandle", "RAW_HANDLE")
        .expect("unsupported-field should parse as an unsupported field fact");
    assert_eq!(unsupported_field.pack_name, "native");
    assert_eq!(unsupported_field.reason, "exposes native pointer");
    assert_eq!(unsupported_field.provenance, FactProvenance::UserOverride);

    let qualified_unsupported_field = parsed
        .facts
        .unsupported_external_field_for("dep.variant.native_handle", "RAW_HANDLE")
        .expect("unsupported-field lookup should normalize qualified type spellings");
    assert_eq!(
        qualified_unsupported_field.reason,
        "exposes qualified native pointer"
    );
    assert_eq!(
        qualified_unsupported_field.provenance,
        FactProvenance::UserOverride
    );
}

#[test]
fn external_nullable_getter_return_shape_flows_as_owned_optional_carrier() {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getName".to_string(),
        receiver_type: Some("ExternalRecord".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    let mut types = TypeEnv::new();
    types.insert_local(
        "record",
        JavaType::Named {
            name: "ExternalRecord".to_string(),
            args: Vec::new(),
        },
    );

    let actual = external_call_lowered_type(
        Some(&Expr::Var("record".to_string())),
        "getName",
        0,
        &facts,
        &types,
        Some("Owner"),
    )
    .expect("nullable getter should have a lowered return carrier");

    assert_eq!(
        actual,
        LoweredType::from_java(new_java2rust::types::optional_java_type(JavaType::String))
    );
}

#[test]
fn external_facts_parser_ingests_provider_analysis_facts() {
    let parsed = parse_external_facts(
        r#"
method-effect dep.AnalysisTarget summarize read-only definitely-non-null throws=dep.DecodeError,dep.DecodeError,IOException
capture method dep.AnalysisTarget summarize 7 lambda capture Runnable capturedName local String true false
trivial-getter dep.AnalysisTarget getValue value policy=inline-candidate
trivial-setter dep.AnalysisTarget setValue value value fluent=true policy=preserve-api
recursion dep.AnalysisTarget summarize dep.AnalysisTarget summarize direct
control-flow dep.AnalysisTarget summarize tail-recursive-self-call
dispatch-requirement dep.AnalysisTarget needs-subtype-dispatch
catch dep.AnalysisTarget summarize 0 dep.DecodeError,IOException assignable=dep.DecodeError,dep.SpecificDecodeError,dep.SpecificDecodeError
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

    let method_effect = parsed
        .facts
        .method_effect_for("dep.AnalysisTarget", "summarize")
        .expect("method-effect fact should be parsed");
    assert_eq!(
        method_effect.receiver_mutability,
        ReceiverMutabilityFact::ReadOnly
    );
    assert_eq!(
        method_effect.return_nullability,
        NullabilityState::DefinitelyNonNull
    );
    assert_eq!(
        method_effect.thrown,
        vec![
            JavaType::Named {
                name: "DecodeError".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "IOException".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(method_effect.provenance, FactProvenance::UserOverride);
    assert!(
        parsed
            .facts
            .method_effect_for("dep.analysis_target", "summarize")
            .is_some(),
        "method-effect lookup should normalize qualified type spellings"
    );

    let capture_owner = parsed
        .facts
        .captures
        .owners
        .iter()
        .find(|owner| owner.id.0 == 7)
        .expect("capture fact should be parsed");
    assert_eq!(capture_owner.kind, CaptureOwnerKind::Lambda);
    assert_eq!(capture_owner.name.as_deref(), Some("capture"));
    assert_eq!(
        capture_owner.target_type,
        Some(JavaType::Named {
            name: "Runnable".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        capture_owner.origin,
        CaptureOrigin::Method {
            type_name: "dep.AnalysisTarget".to_string(),
            method_name: "summarize".to_string(),
        }
    );
    assert_eq!(capture_owner.captures.len(), 1);
    assert_eq!(capture_owner.captures[0].name, "capturedName");
    assert_eq!(capture_owner.captures[0].source, CaptureSource::Local);
    assert_eq!(capture_owner.captures[0].ty, JavaType::String);
    assert!(capture_owner.captures[0].effectively_final);
    assert!(!capture_owner.captures[0].mutable_after_capture);

    let getter = parsed
        .facts
        .trivial_accessor_for("dep.AnalysisTarget", "getValue")
        .expect("trivial-getter fact should be parsed");
    assert_eq!(
        getter.kind,
        TrivialAccessorKind::Getter {
            field: FactFieldKey {
                type_name: "dep.AnalysisTarget".to_string(),
                field_name: "value".to_string(),
            },
        }
    );
    assert_eq!(getter.inline_policy, AccessorInlinePolicy::InlineCandidate);
    assert_eq!(getter.provenance, FactProvenance::UserOverride);
    assert!(
        parsed
            .facts
            .trivial_accessor_for("dep.analysis_target", "getValue")
            .is_some(),
        "trivial accessor lookup should normalize qualified type spellings"
    );

    let setter = parsed
        .facts
        .trivial_accessor_for("dep.AnalysisTarget", "setValue")
        .expect("trivial-setter fact should be parsed");
    assert_eq!(
        setter.kind,
        TrivialAccessorKind::Setter {
            field: FactFieldKey {
                type_name: "dep.AnalysisTarget".to_string(),
                field_name: "value".to_string(),
            },
            param_name: "value".to_string(),
            fluent: true,
        }
    );
    assert_eq!(setter.inline_policy, AccessorInlinePolicy::PreserveApi);
    assert_eq!(setter.provenance, FactProvenance::UserOverride);

    let recursion = parsed
        .facts
        .recursion_facts_for("dep.AnalysisTarget", "summarize");
    assert_eq!(recursion.len(), 1);
    assert_eq!(recursion[0].calls.type_name, "dep.AnalysisTarget");
    assert_eq!(recursion[0].calls.method_name, "summarize");
    assert_eq!(recursion[0].kind, RecursionKind::Direct);
    assert_eq!(recursion[0].provenance, FactProvenance::UserOverride);
    assert_eq!(
        parsed
            .facts
            .recursion_facts_for("dep.analysis_target", "summarize")
            .len(),
        1,
        "recursion fact lookup should normalize qualified type spellings"
    );

    assert!(parsed.facts.has_control_flow_fact(
        "dep.AnalysisTarget",
        "summarize",
        ControlFlowFactKind::TailRecursiveSelfCall
    ));
    assert!(parsed.facts.has_control_flow_fact(
        "dep.analysis_target",
        "summarize",
        ControlFlowFactKind::TailRecursiveSelfCall
    ));
    assert!(parsed.facts.control_flow_facts.iter().any(|fact| {
        fact.method.type_name == "dep.AnalysisTarget"
            && fact.method.method_name == "summarize"
            && fact.provenance == FactProvenance::UserOverride
    }));
    assert_eq!(
        parsed
            .facts
            .dispatch_requirements_for("dep.analysis_target"),
        vec![DispatchRequirementKind::NeedsSubtypeDispatch],
        "dispatch requirement lookup should normalize qualified type spellings"
    );
    assert_eq!(
        parsed
            .facts
            .dispatch_requirement_facts_for("dep.analysis_target")
            .len(),
        1
    );

    let catches = parsed
        .facts
        .catch_facts_for("dep.AnalysisTarget", "summarize");
    assert_eq!(catches.len(), 1);
    assert_eq!(catches[0].clause_index, 0);
    assert_eq!(
        catches[0].types,
        vec![
            JavaType::Named {
                name: "DecodeError".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "IOException".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(
        catches[0].assignable_from,
        vec![
            JavaType::Named {
                name: "DecodeError".to_string(),
                args: Vec::new(),
            },
            JavaType::Named {
                name: "SpecificDecodeError".to_string(),
                args: Vec::new(),
            },
        ]
    );
    assert_eq!(catches[0].provenance, FactProvenance::UserOverride);
    assert_eq!(
        parsed
            .facts
            .catch_facts_for("dep.analysis_target", "summarize")
            .len(),
        1,
        "catch fact lookup should normalize qualified type spellings"
    );
}

#[test]
fn external_facts_parser_ingests_field_nullability_overrides() {
    let parsed = parse_external_facts(
        r#"
field-nullability User name definitely-non-null
field-nullability Cache.value maybe-null
field-nullability dep.AnalysisTarget RAW_VALUE definitely-non-null
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(
        parsed.facts.field_nullability_for("User", "name"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert_eq!(
        parsed.facts.field_nullability_for("Cache", "value"),
        Some(NullabilityState::MaybeNull)
    );
    assert_eq!(
        parsed
            .facts
            .field_nullability_for("dep.analysis_target", "RAW_VALUE"),
        Some(NullabilityState::DefinitelyNonNull),
        "field-nullability lookup should normalize qualified type spellings"
    );
    assert!(parsed.facts.field_nullability_facts.iter().any(|fact| {
        fact.field.type_name == "User"
            && fact.field.field_name == "name"
            && fact.provenance == FieldNullabilityProvenance::ExternalOverride
    }));
}

#[test]
fn external_object_safety_violation_facts_drive_representation_fallback_diagnostics() {
    let parsed = parse_java_source(
        r#"
interface Pet {
    void accept(String value);
}

class Dog implements Pet {
    public void accept(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let external = parse_external_facts(
        r#"
object-safety-violation Pet self-param accept value
"#,
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Pet".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "non_object_safe_trait_fallback"
                && diagnostic.message.contains("interface Pet")
                && diagnostic
                    .message
                    .contains("method accept parameter value mentions the interface type")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn external_pack_manifest_object_safety_facts_drive_representation_fallback() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack dep.runtime dep.runtime external-fact-model semantic
        external-pack-object dep.runtime dep.runtime.GenericFactory dyn-trait GenericFactoryTrait
        external-pack-object-safety dep.runtime dep.runtime.GenericFactory nested-self-return copyAll
        external-pack-object-safety dep.runtime dep.runtime.GenericFactory nested-self-param acceptAll values
        "#,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed.diagnostics
    );

    let policy = object_representation_policy(
        ObjectRepresentationRequest::DynTraitOverride {
            type_name: "dep.runtime.GenericFactory".to_string(),
            trait_name: "GenericFactoryTrait".to_string(),
        },
        &parsed.facts,
    );

    assert_eq!(policy.representation, ObjectRepresentation::ErasedObject);
    assert_eq!(
        policy.object_safety,
        ObjectSafetyPolicy::RejectedNonObjectSafe
    );
    assert!(
        policy.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "non_object_safe_trait_fallback"
                && diagnostic
                    .message
                    .contains("dyn trait representation for dep.runtime.GenericFactory")
                && diagnostic
                    .message
                    .contains("method copyAll returns a type containing the interface type")
        }),
        "{:?}",
        policy.diagnostics
    );
    let safety_fact = parsed
        .facts
        .object_safety_fact_for_name("dep.runtime.GenericFactory")
        .expect("manifest should emit an object-safety fact");
    assert_eq!(safety_fact.provenance, FactProvenance::ExternalLibraryPack);
    assert_eq!(
        parsed
            .facts
            .object_safety_violation_for_name("dep.runtime.GenericFactory"),
        Some(ObjectSafetyViolation::NestedSelfReferentialReturn {
            method_name: "copyAll".to_string(),
        })
    );
}

#[test]
fn external_capability_facts_drive_existing_shape_policy() {
    let parsed = parse_external_facts(
        r#"
copy ExternalId
sequence-like ExternalIds ExternalId
"#,
    );
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let mut env = TypeEnv::new();
    env.insert_local("ids", external_ids);

    let shape =
        infer_stream_source_element_shape_with_facts(&parse_expr("ids"), &env, &parsed.facts)
            .unwrap();

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert_eq!(
        shape.java_ty,
        JavaType::Named {
            name: "ExternalId".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn external_facts_parser_reports_invalid_lines() {
    let parsed = parse_external_facts(
        r#"
closed Expr maybe
copy ExternalId maybe
dispatch-requirement Expr maybe
method-effect Expr call maybe unknown
capture method Expr call nope lambda capture Runnable value local String true false
capture method Expr call 1 maybe capture Runnable value local String true false
capture method Expr call 1 lambda capture Runnable value maybe String true false
capture method Expr call 1 lambda capture Runnable value local String maybe false
trivial-getter Expr call value policy=maybe
trivial-setter Expr call value value fluent=maybe
recursion Expr call Expr call maybe
control-flow Expr call maybe
catch Expr call nope Exception
unknown Fact
"#,
    );

    assert!(parsed.diagnostics.has_code("external_fact_parse_error"));
    assert_eq!(parsed.diagnostics.len(), 14);
}

#[test]
fn conversion_plan_owns_cast_and_instanceof_shape_pairing_before_lowering() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Animal".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);
    let ctx = ExprShapeContext {
        signatures: None,
        representations: Some(&representations),
        facts: None,
    };
    let fact_ctx = ExprShapeContext {
        signatures: None,
        representations: Some(&representations),
        facts: Some(&facts),
    };
    let mut env = ShapeEnv::new();
    env.insert_local(
        "dog",
        JavaType::Named {
            name: "Dog".to_string(),
            args: Vec::new(),
        },
    );
    let animal_ty = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    env.insert_local_shape(
        "animal",
        value_shape_for_java_type(&representations, animal_ty.clone()),
    );
    let cast_plan = plan_cast_conversion(&parse_expr("dog"), &animal_ty, &env, ctx);
    assert_eq!(
        cast_plan.target.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "AnimalValue".to_string()
        }
    );
    assert_eq!(
        cast_plan.coercion.coercions(),
        vec![Coercion::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }]
    );
    assert_eq!(
        cast_plan.semantics,
        JavaCastSemanticsPolicy::ReferenceUpcast
    );
    assert_eq!(
        apply_cast_conversion(RustExpr::Var("dog".to_string()), &cast_plan),
        RustExpr::Raw("AnimalValue::Dog(dog)".to_string())
    );

    let dog_ty = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };
    let instanceof_plan = plan_instanceof(&parse_expr("animal"), &dog_ty, &env, ctx);
    assert_eq!(
        instanceof_plan.check,
        InstanceOfCheck::ClosedEnumVariant {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }
    );
    assert_eq!(
        apply_instanceof(RustExpr::Var("animal".to_string()), &instanceof_plan),
        RustExpr::Raw("matches!(animal, AnimalValue::Dog(_))".to_string())
    );

    let interface_instanceof_plan = plan_instanceof(&parse_expr("dog"), &animal_ty, &env, fact_ctx);
    assert_eq!(interface_instanceof_plan.check, InstanceOfCheck::AlwaysTrue);
}

#[test]
fn conversion_plan_does_not_render_rust_expressions() {
    let source = include_str!("../src/conversion_plan.rs");
    let lowering_source = include_str!("../src/conversion_lowering.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("apply_expr_to_shape"));
    assert!(!source.contains("apply_instanceof_check"));
    assert!(source.contains("pub struct ShapeCoercionPlan"));
    assert!(source.contains("pub struct SyntaxCoercionPlan"));
    assert!(source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(source.contains("pub coercion: ShapeCoercionPlan"));
    assert!(source.contains("pub check: InstanceOfCheck"));
    assert!(source.contains("pub enum LoweredType"));
    assert!(source.contains("pub enum SlotConversionPlan"));
    assert!(source.contains("pub struct SlotExprConversionPlan"));
    assert!(source.contains("pub struct LoweredSlotExprConversionPlan"));
    assert!(source.contains("pub conversion: SlotConversionPlan"));
    assert!(source.contains("pub fn actual_shape_for_slot_expr"));
    assert!(source.contains("pub fn plan_slot_conversion"));
    assert!(source.contains("pub fn plan_lowered_slot_conversion"));
    assert!(source.contains("shape_coercion_plan_with_facts"));
    assert!(source.contains("plan_instanceof_check"));
    assert!(source.contains("slot_conversion_plan_with_facts"));
    assert!(source.contains("lowered_slot_conversion_plan_with_facts"));
    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, &plan.coercion)"));
    assert!(lowering_source.contains("render_instanceof_check(expr, &plan.check)"));
    assert!(lowering_source.contains("pub fn apply_slot_expr_conversion"));
    assert!(lowering_source.contains("pub fn apply_lowered_slot_expr_conversion"));
    assert!(!lowering_source.contains("FactSet"));
    assert!(!lowering_source.contains("plan_slot_conversion"));
    assert!(!lowering_source.contains("plan_lowered_slot_conversion"));
    assert!(!lowering_source.contains("slot_conversion_plan_with_facts"));
    assert!(!lowering_source.contains("lowered_slot_conversion_plan_with_facts"));
    assert!(!lowering_source.contains("apply_expr_to_shape_with_facts"));
    assert!(!lowering_source.contains("plan_instanceof_check"));

    let coercion_source = include_str!("../src/coercion.rs");
    assert!(!coercion_source.contains("pub enum LoweredType"));
    assert!(!coercion_source.contains("pub fn actual_shape_for_slot_expr"));
}

#[test]
fn conversion_plan_uses_target_slot_shape_for_unresolved_unknown_calls() {
    let unresolved_call = Expr::Call {
        receiver: None,
        name: "defaultNames".to_string(),
        args: Vec::new(),
        resolved: None,
    };
    let target_shape = RustSlotShape::for_storage(JavaType::Named {
        name: "Set".to_string(),
        args: vec![JavaType::String],
    });
    assert_eq!(
        actual_shape_for_slot_expr_with_slot(
            &unresolved_call,
            &ShapeEnv::new(),
            ExprShapeContext::default(),
            &argument_slot(target_shape.clone()),
        ),
        target_shape
    );

    let explicit_cast = Expr::Cast {
        ty: JavaType::String,
        expr: Box::new(unresolved_call),
    };
    assert_ne!(
        actual_shape_for_slot_expr_with_slot(
            &explicit_cast,
            &ShapeEnv::new(),
            ExprShapeContext::default(),
            &argument_slot(target_shape.clone()),
        ),
        target_shape
    );
}

#[test]
fn lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts() {
    let boundary = cast_and_instanceof_planning_boundary();
    assert_eq!(
        boundary,
        CastAndInstanceOfPlanningBoundary {
            casts_use_representation_shapes: true,
            instanceof_uses_representation_shapes: true,
            hierarchy_reasoning_before_lowering: true,
            rendering_uses_conversion_plans_only: true,
        }
    );

    let lower_source = include_str!("../src/lower.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let lowering_source = include_str!("../src/conversion_lowering.rs");

    assert!(conversion_source.contains("pub struct CastAndInstanceOfPlanningBoundary"));
    assert!(conversion_source.contains("pub fn cast_and_instanceof_planning_boundary"));
    assert!(conversion_source.contains("project_java_value_shape(ctx.representations"));
    assert!(conversion_source.contains("ctx.facts.unwrap_or"));
    assert!(
        conversion_source.contains("plan_instanceof_check_with_facts(&actual, &target, facts)")
    );
    assert!(
        conversion_source
            .contains("java_cast_semantics_policy_with_facts(&actual, &target, &coercion, facts)")
    );

    assert!(lower_source.contains("plan_cast_conversion("));
    assert!(lower_source.contains("apply_cast_conversion("));
    assert!(lower_source.contains("plan_instanceof("));
    assert!(lower_source.contains("apply_instanceof("));
    assert!(lower_source.contains("lower_expr_with_refinements(expr, env, ctx, refinements)"));
    assert!(lower_source.contains("representations: Some(ctx.representations)"));
    assert!(lower_source.contains("facts: Some(ctx.facts)"));
    assert!(!lower_source.contains("ObjectRepresentation"));
    assert!(!lower_source.contains("representation_for("));
    assert!(!lower_source.contains("dispatch_for("));
    assert!(!lower_source.contains("HierarchyStrategy"));
    assert!(!lower_source.contains("hierarchy_subtype_policy"));
    assert!(!lower_source.contains("reachable_hierarchy_edges"));
    assert!(!lower_source.contains("type_capability_overrides"));
    assert!(!lower_source.contains("instanceof_target_policy"));
    assert!(!lower_source.contains("plan_instanceof_check"));
    assert!(!lower_source.contains("JavaCastSemanticsPolicy"));
    assert!(!lower_source.contains("java_cast_semantics_policy"));
    assert!(!lower_source.contains("InvalidCastPolicy"));

    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, &plan.coercion)"));
    assert!(lowering_source.contains("render_instanceof_check(expr, &plan.check)"));
    assert!(!lowering_source.contains("ObjectRepresentation"));
    assert!(!lowering_source.contains("HierarchyStrategy"));
    assert!(!lowering_source.contains("hierarchy_subtype_policy"));
    assert!(!lowering_source.contains("java_cast_semantics_policy"));
}

#[test]
fn signature_plan_owns_field_param_and_return_shapes_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Holder {
    String name;
    Holder(String name) { this.name = name; }
    public String echo(String value) { return value; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();

    let field_shape = signatures.field_shape("Holder", "name").unwrap();
    assert_eq!(field_shape.java_ty, JavaType::String);
    assert_eq!(field_shape.mutability, MutabilityShape::Mutable);
    assert_eq!(
        field_shape_constraints(),
        vec![SignatureConstraint::StorageSlot]
    );
    assert!(signatures.field_shape("Holder", "missing").is_none());

    let method_signature = signatures.method_signature(ty, method).unwrap();
    assert_eq!(method_signature.params[0].java_ty, JavaType::String);
    assert_eq!(
        signatures.method_param_plans(ty, method).unwrap()[0].java_ty,
        JavaType::String
    );
    assert_eq!(
        signatures.constructor_param_plans(ty, ctor).unwrap()[0].java_ty,
        JavaType::String
    );
    assert_eq!(
        method_signature.return_shape.as_ref().unwrap().java_ty,
        JavaType::String
    );
}

#[test]
fn signature_plan_does_not_render_rust_types() {
    let source = include_str!("../src/signature.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustType"));
    assert!(!source.contains("slot_render"));
    assert!(!source.contains("rust_type_for_slot"));
}

#[test]
fn signature_solver_collects_param_constraints_before_applying_shapes() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub enum SignatureConstraint"));
    assert!(source.contains("RequiredOwnership"));
    assert!(source.contains("RequiredView"));
    assert!(source.contains("RequiredDispatch"));
    assert!(source.contains("pub enum SignatureSlotRole"));
    assert!(source.contains("pub struct SignatureCapabilityPolicy"));
    assert!(source.contains("pub struct SignatureShapeRequest"));
    assert!(source.contains("pub struct MergedSignatureConstraints"));
    assert!(source.contains("pub struct SignatureConstraintConflict"));
    assert!(source.contains("pub enum SignatureConstraintAxis"));
    assert!(source.contains("from_constraints"));
    assert!(source.contains("apply_to_shape"));
    assert!(source.contains("requires_mutable_binding"));
    assert!(source.contains("conflicts"));
    assert!(source.contains("pub struct SignatureConstraintState"));
    assert!(source.contains("pub struct SignatureFixpointResult"));
    assert!(source.contains("pub struct SignatureSaturationReport"));
    assert!(source.contains("pub fn signature_saturation_report"));
    assert!(source.contains("pub fn signature_constraint_state_conflict_diagnostics"));
    assert!(source.contains("pub struct MethodConstraintSet"));
    assert!(source.contains("pub struct ConstructorConstraintSet"));
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    assert!(source.contains("public_api: signature_member_api_boundary_policy(&ctor.modifiers)"));
    assert!(source.contains("pub struct LocalConstraintSet"));
    assert!(source.contains("java_ty: JavaType"));
    assert!(source.contains("pub enum ReceiverConstraint"));
    assert!(source.contains("pub enum SignatureConstraintPropagationKind"));
    assert!(source.contains("pub struct SignatureConstraintPropagationPolicy"));
    assert!(source.contains("signature_shape_constraint_propagation_policy"));
    assert!(source.contains("signature_slot_constraint_propagation_policy"));
    assert!(source.contains("collect_signature_constraints"));
    assert!(source.contains("solve_signature_constraints_to_fixpoint"));
    assert!(source.contains("solve_signature_constraints_to_fixpoint_result"));
    assert!(source.contains("signature_fixpoint_saturation_policy"));
    assert!(source.contains("signature_fixpoint_iteration_budget"));
    assert!(source.contains("FiniteSaturationPolicy"));
    assert!(source.contains("SaturationStep"));
    assert!(source.contains("signature_capability_policy"));
    assert!(source.contains("capability_diagnostics_for_signature_constraints"));
    assert!(source.contains("signature_clone_recovery_requires_audit"));
    assert!(!source.contains("capability_baseline_constraints"));
    assert!(source.contains("propagate_call_site_constraints"));
    assert!(source.contains("propagate_constructor_call_site_constraints"));
    assert!(source.contains("propagate_call_argument_constraints_to_callees"));
    assert!(source.contains("propagate_constructor_call_argument_constraints_to_callees"));
    assert!(source.contains("propagate_call_result_constraints_to_bindings"));
    assert!(source.contains("propagate_constructor_call_result_constraints_to_bindings"));
    assert!(source.contains("propagate_call_result_constraints_to_fields"));
    assert!(source.contains("propagate_constructor_call_result_constraints_to_fields"));
    assert!(source.contains("propagate_method_local_alias_constraints"));
    assert!(source.contains("propagate_method_local_alias_source_constraints"));
    assert!(source.contains("propagate_constructor_local_alias_constraints"));
    assert!(source.contains("propagate_constructor_local_alias_source_constraints"));
    assert!(source.contains("propagate_method_field_storage_constraints"));
    assert!(source.contains("propagate_constructor_field_storage_constraints"));
    assert!(source.contains("propagate_method_return_binding_constraints"));
    assert!(source.contains("propagate_method_binding_return_constraints"));
    assert!(source.contains("add_method_param_constraint"));
    assert!(source.contains("add_method_return_constraint"));
    assert!(source.contains("method_param_shape_constraints"));
    assert!(source.contains("constructor_param_shape_constraints"));
    assert!(source.contains("method_return_shape_constraints"));
    assert!(source.contains("local_shape_constraints"));
    assert!(source.contains("field_shape_constraints"));
    assert!(source.contains("field_shape_constraints_with_facts"));
    assert!(source.contains("signature_member_api_boundary_policy"));
    assert!(!source.contains("fn member_is_api_boundary"));
    assert!(source.contains("receiver_constraints"));
    assert!(source.contains("solve_signature_shape"));
    assert!(source.contains("apply_signature_constraints"));
    assert_eq!(source.matches("param_shape_for_java_type").count(), 2);
    assert_eq!(source.matches("return_shape_for_java_type").count(), 2);
    assert_eq!(source.matches("storage_shape_for_java_type").count(), 3);

    let slot_policy_source = include_str!("../src/slot_policy.rs");
    assert!(!slot_policy_source.contains("| CallArgumentBoundary::InternalSameType"));

    let fact_saturation_source = include_str!("../src/fact_saturation.rs");
    assert!(fact_saturation_source.contains("pub struct FiniteFactDomain"));
    assert!(fact_saturation_source.contains("pub struct FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("pub fn finite_fact_iteration_budget"));
    assert!(fact_saturation_source.contains("pub enum SaturationStep"));
    assert!(fact_saturation_source.contains("pub struct FiniteSaturationResult"));
    assert!(fact_saturation_source.contains("pub fn saturate_finite"));
}

#[test]
fn signature_constraint_propagation_policy_separates_shape_from_storage_edges() {
    assert_eq!(
        signature_shape_constraint_propagation_policy(),
        SignatureConstraintPropagationPolicy {
            kind: SignatureConstraintPropagationKind::ShapeOnly,
        }
    );
    assert_eq!(
        signature_slot_constraint_propagation_policy(),
        SignatureConstraintPropagationPolicy {
            kind: SignatureConstraintPropagationKind::SlotShapeAndStorage,
        }
    );

    let ownership = SignatureConstraint::RequiredOwnership(OwnershipShape::Owned);
    let nullability = SignatureConstraint::RequiredStorageNullability(NullabilityState::MaybeNull);
    let storage = SignatureConstraint::StorageSlot;
    let mutable = SignatureConstraint::MutableBinding;

    assert!(signature_shape_constraint_propagation_policy().allows(&ownership));
    assert!(signature_shape_constraint_propagation_policy().allows(&nullability));
    assert!(!signature_shape_constraint_propagation_policy().allows(&storage));
    assert!(!signature_shape_constraint_propagation_policy().allows(&mutable));
    assert!(signature_slot_constraint_propagation_policy().allows(&ownership));
    assert!(signature_slot_constraint_propagation_policy().allows(&nullability));
    assert!(signature_slot_constraint_propagation_policy().allows(&storage));
    assert!(!signature_slot_constraint_propagation_policy().allows(&mutable));
}

#[test]
fn signature_solver_runs_explicit_constraint_fixpoint_before_shape_building() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int outer(String value) {
        return inner(value);
    }

    private int inner(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let (solved_constraints, iterations) =
        solve_signature_constraints_to_fixpoint(constraints.clone(), &usage);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let outer_key = new_java2rust::signature::MethodKey {
        type_name: "StringUse".to_string(),
        rust_name: "outer".to_string(),
    };

    assert_eq!(iterations, 2);
    assert_eq!(signatures.solve_iterations(), 2);
    assert!(constraints.methods.get(&outer_key).unwrap().public_api == false);
    assert!(
        constraints
            .methods
            .get(&outer_key)
            .unwrap()
            .params
            .first()
            .unwrap()
            .is_empty()
    );
    assert!(
        solved_constraints
            .methods
            .get(&outer_key)
            .unwrap()
            .params
            .first()
            .unwrap()
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Str)
            ))
    );
    assert!(solved_constraints.methods.values().any(|constraints| {
        constraints.params.iter().flatten().any(|constraint| {
            matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
            )
        })
    }));
}

#[test]
fn signature_shape_request_centralizes_role_base_shape_and_constraints() {
    let parsed = parse_java_source("class ShapeOwner {}");
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    let param_shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Array(Box::new(JavaType::String)),
            role: SignatureSlotRole::Param { is_varargs: false },
            constraints: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(param_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(param_shape.view, ViewShape::Slice);

    let storage_shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::String,
            role: SignatureSlotRole::Storage,
            constraints: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(storage_shape.ownership, OwnershipShape::Owned);
    assert_eq!(storage_shape.mutability, MutabilityShape::Mutable);

    assert_eq!(
        solve_signature_shape(
            &representations,
            SignatureShapeRequest {
                java_ty: JavaType::Void,
                role: SignatureSlotRole::Return,
                constraints: Vec::new(),
            },
        ),
        None
    );

    let borrowed_return = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::String,
            role: SignatureSlotRole::Return,
            constraints: signature_constraints_for_borrowed_content_policy(
                BorrowedContentPolicy::string_like_view(),
            ),
        },
    )
    .unwrap();
    assert_eq!(borrowed_return, borrowed_string_content_shape());

    let enum_dispatch = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            },
            role: SignatureSlotRole::Param { is_varargs: false },
            constraints: vec![SignatureConstraint::RequiredDispatch(
                DispatchShape::ClosedEnum {
                    enum_name: "AnimalValue".to_string(),
                },
            )],
        },
    )
    .unwrap();
    assert_eq!(
        enum_dispatch.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "AnimalValue".to_string()
        }
    );
}

#[test]
fn signature_slot_dispatch_requirements_project_through_representation_dispatch() {
    let animal = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
            provenance: FactProvenance::ParsedSource,
        });
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "Animal".to_string(),
            requirement: DispatchRequirementKind::NeedsBorrowedTraitView,
            provenance: FactProvenance::ParsedSource,
        });
    let source_dispatch = DispatchShape::DynTrait {
        trait_name: "AnimalApi".to_string(),
    };
    let effective_facts = EffectiveFacts::new(&facts);

    let storage_policy = signature_slot_dispatch_requirement_policy(
        &animal,
        SignatureSlotRole::Storage,
        source_dispatch.clone(),
        &facts,
    );
    let return_policy = signature_slot_dispatch_requirement_policy_with_effective_facts(
        &animal,
        SignatureSlotRole::Return,
        source_dispatch.clone(),
        &effective_facts,
    );
    let param_policy = signature_slot_dispatch_requirement_policy(
        &animal,
        SignatureSlotRole::Param { is_varargs: false },
        source_dispatch.clone(),
        &facts,
    );

    assert_eq!(
        storage_policy.constraints,
        vec![SignatureConstraint::RequiredDispatch(
            DispatchShape::BoxedDynTrait {
                trait_name: "AnimalApi".to_string(),
            }
        )]
    );
    assert_eq!(return_policy.constraints, storage_policy.constraints);
    assert_eq!(
        param_policy.constraints,
        vec![SignatureConstraint::RequiredDispatch(source_dispatch)]
    );

    let signature_source = include_str!("../src/signature.rs");
    assert!(signature_source.contains("dispatch_requirement_projection_policy"));
    assert!(signature_source.contains("fn dispatch_requirement_projection_role"));
    assert!(!signature_source.contains(
        "(DispatchRequirementKind::NeedsOwnedPolymorphicStorage, SignatureSlotRole::Storage)"
    ));
}

#[test]
fn signature_solver_consumes_slot_dispatch_requirement_facts_without_rechoosing_object_strategy() {
    let parsed = parse_java_source(
        r#"
class ExternalThing {}
class Use {
    ExternalThing value;

    ExternalThing id(ExternalThing input) {
        return input;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .dispatch_requirement_facts
        .push(DispatchRequirementFact {
            type_name: "ExternalThing".to_string(),
            requirement: DispatchRequirementKind::NeedsUnknownRuntimeObject,
            provenance: FactProvenance::CorpusAnalysis,
        });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let use_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let id_method = use_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "id" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "ExternalThing".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::Concrete
    );
    assert_eq!(
        signatures.field_shape("Use", "value").unwrap().dispatch,
        DispatchShape::ErasedObject
    );
    assert_eq!(
        signatures.method_param_plans(use_ty, id_method).unwrap()[0]
            .shape
            .dispatch,
        DispatchShape::ErasedObject
    );
    assert_eq!(
        signatures
            .method_signature(use_ty, id_method)
            .unwrap()
            .return_shape
            .as_ref()
            .unwrap()
            .dispatch,
        DispatchShape::ErasedObject
    );

    let direct_shape = solve_signature_shape_with_facts(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Named {
                name: "ExternalThing".to_string(),
                args: Vec::new(),
            },
            role: SignatureSlotRole::Storage,
            constraints: Vec::new(),
        },
        &facts,
    )
    .unwrap();
    assert_eq!(direct_shape.dispatch, DispatchShape::ErasedObject);
}

#[test]
fn signature_constraint_merge_is_order_independent_for_boundary_fallbacks() {
    let mut conservative_then_borrowed =
        conservative_boundary_constraints(&JavaType::String, &FactSet::default());
    conservative_then_borrowed.extend(signature_constraints_for_borrowed_content_policy(
        BorrowedContentPolicy::string_like_view(),
    ));
    let mut borrowed_then_conservative = signature_constraints_for_borrowed_content_policy(
        BorrowedContentPolicy::string_like_view(),
    );
    borrowed_then_conservative.extend(conservative_boundary_constraints(
        &JavaType::String,
        &FactSet::default(),
    ));

    let conservative_first = apply_signature_constraints(
        RustSlotShape::conservative(JavaType::String),
        &conservative_then_borrowed,
    );
    let borrowed_first = apply_signature_constraints(
        RustSlotShape::conservative(JavaType::String),
        &borrowed_then_conservative,
    );

    assert_eq!(conservative_first, borrowed_first);
    assert_eq!(conservative_first.ownership, OwnershipShape::Owned);
    assert_eq!(conservative_first.view, ViewShape::Native);
    assert_eq!(conservative_first.dispatch, DispatchShape::Concrete);
}

#[test]
fn signature_requirement_merge_policy_owns_axis_priorities() {
    assert_eq!(
        signature_ownership_requirement_priority_policy(OwnershipShape::Owned),
        SignatureOwnershipRequirementPriorityPolicy {
            requirement: OwnershipShape::Owned,
            rank: 40,
        }
    );
    assert_eq!(
        signature_view_requirement_priority_policy(ViewShape::Native),
        SignatureViewRequirementPriorityPolicy {
            requirement: ViewShape::Native,
            rank: 50,
        }
    );
    assert_eq!(
        signature_dispatch_requirement_priority_policy(DispatchShape::ErasedObject),
        SignatureDispatchRequirementPriorityPolicy {
            requirement: DispatchShape::ErasedObject,
            rank: 40,
        }
    );
    assert_eq!(
        merge_signature_flag_requirement_policy(false, true),
        SignatureFlagMerge {
            kept: true,
            changed: true,
        }
    );
    assert_eq!(
        merge_signature_flag_requirement_policy(true, true),
        SignatureFlagMerge {
            kept: true,
            changed: false,
        }
    );
    assert_eq!(
        merge_ownership_requirement_policy(
            Some(OwnershipShape::SharedBorrow),
            OwnershipShape::Owned,
        ),
        SignatureRequirementMerge {
            kept: OwnershipShape::Owned,
            rejected: Some(OwnershipShape::SharedBorrow),
        }
    );
    assert_eq!(
        merge_ownership_requirement_policy(
            Some(OwnershipShape::CopyValue),
            OwnershipShape::CopyValue,
        ),
        SignatureRequirementMerge {
            kept: OwnershipShape::CopyValue,
            rejected: None,
        }
    );
    assert_eq!(
        merge_view_requirement_policy(Some(ViewShape::Str), ViewShape::Native),
        SignatureRequirementMerge {
            kept: ViewShape::Native,
            rejected: Some(ViewShape::Str),
        }
    );
    assert_eq!(
        merge_dispatch_requirement_policy(
            Some(DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            }),
            DispatchShape::ErasedObject,
        ),
        SignatureRequirementMerge {
            kept: DispatchShape::ErasedObject,
            rejected: Some(DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            }),
        }
    );

    let source = include_str!("../src/signature.rs");
    assert!(source.contains("pub struct SignatureRequirementMerge"));
    assert!(source.contains("pub struct SignatureOwnershipRequirementPriorityPolicy"));
    assert!(source.contains("pub fn signature_ownership_requirement_priority_policy"));
    assert!(source.contains("pub struct SignatureViewRequirementPriorityPolicy"));
    assert!(source.contains("pub fn signature_view_requirement_priority_policy"));
    assert!(source.contains("pub struct SignatureDispatchRequirementPriorityPolicy"));
    assert!(source.contains("pub fn signature_dispatch_requirement_priority_policy"));
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    assert!(source.contains("pub fn merge_ownership_requirement_policy"));
    assert!(source.contains("pub fn merge_view_requirement_policy"));
    assert!(source.contains("pub fn merge_dispatch_requirement_policy"));
    assert!(source.contains("join_ranked_lattice"));
    assert!(source.contains("ownership_lattice_rank"));
    assert!(!source.contains("fn ownership_requirement_rank"));
    assert!(!source.contains("fn view_requirement_rank"));
    assert!(!source.contains("fn dispatch_requirement_rank"));
    assert!(!source.contains("fn storage_nullability_requirement_rank"));
}

#[test]
fn merged_signature_constraints_own_marker_policy() {
    let merged = MergedSignatureConstraints::from_constraints(&[
        SignatureConstraint::MutableBinding,
        SignatureConstraint::StorageSlot,
    ]);

    assert!(merged.requires_mutable_binding());
    assert!(merged.requires_storage_slot());
}

#[test]
fn merged_signature_constraints_record_conflicting_axis_requirements() {
    let mut constraints = conservative_boundary_constraints(&JavaType::String, &FactSet::default());
    constraints.extend(signature_constraints_for_borrowed_content_policy(
        BorrowedContentPolicy::string_like_view(),
    ));

    let merged = MergedSignatureConstraints::from_constraints(&constraints);

    assert!(merged.conflicts().iter().any(|conflict| {
        conflict.axis == SignatureConstraintAxis::Owner
            && conflict.kept == SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            && conflict.rejected
                == SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
    }));
    assert!(merged.conflicts().iter().any(|conflict| {
        conflict.axis == SignatureConstraintAxis::View
            && conflict.kept == SignatureConstraint::RequiredView(ViewShape::Native)
            && conflict.rejected == SignatureConstraint::RequiredView(ViewShape::Str)
    }));
}

#[test]
fn signature_constraint_conflict_diagnostics_explain_lattice_join_provenance() {
    assert_eq!(
        signature_constraint_provenance(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned
        )),
        SignatureConstraintProvenance::OwnedRequirement
    );
    assert_eq!(
        signature_constraint_provenance_label(SignatureConstraintProvenance::OwnedRequirement),
        "owned"
    );

    let constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull),
        SignatureConstraint::RequiredStorageNullability(NullabilityState::MaybeNull),
        SignatureConstraint::RequiredDispatch(DispatchShape::DynTrait {
            trait_name: "Animal".to_string(),
        }),
        SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject),
        SignatureConstraint::RequiredView(ViewShape::Str),
        SignatureConstraint::RequiredView(ViewShape::ErasedObjectView),
    ];

    let diagnostics =
        MergedSignatureConstraints::from_constraints(&constraints).conflict_diagnostics();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.axis == SignatureConstraintAxis::Owner
            && diagnostic.kept_provenance == SignatureConstraintProvenance::OwnedRequirement
            && diagnostic.rejected_provenance == SignatureConstraintProvenance::BorrowedRequirement
            && diagnostic.message.contains("owned")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.axis == SignatureConstraintAxis::Nullability
            && diagnostic.kept_provenance
                == SignatureConstraintProvenance::OptionalStorageRequirement
            && diagnostic.message.contains("Option")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.axis == SignatureConstraintAxis::Dispatch
            && diagnostic.kept_provenance == SignatureConstraintProvenance::ErasedObjectRequirement
            && diagnostic.rejected_provenance == SignatureConstraintProvenance::DynTraitRequirement
            && diagnostic.message.contains("erased object")
            && diagnostic.message.contains("Box<dyn Trait>")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.axis == SignatureConstraintAxis::View
            && diagnostic.kept_provenance == SignatureConstraintProvenance::ErasedObjectRequirement
            && diagnostic.message.contains("erased object")
    }));
}

#[test]
fn signature_constraint_conflict_diagnostic_is_public_and_structured() {
    let conflict = new_java2rust::signature::SignatureConstraintConflict {
        axis: SignatureConstraintAxis::Owner,
        kept: SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        rejected: SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    };

    assert_eq!(
        signature_constraint_conflict_diagnostic(&conflict),
        SignatureConstraintConflictDiagnostic {
            axis: SignatureConstraintAxis::Owner,
            kept: SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            rejected: SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
            kept_provenance: SignatureConstraintProvenance::OwnedRequirement,
            rejected_provenance: SignatureConstraintProvenance::BorrowedRequirement,
            message: "Owner join kept RequiredOwnership(Owned) because it requires owned; rejected RequiredOwnership(SharedBorrow) from borrowed view"
                .to_string(),
        }
    );
}

#[test]
fn signature_saturation_report_summarizes_iterations_conflicts_and_fallback_reasons() {
    let method_key = MethodKey {
        type_name: "ConflictFlow".to_string(),
        rust_name: "entry".to_string(),
    };
    let mut constraints = SignatureConstraintState::default();
    constraints.methods.insert(
        method_key,
        MethodConstraintSet {
            return_constraints: vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Str),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
            ..MethodConstraintSet::default()
        },
    );
    let usage = new_java2rust::usage::UsageFacts::default();
    let solved = solve_signature_constraints_to_fixpoint_result(constraints.clone(), &usage);
    let conflict_diagnostics = signature_constraint_state_conflict_diagnostics(&solved.constraints);

    assert!(
        conflict_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.axis == SignatureConstraintAxis::Owner),
        "{conflict_diagnostics:?}"
    );

    let report = signature_saturation_report_for_constraints(constraints, &usage);

    assert!(report.domain_stats.slot_count > 0, "{report:?}");
    assert!(
        report.domain_stats.unique_constraint_count > 0,
        "{report:?}"
    );
    assert!(report.domain_stats.iteration_budget > 0, "{report:?}");
    assert!(
        report.iterations <= report.domain_stats.iteration_budget,
        "{report:?}"
    );
    assert!(report.join_conflicts > 0, "{report:?}");
    assert!(
        report
            .top_conflict_axes
            .iter()
            .any(|entry| entry.label == "Owner" && entry.count > 0),
        "{report:?}"
    );
    assert!(
        report
            .top_fallback_reasons
            .iter()
            .any(|entry| entry.label == "owned" && entry.count > 0),
        "{report:?}"
    );
}

#[test]
fn signature_owned_recovery_policy_classifies_capability_proofs() {
    let external = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let copy = JavaType::Named {
        name: "CopyHandle".to_string(),
        args: Vec::new(),
    };
    let explicit_clone = JavaType::Named {
        name: "ExplicitCloneHandle".to_string(),
        args: Vec::new(),
    };
    let conservative_clone = JavaType::Named {
        name: "ConservativeCloneHandle".to_string(),
        args: Vec::new(),
    };
    let clone_box = JavaType::Named {
        name: "CloneBoxHandle".to_string(),
        args: Vec::new(),
    };
    let factory = JavaType::Named {
        name: "FactoryCopyHandle".to_string(),
        args: Vec::new(),
    };
    let immutable = JavaType::Named {
        name: "ImmutableSharedHandle".to_string(),
        args: Vec::new(),
    };
    let facade = JavaType::Named {
        name: "FacadeHandle".to_string(),
        args: Vec::new(),
    };
    let ffi = JavaType::Named {
        name: "FfiHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "CopyHandle".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExplicitCloneHandle".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    for (type_name, capability) in [
        (
            "CloneBoxHandle",
            OwnedRecoveryCapability::CloneBoxAdapter {
                java_ty: clone_box.clone(),
            },
        ),
        (
            "FactoryCopyHandle",
            OwnedRecoveryCapability::FactoryCopy {
                java_ty: factory.clone(),
            },
        ),
        (
            "ImmutableSharedHandle",
            OwnedRecoveryCapability::ImmutableSharedHandle {
                java_ty: immutable.clone(),
            },
        ),
        (
            "FacadeHandle",
            OwnedRecoveryCapability::FacadeOwnedExternalHandle {
                java_ty: facade.clone(),
            },
        ),
        (
            "FfiHandle",
            OwnedRecoveryCapability::FfiOwnedHandle {
                java_ty: ffi.clone(),
            },
        ),
    ] {
        facts
            .owned_recovery_capability_facts
            .push(OwnedRecoveryCapabilityFact {
                type_name: type_name.to_string(),
                capability,
                provenance: FactProvenance::ExternalLibraryPack,
            });
    }
    let owned_recovery_constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    ];
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(
        signature_owned_recovery_capability_policy(&copy, &facts),
        SignatureOwnedRecoveryCapabilityPolicy {
            java_ty: copy.clone(),
            recovery: OwnedRecoveryCapability::Copy,
        }
    );
    assert_eq!(
        signature_owned_recovery_capability_policy(&explicit_clone, &facts),
        SignatureOwnedRecoveryCapabilityPolicy {
            java_ty: explicit_clone.clone(),
            recovery: OwnedRecoveryCapability::ExplicitClone {
                java_ty: explicit_clone.clone(),
            },
        }
    );
    assert_eq!(
        signature_owned_recovery_capability_policy_with_effective_facts(
            &explicit_clone,
            &effective_facts,
        ),
        signature_owned_recovery_capability_policy(&explicit_clone, &facts)
    );
    assert_eq!(
        signature_owned_recovery_capability_policy(&external, &facts),
        SignatureOwnedRecoveryCapabilityPolicy {
            java_ty: external.clone(),
            recovery: OwnedRecoveryCapability::NonCloneable,
        }
    );
    assert_eq!(
        signature_owned_recovery_policy(
            &external,
            &[SignatureConstraint::RequiredOwnership(
                OwnershipShape::Owned
            )],
            &facts,
        ),
        SignatureOwnedRecoveryPolicy::NotRequired
    );
    assert_eq!(
        signature_owned_recovery_policy(&copy, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::ProvenByCopy
    );
    assert_eq!(
        signature_owned_recovery_policy_with_effective_facts(
            &copy,
            &owned_recovery_constraints,
            &effective_facts,
        ),
        signature_owned_recovery_policy(&copy, &owned_recovery_constraints, &facts)
    );
    assert_eq!(
        signature_owned_recovery_policy(&explicit_clone, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::ProvenByExplicitClone
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(OwnedRecoveryCapability::CloneBoxAdapter {
            java_ty: clone_box.clone(),
        }),
        SignatureOwnedRecoveryPolicy::ProvenByCloneBoxAdapter
    );
    assert_eq!(
        signature_owned_recovery_policy(&clone_box, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::ProvenByCloneBoxAdapter
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(OwnedRecoveryCapability::FactoryCopy {
            java_ty: factory.clone(),
        }),
        SignatureOwnedRecoveryPolicy::ProvenByFactoryCopy
    );
    assert_eq!(
        signature_owned_recovery_policy(&factory, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::ProvenByFactoryCopy
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(
            OwnedRecoveryCapability::ImmutableSharedHandle {
                java_ty: immutable.clone(),
            }
        ),
        SignatureOwnedRecoveryPolicy::ProvenByImmutableSharedHandle
    );
    assert_eq!(
        signature_owned_recovery_policy(&immutable, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::ProvenByImmutableSharedHandle
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(
            OwnedRecoveryCapability::FacadeOwnedExternalHandle {
                java_ty: facade.clone(),
            }
        ),
        SignatureOwnedRecoveryPolicy::UnsupportedFacadeOwnedExternalHandle
    );
    assert_eq!(
        signature_owned_recovery_policy(&facade, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::UnsupportedFacadeOwnedExternalHandle
    );
    assert_eq!(
        signature_owned_recovery_policy_from_capability(OwnedRecoveryCapability::FfiOwnedHandle {
            java_ty: ffi.clone(),
        }),
        SignatureOwnedRecoveryPolicy::UnsupportedFfiOwnedHandle
    );
    assert_eq!(
        signature_owned_recovery_policy(&ffi, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::UnsupportedFfiOwnedHandle
    );
    assert_eq!(
        signature_owned_recovery_policy_with_effective_facts(
            &explicit_clone,
            &owned_recovery_constraints,
            &effective_facts,
        ),
        signature_owned_recovery_policy(&explicit_clone, &owned_recovery_constraints, &facts)
    );
    assert_eq!(
        signature_owned_recovery_policy(
            &conservative_clone,
            &owned_recovery_constraints,
            &FactSet::default(),
        ),
        SignatureOwnedRecoveryPolicy::AuditConservativeClone
    );
    assert_eq!(
        signature_owned_recovery_policy(&external, &owned_recovery_constraints, &facts),
        SignatureOwnedRecoveryPolicy::UnsupportedNonCloneable
    );

    let source = include_str!("../src/signature.rs");
    assert!(source.contains("pub enum SignatureOwnedRecoveryPolicy"));
    assert!(source.contains("pub struct SignatureOwnedRecoveryCapabilityPolicy"));
    assert!(source.contains("ProvenByCloneBoxAdapter"));
    assert!(source.contains("ProvenByFactoryCopy"));
    assert!(source.contains("ProvenByImmutableSharedHandle"));
    assert!(source.contains("UnsupportedFacadeOwnedExternalHandle"));
    assert!(source.contains("UnsupportedFfiOwnedHandle"));
    assert!(source.contains("pub fn signature_owned_recovery_capability_policy"));
    assert!(source.contains("signature_owned_recovery_capability_policy_with_effective_facts"));
    assert!(source.contains("pub fn signature_owned_recovery_policy"));
    assert!(source.contains("pub fn signature_owned_recovery_policy_with_effective_facts"));
    assert!(source.contains("signature_owned_recovery_policy_with_effective_facts(java_ty, constraints, &effective_facts)"));
    assert!(source.contains(
        "signature_owned_recovery_capability_policy_with_effective_facts(java_ty, facts)"
    ));
    assert!(source.contains("use crate::effective_facts::EffectiveFacts"));
}

#[test]
fn signature_capability_audit_reports_non_cloneable_owned_recovery() {
    let external = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    ];

    let diagnostics = capability_diagnostics_for_signature_constraints(
        "External::use parameter value".to_string(),
        &external,
        &constraints,
        &facts,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "signature_owned_recovery_requires_clone"
            && diagnostic.message.contains("External::use parameter value")
            && diagnostic.message.contains("ExternalHandle")
    }));
}

#[test]
fn signature_capability_audit_accepts_cloneable_owned_recovery() {
    let external = JavaType::Named {
        name: "CloneableHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "CloneableHandle".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    ];

    let diagnostics = capability_diagnostics_for_signature_constraints(
        "External::use parameter value".to_string(),
        &external,
        &constraints,
        &facts,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn signature_capability_audit_reports_conservative_clone_recovery() {
    let external = JavaType::Named {
        name: "DefaultCloneHandle".to_string(),
        args: Vec::new(),
    };
    let constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    ];

    let diagnostics = capability_diagnostics_for_signature_constraints(
        "External::use parameter value".to_string(),
        &external,
        &constraints,
        &FactSet::default(),
    );

    assert_eq!(
        diagnostics,
        vec![SignatureDiagnostic {
            code: "signature_clone_recovery_requires_audit",
            message:
                "External::use parameter value needs owned recovery from a borrowed view; Named { name: \"DefaultCloneHandle\", args: [] } is Clone only by conservative/default capability, so the inserted clone requires audit"
                    .to_string(),
        }]
    );
}

#[test]
fn signature_capability_audit_reports_return_owned_recovery() {
    let external = JavaType::Named {
        name: "ReturnHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ReturnHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let constraints = vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
    ];

    let diagnostics = capability_diagnostics_for_signature_constraints(
        "ReturnFlow::get return".to_string(),
        &external,
        &constraints,
        &facts,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "signature_owned_recovery_requires_clone"
            && diagnostic.message.contains("ReturnFlow::get return")
            && diagnostic.message.contains("ReturnHandle")
    }));
}

#[test]
fn signature_capability_audit_reports_field_owned_recovery() {
    let external = JavaType::Named {
        name: "FieldHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "FieldHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let constraints = vec![
        SignatureConstraint::StorageSlot,
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
    ];

    let diagnostics = capability_diagnostics_for_signature_constraints(
        "Holder field value".to_string(),
        &external,
        &constraints,
        &facts,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "signature_owned_recovery_requires_clone"
            && diagnostic.message.contains("Holder field value")
            && diagnostic.message.contains("FieldHandle")
    }));
}

#[test]
fn signature_diagnostics_route_fields_through_capability_audit() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("fn push_field_signature_diagnostics"));
    assert!(source.contains("push_capability_diagnostics_for_signature_constraints(\n        diagnostics,\n        slot,\n        &field.ty,"));
    assert!(source.contains("push_field_signature_diagnostics("));
}

#[test]
fn signature_plan_reports_local_capability_conflicts_as_diagnostics() {
    let parsed = parse_java_source(
        r#"
class LocalCapabilityFlow {
    private void use(ExternalHandle input, Helper helper) {
        ExternalHandle local = input;
        helper.consume(local);
        this.borrowed(local);
    }

    private void borrowed(ExternalHandle value) {
        int len = value.length;
    }
}

class ExternalHandle {}
class Helper {
    void consume(ExternalHandle value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let (solved_constraints, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let method_key = MethodKey {
        type_name: "LocalCapabilityFlow".to_string(),
        rust_name: "use".to_string(),
    };
    let local_constraints = solved_constraints
        .methods
        .get(&method_key)
        .unwrap()
        .locals
        .iter()
        .find(|local| local.name == "local")
        .unwrap();
    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )),
        "{local_constraints:?}"
    );
    assert!(
        local_constraints
            .constraints
            .iter()
            .any(|constraint| matches!(
                constraint,
                SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
            )),
        "{local_constraints:?}"
    );
    assert!(
        !capability_diagnostics_for_signature_constraints(
            "LocalCapabilityFlow::use local local".to_string(),
            &local_constraints.java_ty,
            &local_constraints.constraints,
            &facts,
        )
        .is_empty(),
        "{local_constraints:?}"
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);

    assert!(
        signatures.diagnostics().iter().any(|diagnostic| {
            diagnostic.code == "signature_owned_recovery_requires_clone"
                && diagnostic
                    .message
                    .contains("LocalCapabilityFlow::use local local")
                && diagnostic.message.contains("ExternalHandle")
        }),
        "{:?}",
        signatures.diagnostics()
    );
}

#[test]
fn signature_plan_reports_constraint_conflicts_as_diagnostics() {
    let parsed = parse_java_source(
        r#"
class ConflictFlow {
    private int outer(Helper helper, String value) {
        helper.consume(value);
        return inner(value);
    }

    private int inner(String value) {
        return value.length();
    }
}

class Helper {
    int consume(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);

    assert!(signatures.diagnostics().iter().any(|diagnostic| {
        diagnostic.code == "signature_constraint_conflict"
            && diagnostic
                .message
                .contains("ConflictFlow::outer parameter value")
            && diagnostic.message.contains("Owner")
            && diagnostic.message.contains("kept RequiredOwnership(Owned)")
            && diagnostic
                .message
                .contains("rejected RequiredOwnership(SharedBorrow)")
    }));
}

#[test]
fn item_assembly_owns_struct_impl_and_function_shells_from_signatures() {
    let parsed = parse_java_source(
        r#"
class Holder {
    String name;
    Holder(String name) { this.name = name; }
    public String echo(String value) { return value; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();

    let struct_item = struct_item_for_type(ty, &signatures);
    let RustItem::Struct(rust_struct) = struct_item else {
        panic!("type shell should be assembled as a struct item");
    };
    assert_eq!(rust_struct.name, "Holder");
    assert_eq!(rust_struct.fields.len(), 1);
    assert_eq!(rust_struct.fields[0].name, "name");
    assert_eq!(
        rust_struct.fields[0].ty,
        rust_type_for_slot(signatures.field_shape("Holder", "name").unwrap())
    );

    let ctor_body = vec![RustStmt::Return(None)];
    let ctor_fn = constructor_function_shell(ty, ctor, &signatures, ctor_body.clone());
    assert_eq!(ctor_fn.name, "new");
    assert_eq!(ctor_fn.params.len(), 1);
    assert_eq!(ctor_fn.params[0].name, "name");
    assert_eq!(
        ctor_fn.params[0].ty,
        rust_type_for_slot(&signatures.constructor_param_plans(ty, ctor).unwrap()[0].shape)
    );
    assert_eq!(
        ctor_fn.return_type,
        Some(RustType::Named("Holder".to_string()))
    );
    assert_eq!(ctor_fn.receiver, None);
    assert_eq!(ctor_fn.body, ctor_body);
    assert_eq!(
        constructor_return_body(
            ty,
            vec![("name".to_string(), RustExpr::Var("name".to_string()))]
        ),
        vec![RustStmt::Return(Some(RustExpr::StructLit {
            name: "Holder".to_string(),
            fields: vec![("name".to_string(), RustExpr::Var("name".to_string()))],
        }))]
    );
    assert_eq!(
        constructor_return_body(
            ty,
            vec![
                ("reader".to_string(), RustExpr::Var("reader".to_string())),
                (
                    "query".to_string(),
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("reader".to_string())),
                        method: "query_3".to_string(),
                        args: Vec::new(),
                    },
                ),
            ],
        ),
        vec![RustStmt::Return(Some(RustExpr::StructLit {
            name: "Holder".to_string(),
            fields: vec![
                (
                    "query".to_string(),
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("reader".to_string())),
                        method: "query_3".to_string(),
                        args: Vec::new(),
                    },
                ),
                ("reader".to_string(), RustExpr::Var("reader".to_string())),
            ],
        }))]
    );
    assert_eq!(
        constructor_return_body(
            ty,
            vec![
                ("reader".to_string(), RustExpr::Raw("reader".to_string())),
                (
                    "query".to_string(),
                    RustExpr::Raw("Some(reader.query_3(contig, start, end))".to_string()),
                ),
            ],
        ),
        vec![RustStmt::Return(Some(RustExpr::StructLit {
            name: "Holder".to_string(),
            fields: vec![
                (
                    "query".to_string(),
                    RustExpr::Raw("Some(reader.query_3(contig, start, end))".to_string()),
                ),
                ("reader".to_string(), RustExpr::Raw("reader".to_string())),
            ],
        }))]
    );
    assert_eq!(
        delegating_constructor_return_body(
            "new_with_name",
            vec![RustExpr::Var("name".to_string())],
        ),
        vec![RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw("Self::new_with_name".to_string())),
            args: vec![RustExpr::Var("name".to_string())],
        }))]
    );
    assert_eq!(
        constructor_default_field_value(
            "count".to_string(),
            RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int))
        ),
        (
            "count".to_string(),
            default_expr_for_slot(&RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int
            )))
        )
    );

    let implicit_ctor_translation = translate_source(
        r#"
class ImplicitDefaultConstructorCaller {
    ImplicitDefaultConstructorTarget make() {
        return new ImplicitDefaultConstructorTarget();
    }
}

class ImplicitDefaultConstructorTarget {
    int count = 7;
    String name;
}
"#,
    );
    assert!(
        !implicit_ctor_translation.diagnostics.has_errors(),
        "{:?}",
        implicit_ctor_translation.diagnostics
    );
    assert!(
        implicit_ctor_translation
            .rust
            .contains("pub fn new() -> ImplicitDefaultConstructorTarget"),
        "{}",
        implicit_ctor_translation.rust
    );
    assert!(
        implicit_ctor_translation.rust.contains(
            "return ImplicitDefaultConstructorTarget { count: 7, name: Default::default() };"
        ),
        "{}",
        implicit_ctor_translation.rust
    );
    assert!(
        implicit_ctor_translation
            .rust
            .contains("return ImplicitDefaultConstructorTarget::new();"),
        "{}",
        implicit_ctor_translation.rust
    );

    let method_body = vec![RustStmt::Return(Some(RustExpr::Var("value".to_string())))];
    let method_fn = method_function_shell(ty, method, &signatures, method_body.clone());
    assert_eq!(method_fn.name, "echo");
    assert_eq!(method_fn.params.len(), 1);
    assert_eq!(method_fn.params[0].name, "value");
    assert_eq!(
        method_fn.return_type,
        signatures
            .method_return_shape(ty, method)
            .map(|shape| rust_type_for_slot(&shape))
    );
    assert_eq!(method_fn.receiver, Some(RustReceiver::Shared));
    assert_eq!(method_fn.body, method_body);

    let impl_item = impl_item_for_type(ty, vec![method_fn.clone()]);
    let RustItem::Impl(rust_impl) = impl_item else {
        panic!("impl shell should be assembled as an impl item");
    };
    assert_eq!(rust_impl.target, "Holder");
    assert_eq!(rust_impl.functions, vec![method_fn]);
    assert_eq!(
        raw_item("pub enum HolderKind {}".to_string()),
        RustItem::Raw("pub enum HolderKind {}".to_string())
    );
}

#[test]
fn item_assembly_uses_fact_aware_slot_rendering_for_function_shells() {
    let parsed = parse_java_source(
        r#"
class ExternalShell {
    private int len(ExternalLabel value) {
        return value.length();
    }
}

class ExternalLabel {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalLabel".to_string(),
            copy: None,
            clone: None,
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let representations = plan_representations(&resolved.program, &facts);
    let usage = analyze_usage(&resolved.program);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let body = vec![RustStmt::Return(Some(RustExpr::Literal("0".to_string())))];

    let default_shell = method_function_shell(ty, method, &signatures, body.clone());
    let fact_shell = method_function_shell_with_facts(ty, method, &signatures, body, &facts);
    let planned_shell = method_function_shell_with_facts_and_exception_plan(
        ty,
        method,
        &signatures,
        vec![RustStmt::Return(Some(RustExpr::Literal("0".to_string())))],
        &facts,
        MethodExceptionPlan {
            wrap_returns_in_result: false,
        },
    );

    assert_eq!(
        default_shell.params[0].ty,
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Named("ExternalLabel".to_string())),
        }
    );
    assert_eq!(
        fact_shell.params[0].ty,
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string())),
        }
    );
    assert_eq!(planned_shell.return_type, fact_shell.return_type);
}

#[test]
fn lowering_passes_facts_to_item_assembly_for_function_shells() {
    let source = r#"
class ExternalShell {
    private int len(ExternalLabel value) {
        return value.length();
    }
}

class ExternalLabel {}
"#;
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalLabel".to_string(),
            copy: None,
            clone: None,
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("pub fn len(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
}

#[test]
fn method_effect_facts_drive_exception_result_shell_and_return_wrapping() {
    let source = r#"
class EffectOnlyThrows {
    int read() {
        return 7;
    }
}
"#;
    let mut facts = FactSet::default();
    facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "EffectOnlyThrows".to_string(),
            method_name: "read".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Named {
            name: "IOException".to_string(),
            args: Vec::new(),
        }],
        provenance: FactProvenance::UserOverride,
    });

    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("pub fn read(&self) -> Result<i32, JavaException>"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return Ok(7);"),
        "{}",
        translation.rust
    );
}

#[test]
fn item_assembly_return_type_wrappers_delegate_to_fact_aware_rendering() {
    let source = include_str!("../src/item_assembly.rs");

    assert!(source.contains("pub fn method_return_type_with_facts"));
    assert!(
        source
            .contains("method_return_type_with_facts(ty, method, signatures, &FactSet::default())")
    );
    assert!(!source.contains("rust_type_for_slot(&shape)"));
}

#[test]
fn lower_orchestrator_does_not_render_default_slot_values() {
    let source = include_str!("../src/lower.rs");

    assert!(!source.contains("default_expr_for_slot"));
    assert!(!source.contains("slot_render"));
}

#[test]
fn lowering_consumes_solved_plans_without_choosing_shape_axes() {
    let lowering_sources = [
        include_str!("../src/lower.rs"),
        include_str!("../src/access_lowering.rs"),
        include_str!("../src/call_lowering.rs"),
        include_str!("../src/conversion_lowering.rs"),
        include_str!("../src/exception_lowering.rs"),
        include_str!("../src/literal_lowering.rs"),
        include_str!("../src/loop_lowering.rs"),
        include_str!("../src/operator_lowering.rs"),
        include_str!("../src/stream_lowering.rs"),
        include_str!("../src/string_concat_render.rs"),
        include_str!("../src/switch_lowering.rs"),
    ];

    for source in lowering_sources {
        assert!(!source.contains("OwnershipShape"));
        assert!(!source.contains("ViewShape"));
        assert!(!source.contains("DispatchShape"));
        assert!(!source.contains("MutabilityShape"));
        assert!(!source.contains("RustSlotShape::conservative"));
        assert!(!source.contains("RustSlotShape::for_storage"));
        assert!(!source.contains("ObjectRepresentation"));
        assert!(!source.contains("dispatch_for"));
        assert!(!source.contains("representation_for"));
    }

    let lower_source = include_str!("../src/lower.rs");
    assert!(lower_source.contains("plan_slot_conversion"));
    assert!(lower_source.contains("plan_lowered_slot_conversion"));
    assert!(lower_source.contains("apply_slot_expr_conversion"));
    assert!(lower_source.contains("apply_lowered_slot_expr_conversion"));
    assert!(!lower_source.contains("coercion::apply_expr_to_slot_with_facts"));
    assert!(!lower_source.contains("coercion::apply_lowered_expr_to_slot_with_facts"));
}

#[test]
fn new_feature_responsibility_checklist_is_tracked() {
    let checklist = include_str!("../docs/new-feature-responsibility-checklist.md");

    assert!(checklist.contains("Java Semantics"));
    assert!(checklist.contains("Capabilities And External Facts"));
    assert!(checklist.contains("Object Representation"));
    assert!(checklist.contains("Signature And Slot Shape"));
    assert!(checklist.contains("Shape Projection"));
    assert!(checklist.contains("Coercion"));
    assert!(checklist.contains("Rendering And Lowering"));
    assert!(checklist.contains("Required Tests"));
    assert!(checklist.contains("ObjectRepresentationPolicy"));
    assert!(checklist.contains("SignatureBoundaryPolicy"));
    assert!(checklist.contains("SignatureCapabilityPolicy"));
    assert!(checklist.contains("lowering"));
    assert!(checklist.contains("must not recompute hierarchy"));
    assert!(checklist.contains("finite graph edges"));
    assert!(checklist.contains("bounded-domain facts"));
    assert!(checklist.contains("finite-budget diagnostic"));
    assert!(checklist.contains("recursively specialized Rust shapes"));
}

#[test]
fn recursive_code_policy_is_finite_fixpoint_not_lowering_expansion() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    let signature_source = include_str!("../src/signature.rs");
    let fact_saturation_source = include_str!("../src/fact_saturation.rs");

    assert!(design.contains("Recursive Java code"));
    assert!(design.contains("finite graph edges"));
    assert!(design.contains("bounded"));
    assert!(design.contains("finite-budget diagnostic"));
    assert!(design.contains("create unbounded path-specific facts"));
    assert!(signature_source.contains("signature_fixpoint_saturation_policy"));
    assert!(signature_source.contains("signature_fixpoint_iteration_budget"));
    assert!(signature_source.contains("signature_fixpoint_budget_exceeded"));
    assert!(signature_source.contains("FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("FiniteFactDomain"));
    assert!(fact_saturation_source.contains("FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("finite_fact_iteration_budget"));
    assert!(fact_saturation_source.contains("FiniteSaturationResult"));
    assert!(fact_saturation_source.contains("SaturationStep::Stable"));
}

#[test]
fn object_representation_fallback_matrix_is_documented() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");

    assert!(design.contains("### Object Representation Fallback Matrix"));
    assert!(design.contains("| Role | Concrete Struct | Closed Enum | Dyn Trait | Boxed Dyn Trait | Erased Object | Unsupported Hierarchy |"));
    for role in [
        "Public API param",
        "Public API return",
        "Unknown API param",
        "Unknown API return",
        "Storage/field",
        "Local",
        "Captured value",
        "External pack fact",
        "Non-object-safe interface",
        "Closed hierarchy",
        "Raw object / wildcard object",
    ] {
        assert!(design.contains(role), "missing matrix role {role}");
        let row = design
            .lines()
            .find(|line| line.starts_with(&format!("| {role} |")))
            .unwrap_or_else(|| panic!("missing matrix row {role}"));
        let cells = row
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 7, "matrix row has wrong cell count: {row}");
        assert!(
            cells.iter().all(|cell| !cell.is_empty()),
            "matrix row has an empty cell: {row}"
        );
    }
    assert!(design.contains("`BoxedDynTrait` is intentionally not an `ObjectRepresentation`"));
    assert!(!design.contains("    BoxedDynTrait,\n    ClosedEnum,"));
}

#[test]
fn erased_object_runtime_contract_is_documented_and_shared() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");

    assert!(design.contains("## Erased Object Runtime Contract"));
    for phrase in [
        "`ErasedObject` lowers to `Box<dyn std::any::Any>`",
        "storing, returning, and passing erased values preserves",
        "Box::new(value) as Box<dyn std::any::Any>",
        "downcast::<Target>()",
        "is::<Target>()",
        "erased-to-borrowed-dyn and erased-to-boxed-dyn recovery are unsupported",
        "anonymous erased boundaries and external-pack erased handles",
        "carrier instead of generating anonymous concrete implementations",
    ] {
        assert!(
            design.contains(phrase),
            "missing erased contract phrase {phrase}"
        );
    }
    assert!(conversion_source.contains("pub struct ErasedObjectRuntimeContract"));
    assert!(conversion_source.contains("pub fn erased_object_runtime_contract"));
    assert!(coercion_source.contains("Box::new({}) as Box<dyn std::any::Any>"));
    assert!(coercion_source.contains("downcast::<{target_type}>()"));
    assert!(coercion_source.contains(".is::<{target_type}>()"));
    assert!(slot_render_source.contains("Box<dyn std::any::Any>"));
}

#[test]
fn object_representation_fallback_matrix_covers_core_outcomes() {
    let parsed = parse_java_source(
        r#"
class Plain {}
interface Animal {}
class Dog implements Animal {}
sealed interface Expr permits Add, Num {}
final class Add implements Expr {}
final class Num implements Expr {}
interface Factory { Factory next(); }
class Poodle extends Dog {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Plain".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ConcreteStruct
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        storage_shape_for_java_type(
            &representations,
            JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            },
        )
        .dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Factory".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        project_java_value_shape(Some(&representations), JavaType::Wildcard(None)).dispatch,
        DispatchShape::ErasedObject
    );

    let diagnostics = representations.diagnostics_for_program(&resolved.program);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_inheritance"
                && diagnostic.message.contains("Poodle")),
        "{diagnostics:?}"
    );
}

#[test]
fn signature_plan_owns_receiver_mutability_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Counter {
    int value;

    public int get() {
        return this.value;
    }

    public void set(int next) {
        this.value = next;
    }

    public static int id(int value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];

    let method_for = |name: &str| {
        ty.members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap()
    };
    let receiver_for = |name: &str| {
        let method = method_for(name);
        signatures.method_receiver(ty, method).unwrap()
    };
    let method_usage = |name: &str| {
        usage.methods.get(&new_java2rust::usage::UsageMethodKey {
            type_name: "Counter".to_string(),
            method_name: name.to_string(),
        })
    };

    assert_eq!(receiver_for("get"), ReceiverPlan::Shared);
    assert_eq!(receiver_for("set"), ReceiverPlan::Mutable);
    assert_eq!(receiver_for("id"), ReceiverPlan::None);
    assert_eq!(receiver_constraints(method_for("get")), Vec::new());
    assert_eq!(receiver_constraints(method_for("set")), Vec::new());
    assert_eq!(
        receiver_constraints_with_usage(method_for("set"), method_usage("set")),
        vec![ReceiverConstraint::MutableReceiver]
    );
    assert_eq!(
        receiver_constraints(method_for("id")),
        vec![ReceiverConstraint::StaticReceiver]
    );
    assert_eq!(constrained_receiver_plan(&[]), ReceiverPlan::Shared);
    assert_eq!(
        constrained_receiver_plan(&[ReceiverConstraint::MutableReceiver]),
        ReceiverPlan::Mutable
    );
    assert_eq!(
        constrained_receiver_plan(&[
            ReceiverConstraint::StaticReceiver,
            ReceiverConstraint::MutableReceiver,
        ]),
        ReceiverPlan::None
    );

    let get_method = method_for("get");
    let mut wrong_type = ty.clone();
    wrong_type.name = "MissingCounter".to_string();
    assert!(
        signatures
            .method_receiver(&wrong_type, get_method)
            .is_none()
    );
}

#[test]
fn method_effect_fact_can_drive_receiver_mutability_without_body_reanalysis() {
    let parsed = parse_java_source(
        r#"
class Counter {
    int get() {
        return 1;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "Counter".to_string(),
            method_name: "get".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage_with_facts(&resolved.program, &facts);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "get" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        receiver_constraints_with_facts(ty, method, &facts),
        vec![ReceiverConstraint::MutableReceiver]
    );
    assert_eq!(
        signatures.method_receiver(ty, method),
        Some(ReceiverPlan::Mutable)
    );
}

#[test]
fn signature_solver_can_choose_borrowed_str_for_private_content_only_params() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int len(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();

    let param = &signatures.method_param_plans(ty, method).unwrap()[0];
    assert_eq!(param.shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(param.shape.view, ViewShape::Str);
    assert_eq!(
        rust_type_for_slot(&param.shape),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string()))
        }
    );
}

#[test]
fn slot_policy_owns_borrowed_content_param_and_return_rules() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int len(String value) {
        return value.length();
    }

    private static String id(String value) {
        return value;
    }

    private StringUse(String name) {
        name.length();
    }
}

class ListUse {
    private int empty(List<String> values) {
        return values.isEmpty() ? 1 : 0;
    }

    private static List<String> id(List<String> values) {
        return values;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let ty = &resolved.program.types[0];
    let method = |name: &str| {
        ty.members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap()
    };
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let effective_facts = EffectiveFacts::new(&resolved.facts);

    assert_eq!(
        method_param_borrowed_content_policy(ty, method("len"), &method("len").params[0], &usage),
        Some(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(
        method_param_borrowed_content_policy_with_effective_facts(
            ty,
            method("len"),
            &method("len").params[0],
            &usage,
            &effective_facts,
        ),
        method_param_borrowed_content_policy(ty, method("len"), &method("len").params[0], &usage)
    );
    assert_eq!(
        borrowed_content_capability_policy(&JavaType::String, &resolved.facts),
        BorrowedContentCapabilityPolicy {
            java_ty: JavaType::String,
            string_content: true,
            sequence_element: None,
        }
    );
    assert_eq!(
        borrowed_content_capability_policy_with_effective_facts(
            &JavaType::String,
            &effective_facts
        ),
        borrowed_content_capability_policy(&JavaType::String, &resolved.facts)
    );
    assert_eq!(
        borrowed_content_type_policy(&JavaType::String, &resolved.facts),
        BorrowedContentTypePolicy {
            java_ty: JavaType::String,
            policy: Some(BorrowedContentPolicy::string_like_view()),
        }
    );
    assert_eq!(
        borrowed_content_type_policy_with_effective_facts(&JavaType::String, &effective_facts),
        borrowed_content_type_policy(&JavaType::String, &resolved.facts)
    );
    assert_eq!(
        method_param_borrowed_content_policy(ty, method("id"), &method("id").params[0], &usage),
        Some(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(
        constructor_param_borrowed_content_policy(ty, ctor, &ctor.params[0], &usage),
        Some(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(
        constructor_param_borrowed_content_policy_with_effective_facts(
            ty,
            ctor,
            &ctor.params[0],
            &usage,
            &effective_facts,
        ),
        constructor_param_borrowed_content_policy(ty, ctor, &ctor.params[0], &usage)
    );
    assert_eq!(
        method_param_shape_constraints(
            ty,
            method("len"),
            &method("len").params[0],
            &usage,
            &resolved.facts
        ),
        signature_constraints_for_borrowed_content_policy(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(
        constructor_param_shape_constraints(ty, ctor, &ctor.params[0], &usage, &resolved.facts),
        signature_constraints_for_borrowed_content_policy(BorrowedContentPolicy::string_like_view())
    );

    let mut shape = RustSlotShape::conservative(JavaType::String);
    apply_borrowed_content_policy(&mut shape, BorrowedContentPolicy::string_like_view());
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Str);
    let constrained_shape = apply_signature_constraints(
        RustSlotShape::conservative(JavaType::String),
        &signature_constraints_for_borrowed_content_policy(
            BorrowedContentPolicy::string_like_view(),
        ),
    );
    assert_eq!(constrained_shape, shape);

    let return_shape =
        borrowed_return_shape_from_param(ty, method("id"), &representations, &usage).unwrap();
    assert_eq!(
        borrowed_return_content_policy_from_param(ty, method("id"), &usage),
        Some(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(
        borrowed_return_content_policy_from_param_with_effective_facts(
            ty,
            method("id"),
            &usage,
            &effective_facts,
        ),
        borrowed_return_content_policy_from_param(ty, method("id"), &usage)
    );
    assert_eq!(
        borrowed_return_shape_from_param_with_effective_facts(
            ty,
            method("id"),
            &representations,
            &usage,
            &effective_facts,
        )
        .unwrap(),
        return_shape,
    );
    assert_eq!(
        method_return_shape_constraints(ty, method("id"), &usage, &resolved.facts),
        signature_constraints_for_borrowed_content_policy(BorrowedContentPolicy::string_like_view())
    );
    assert_eq!(return_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(return_shape.view, ViewShape::Str);

    let list_ty = &resolved.program.types[1];
    let list_java_string = list_java_type(JavaType::String);
    assert_eq!(
        borrowed_content_capability_policy(&list_java_string, &resolved.facts),
        BorrowedContentCapabilityPolicy {
            java_ty: list_java_string.clone(),
            string_content: false,
            sequence_element: Some(JavaType::String),
        }
    );
    assert_eq!(
        borrowed_content_type_policy(&list_java_string, &resolved.facts),
        BorrowedContentTypePolicy {
            java_ty: list_java_string.clone(),
            policy: Some(BorrowedContentPolicy::sequence_like_view()),
        }
    );
    assert_eq!(
        borrowed_content_type_policy_with_effective_facts(&list_java_string, &effective_facts),
        borrowed_content_type_policy(&list_java_string, &resolved.facts)
    );
    assert_eq!(
        borrowed_content_type_policy(&JavaType::Primitive(PrimitiveType::Int), &resolved.facts),
        BorrowedContentTypePolicy {
            java_ty: JavaType::Primitive(PrimitiveType::Int),
            policy: None,
        }
    );
    let list_method = |name: &str| {
        list_ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(
        method_param_borrowed_content_policy(
            list_ty,
            list_method("empty"),
            &list_method("empty").params[0],
            &usage,
        ),
        Some(BorrowedContentPolicy::sequence_like_view())
    );
    assert_eq!(
        method_param_borrowed_content_policy_with_effective_facts(
            list_ty,
            list_method("empty"),
            &list_method("empty").params[0],
            &usage,
            &effective_facts,
        ),
        method_param_borrowed_content_policy(
            list_ty,
            list_method("empty"),
            &list_method("empty").params[0],
            &usage,
        )
    );
    assert_eq!(
        method_param_shape_constraints(
            list_ty,
            list_method("empty"),
            &list_method("empty").params[0],
            &usage,
            &resolved.facts,
        ),
        signature_constraints_for_borrowed_content_policy(
            BorrowedContentPolicy::sequence_like_view(),
        )
    );
    let mut list_shape = RustSlotShape::conservative(list_java_string);
    apply_borrowed_content_policy(&mut list_shape, BorrowedContentPolicy::sequence_like_view());
    assert_eq!(list_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(list_shape.view, ViewShape::Slice);
    assert_eq!(
        borrowed_return_content_policy_from_param(list_ty, list_method("id"), &usage),
        Some(BorrowedContentPolicy::sequence_like_view())
    );
    let list_return_shape =
        borrowed_return_shape_from_param(list_ty, list_method("id"), &representations, &usage)
            .unwrap();
    assert_eq!(list_return_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(list_return_shape.view, ViewShape::Slice);
}

#[test]
fn usage_classifies_unknown_call_arguments_as_conservative_boundaries() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int use(Helper helper, String value) {
        return helper.consume(value);
    }
}

class Helper {
    int consume(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "use" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: ty.name.clone(),
            method_name: "use".to_string(),
        })
        .unwrap();
    let value_usage = method_usage.bindings.get("value").unwrap();

    assert!(method_usage.call_sites.iter().any(|call_site| {
        call_site.name == "consume"
            && call_site.arg_count == 1
            && call_site.result_returned
            && call_site.boundary == CallArgumentBoundary::Unknown
            && call_site
                .resolved
                .as_ref()
                .is_some_and(|resolved| resolved.declaring_type == "Helper")
    }));
    assert!(value_usage.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::PassedToCall {
                name,
                arg_index: 0,
                boundary: CallArgumentBoundary::Unknown,
            } if name == "consume"
        )
    }));
    assert_eq!(
        method_param_borrowed_content_policy(ty, method, &method.params[1], &usage),
        None
    );
    assert_eq!(
        method_param_shape_constraints(ty, method, &method.params[1], &usage, &resolved.facts),
        conservative_boundary_constraints(&JavaType::String, &resolved.facts)
    );
}

fn external_name_boundary_facts(base: &FactSet) -> FactSet {
    let mut facts = base.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalName".to_string(),
            copy: Some(false),
            clone: Some(false),
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
}

fn external_name_type() -> JavaType {
    JavaType::Named {
        name: "ExternalName".to_string(),
        args: Vec::new(),
    }
}

#[test]
fn usage_call_result_boundary_policy_names_result_boundaries() {
    let internal_methods = HashSet::from(["local_fetch".to_string()]);
    let empty_usage = MethodUsageFacts::default();

    assert_eq!(
        call_argument_boundary_policy(None, "trim", &internal_methods),
        CallArgumentBoundaryPolicy {
            name: "trim".to_string(),
            boundary: CallArgumentBoundary::KnownContentRead,
        }
    );
    assert_eq!(
        call_argument_boundary_policy(None, "local_fetch", &internal_methods),
        CallArgumentBoundaryPolicy {
            name: "local_fetch".to_string(),
            boundary: CallArgumentBoundary::InternalSameType,
        }
    );
    assert_eq!(
        call_argument_boundary_policy(
            Some(&parse_expr("helper")),
            "local_fetch",
            &internal_methods
        ),
        CallArgumentBoundaryPolicy {
            name: "local_fetch".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        }
    );
    assert_eq!(
        call_argument_boundary_policy(None, "addAll", &internal_methods),
        CallArgumentBoundaryPolicy {
            name: "addAll".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        }
    );
    assert_eq!(
        call_argument_use_boundary_policy("addAll", 0, CallArgumentBoundary::Unknown),
        CallArgumentBoundary::KnownContentRead
    );
    assert_eq!(
        call_result_boundary_policy(&parse_expr("localFetch()"), &internal_methods, &empty_usage),
        Some(CallResultBoundaryPolicy {
            name: "localFetch".to_string(),
            receiver_type: None,
            boundary: CallArgumentBoundary::Unknown,
        })
    );
    assert_eq!(
        call_result_boundary_policy(
            &parse_expr("local_fetch()"),
            &internal_methods,
            &empty_usage
        ),
        Some(CallResultBoundaryPolicy {
            name: "local_fetch".to_string(),
            receiver_type: None,
            boundary: CallArgumentBoundary::InternalSameType,
        })
    );
    assert_eq!(
        call_result_boundary_policy(
            &parse_expr("helper.fetch()"),
            &internal_methods,
            &empty_usage
        ),
        Some(CallResultBoundaryPolicy {
            name: "fetch".to_string(),
            receiver_type: None,
            boundary: CallArgumentBoundary::Unknown,
        })
    );
    assert_eq!(
        call_result_boundary_policy(
            &parse_expr("new ExternalName()"),
            &internal_methods,
            &empty_usage
        ),
        Some(CallResultBoundaryPolicy {
            name: "new".to_string(),
            receiver_type: None,
            boundary: CallArgumentBoundary::Unknown,
        })
    );

    let mut usage = MethodUsageFacts::default();
    usage.bindings.insert(
        "helper".to_string(),
        BindingUseFacts {
            name: "helper".to_string(),
            java_ty: JavaType::Named {
                name: "Helper".to_string(),
                args: Vec::new(),
            },
            uses: Vec::new(),
        },
    );
    assert_eq!(
        call_result_boundary_policy(&parse_expr("helper.fetch()"), &internal_methods, &usage),
        Some(CallResultBoundaryPolicy {
            name: "fetch".to_string(),
            receiver_type: Some(JavaType::Named {
                name: "Helper".to_string(),
                args: Vec::new(),
            }),
            boundary: CallArgumentBoundary::Unknown,
        })
    );
}

#[test]
fn native_collection_add_all_argument_boundary_borrows_source_content() {
    let parsed = parse_java_source(
        r#"
import java.util.List;

class BulkAddAllBorrowed {
    void merge(List<String> values, List<String> other) {
        values.addAll(other);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let usage = &semantic.usage;
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "BulkAddAllBorrowed".to_string(),
            method_name: "merge".to_string(),
        })
        .unwrap();

    let values_usage = method_usage.bindings.get("values").unwrap();
    assert!(values_usage.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::ReceiverForCall {
                name,
                boundary: CallArgumentBoundary::Unknown,
            } if name == "addAll"
        )
    }));
    let other_usage = method_usage.bindings.get("other").unwrap();
    assert!(other_usage.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::PassedToCall {
                name,
                arg_index: 0,
                boundary: CallArgumentBoundary::KnownContentRead,
            } if name == "addAll"
        )
    }));
    assert!(
        semantic
            .facts
            .external_api_effect_for_receiver(Some("List"), "addAll")
            .is_some_and(|effect| {
                effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
            })
    );

    let representations = plan_representations(&resolved.program, &semantic.facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, usage, &semantic.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "merge" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.required_method_signature(ty, method);
    let values = &signature.params[0].shape;
    let other = &signature.params[1].shape;

    assert_eq!(values.ownership, OwnershipShape::MutableBorrow);
    assert_eq!(values.view, ViewShape::Native);
    assert_eq!(other.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(other.view, ViewShape::Slice);
}

#[test]
fn usage_method_api_boundary_policy_names_public_and_internal_methods() {
    assert_eq!(
        method_api_boundary_policy(&[Modifier::Public]),
        MethodApiBoundaryPolicy { public_api: true }
    );
    assert_eq!(
        method_api_boundary_policy(&[Modifier::Protected]),
        MethodApiBoundaryPolicy { public_api: true }
    );
    assert_eq!(
        method_api_boundary_policy(&[Modifier::Private]),
        MethodApiBoundaryPolicy { public_api: false }
    );
    assert_eq!(
        method_api_boundary_policy(&[]),
        MethodApiBoundaryPolicy { public_api: false }
    );
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_locals_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownLocalSource {
    private int use(Helper helper) {
        ExternalName value = helper.fetch();
        return value.length();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownLocalSource".to_string(),
            method_name: "use".to_string(),
        })
        .unwrap();
    let value_usage = method_usage.bindings.get("value").unwrap();

    assert!(value_usage.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::AssignedFromCall {
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if name == "fetch"
        )
    }));

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "UnknownLocalSource".to_string(),
            rust_name: "use".to_string(),
        })
        .unwrap();
    let local_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert_eq!(
        local_constraints.constraints,
        conservative_boundary_constraints(&external_name_type(), &facts)
    );
}

#[test]
fn constructor_assigned_local_keeps_owned_storage_despite_borrowed_external_uses() {
    let parsed = parse_java_source(
        r#"
class DeferredExternalStorage {
    private void run(Config config) {
        Importer importer;
        try {
            importer = new Importer(config);
            Helper.update(importer);
        } catch (Exception ex) {
            throw ex;
        }
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let external = parse_external_facts(
        "external-argument update * 0 definitely-non-null read-only borrowed type=Importer\n",
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "DeferredExternalStorage".to_string(),
            rust_name: "run".to_string(),
        })
        .unwrap();
    let importer_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "importer")
        .unwrap();

    assert!(
        importer_constraints
            .constraints
            .contains(&SignatureConstraint::RequiredOwnership(
                OwnershipShape::Owned
            )),
        "{importer_constraints:?}"
    );

    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "DeferredExternalStorage")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let importer_shape = signatures
        .local_shape(&method_signature.locals, "importer")
        .expect("constructor-assigned local should have a solved shape");

    assert_eq!(importer_shape.ownership, OwnershipShape::Owned);

    let translation = translate_source_with_facts(
        r#"
class DeferredExternalStorage {
    private void run(Config config) {
        Importer importer;
        try {
            importer = new Importer(config);
            Helper.update(importer);
        } catch (Exception ex) {
            throw ex;
        }
    }
}
"#,
        &facts,
    );
    assert!(
        translation.rust.contains("let mut importer: Importer")
            && !translation.rust.contains("let mut importer: &Importer;"),
        "constructor-assigned local should render owned storage before borrowed uses:\n{}",
        translation.rust
    );

    let genomicsdb_translation = translate_source_with_facts(
        r#"
import com.intel.genomicsdb.GenomicsDBImporter;
import com.intel.genomicsdb.importer.model.ImportConfig;

class DeferredGenomicsDbStorage {
    private void run(ImportConfig config) {
        GenomicsDBImporter importer;
        try {
            importer = new GenomicsDBImporter(config);
            importer.executeImport(1);
        } catch (Exception ex) {
            throw ex;
        }
    }
}
"#,
        &new_java2rust::external_packs::genomicsdb_pack_facts(),
    );
    assert!(
        genomicsdb_translation
            .rust
            .contains("let mut importer: GenomicsDBImporter")
            && !genomicsdb_translation
                .rust
                .contains("let mut importer: &GenomicsDBImporter;"),
        "pack-owned constructor-assigned local should render owned storage:\n{}",
        genomicsdb_translation.rust
    );

    let helper_parsed = parse_java_source(
        r#"
class DeferredExternalStorageWithHelper {
    private void run(Config config, Interval[] intervals) {
        Importer importer;
        try {
            importer = new Importer(config);
            inspect(importer, intervals);
            importer.executeImport(1);
        } catch (Exception ex) {
            throw ex;
        }
    }

    private static void inspect(Importer importer, Interval[] intervals) {
        importer.describe();
        int count = intervals.length;
    }
}
"#,
    );
    let helper_resolved = resolve_program(&helper_parsed.program, helper_parsed.diagnostics);
    let helper_external = parse_external_facts(
        "\
external-api describe Importer getter read-only definitely-non-null
external-api executeImport Importer side-effect unknown
",
    );
    assert!(
        helper_external.diagnostics.is_empty(),
        "{:?}",
        helper_external.diagnostics
    );
    let mut helper_facts = helper_resolved.facts.clone();
    helper_facts.extend_from(helper_external.facts);
    let helper_usage = analyze_usage(&helper_resolved.program);
    let helper_representations = plan_representations(&helper_resolved.program, &helper_facts);
    let helper_signatures = solve_signatures_with_facts(
        &helper_resolved.program,
        &helper_representations,
        &helper_usage,
        &helper_facts,
    );
    let helper_ty = helper_resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "DeferredExternalStorageWithHelper")
        .unwrap();
    let helper_method = helper_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let helper_method_signature = helper_signatures
        .method_signature(helper_ty, helper_method)
        .unwrap();
    let helper_importer_shape = helper_signatures
        .local_shape(&helper_method_signature.locals, "importer")
        .expect("constructor-assigned helper local should have a solved shape");

    assert_eq!(helper_importer_shape.ownership, OwnershipShape::Owned);

    let helper_translation = translate_source_with_facts(
        r#"
class DeferredExternalStorageWithHelper {
    private void run(Config config, Interval[] intervals) {
        Importer importer;
        try {
            importer = new Importer(config);
            inspect(importer, intervals);
            importer.executeImport(1);
        } catch (Exception ex) {
            throw ex;
        }
    }

    private static void inspect(Importer importer, Interval[] intervals) {
        importer.describe();
        int count = intervals.length;
    }
}
"#,
        &helper_facts,
    );
    assert!(
        helper_translation
            .rust
            .contains("let mut importer: Importer")
            && !helper_translation
                .rust
                .contains("let mut importer: &Importer;"),
        "constructor-assigned local should stay owned even when an internal helper borrows it:\n{}",
        helper_translation.rust
    );
}

#[test]
fn signature_constraints_apply_external_call_return_nullability_to_locals() {
    let parsed = parse_java_source(
        r#"
class ExternalReturnLocal {
    private String label(Formatter formatter, String name, int count) {
        String label = formatter.renderLabel(name, count);
        return label;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let external = parse_external_facts(
        "external-api renderLabel Formatter getter read-only definitely-non-null\n",
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "ExternalReturnLocal".to_string(),
            rust_name: "label".to_string(),
        })
        .unwrap();
    let local_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "label")
        .unwrap();

    assert!(local_constraints.constraints.contains(
        &SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull)
    ));
}

#[test]
fn variantcontext_nullable_attribute_getter_flows_into_option_slot_planning() {
    let parsed = parse_java_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;

class VariantAttributeOptionPlanning {
    private Object attribute(VariantContext variant) {
        Object value = variant.getAttribute("DP");
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let mut facts = resolved.facts.clone();
    facts.extend_from(new_java2rust::external_packs::htsjdk_variantcontext_pack_facts());

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "VariantAttributeOptionPlanning".to_string(),
            rust_name: "attribute".to_string(),
        })
        .unwrap();
    let local_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();
    assert!(
        local_constraints
            .constraints
            .contains(&SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull
            )),
        "{local_constraints:?}"
    );

    let representations = plan_representations(&resolved.program, &facts);
    let direct_value_shape = solve_signature_shape_with_facts(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
            role: SignatureSlotRole::Storage,
            constraints: local_constraints.constraints.clone(),
        },
        &facts,
    )
    .unwrap();
    assert_eq!(
        direct_value_shape.java_ty,
        optional_java_type(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "VariantAttributeOptionPlanning")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "attribute" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let value_shape = signatures
        .local_shape(&method_signature.locals, "value")
        .expect("external nullable attribute local should have a solved shape");

    assert_eq!(
        value_shape.java_ty,
        optional_java_type(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        signatures.method_return_shape(ty, method).unwrap().java_ty,
        optional_java_type(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        })
    );
    assert!(matches!(
        rust_type_for_slot(value_shape),
        RustType::Option(_)
    ));
}

#[test]
fn source_nullability_constraints_shape_locals_params_and_returns() {
    let parsed = parse_java_source(
        r#"
class SourceNullableSlots {
    Object keep(Object value) {
        Object current = null;
        if (value == null) {
            return current;
        }
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "keep" => Some(method),
            _ => None,
        })
        .unwrap();
    let body = method.body.as_ref().unwrap();

    assert_eq!(
        source_binding_nullability_constraints(body, "current"),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::DefinitelyNull
        )]
    );
    assert_eq!(
        source_binding_nullability_constraints(body, "value"),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );
    assert_eq!(
        source_method_return_nullability_constraints(method),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );

    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let method_signature = signatures.method_signature(ty, method).unwrap();
    assert!(matches!(
        rust_type_for_slot(
            signatures
                .local_shape(&method_signature.locals, "current")
                .unwrap()
        ),
        RustType::Option(_)
    ));
    assert!(matches!(
        rust_type_for_slot(&method_signature.params[0].shape),
        RustType::Option(_)
    ));
    assert!(matches!(
        rust_type_for_slot(method_signature.return_shape.as_ref().unwrap()),
        RustType::Option(_)
    ));
}

#[test]
fn ternary_null_test_marks_overloaded_path_parameter_nullable() {
    let parsed = parse_java_source(
        r#"
import java.nio.file.Path;

class NullablePathOverloadArgument {
    void check(Path value) {
        check(value, null);
    }

    void check(Path value, Path optionalIndex) {
        Path index = optionalIndex != null ? optionalIndex : value.resolveSibling("index");
        use(index);
    }

    void use(Path path) {
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "NullablePathOverloadArgument")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.params.len() == 2 => Some(method),
            _ => None,
        })
        .unwrap();
    let body = method.body.as_ref().unwrap();
    assert_eq!(
        parse_expr("optionalIndex != null"),
        Expr::Binary {
            op: new_java2rust::java_ir::BinaryOp::Ne,
            left: Box::new(Expr::Var("optional_index".to_string())),
            right: Box::new(Expr::Literal(Literal::Null)),
        }
    );
    let initializer = body.statements.iter().find_map(|stmt| match stmt {
        Stmt::VarDecl {
            name,
            initializer: Some(expr),
            ..
        } if name == "index" => Some(expr),
        _ => None,
    });
    assert!(
        matches!(
            initializer,
            Some(Expr::Ternary { condition, .. })
                if matches!(
                    condition.as_ref(),
                    Expr::Binary {
                        op: new_java2rust::java_ir::BinaryOp::Ne,
                        left,
                        right,
                    } if matches!(left.as_ref(), Expr::Var(name) if name == "optional_index")
                        && matches!(right.as_ref(), Expr::Literal(Literal::Null))
                )
        ),
        "{initializer:#?}"
    );

    assert_eq!(
        source_binding_nullability_constraints(body, "optional_index"),
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );
    assert!(
        method_param_shape_constraints(ty, method, &method.params[1], &usage, &resolved.facts)
            .contains(&SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull
            )),
        "{:?}",
        method_param_shape_constraints(ty, method, &method.params[1], &usage, &resolved.facts)
    );
}

#[test]
fn direct_null_call_argument_marks_resolved_method_parameter_nullable() {
    let parsed = parse_java_source(
        r#"
import java.nio.file.Path;

class DirectNullMethodArgument {
    void call(Path path) {
        target(path, null);
    }

    void target(Path path, Path optionalIndex) {
        use(path);
    }

    void use(Path path) {
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "DirectNullMethodArgument")
        .unwrap();
    let target = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "target" => Some(method),
            _ => None,
        })
        .unwrap();
    let target_signature = signatures.method_signature(ty, target).unwrap();

    assert!(matches!(
        rust_type_for_slot(&target_signature.params[1].shape),
        RustType::Option(_)
    ));
}

#[test]
fn direct_null_constructor_delegation_marks_resolved_constructor_parameter_nullable() {
    let parsed = parse_java_source(
        r#"
import java.nio.file.Path;

class DirectNullConstructorDelegation {
    DirectNullConstructorDelegation() {
        this(null);
    }

    DirectNullConstructorDelegation(Path optionalPath) {
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "DirectNullConstructorDelegation")
        .unwrap();
    let delegated = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(constructor) if constructor.params.len() == 1 => Some(constructor),
            _ => None,
        })
        .unwrap();
    let constructor_signature = signatures.constructor_signature(ty, delegated).unwrap();

    assert!(matches!(
        rust_type_for_slot(&constructor_signature.params[0].shape),
        RustType::Option(_)
    ));
}

#[test]
fn constructor_initialized_variantcontext_field_stays_non_optional() {
    let parsed = parse_java_source(
        r#"
import htsjdk.variant.variantcontext.VariantContext;

class VariantHolder {
    private VariantContext variant;

    VariantHolder(VariantContext variant) {
        this.variant = variant;
    }

    VariantContext current() {
        return this.variant;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.extend_from(new_java2rust::external_packs::htsjdk_variantcontext_pack_facts());
    let semantic =
        new_java2rust::semantic_analysis::analyze_program(&resolved.program, &facts, &facts);
    let mut facts = facts;
    facts.extend_from(semantic.facts);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "VariantHolder")
        .unwrap();
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "variant" => Some(field),
            _ => None,
        })
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "current" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        facts.field_nullability_for("VariantHolder", "variant"),
        Some(NullabilityState::DefinitelyNonNull)
    );
    assert!(
        field_shape_constraints_with_usage(ty, field, &usage, &facts).contains(
            &SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull)
        )
    );
    assert_eq!(
        signatures
            .field_shape("VariantHolder", "variant")
            .unwrap()
            .java_ty,
        JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        signatures.method_return_shape(ty, method).unwrap().java_ty,
        JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        }
    );
    assert!(
        !matches!(
            rust_type_for_slot(signatures.field_shape("VariantHolder", "variant").unwrap()),
            RustType::Option(_)
        ),
        "constructor-proven non-null external fields should not be optionalized"
    );
}

#[test]
fn variantcontext_external_method_boundary_preserves_facade_shape() {
    let parsed = parse_java_source(
        r#"
import htsjdk.variant.variantcontext.Allele;
import htsjdk.variant.variantcontext.Genotype;
import htsjdk.variant.variantcontext.GenotypesContext;
import htsjdk.variant.variantcontext.VariantContext;

class VariantBoundaryUse {
    private VariantContext route(
            VariantBoundary boundary,
            VariantContext variant) {
        VariantContext copy = boundary.copy(variant);
        boundary.consume(copy);
        return copy;
    }

    private Allele routeAllele(VariantBoundary boundary, Allele allele) {
        Allele copy = boundary.copyAllele(allele);
        boundary.consumeAllele(copy);
        return copy;
    }

    private Genotype routeGenotype(VariantBoundary boundary, Genotype genotype) {
        Genotype copy = boundary.copyGenotype(genotype);
        boundary.consumeGenotype(copy);
        return copy;
    }

    private GenotypesContext routeGenotypes(VariantBoundary boundary, GenotypesContext genotypes) {
        GenotypesContext copy = boundary.copyGenotypes(genotypes);
        boundary.consumeGenotypes(copy);
        return copy;
    }
}

class VariantBoundary {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let external = parse_external_facts(
        r#"
external-api copy VariantBoundary getter read-only definitely-non-null args=1 return=VariantContext
external-api consume VariantBoundary side-effect read-only definitely-non-null args=1
external-api copyAllele VariantBoundary getter read-only definitely-non-null args=1 return=Allele
external-api consumeAllele VariantBoundary side-effect read-only definitely-non-null args=1
external-api copyGenotype VariantBoundary getter read-only definitely-non-null args=1 return=Genotype
external-api consumeGenotype VariantBoundary side-effect read-only definitely-non-null args=1
external-api copyGenotypes VariantBoundary getter read-only definitely-non-null args=1 return=GenotypesContext
external-api consumeGenotypes VariantBoundary side-effect read-only definitely-non-null args=1
"#,
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(new_java2rust::external_packs::htsjdk_variantcontext_pack_facts());
    facts.extend_from(external.facts);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "VariantBoundaryUse")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "route" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let variant_ty = JavaType::Named {
        name: "VariantContext".to_string(),
        args: Vec::new(),
    };
    let allele_ty = JavaType::Named {
        name: "Allele".to_string(),
        args: Vec::new(),
    };
    let genotype_ty = JavaType::Named {
        name: "Genotype".to_string(),
        args: Vec::new(),
    };
    let genotypes_ty = JavaType::Named {
        name: "GenotypesContext".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        facts
            .external_api_effect_for_receiver(Some("VariantBoundary"), "copy")
            .unwrap()
            .return_type,
        Some(variant_ty.clone())
    );
    assert_eq!(
        facts
            .external_api_effect_for_receiver(Some("VariantBoundary"), "copyAllele")
            .unwrap()
            .return_type,
        Some(allele_ty.clone())
    );
    assert_eq!(
        facts
            .external_api_effect_for_receiver(Some("VariantBoundary"), "copyGenotype")
            .unwrap()
            .return_type,
        Some(genotype_ty.clone())
    );
    assert_eq!(
        facts
            .external_api_effect_for_receiver(Some("VariantBoundary"), "copyGenotypes")
            .unwrap()
            .return_type,
        Some(genotypes_ty.clone())
    );
    assert_eq!(
        signatures
            .local_shape(&method_signature.locals, "copy")
            .unwrap()
            .java_ty,
        variant_ty.clone()
    );
    assert_eq!(
        signatures.method_return_shape(ty, method).unwrap().java_ty,
        variant_ty.clone()
    );
    assert_eq!(
        method_signature
            .params
            .iter()
            .find(|param| param.name == "variant")
            .unwrap()
            .shape
            .java_ty,
        variant_ty
    );

    for (method_name, param_name, expected_ty) in [
        ("routeAllele", "allele", allele_ty),
        ("routeGenotype", "genotype", genotype_ty),
        ("routeGenotypes", "genotypes", genotypes_ty),
    ] {
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == method_name => Some(method),
                _ => None,
            })
            .unwrap();
        let method_signature = signatures.method_signature(ty, method).unwrap();
        assert_eq!(
            signatures
                .local_shape(&method_signature.locals, "copy")
                .unwrap()
                .java_ty,
            expected_ty.clone()
        );
        assert_eq!(
            signatures.method_return_shape(ty, method).unwrap().java_ty,
            expected_ty.clone()
        );
        assert_eq!(
            method_signature
                .params
                .iter()
                .find(|param| param.name == param_name)
                .unwrap()
                .shape
                .java_ty,
            expected_ty
        );
    }
}

#[test]
fn usage_classifies_returned_new_results_as_unknown_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownDirectNewReturn {
    private ExternalName create() {
        return new ExternalName();
    }
}

class ExternalName {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownDirectNewReturn".to_string(),
            method_name: "create".to_string(),
        })
        .unwrap();

    assert!(method_usage.returned_call_results.iter().any(|returned| {
        returned.name == "new" && returned.boundary == CallArgumentBoundary::Unknown
    }));
}

#[test]
fn usage_preserves_returned_result_boundaries_through_casts_and_ternaries() {
    let parsed = parse_java_source(
        r#"
class UnknownReturnedExpression {
    private ExternalName casted(Helper helper) {
        return (ExternalName) helper.fetch();
    }

    private ExternalName chosen(boolean ready) {
        return ready ? new ExternalName() : new ExternalName();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let casted_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownReturnedExpression".to_string(),
            method_name: "casted".to_string(),
        })
        .unwrap();
    let chosen_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownReturnedExpression".to_string(),
            method_name: "chosen".to_string(),
        })
        .unwrap();

    assert!(casted_usage.call_sites.iter().any(|call_site| {
        call_site.name == "fetch"
            && call_site.result_returned
            && call_site.boundary == CallArgumentBoundary::Unknown
    }));
    assert!(casted_usage.returned_call_results.iter().any(|returned| {
        returned.name == "fetch" && returned.boundary == CallArgumentBoundary::Unknown
    }));
    assert_eq!(
        chosen_usage
            .returned_call_results
            .iter()
            .filter(|returned| {
                returned.name == "new" && returned.boundary == CallArgumentBoundary::Unknown
            })
            .count(),
        2
    );
}

#[test]
fn signature_constraints_apply_external_call_return_nullability_to_method_returns() {
    let parsed = parse_java_source(
        r#"
class ExternalReturnMethod {
    private String label(Formatter formatter, String name) {
        return formatter.renderLabel(name);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let external = parse_external_facts(
        "external-api renderLabel Formatter getter read-only definitely-non-null\n",
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "label" => Some(method),
            _ => None,
        })
        .unwrap();

    assert!(
        method_return_shape_constraints(ty, method, &usage, &facts).contains(
            &SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull)
        )
    );
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_constructor_locals_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownConstructorLocalSource {
    private UnknownConstructorLocalSource(Helper helper) {
        ExternalName value = helper.fetch();
        value.length();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let constructor_usage = usage
        .constructors
        .get(&new_java2rust::usage::UsageConstructorKey {
            type_name: "UnknownConstructorLocalSource".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();
    let value_usage = constructor_usage.bindings.get("value").unwrap();

    assert!(value_usage.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::AssignedFromCall {
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if name == "fetch"
        )
    }));

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    let constructor_constraints = constraints
        .constructors
        .get(&ConstructorKey {
            type_name: "UnknownConstructorLocalSource".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();
    let local_constraints = constructor_constraints
        .locals
        .iter()
        .find(|local| local.name == "value")
        .unwrap();

    assert_eq!(
        local_constraints.constraints,
        conservative_boundary_constraints(&external_name_type(), &facts)
    );
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_fields_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownFieldSource {
    ExternalName cached;

    private void use(Helper helper) {
        this.cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownFieldSource".to_string(),
            method_name: "use".to_string(),
        })
        .unwrap();

    assert!(method_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if field_name == "cached" && name == "fetch"
        )
    }));
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_bare_fields_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownBareFieldSource {
    ExternalName cached;

    private void use(Helper helper) {
        cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "UnknownBareFieldSource".to_string(),
            method_name: "use".to_string(),
        })
        .unwrap();

    assert!(method_usage.bindings.get("cached").is_none());
    assert!(method_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if field_name == "cached" && name == "fetch"
        )
    }));
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_constructor_fields_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownConstructorFieldSource {
    ExternalName cached;

    private UnknownConstructorFieldSource(Helper helper) {
        this.cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constructor_usage = usage
        .constructors
        .get(&new_java2rust::usage::UsageConstructorKey {
            type_name: "UnknownConstructorFieldSource".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();

    assert!(constructor_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if field_name == "cached" && name == "fetch"
        )
    }));
}

#[test]
fn usage_classifies_unknown_call_results_assigned_to_constructor_bare_fields_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownConstructorBareFieldSource {
    ExternalName cached;

    private UnknownConstructorBareFieldSource(Helper helper) {
        cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let constructor_usage = usage
        .constructors
        .get(&new_java2rust::usage::UsageConstructorKey {
            type_name: "UnknownConstructorBareFieldSource".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();

    assert!(constructor_usage.bindings.get("cached").is_none());
    assert!(constructor_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if field_name == "cached" && name == "fetch"
        )
    }));
}

#[test]
fn usage_classifies_fields_passed_to_unknown_calls_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownFieldArgument {
    ExternalName cached;

    private void explicit(Helper helper) {
        helper.consume(this.cached);
    }

    private void bare(Helper helper) {
        helper.consume(cached);
    }
}

class ExternalName {}
class Helper {
    void consume(ExternalName value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    for method_name in ["explicit", "bare"] {
        let method_usage = usage
            .methods
            .get(&new_java2rust::usage::UsageMethodKey {
                type_name: "UnknownFieldArgument".to_string(),
                method_name: method_name.to_string(),
            })
            .unwrap();

        assert!(method_usage.field_uses.iter().any(|usage| {
            matches!(
                usage,
                FieldUse::PassedToCall {
                    field_name,
                    name,
                    arg_index: 0,
                    boundary: CallArgumentBoundary::Unknown,
                } if field_name == "cached" && name == "consume"
            )
        }));
    }
}

#[test]
fn usage_classifies_fields_passed_to_new_and_delegating_constructors_as_boundaries() {
    let parsed = parse_java_source(
        r#"
class UnknownConstructorFieldArgument {
    ExternalName cached;

    UnknownConstructorFieldArgument() {
        this(cached);
    }

    UnknownConstructorFieldArgument(ExternalName value) {}

    private void explicit() {
        new Helper(this.cached);
    }

    private void bare() {
        new Helper(cached);
    }
}

class ExternalName {}
class Helper {
    Helper(ExternalName value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);

    for method_name in ["explicit", "bare"] {
        let method_usage = usage
            .methods
            .get(&new_java2rust::usage::UsageMethodKey {
                type_name: "UnknownConstructorFieldArgument".to_string(),
                method_name: method_name.to_string(),
            })
            .unwrap();

        assert!(method_usage.field_uses.iter().any(|usage| {
            matches!(
                usage,
                FieldUse::PassedToCall {
                    field_name,
                    name,
                    arg_index: 0,
                    boundary: CallArgumentBoundary::Unknown,
                } if field_name == "cached" && name == "new"
            )
        }));
    }

    let constructor_usage = usage
        .constructors
        .get(&new_java2rust::usage::UsageConstructorKey {
            type_name: "UnknownConstructorFieldArgument".to_string(),
            rust_name: "new".to_string(),
        })
        .unwrap();

    assert!(constructor_usage.field_uses.iter().any(|usage| {
        matches!(
        usage,
            FieldUse::PassedToCall {
                field_name,
                name,
                arg_index: 0,
                boundary: CallArgumentBoundary::InternalSameType,
            } if field_name == "cached" && name == "constructor"
        )
    }));
}

#[test]
fn signature_constraints_apply_unknown_call_result_boundary_to_fields() {
    let parsed = parse_java_source(
        r#"
class UnknownFieldSource {
    ExternalName cached;

    private void use(Helper helper) {
        this.cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();
    let mut expected = field_shape_constraints();
    expected.extend(conservative_boundary_constraints(
        &field.ty,
        &resolved.facts,
    ));

    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &resolved.facts),
        expected
    );

    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    assert_eq!(
        constraints
            .fields
            .get(&new_java2rust::signature::FieldKey {
                type_name: "UnknownFieldSource".to_string(),
                field_name: "cached".to_string(),
            })
            .unwrap(),
        &expected
    );
}

#[test]
fn signature_constraints_apply_external_call_return_nullability_to_fields() {
    let parsed = parse_java_source(
        r#"
class ExternalReturnField {
    String cached;

    private void cache(Formatter formatter, String name) {
        this.cached = formatter.renderLabel(name);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let external = parse_external_facts(
        "external-api renderLabel Formatter getter read-only definitely-non-null\n",
    );
    assert!(
        external.diagnostics.is_empty(),
        "{:?}",
        external.diagnostics
    );
    let mut facts = resolved.facts.clone();
    facts.extend_from(external.facts);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();

    assert!(
        field_shape_constraints_with_usage(ty, field, &usage, &facts).contains(
            &SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull)
        )
    );
}

#[test]
fn signature_constraints_apply_unknown_field_argument_boundary_to_fields() {
    let parsed = parse_java_source(
        r#"
class UnknownFieldArgument {
    ExternalName cached;

    private void use(Helper helper) {
        helper.consume(this.cached);
    }
}

class ExternalName {}
class Helper {
    void consume(ExternalName value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();
    let mut expected = field_shape_constraints();
    expected.extend(conservative_boundary_constraints(
        &external_name_type(),
        &facts,
    ));

    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &facts),
        expected
    );

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    assert_eq!(
        constraints
            .fields
            .get(&new_java2rust::signature::FieldKey {
                type_name: "UnknownFieldArgument".to_string(),
                field_name: "cached".to_string(),
            })
            .unwrap(),
        &expected
    );
}

#[test]
fn signature_constraints_apply_unknown_new_field_argument_boundary_to_fields() {
    let parsed = parse_java_source(
        r#"
class UnknownNewFieldArgument {
    ExternalName cached;

    private void use() {
        new Helper(cached);
    }
}

class ExternalName {}
class Helper {
    Helper(ExternalName value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();
    let mut expected = field_shape_constraints();
    expected.extend(conservative_boundary_constraints(
        &external_name_type(),
        &facts,
    ));

    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &facts),
        expected
    );
}

#[test]
fn signature_constraints_apply_unknown_call_result_boundary_to_bare_fields() {
    let parsed = parse_java_source(
        r#"
class UnknownBareFieldSource {
    ExternalName cached;

    private void use(Helper helper) {
        cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();
    let mut expected = field_shape_constraints();
    expected.extend(conservative_boundary_constraints(
        &field.ty,
        &resolved.facts,
    ));

    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &resolved.facts),
        expected
    );
}

#[test]
fn signature_constraints_apply_unknown_constructor_call_result_boundary_to_fields() {
    let parsed = parse_java_source(
        r#"
class UnknownConstructorFieldSource {
    ExternalName cached;

    private UnknownConstructorFieldSource(Helper helper) {
        this.cached = helper.fetch();
    }
}

class ExternalName {}
class Helper {
    ExternalName fetch() {
        return new ExternalName();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let facts = external_name_boundary_facts(&resolved.facts);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();
    let mut expected = field_shape_constraints();
    expected.extend(conservative_boundary_constraints(
        &external_name_type(),
        &facts,
    ));

    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &facts),
        expected
    );

    let constraints = collect_signature_constraints(&resolved.program, &usage, &facts);
    assert_eq!(
        constraints
            .fields
            .get(&new_java2rust::signature::FieldKey {
                type_name: "UnknownConstructorFieldSource".to_string(),
                field_name: "cached".to_string(),
            })
            .unwrap(),
        &expected
    );
}

#[test]
fn usage_records_returned_bare_field_facts_without_shadowing_locals() {
    let parsed = parse_java_source(
        r#"
class ReturnedBareFieldFlow {
    String cached;

    private String get() {
        return cached;
    }

    private String shadow() {
        String cached = "local";
        return cached;
    }
}
"#,
    );
    let usage = analyze_usage(&parsed.program);
    let get_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "ReturnedBareFieldFlow".to_string(),
            method_name: "get".to_string(),
        })
        .unwrap();
    let shadow_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "ReturnedBareFieldFlow".to_string(),
            method_name: "shadow".to_string(),
        })
        .unwrap();

    assert!(get_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::Returned { field_name } if field_name == "cached"
        )
    }));
    assert!(shadow_usage.field_uses.is_empty());
    assert!(
        shadow_usage
            .bindings
            .get("cached")
            .unwrap()
            .uses
            .iter()
            .any(|usage| matches!(usage, BindingUse::Returned))
    );
}

#[test]
fn usage_records_returned_self_field_facts() {
    let parsed = parse_java_source(
        r#"
class ReturnedFieldFlow {
    String cached;

    private String get() {
        return this.cached;
    }
}
"#,
    );
    let usage = analyze_usage(&parsed.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "ReturnedFieldFlow".to_string(),
            method_name: "get".to_string(),
        })
        .unwrap();

    assert!(method_usage.field_uses.iter().any(|usage| {
        matches!(
            usage,
            FieldUse::Returned { field_name } if field_name == "cached"
        )
    }));
}

#[test]
fn signature_fixpoint_propagates_returned_field_constraints_bidirectionally() {
    let method_key = MethodKey {
        type_name: "ReturnedFieldFlow".to_string(),
        rust_name: "get".to_string(),
    };
    let field_key = FieldKey {
        type_name: "ReturnedFieldFlow".to_string(),
        field_name: "cached".to_string(),
    };
    let mut constraints = SignatureConstraintState::default();
    constraints.methods.insert(
        method_key.clone(),
        MethodConstraintSet {
            return_constraints: vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
                SignatureConstraint::RequiredView(ViewShape::Str),
            ],
            ..MethodConstraintSet::default()
        },
    );
    constraints.fields.insert(
        field_key.clone(),
        vec![
            SignatureConstraint::StorageSlot,
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ],
    );
    let mut usage = new_java2rust::usage::UsageFacts::default();
    usage.methods.insert(
        new_java2rust::usage::UsageMethodKey {
            type_name: "ReturnedFieldFlow".to_string(),
            method_name: "get".to_string(),
        },
        new_java2rust::usage::MethodUsageFacts {
            field_uses: vec![FieldUse::Returned {
                field_name: "cached".to_string(),
            }],
            ..new_java2rust::usage::MethodUsageFacts::default()
        },
    );

    let (solved, _) = solve_signature_constraints_to_fixpoint(constraints, &usage);
    let return_constraints = &solved.methods.get(&method_key).unwrap().return_constraints;
    let field_constraints = solved.fields.get(&field_key).unwrap();

    assert!(
        return_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
        )),
        "{return_constraints:?}"
    );
    assert!(
        return_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredView(ViewShape::Native)
        )),
        "{return_constraints:?}"
    );
    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
        )),
        "{field_constraints:?}"
    );
    assert!(
        field_constraints.iter().any(|constraint| matches!(
            constraint,
            SignatureConstraint::RequiredView(ViewShape::Str)
        )),
        "{field_constraints:?}"
    );
}

#[test]
fn lowering_reports_signature_constraint_conflicts_as_diagnostics() {
    let translation = translate_source(
        r#"
class ConflictFlow {
    private int outer(Helper helper, String value) {
        helper.consume(value);
        return inner(value);
    }

    private int inner(String value) {
        return value.length();
    }
}

class Helper {
    int consume(String value) {
        return value.length();
    }
}
"#,
    );

    assert!(
        translation
            .diagnostics
            .has_code("signature_constraint_conflict"),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("ConflictFlow::outer parameter value")),
        "{:?}",
        translation.diagnostics
    );
}

#[test]
fn usage_records_resolved_internal_call_site_boundaries() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int outer(String value) {
        return inner(value);
    }

    private int inner(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "StringUse".to_string(),
            method_name: "outer".to_string(),
        })
        .unwrap();
    let call_site = method_usage
        .call_sites
        .iter()
        .find(|call_site| call_site.name == "inner")
        .unwrap();

    assert_eq!(call_site.arg_count, 1);
    assert!(call_site.result_returned);
    assert_eq!(call_site.boundary, CallArgumentBoundary::InternalSameType);
    assert_eq!(
        call_site
            .resolved
            .as_ref()
            .map(|resolved| (&resolved.declaring_type, &resolved.rust_name)),
        Some((&"StringUse".to_string(), &"inner".to_string()))
    );
}

#[test]
fn usage_treats_protected_same_type_calls_as_api_boundaries() {
    let parsed = parse_java_source(
        r#"
class ProtectedUse {
    private int outer(String value) {
        return inner(value);
    }

    protected int inner(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let method_usage = usage
        .methods
        .get(&new_java2rust::usage::UsageMethodKey {
            type_name: "ProtectedUse".to_string(),
            method_name: "outer".to_string(),
        })
        .unwrap();
    let call_site = method_usage
        .call_sites
        .iter()
        .find(|call_site| call_site.name == "inner")
        .unwrap();

    assert_eq!(call_site.arg_count, 1);
    assert!(call_site.result_returned);
    assert_eq!(call_site.boundary, CallArgumentBoundary::Unknown);
    assert_eq!(
        call_site
            .resolved
            .as_ref()
            .map(|resolved| (&resolved.declaring_type, &resolved.rust_name)),
        Some((&"ProtectedUse".to_string(), &"inner".to_string()))
    );
}

#[test]
fn call_graph_records_direct_and_mutual_method_edges() {
    let parsed = parse_java_source(
        r#"
class GraphCalls {
    int entry(int x) { return even(x); }
    int even(int x) { return x == 0 ? 1 : odd(x - 1); }
    int odd(int x) { return x == 0 ? 0 : even(x - 1); }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let graph = new_java2rust::usage::build_call_graph(&resolved.program);

    let entry = new_java2rust::usage::CallableKey::Method {
        type_name: "GraphCalls".to_string(),
        method_name: "entry".to_string(),
    };
    let even = new_java2rust::usage::CallableKey::Method {
        type_name: "GraphCalls".to_string(),
        method_name: "even".to_string(),
    };
    let odd = new_java2rust::usage::CallableKey::Method {
        type_name: "GraphCalls".to_string(),
        method_name: "odd".to_string(),
    };

    assert!(graph.edges.iter().any(|edge| {
        edge.caller == entry
            && edge.callee == even
            && edge.kind == new_java2rust::usage::CallGraphEdgeKind::DirectMethodCall
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.caller == even
            && edge.callee == odd
            && edge.kind == new_java2rust::usage::CallGraphEdgeKind::DirectMethodCall
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.caller == odd
            && edge.callee == even
            && edge.kind == new_java2rust::usage::CallGraphEdgeKind::DirectMethodCall
    }));
}

#[test]
fn call_graph_records_constructor_delegation_edges() {
    let parsed = parse_java_source(
        r#"
class GraphCtor {
    GraphCtor() { this(1); }
    GraphCtor(int x) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let graph = new_java2rust::usage::build_call_graph(&resolved.program);

    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::usage::CallGraphEdgeKind::ConstructorDelegationThis
            && matches!(
                &edge.caller,
                new_java2rust::usage::CallableKey::Constructor { type_name, .. }
                    if type_name == "GraphCtor"
            )
            && matches!(
                &edge.callee,
                new_java2rust::usage::CallableKey::Constructor { type_name, .. }
                    if type_name == "GraphCtor"
            )
    }));
}

#[test]
fn call_graph_records_lambda_and_unresolved_external_edges() {
    let parsed = parse_java_source(
        r#"
interface Runner { void run(); }
class GraphLambda {
    void use(Runner runner) {
        Runner local = () -> runner.run();
        missing();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let graph = new_java2rust::usage::build_call_graph(&resolved.program);

    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::usage::CallGraphEdgeKind::LambdaBody
            && matches!(
                edge.callee,
                new_java2rust::usage::CallableKey::Lambda { .. }
            )
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::usage::CallGraphEdgeKind::UnresolvedExternalMethodCall
            && matches!(
                &edge.callee,
                new_java2rust::usage::CallableKey::ExternalMethod { method_name, .. }
                    if method_name == "missing"
            )
    }));
}

#[test]
fn call_graph_records_anonymous_class_method_edges() {
    let parsed = parse_java_source(
        r#"
interface Runner { void run(); }
class GraphAnonymous {
    void helper() {}
    void use() {
        Runner local = new Runner() {
            public void run() { helper(); }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let graph = new_java2rust::usage::build_call_graph(&resolved.program);

    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::usage::CallGraphEdgeKind::AnonymousClassConstructor
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.kind == new_java2rust::usage::CallGraphEdgeKind::AnonymousClassMethod
            && matches!(
                &edge.callee,
                new_java2rust::usage::CallableKey::AnonymousMethod { method_name, .. }
                    if method_name == "run"
            )
    }));
}

#[test]
fn usage_owns_call_site_boundary_facts() {
    let usage_source = include_str!("../src/usage.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");

    assert!(usage_source.contains("pub struct CallSiteUse"));
    assert!(usage_source.contains("pub call_sites: Vec<CallSiteUse>"));
    assert!(usage_source.contains("pub struct MethodApiBoundaryPolicy"));
    assert!(usage_source.contains("pub fn method_api_boundary_policy"));
    assert!(usage_source.contains("pub fn call_argument_boundary_policy"));
    assert!(!usage_source.contains("fn method_is_api_boundary"));
    assert!(!slot_policy_source.contains("ResolvedCall"));
    assert!(!slot_policy_source.contains("resolved"));
}

#[test]
fn signature_constraints_make_public_api_fallback_explicit() {
    let parsed = parse_java_source(
        r#"
class PublicApi {
    public String id(String value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let expected = conservative_boundary_constraints(&JavaType::String, &resolved.facts);

    assert_eq!(
        method_param_shape_constraints(ty, method, &method.params[0], &usage, &resolved.facts),
        expected
    );
    assert_eq!(
        method_return_shape_constraints(ty, method, &usage, &resolved.facts),
        expected
    );
}

#[test]
fn signature_boundary_policy_owns_public_unknown_and_escape_constraints() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Animal".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: Some(true),
            provenance: FactProvenance::UserOverride,
        });
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let animal = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let unknown_object = JavaType::Unknown("Object".to_string());
    let wildcard_object = JavaType::Wildcard(None);
    let enum_overridden_object = JavaType::Named {
        name: "ExternalExpr".to_string(),
        args: Vec::new(),
    };
    let final_erased_facade = JavaType::Named {
        name: "FinalFacade".to_string(),
        args: Vec::new(),
    };
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalExpr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExternalExprValue".to_string(),
            variants: vec!["ExternalLiteral".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "FinalFacade".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "FinalFacade".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let public_copy =
        signature_boundary_policy(SignatureBoundaryKind::PublicApi, &external_id, &facts);
    let unknown_owned =
        signature_boundary_policy(SignatureBoundaryKind::UnknownApi, &external_handle, &facts);
    let escaping_owned = signature_boundary_policy(
        SignatureBoundaryKind::EscapingStorage,
        &external_handle,
        &facts,
    );
    let escaping_object =
        signature_boundary_policy(SignatureBoundaryKind::EscapingStorage, &animal, &facts);
    let public_object =
        signature_boundary_policy(SignatureBoundaryKind::PublicApi, &animal, &facts);
    let public_unknown_object =
        signature_boundary_policy(SignatureBoundaryKind::PublicApi, &unknown_object, &facts);
    let public_wildcard_object =
        signature_boundary_policy(SignatureBoundaryKind::PublicApi, &wildcard_object, &facts);
    let unknown_enum_overridden_object = signature_boundary_policy(
        SignatureBoundaryKind::UnknownApi,
        &enum_overridden_object,
        &facts,
    );
    let public_final_erased_facade = signature_boundary_policy(
        SignatureBoundaryKind::PublicApi,
        &final_erased_facade,
        &facts,
    );
    let escaping_unknown_object = signature_boundary_policy(
        SignatureBoundaryKind::EscapingStorage,
        &unknown_object,
        &facts,
    );
    let escaping_wildcard_object = signature_boundary_policy(
        SignatureBoundaryKind::EscapingStorage,
        &wildcard_object,
        &facts,
    );
    let native_copy_policy = signature_native_boundary_policy(&external_id, &facts);
    let native_owned_policy = signature_native_boundary_policy(&external_handle, &facts);
    let native_representation_policy =
        signature_boundary_representation_policy(&external_id, &facts);
    let interface_representation_policy = signature_boundary_representation_policy(&animal, &facts);
    let unknown_representation_policy =
        signature_boundary_representation_policy(&unknown_object, &facts);
    let wildcard_representation_policy =
        signature_boundary_representation_policy(&wildcard_object, &facts);
    let enum_override_representation_policy =
        signature_boundary_representation_policy(&enum_overridden_object, &facts);
    let final_erased_facade_representation_policy =
        signature_boundary_representation_policy(&final_erased_facade, &facts);
    let native_representation_fact_policy =
        signature_boundary_representation_fact_policy(&external_id, &facts);
    let interface_representation_fact_policy =
        signature_boundary_representation_fact_policy(&animal, &facts);
    let unknown_representation_fact_policy =
        signature_boundary_representation_fact_policy(&unknown_object, &facts);
    let wildcard_representation_fact_policy =
        signature_boundary_representation_fact_policy(&wildcard_object, &facts);
    let enum_override_representation_fact_policy =
        signature_boundary_representation_fact_policy(&enum_overridden_object, &facts);
    let final_erased_facade_representation_fact_policy =
        signature_boundary_representation_fact_policy(&final_erased_facade, &facts);
    let effective_facts = EffectiveFacts::new(&facts);
    let interface_effective_representation_fact_policy =
        signature_boundary_representation_fact_policy_with_effective_facts(
            &animal,
            &effective_facts,
        );
    let effective_public_copy = signature_boundary_policy_with_effective_facts(
        SignatureBoundaryKind::PublicApi,
        &external_id,
        &effective_facts,
    );
    let effective_escaping_object = signature_boundary_policy_with_effective_facts(
        SignatureBoundaryKind::EscapingStorage,
        &animal,
        &effective_facts,
    );
    let effective_native_copy =
        signature_native_boundary_policy_with_effective_facts(&external_id, &effective_facts);

    assert_eq!(public_copy.kind, SignatureBoundaryKind::PublicApi);
    assert_eq!(effective_public_copy, public_copy);
    assert_eq!(effective_escaping_object, escaping_object);
    assert_eq!(effective_native_copy, native_copy_policy);
    assert_eq!(
        native_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: external_id.clone(),
            object_representation_facts: false,
        }
    );
    assert_eq!(
        interface_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: animal.clone(),
            object_representation_facts: true,
        }
    );
    assert_eq!(
        interface_effective_representation_fact_policy,
        interface_representation_fact_policy
    );
    assert_eq!(
        unknown_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: unknown_object.clone(),
            object_representation_facts: false,
        }
    );
    assert_eq!(
        wildcard_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: wildcard_object.clone(),
            object_representation_facts: false,
        }
    );
    assert_eq!(
        enum_override_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: enum_overridden_object.clone(),
            object_representation_facts: true,
        }
    );
    assert_eq!(
        final_erased_facade_representation_fact_policy,
        SignatureBoundaryRepresentationFactPolicy {
            java_ty: final_erased_facade.clone(),
            object_representation_facts: true,
        }
    );
    assert_eq!(
        native_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: external_id.clone(),
            preserve_object_representation: false,
        }
    );
    assert_eq!(
        interface_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: animal.clone(),
            preserve_object_representation: true,
        }
    );
    assert_eq!(
        unknown_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: unknown_object.clone(),
            preserve_object_representation: true,
        }
    );
    assert_eq!(
        wildcard_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: wildcard_object.clone(),
            preserve_object_representation: true,
        }
    );
    assert_eq!(
        enum_override_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: enum_overridden_object.clone(),
            preserve_object_representation: true,
        }
    );
    assert_eq!(
        final_erased_facade_representation_policy,
        SignatureBoundaryRepresentationPolicy {
            java_ty: final_erased_facade.clone(),
            preserve_object_representation: false,
        }
    );
    assert_eq!(
        native_copy_policy,
        SignatureNativeBoundaryPolicy {
            java_ty: external_id.clone(),
            required_ownership: OwnershipShape::CopyValue,
            constraints: vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        }
    );
    assert_eq!(
        public_copy.fallback,
        SignatureBoundaryFallback::ConservativeNative
    );
    assert_eq!(
        public_copy.constraints,
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
    assert_eq!(unknown_owned.kind, SignatureBoundaryKind::UnknownApi);
    assert_eq!(
        native_owned_policy,
        SignatureNativeBoundaryPolicy {
            java_ty: external_handle.clone(),
            required_ownership: OwnershipShape::Owned,
            constraints: vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        }
    );
    assert_eq!(
        unknown_owned.fallback,
        SignatureBoundaryFallback::ConservativeNative
    );
    assert_eq!(
        unknown_owned.constraints,
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
    assert_eq!(escaping_owned.kind, SignatureBoundaryKind::EscapingStorage);
    assert_eq!(
        escaping_owned.fallback,
        SignatureBoundaryFallback::EscapingNativeStorage
    );
    assert_eq!(
        escaping_owned.constraints,
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
    assert_eq!(escaping_object.kind, SignatureBoundaryKind::EscapingStorage);
    assert_eq!(
        escaping_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage
    );
    assert_eq!(
        escaping_object.constraints,
        vec![SignatureConstraint::StorageSlot]
    );
    assert_eq!(
        public_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert!(public_object.constraints.is_empty());
    assert_eq!(
        public_unknown_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert!(public_unknown_object.constraints.is_empty());
    assert_eq!(
        public_wildcard_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert!(public_wildcard_object.constraints.is_empty());
    assert_eq!(
        unknown_enum_overridden_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert!(unknown_enum_overridden_object.constraints.is_empty());
    assert_eq!(
        public_final_erased_facade.fallback,
        SignatureBoundaryFallback::ConservativeNative
    );
    assert_eq!(
        public_final_erased_facade.constraints,
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
            SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        ]
    );

    assert_eq!(
        escaping_unknown_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage
    );
    assert_eq!(
        escaping_unknown_object.constraints,
        vec![SignatureConstraint::StorageSlot]
    );
    assert_eq!(
        escaping_wildcard_object.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage
    );
    assert_eq!(
        escaping_wildcard_object.constraints,
        vec![SignatureConstraint::StorageSlot]
    );

    assert_eq!(
        signature_boundary_constraints(
            SignatureBoundaryFallback::ConservativeNative,
            &external_id,
            &facts,
        ),
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
    assert_eq!(
        signature_boundary_constraints_with_effective_facts(
            SignatureBoundaryFallback::ConservativeNative,
            &external_id,
            &effective_facts,
        ),
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
    assert!(
        signature_boundary_constraints(
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            &animal,
            &facts,
        )
        .is_empty()
    );
    assert_eq!(
        signature_boundary_constraints(
            SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage,
            &animal,
            &facts,
        ),
        vec![SignatureConstraint::StorageSlot]
    );
    assert_eq!(
        signature_boundary_constraints(
            SignatureBoundaryFallback::EscapingNativeStorage,
            &external_handle,
            &facts,
        ),
        vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ]
    );
}

#[test]
fn signature_public_unknown_fallback_is_type_aware_across_java_type_categories() {
    let mut facts = FactSet::default();
    for (type_name, copy, object_safe_trait) in [
        ("Integer", Some(true), None),
        ("RunnableLike", None, Some(true)),
    ] {
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: type_name.to_string(),
                copy,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait,
                provenance: FactProvenance::UserOverride,
            });
    }
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expression".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExpressionValue".to_string(),
            variants: vec!["Literal".to_string(), "Binary".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "OpaqueRuntime".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "FinalFacade".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "FinalFacade".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .owned_recovery_capability_facts
        .push(OwnedRecoveryCapabilityFact {
            type_name: "FacadeHandle".to_string(),
            capability: OwnedRecoveryCapability::FacadeOwnedExternalHandle {
                java_ty: JavaType::Named {
                    name: "FacadeHandle".to_string(),
                    args: Vec::new(),
                },
            },
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let string_array = JavaType::Array(Box::new(JavaType::String));
    let cases = vec![
        (
            "primitive",
            JavaType::Primitive(PrimitiveType::Int),
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "boxed primitive",
            JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "string",
            JavaType::String,
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "array",
            string_array.clone(),
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "collection",
            JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::String],
            },
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "optional",
            JavaType::Named {
                name: "Optional".to_string(),
                args: vec![JavaType::String],
            },
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "raw object",
            JavaType::Unknown("Object".to_string()),
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            Vec::new(),
        ),
        (
            "wildcard",
            JavaType::Wildcard(None),
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            Vec::new(),
        ),
        (
            "interface",
            JavaType::Named {
                name: "RunnableLike".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            Vec::new(),
        ),
        (
            "enum",
            JavaType::Named {
                name: "Expression".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            Vec::new(),
        ),
        (
            "external mapped facade",
            JavaType::Named {
                name: "FacadeHandle".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        ),
        (
            "erased object",
            JavaType::Named {
                name: "OpaqueRuntime".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::PreserveObjectRepresentation,
            Vec::new(),
        ),
        (
            "exact final erased facade",
            JavaType::Named {
                name: "FinalFacade".to_string(),
                args: Vec::new(),
            },
            SignatureBoundaryFallback::ConservativeNative,
            vec![
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
                SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
            ],
        ),
    ];

    for (label, java_ty, expected_fallback, expected_constraints) in cases {
        let public = signature_boundary_policy(SignatureBoundaryKind::PublicApi, &java_ty, &facts);
        let unknown =
            signature_boundary_policy(SignatureBoundaryKind::UnknownApi, &java_ty, &facts);

        assert_eq!(public.fallback, expected_fallback, "{label}");
        assert_eq!(unknown.fallback, expected_fallback, "{label}");
        assert_eq!(public.constraints, expected_constraints, "{label}");
        assert_eq!(unknown.constraints, expected_constraints, "{label}");
    }

    assert_eq!(
        signature_owned_recovery_capability_policy(
            &JavaType::Named {
                name: "FacadeHandle".to_string(),
                args: Vec::new(),
            },
            &facts,
        )
        .recovery,
        OwnedRecoveryCapability::FacadeOwnedExternalHandle {
            java_ty: JavaType::Named {
                name: "FacadeHandle".to_string(),
                args: Vec::new(),
            },
        }
    );
}

#[test]
fn signature_boundary_preserves_unknown_erased_object_shape() {
    let representations = plan_representations(
        &parse_java_source("class Known {}").program,
        &FactSet::default(),
    );
    let shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Unknown("Object".to_string()),
            role: SignatureSlotRole::Param { is_varargs: false },
            constraints: conservative_boundary_constraints(
                &JavaType::Unknown("Object".to_string()),
                &FactSet::default(),
            ),
        },
    )
    .unwrap();

    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
}

#[test]
fn signature_boundary_preserves_wildcard_erased_object_shape() {
    let representations = plan_representations(
        &parse_java_source("class Known {}").program,
        &FactSet::default(),
    );
    let shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: JavaType::Wildcard(None),
            role: SignatureSlotRole::Param { is_varargs: false },
            constraints: conservative_boundary_constraints(
                &JavaType::Wildcard(None),
                &FactSet::default(),
            ),
        },
    )
    .unwrap();

    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
}

#[test]
fn signature_boundary_kind_merge_policy_owns_boundary_precedence() {
    assert_eq!(
        signature_boundary_kind_priority_policy(SignatureBoundaryKind::EscapingStorage),
        SignatureBoundaryKindPriorityPolicy {
            requirement: SignatureBoundaryKind::EscapingStorage,
            rank: 30,
        }
    );
    assert_eq!(
        signature_boundary_kind_priority_policy(SignatureBoundaryKind::PublicApi),
        SignatureBoundaryKindPriorityPolicy {
            requirement: SignatureBoundaryKind::PublicApi,
            rank: 20,
        }
    );
    assert_eq!(
        signature_boundary_kind_priority_policy(SignatureBoundaryKind::UnknownApi),
        SignatureBoundaryKindPriorityPolicy {
            requirement: SignatureBoundaryKind::UnknownApi,
            rank: 10,
        }
    );
    assert_eq!(
        merge_signature_boundary_kind_policy(None, Some(SignatureBoundaryKind::UnknownApi)),
        SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::UnknownApi),
            rejected: None,
        }
    );
    assert_eq!(
        merge_signature_boundary_kind_policy(
            Some(SignatureBoundaryKind::PublicApi),
            Some(SignatureBoundaryKind::UnknownApi),
        ),
        SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::PublicApi),
            rejected: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
    assert_eq!(
        merge_signature_boundary_kind_policy(
            Some(SignatureBoundaryKind::UnknownApi),
            Some(SignatureBoundaryKind::EscapingStorage),
        ),
        SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::EscapingStorage),
            rejected: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
    assert_eq!(
        merge_signature_boundary_kind_policy(
            Some(SignatureBoundaryKind::EscapingStorage),
            Some(SignatureBoundaryKind::PublicApi),
        ),
        SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::EscapingStorage),
            rejected: Some(SignatureBoundaryKind::PublicApi),
        }
    );
}

#[test]
fn signature_member_api_boundary_policy_names_public_and_internal_members() {
    assert_eq!(
        signature_member_api_boundary_policy(&[Modifier::Public]),
        SignatureMemberApiBoundaryPolicy { public_api: true }
    );
    assert_eq!(
        signature_member_api_boundary_policy(&[Modifier::Protected]),
        SignatureMemberApiBoundaryPolicy { public_api: true }
    );
    assert_eq!(
        signature_member_api_boundary_policy(&[Modifier::Private]),
        SignatureMemberApiBoundaryPolicy { public_api: false }
    );
    assert_eq!(
        signature_member_api_boundary_policy(&[]),
        SignatureMemberApiBoundaryPolicy { public_api: false }
    );
}

#[test]
fn binding_boundary_policy_owns_binding_use_boundary_classification() {
    let unknown_binding = BindingUseFacts {
        name: "value".to_string(),
        java_ty: JavaType::String,
        uses: vec![BindingUse::PassedToCall {
            name: "external".to_string(),
            arg_index: 0,
            boundary: CallArgumentBoundary::Unknown,
        }],
    };
    let stored_binding = BindingUseFacts {
        name: "stored".to_string(),
        java_ty: JavaType::String,
        uses: vec![BindingUse::StoredInField {
            field_name: "field".to_string(),
        }],
    };
    let mixed_binding = BindingUseFacts {
        name: "mixed".to_string(),
        java_ty: JavaType::String,
        uses: vec![
            BindingUse::PassedToCall {
                name: "external".to_string(),
                arg_index: 0,
                boundary: CallArgumentBoundary::Unknown,
            },
            BindingUse::StoredInField {
                field_name: "field".to_string(),
            },
        ],
    };

    assert_eq!(
        binding_boundary_policy(&unknown_binding),
        BindingBoundaryPolicy {
            binding_name: "value".to_string(),
            kind: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
    assert_eq!(
        binding_boundary_policy(&stored_binding),
        BindingBoundaryPolicy {
            binding_name: "stored".to_string(),
            kind: Some(SignatureBoundaryKind::EscapingStorage),
        }
    );
    assert_eq!(
        binding_boundary_policy(&mixed_binding),
        BindingBoundaryPolicy {
            binding_name: "mixed".to_string(),
            kind: Some(SignatureBoundaryKind::EscapingStorage),
        }
    );
}

#[test]
fn param_boundary_policies_own_param_use_boundary_classification() {
    let public_parsed = parse_java_source(
        r#"
class PublicParamPolicySource {
    public PublicParamPolicySource(String seed) {}

    public void expose(String value) {}
}
"#,
    );
    let public_resolved = resolve_program(&public_parsed.program, public_parsed.diagnostics);
    let public_ty = &public_resolved.program.types[0];
    let public_ctor = public_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let public_method = public_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();
    let empty_usage = new_java2rust::usage::UsageFacts::default();

    assert_eq!(
        constructor_param_boundary_policy(
            public_ty,
            public_ctor,
            &public_ctor.params[0],
            &empty_usage
        ),
        ConstructorParamBoundaryPolicy {
            type_name: "PublicParamPolicySource".to_string(),
            constructor_name: "new".to_string(),
            param_name: "seed".to_string(),
            kind: Some(SignatureBoundaryKind::PublicApi),
        }
    );
    assert_eq!(
        method_param_boundary_policy(
            public_ty,
            public_method,
            &public_method.params[0],
            &empty_usage
        ),
        MethodParamBoundaryPolicy {
            type_name: "PublicParamPolicySource".to_string(),
            method_name: "expose".to_string(),
            param_name: "value".to_string(),
            kind: Some(SignatureBoundaryKind::PublicApi),
        }
    );

    let unknown_parsed = parse_java_source(
        r#"
class UnknownParamPolicySource {
    private UnknownParamPolicySource(String seed, Helper helper) {
        helper.consume(seed);
    }

    private void send(String value, Helper helper) {
        helper.consume(value);
    }
}

class Helper {
    void consume(String value) {}
}
"#,
    );
    let unknown_resolved = resolve_program(&unknown_parsed.program, unknown_parsed.diagnostics);
    let usage = analyze_usage(&unknown_resolved.program);
    let unknown_ty = &unknown_resolved.program.types[0];
    let unknown_ctor = unknown_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let unknown_method = unknown_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "send" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        constructor_param_boundary_policy(
            unknown_ty,
            unknown_ctor,
            &unknown_ctor.params[0],
            &usage
        ),
        ConstructorParamBoundaryPolicy {
            type_name: "UnknownParamPolicySource".to_string(),
            constructor_name: "new".to_string(),
            param_name: "seed".to_string(),
            kind: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
    assert_eq!(
        method_param_boundary_policy(
            unknown_ty,
            unknown_method,
            &unknown_method.params[0],
            &usage
        ),
        MethodParamBoundaryPolicy {
            type_name: "UnknownParamPolicySource".to_string(),
            method_name: "send".to_string(),
            param_name: "value".to_string(),
            kind: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
}

#[test]
fn field_boundary_policy_owns_field_use_boundary_classification() {
    let parsed = parse_java_source(
        r#"
class UnknownFieldPolicySource {
    String cached;

    private void use(Helper helper) {
        this.cached = helper.fetch();
    }
}

class Helper {
    String fetch() {
        return "cached";
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "cached" => Some(field),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        field_boundary_policy(ty, field, &usage),
        FieldBoundaryPolicy {
            type_name: "UnknownFieldPolicySource".to_string(),
            field_name: "cached".to_string(),
            kind: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
}

#[test]
fn method_return_boundary_policy_owns_return_use_boundary_classification() {
    let public_parsed = parse_java_source(
        r#"
class PublicReturnPolicySource {
    public String expose() {
        return "value";
    }
}
"#,
    );
    let public_resolved = resolve_program(&public_parsed.program, public_parsed.diagnostics);
    let public_ty = &public_resolved.program.types[0];
    let public_method = public_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_return_boundary_policy(
            public_ty,
            public_method,
            &new_java2rust::usage::UsageFacts::default(),
            &FactSet::default(),
        ),
        MethodReturnBoundaryPolicy {
            type_name: "PublicReturnPolicySource".to_string(),
            method_name: "expose".to_string(),
            kind: Some(SignatureBoundaryKind::PublicApi),
        }
    );

    let unknown_parsed = parse_java_source(
        r#"
class UnknownReturnPolicySource {
    private String load(Helper helper) {
        return helper.fetch();
    }
}

class Helper {
    String fetch() {
        return "value";
    }
}
"#,
    );
    let unknown_resolved = resolve_program(&unknown_parsed.program, unknown_parsed.diagnostics);
    let usage = analyze_usage(&unknown_resolved.program);
    let unknown_ty = &unknown_resolved.program.types[0];
    let unknown_method = unknown_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "load" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_return_boundary_policy(unknown_ty, unknown_method, &usage, &FactSet::default()),
        MethodReturnBoundaryPolicy {
            type_name: "UnknownReturnPolicySource".to_string(),
            method_name: "load".to_string(),
            kind: Some(SignatureBoundaryKind::UnknownApi),
        }
    );
}

#[test]
fn signature_boundary_policy_is_the_single_boundary_constraint_entrypoint() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub enum SignatureBoundaryKind"));
    assert!(source.contains("pub enum SignatureBoundaryRole"));
    assert!(source.contains("pub struct SignatureBoundaryRolePolicy"));
    assert!(source.contains("pub fn signature_boundary_role_policies"));
    assert!(source.contains("pub fn signature_boundary_role_policy"));
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    assert!(!source.contains("fn member_is_api_boundary"));
    assert!(source.contains("pub struct SignatureBoundaryKindMerge"));
    assert!(source.contains("pub struct SignatureBoundaryKindPriorityPolicy"));
    assert!(source.contains("pub fn signature_boundary_kind_priority_policy"));
    assert!(source.contains("pub fn merge_signature_boundary_kind_policy"));
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    assert!(source.contains("pub enum SignatureBoundaryFallback"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationFactPolicy"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationPolicy"));
    assert!(source.contains("pub struct SignatureNativeBoundaryPolicy"));
    assert!(source.contains("PreserveObjectRepresentationEscapingStorage"));
    assert!(source.contains("EscapingNativeStorage"));
    assert!(source.contains("pub fn signature_boundary_policy"));
    assert!(source.contains("pub fn signature_boundary_representation_fact_policy"));
    assert!(source.contains("signature_boundary_representation_fact_policy_with_effective_facts"));
    assert!(source.contains("pub fn signature_boundary_representation_policy"));
    assert!(source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(source.contains("pub fn signature_boundary_policy_with_effective_facts"));
    assert!(source.contains("pub fn signature_native_boundary_policy"));
    assert!(source.contains("pub fn signature_native_boundary_policy_with_effective_facts"));
    assert!(source.contains("signature_capability_policy_with_effective_facts(java_ty, facts)"));
    assert!(source.contains("fn signature_boundary_fallback_with_effective_facts"));
    assert!(source.contains("pub fn signature_boundary_constraints"));
    assert!(source.contains("pub fn signature_boundary_constraints_with_effective_facts"));
    assert!(!source.contains("fn boundary_type_has_object_representation"));
    assert!(!source.contains("fn native_boundary_constraints"));
    assert!(source.contains("pub struct BindingBoundaryPolicy"));
    assert!(source.contains("pub fn binding_boundary_policy"));
    assert!(!source.contains("fn binding_boundary_kind"));
    assert!(source.contains("pub struct MethodParamBoundaryPolicy"));
    assert!(source.contains("pub fn method_param_boundary_policy"));
    assert!(!source.contains("fn method_param_boundary_kind"));
    assert!(source.contains("pub struct ConstructorParamBoundaryPolicy"));
    assert!(source.contains("pub fn constructor_param_boundary_policy"));
    assert!(!source.contains("fn constructor_param_boundary_kind"));
    assert!(source.contains("pub struct FieldBoundaryPolicy"));
    assert!(source.contains("pub fn field_boundary_policy"));
    assert!(!source.contains("fn field_boundary_kind"));
    assert!(source.contains("pub struct MethodReturnBoundaryPolicy"));
    assert!(source.contains("pub fn method_return_boundary_policy"));
    assert!(!source.contains("fn method_return_boundary_kind"));
    assert!(!source.contains("fn binding_requires_conservative_boundary"));
    assert!(!source.contains("fn field_requires_unknown_boundary"));
    assert!(!source.contains("fn method_return_requires_conservative_boundary"));
    assert!(!source.contains("fn public_api_boundary_constraints"));
    assert!(!source.contains("fn escaping_boundary_constraints"));
}

#[test]
fn signature_boundary_role_inventory_covers_all_sig1_roles() {
    let policies = signature_boundary_role_policies();
    let roles = policies
        .iter()
        .map(|policy| policy.role)
        .collect::<HashSet<_>>();
    let expected = [
        SignatureBoundaryRole::MethodParam,
        SignatureBoundaryRole::MethodReturn,
        SignatureBoundaryRole::ConstructorParam,
        SignatureBoundaryRole::MethodLocal,
        SignatureBoundaryRole::ConstructorLocal,
        SignatureBoundaryRole::Field,
        SignatureBoundaryRole::MethodReceiver,
        SignatureBoundaryRole::CapturedValue,
        SignatureBoundaryRole::CallArgument,
        SignatureBoundaryRole::CallResult,
        SignatureBoundaryRole::ConstructorInvocation,
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    assert_eq!(roles, expected);
    assert_eq!(policies.len(), expected.len());
    for role in expected {
        let SignatureBoundaryRolePolicy {
            role: actual_role,
            constraint_owner,
            participates_in_dependency_graph,
        } = signature_boundary_role_policy(role);
        assert_eq!(actual_role, role);
        assert!(!constraint_owner.is_empty(), "{role:?}");
        assert!(participates_in_dependency_graph, "{role:?}");
    }
}

#[test]
fn signature_boundary_constraints_cover_params_locals_fields_returns_and_receivers() {
    let parsed = parse_java_source(
        r#"
import java.util.List;

class BoundaryRoleFixture {
    String field;

    public BoundaryRoleFixture(String seed, List<String> helper) {
        String local = helper.get(0);
        this.field = seed;
        helper.add(local);
    }

    public String expose(String input, List<String> helper) {
        String local = helper.get(0);
        this.field = local;
        helper.add(input);
        return local;
    }

    Runnable capture(String captured) {
        return new Runnable() {
            public void run() {
                helper(captured);
            }
        };
    }

    void helper(String value) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "BoundaryRoleFixture")
        .unwrap();
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let expose = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "expose" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        constructor_param_boundary_policy(ty, ctor, &ctor.params[0], &usage).kind,
        Some(SignatureBoundaryKind::PublicApi)
    );
    assert_eq!(
        constructor_param_boundary_policy(ty, ctor, &ctor.params[1], &usage).kind,
        Some(SignatureBoundaryKind::PublicApi)
    );
    assert!(
        field_shape_constraints_with_usage(ty, field, &usage, &resolved.facts)
            .contains(&SignatureConstraint::StorageSlot),
        "field storage remains a boundary role"
    );
    assert_eq!(
        method_param_boundary_policy(ty, expose, &expose.params[0], &usage).kind,
        Some(SignatureBoundaryKind::PublicApi)
    );
    assert_eq!(
        method_return_boundary_policy(ty, expose, &usage, &FactSet::default()).kind,
        Some(SignatureBoundaryKind::PublicApi)
    );
    assert_eq!(
        receiver_constraints_with_usage(
            expose,
            usage.methods.get(&new_java2rust::usage::UsageMethodKey {
                type_name: ty.name.clone(),
                method_name: expose.name.clone(),
            })
        ),
        vec![ReceiverConstraint::MutableReceiver]
    );
    assert!(
        method_return_shape_constraints(ty, expose, &usage, &resolved.facts).contains(
            &SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
        ),
        "public return should receive public API boundary constraints"
    );
    assert!(
        signatures
            .method_local_shapes_or_empty(ty, expose)
            .iter()
            .any(|local| local.name == "local"),
        "method locals should have signature slots"
    );
}

#[test]
fn unknown_api_arguments_preserve_object_representation_boundaries() {
    let parsed = parse_java_source(
        r#"
interface Animal {
    String name();
}

class Caller {
    private void run(Animal animal, Helper helper) {
        helper.visit(animal);
    }
}

class Helper {
    void visit(Animal animal) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Caller")
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let animal_ty = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let expected = signature_boundary_policy(
        SignatureBoundaryKind::UnknownApi,
        &animal_ty,
        &resolved.facts,
    );

    assert_eq!(
        expected.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert_eq!(
        method_param_shape_constraints(ty, method, &method.params[0], &usage, &resolved.facts),
        expected.constraints
    );
}

#[test]
fn unknown_api_field_arguments_preserve_object_representation_boundaries() {
    let parsed = parse_java_source(
        r#"
interface Animal {
    String name();
}

class Caller {
    private Animal cached;

    private void run(Helper helper) {
        helper.visit(this.cached);
    }
}

class Helper {
    void visit(Animal animal) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Caller")
        .unwrap();
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let animal_ty = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let expected_boundary = signature_boundary_policy(
        SignatureBoundaryKind::UnknownApi,
        &animal_ty,
        &resolved.facts,
    );

    assert_eq!(
        expected_boundary.fallback,
        SignatureBoundaryFallback::PreserveObjectRepresentation
    );
    assert_eq!(
        field_shape_constraints_with_usage(ty, field, &usage, &resolved.facts),
        field_shape_constraints()
    );
}

#[test]
fn signature_constraints_treat_protected_members_as_api_boundaries() {
    let parsed = parse_java_source(
        r#"
class ProtectedApi {
    protected String exposed;

    protected ProtectedApi(String value) {}

    protected String id(String value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let ty = &resolved.program.types[0];
    let expected = conservative_boundary_constraints(&JavaType::String, &resolved.facts);
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let mut expected_field = field_shape_constraints();
    expected_field.extend(expected.clone());

    assert_eq!(
        field_shape_constraints_with_facts(ty, field, &resolved.facts),
        expected_field
    );
    assert_eq!(
        constructor_param_shape_constraints(ty, ctor, &ctor.params[0], &usage, &resolved.facts),
        expected
    );
    assert_eq!(
        method_param_shape_constraints(ty, method, &method.params[0], &usage, &resolved.facts),
        expected
    );
    assert_eq!(
        method_return_shape_constraints(ty, method, &usage, &resolved.facts),
        expected
    );
}

#[test]
fn field_nullability_shape_policy_optionalizes_only_nullable_reference_storage() {
    let parsed = parse_java_source(
        r#"
class NullFields {
    String maybe;
    String proven;
    int count;
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let ty = &resolved.program.types[0];
    let field = |name: &str| {
        ty.members
            .iter()
            .find_map(|member| match member {
                Member::Field(field) if field.name == name => Some(field),
                _ => None,
            })
            .unwrap()
    };
    let mut facts = resolved.facts.clone();
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FactFieldKey {
            type_name: "NullFields".to_string(),
            field_name: "maybe".to_string(),
        },
        state: NullabilityState::MaybeNull,
        provenance: FieldNullabilityProvenance::ConservativeFallback,
    });
    facts.field_nullability_facts.push(FieldNullabilityFact {
        field: FactFieldKey {
            type_name: "NullFields".to_string(),
            field_name: "proven".to_string(),
        },
        state: NullabilityState::DefinitelyNonNull,
        provenance: FieldNullabilityProvenance::ConstructorAssignments,
    });

    assert_eq!(
        field_nullability_shape_policy(ty, field("maybe"), &facts).constraints,
        vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull
        )]
    );
    assert_eq!(
        merge_storage_nullability_requirement_policy(
            Some(NullabilityState::DefinitelyNonNull),
            NullabilityState::MaybeNull,
        )
        .kept,
        NullabilityState::MaybeNull
    );

    let maybe_shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: field("maybe").ty.clone(),
            role: SignatureSlotRole::Storage,
            constraints: field_shape_constraints_with_facts(ty, field("maybe"), &facts),
        },
    )
    .unwrap();
    let proven_shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: field("proven").ty.clone(),
            role: SignatureSlotRole::Storage,
            constraints: field_shape_constraints_with_facts(ty, field("proven"), &facts),
        },
    )
    .unwrap();
    let primitive_shape = solve_signature_shape(
        &representations,
        SignatureShapeRequest {
            java_ty: field("count").ty.clone(),
            role: SignatureSlotRole::Storage,
            constraints: vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull,
            )],
        },
    )
    .unwrap();

    assert_eq!(maybe_shape.java_ty, optional_java_type(JavaType::String));
    assert_eq!(proven_shape.java_ty, JavaType::String);
    assert_eq!(
        primitive_shape.java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
}

#[test]
fn signature_constraints_make_public_field_api_fallback_explicit() {
    let parsed = parse_java_source(
        r#"
class PublicFields {
    public String exposed;
    private String internal;
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let ty = &resolved.program.types[0];
    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .collect::<Vec<_>>();
    let exposed = fields.iter().find(|field| field.name == "exposed").unwrap();
    let internal = fields
        .iter()
        .find(|field| field.name == "internal")
        .unwrap();
    let expected_public = {
        let mut constraints = field_shape_constraints();
        constraints.extend(conservative_boundary_constraints(
            &JavaType::String,
            &resolved.facts,
        ));
        constraints
    };

    assert_eq!(
        field_shape_constraints_with_facts(ty, exposed, &resolved.facts),
        expected_public
    );
    assert_eq!(
        field_shape_constraints_with_facts(ty, internal, &resolved.facts),
        field_shape_constraints()
    );

    let usage = analyze_usage(&resolved.program);
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);

    assert_eq!(
        constraints
            .fields
            .get(&new_java2rust::signature::FieldKey {
                type_name: "PublicFields".to_string(),
                field_name: "exposed".to_string(),
            })
            .unwrap(),
        &expected_public
    );
    assert_eq!(
        constraints
            .fields
            .get(&new_java2rust::signature::FieldKey {
                type_name: "PublicFields".to_string(),
                field_name: "internal".to_string(),
            })
            .unwrap(),
        &field_shape_constraints()
    );
}

#[test]
fn signature_public_interface_boundaries_preserve_representation_shapes() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Shelter {
    public Animal adopt(Animal animal) {
        return animal;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let shelter = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Shelter")
        .unwrap();
    let method = shelter
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.required_method_signature(shelter, method);

    assert!(conservative_boundary_constraints(&method.params[0].ty, &resolved.facts).is_empty());
    assert_eq!(
        signature.params[0].shape.dispatch,
        DispatchShape::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(
        signature.params[0].shape.ownership,
        OwnershipShape::SharedBorrow
    );
    assert_eq!(signature.params[0].shape.view, ViewShape::DynTraitView);

    let return_shape = signature.return_shape.as_ref().unwrap();
    assert_eq!(
        return_shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(return_shape.ownership, OwnershipShape::Owned);
    assert_eq!(return_shape.view, ViewShape::DynTraitView);
}

#[test]
fn signature_unknown_interface_param_api_preserves_borrowed_dyn_shape() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Shelter {
    private void send(Animal animal, Helper helper) {
        helper.consume(animal);
    }
}
class Helper {
    void consume(Animal animal) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let shelter = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Shelter")
        .unwrap();
    let method = shelter
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_param_shape_constraints(shelter, method, &method.params[0], &usage, &resolved.facts),
        signature_boundary_policy(
            SignatureBoundaryKind::UnknownApi,
            &method.params[0].ty,
            &resolved.facts
        )
        .constraints
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let signature = signatures.required_method_signature(shelter, method);
    let animal = signature
        .params
        .iter()
        .find(|param| param.name == "animal")
        .unwrap();

    assert_eq!(
        animal.shape.dispatch,
        DispatchShape::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(animal.shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(animal.shape.view, ViewShape::DynTraitView);
}

#[test]
fn signature_unknown_interface_local_api_preserves_storage_role_shape() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Shelter {
    private void run(Helper helper) {
        Animal animal = helper.fetch();
        helper.consume(animal);
    }
}
class Helper {
    Animal fetch() { return new Dog(); }
    void consume(Animal animal) {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let shelter = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Shelter")
        .unwrap();
    let method = shelter
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let constraints = collect_signature_constraints(&resolved.program, &usage, &resolved.facts);
    let method_constraints = constraints
        .methods
        .get(&MethodKey {
            type_name: "Shelter".to_string(),
            rust_name: "run".to_string(),
        })
        .unwrap();
    let local_constraints = method_constraints
        .locals
        .iter()
        .find(|local| local.name == "animal")
        .unwrap();

    assert_eq!(
        local_constraints.constraints,
        signature_boundary_policy(
            SignatureBoundaryKind::UnknownApi,
            &local_constraints.java_ty,
            &resolved.facts
        )
        .constraints
    );

    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &resolved.facts);
    let signature = signatures.required_method_signature(shelter, method);
    let animal = signature
        .locals
        .iter()
        .find(|local| local.name == "animal")
        .unwrap();

    assert_eq!(
        animal.shape.dispatch,
        DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(animal.shape.ownership, OwnershipShape::Owned);
    assert_eq!(animal.shape.view, ViewShape::DynTraitView);
}

#[test]
fn lowering_uses_boxed_dyn_coercion_for_concrete_interface_field_assignment() {
    let translation = translate_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Shelter {
    Animal resident;
    public void set(Dog dog) {
        this.resident = dog;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.resident = Box::new(dog) as Box<dyn Animal>;"),
        "{}",
        translation.rust
    );
}

#[test]
fn lowering_uses_boxed_dyn_coercion_for_concrete_interface_returns() {
    let translation = translate_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Shelter {
    public Animal adopt(Dog dog) {
        return dog;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("return Box::new(dog) as Box<dyn Animal>;"),
        "{}",
        translation.rust
    );
}

#[test]
fn lowering_borrows_boxed_dyn_field_for_interface_arguments() {
    let translation = translate_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Shelter {
    Animal resident;
    public void visit(Animal animal) {}
    public void run() {
        visit(this.resident);
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("self.visit(self.resident.as_ref() as &dyn Animal);"),
        "{}",
        translation.rust
    );
}

#[test]
fn lowering_emits_dyn_trait_methods_impls_and_owned_interface_boundaries_together() {
    let translation = translate_source(
        r#"
interface Metric {
    int score(String sample);
    void merge(Metric other);
}
class CountMetric implements Metric {
    int value;
    public int score(String sample) { return this.value; }
    public void merge(Metric other) { this.value = this.value + other.score("x"); }
}
class FixedMetric implements Metric {
    public int score(String sample) { return 1; }
    public void merge(Metric other) {}
}
class Registry {
    Metric current;
    public Metric fresh(CountMetric metric) { return metric; }
    public void install(CountMetric metric) { this.current = metric; }
    public int rank(Metric left, Metric right) {
        return left.score("a") + right.score("b");
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("pub trait Metric {\n    fn score(&self, sample: String) -> i32;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("    fn merge(&mut self, other: &dyn Metric);"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("impl Metric for CountMetric"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("impl Metric for FixedMetric"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "fn merge(&mut self, other: &dyn Metric) { CountMetric::merge(self, other); }"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn fresh(&self, metric: CountMetric) -> Box<dyn Metric>"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return Box::new(metric) as Box<dyn Metric>;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.current = Box::new(metric) as Box<dyn Metric>;"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
}

#[test]
fn slot_policy_and_signature_solver_use_type_capability_facts() {
    let parsed = parse_java_source(
        r#"
class ExternalUse {
    private static ExternalName id(ExternalName value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalName".to_string(),
            copy: None,
            clone: None,
            string_like: Some(true),
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let representations = plan_representations(&resolved.program, &facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        method_param_borrowed_content_policy(ty, method, &method.params[0], &usage),
        None
    );
    assert_eq!(
        method_param_borrowed_content_policy_with_facts(
            ty,
            method,
            &method.params[0],
            &usage,
            &facts,
        ),
        Some(BorrowedContentPolicy::string_like_view())
    );
    let borrowed_return =
        borrowed_return_shape_from_param_with_facts(ty, method, &representations, &usage, &facts)
            .unwrap();
    assert_eq!(borrowed_return.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(borrowed_return.view, ViewShape::Str);

    let conservative_signatures = solve_signatures(&resolved.program, &representations, &usage);
    let fact_signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let conservative_param = &conservative_signatures
        .method_param_plans(ty, method)
        .unwrap()[0];
    let fact_param = &fact_signatures.method_param_plans(ty, method).unwrap()[0];

    assert_eq!(conservative_param.shape.ownership, OwnershipShape::Owned);
    assert_eq!(fact_param.shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(fact_param.shape.view, ViewShape::Str);

    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    let signature_source = include_str!("../src/signature.rs");
    let representation_source = include_str!("../src/representation.rs");
    let object_model_source = include_str!("../src/object_model.rs");
    let facts_source = include_str!("../src/facts.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    assert!(slot_policy_source.contains("pub struct BorrowedContentCapabilityPolicy"));
    assert!(slot_policy_source.contains("pub fn borrowed_content_capability_policy"));
    assert!(slot_policy_source.contains("borrowed_content_capability_policy_with_effective_facts"));
    assert!(slot_policy_source.contains("pub struct BorrowedContentTypePolicy"));
    assert!(slot_policy_source.contains("pub fn borrowed_content_type_policy"));
    assert!(slot_policy_source.contains("borrowed_content_type_policy_with_effective_facts"));
    assert!(
        slot_policy_source
            .contains("pub fn method_param_borrowed_content_policy_with_effective_facts")
    );
    assert!(
        slot_policy_source
            .contains("pub fn constructor_param_borrowed_content_policy_with_effective_facts")
    );
    assert!(
        slot_policy_source
            .contains("pub fn borrowed_return_content_policy_from_param_with_effective_facts")
    );
    assert!(
        slot_policy_source.contains("pub fn borrowed_return_shape_from_param_with_effective_facts")
    );
    assert!(slot_policy_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(!slot_policy_source.contains("fn borrowed_content_policy_for_type"));
    assert!(!slot_policy_source.contains("fn can_borrow_content"));
    assert_eq!(
        slot_policy_source
            .matches("borrowed_content_type_policy(&")
            .count(),
        0
    );
    assert!(effective_facts_source.contains("pub fn is_string_like_type"));
    assert!(effective_facts_source.contains("pub fn sequence_element_type_for"));
    assert!(effective_facts_source.contains("pub struct CastTypeCapabilityPolicy"));
    assert!(effective_facts_source.contains("pub struct InstanceOfTargetCapabilityPolicy"));
    assert!(effective_facts_source.contains("pub fn cast_type_capability_policy"));
    assert!(effective_facts_source.contains("pub fn instanceof_target_capability_policy"));
    assert!(facts_source.contains("pub fn cast_type_capability_policy"));
    assert!(facts_source.contains("pub fn instanceof_target_capability_policy"));
    assert!(facts_source.contains("EffectiveFacts::new(self)"));
    assert!(facts_source.contains(".cast_type_capability_policy(actual, target)"));
    assert!(facts_source.contains(".instanceof_target_capability_policy(target)"));
    assert!(conversion_source_contains_capability_owned_cast_checks());
    assert!(!slot_policy_source.contains(".string_like"));
    assert!(slot_render_source.contains("slot_content_capability_policy_with_effective_facts"));
    assert!(slot_render_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(!slot_render_source.contains("TypeCapabilities::conservative"));
    assert!(!slot_render_source.contains(".string_like"));
    assert!(
        signature_source
            .contains("signature_owned_recovery_capability_policy_with_effective_facts")
    );
    assert!(signature_source.contains("signature_capability_policy_with_effective_facts"));
    assert!(signature_source.contains("signature_owned_recovery_policy_from_capability"));
    assert!(facts_source.contains("pub enum DispatchRequirementKind"));
    assert!(facts_source.contains("pub struct DispatchRequirementFact"));
    assert!(effective_facts_source.contains("pub struct EffectiveFacts"));
    assert!(effective_facts_source.contains("pub fn has_object_representation_facts"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_for"));
    assert!(effective_facts_source.contains("pub fn is_copy_type"));
    assert!(effective_facts_source.contains("pub fn closed_hierarchy_variants_for"));
    assert!(effective_facts_source.contains("pub fn object_safe_trait_fact_for_name"));
    assert!(!signature_source.contains("capabilities.copy"));
    assert!(!signature_source.contains("capabilities.clone"));
    assert!(!signature_source.contains("type_capability_overrides_for"));
    assert!(object_model_source.contains("object_safe_trait_fact_for_name"));
    assert!(representation_source.contains("hierarchy_edges_for_type"));
    assert!(representation_source.contains("pub struct SourceSummarySuperclassBoundaryPolicy"));
    assert!(representation_source.contains("pub fn source_summary_superclass_boundary_policy"));
    assert!(
        representation_source
            .contains("extend_representation_facts_for_source_summary_superclass_boundaries")
    );
    assert!(representation_source.contains("DependencyMode::SourceSummary"));
    assert!(object_model_source.contains("closed_enum_proof_policy_with_effective_facts"));
    assert!(!representation_source.contains("type_capabilities_for"));
    assert!(!representation_source.contains("type_capability_overrides"));
}

fn conversion_source_contains_capability_owned_cast_checks() -> bool {
    let conversion_source = include_str!("../src/conversion_plan.rs");
    conversion_source.contains("facts.cast_type_capability_policy")
        && conversion_source.contains("facts.instanceof_target_capability_policy")
        && !conversion_source.contains("contains_wildcard_type")
        && !conversion_source.contains("is_known_raw_generic_type")
        && !conversion_source.contains("is_raw_generic_counterpart")
        && !conversion_source.contains("is_unchecked_generic_counterpart")
        && !conversion_source.contains("is_optional_or_result_type")
}

#[test]
fn effective_fact_precedence_queries_are_audited_by_consumer_module() {
    fn assert_count(source_name: &str, source: &str, needle: &str, expected: usize) {
        assert_eq!(
            source.matches(needle).count(),
            expected,
            "{source_name} should contain {expected} occurrence(s) of {needle:?}"
        );
    }

    let object_model_source = include_str!("../src/object_model.rs");
    let representation_source = include_str!("../src/representation.rs");
    let signature_source = include_str!("../src/signature.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let conversion_plan_source = include_str!("../src/conversion_plan.rs");
    let semantic_analysis_source = include_str!("../src/semantic_analysis.rs");
    let facts_source = include_str!("../src/facts.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(effective_facts_source.contains("TypeCapabilities::conservative"));
    assert!(effective_facts_source.contains("pub struct EffectiveFact<T>"));
    assert!(effective_facts_source.contains("pub provenance: FactProvenance"));
    assert!(effective_facts_source.contains("fn fact_precedence_key"));
    assert!(effective_facts_source.contains(
        "FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => 2"
    ));
    assert!(effective_facts_source.contains("FactProvenance::UserOverride => 4"));
    assert!(effective_facts_source.contains("fn apply_type_capability_override"));
    assert!(effective_facts_source.contains("pub fn type_capabilities_for"));
    assert!(effective_facts_source.contains("object_safe_trait_fact_for_name_with_provenance"));
    assert!(effective_facts_source.contains("closed_hierarchy_fact_for_name_with_provenance"));
    assert!(effective_facts_source.contains("pub fn object_safety_fact_for_name"));
    assert!(!facts_source.contains("fn apply_type_capability_override"));
    assert!(facts_source.contains("EffectiveFacts::new(self).type_capabilities_for(java_ty)"));
    assert!(
        facts_source.contains("EffectiveFacts::new(self).owned_recovery_capability_for(java_ty)")
    );

    assert!(object_model_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(object_model_source.contains("plan_object_model_with_effective_facts"));
    assert_count(
        "object_model",
        object_model_source,
        "facts.hierarchy_edges_for_type",
        1,
    );
    assert_count("object_model", object_model_source, "extends_type_for", 1);
    assert_count(
        "object_model",
        object_model_source,
        "implemented_types_for",
        1,
    );
    assert_count(
        "object_model",
        object_model_source,
        "closed_hierarchy_fact_for_name",
        2,
    );
    assert_count(
        "object_model",
        object_model_source,
        "object_safe_trait_fact_for_name",
        1,
    );

    assert!(representation_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(
        representation_source.contains("representation_hierarchy_fact_policy_with_effective_facts")
    );
    assert_count(
        "representation",
        representation_source,
        "extends_type_for",
        1,
    );
    assert_count(
        "representation",
        representation_source,
        "implemented_types_for",
        1,
    );
    assert_count(
        "representation",
        representation_source,
        "hierarchy_edges_for_type",
        1,
    );

    assert!(signature_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(
        signature_source
            .contains("signature_owned_recovery_capability_policy_with_effective_facts")
    );
    assert!(
        signature_source
            .contains("signature_boundary_representation_fact_policy_with_effective_facts")
    );
    assert!(signature_source.contains("signature_capability_policy_with_effective_facts"));
    assert_count(
        "signature",
        signature_source,
        "owned_recovery_capability_for",
        1,
    );
    assert_count(
        "signature",
        signature_source,
        "has_object_representation_facts",
        1,
    );
    assert_count("signature", signature_source, "is_copy_type", 1);

    assert!(shape_projection_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(
        shape_projection_source.contains("shape_projection_capability_policy_with_effective_facts")
    );
    assert_count(
        "shape_projection",
        shape_projection_source,
        "is_copy_type",
        1,
    );
    assert_count(
        "shape_projection",
        shape_projection_source,
        "sequence_element_type_for",
        1,
    );

    assert!(slot_policy_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(slot_policy_source.contains("borrowed_content_capability_policy_with_effective_facts"));
    assert_count("slot_policy", slot_policy_source, "is_string_like_type", 1);
    assert_count(
        "slot_policy",
        slot_policy_source,
        "sequence_element_type_for",
        1,
    );

    assert!(slot_render_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(slot_render_source.contains("slot_content_capability_policy_with_effective_facts"));
    assert_count("slot_render", slot_render_source, "is_string_like_type", 1);
    assert_count(
        "slot_render",
        slot_render_source,
        "sequence_element_type_for",
        1,
    );

    assert!(stream_shape_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(stream_shape_source.contains("stream_source_capability_policy_with_effective_facts"));
    assert_count(
        "stream_shape",
        stream_shape_source,
        "sequence_element_type_for",
        1,
    );

    assert!(!coercion_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_plan_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_plan_source.contains("coercion_content_policy_with_effective_facts"));
    assert!(conversion_plan_source.contains("coercion_shape_content_policy_with_effective_facts"));
    assert!(
        conversion_plan_source
            .contains("coercion_owned_recovery_capability_policy_with_effective_facts")
    );
    assert_count("coercion", coercion_source, "is_string_like_type", 0);
    assert_count(
        "conversion_plan",
        conversion_plan_source,
        "is_string_like_type",
        1,
    );
    assert_count(
        "conversion_plan",
        conversion_plan_source,
        "owned_recovery_capability_for",
        1,
    );

    assert_count(
        "semantic_analysis",
        semantic_analysis_source,
        "reachable_hierarchy_edges_for_type",
        1,
    );
}

#[test]
fn signature_capability_policy_owns_copy_baseline_constraints() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: Some(false),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let copy_policy = signature_capability_policy(&external_id, &facts);
    let non_copy_policy = signature_capability_policy(&external_handle, &facts);
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(copy_policy.java_ty, external_id.clone());
    assert_eq!(
        signature_capability_policy_with_effective_facts(&external_id, &effective_facts),
        copy_policy
    );
    assert_eq!(
        copy_policy.capability_ownership,
        Some(OwnershipShape::CopyValue)
    );
    assert_eq!(
        copy_policy.constraints,
        vec![SignatureConstraint::RequiredOwnership(
            OwnershipShape::CopyValue,
        )]
    );
    assert_eq!(non_copy_policy.java_ty, external_handle.clone());
    assert_eq!(
        signature_capability_policy_with_effective_facts(&external_handle, &effective_facts),
        non_copy_policy
    );
    assert_eq!(non_copy_policy.capability_ownership, None);
    assert!(non_copy_policy.constraints.is_empty());

    let source = include_str!("../src/signature.rs");
    assert!(source.contains("pub fn signature_capability_policy_with_effective_facts"));
    assert_eq!(source.matches("is_copy_type").count(), 1);
}

#[test]
fn signature_slot_baseline_policy_composes_capability_and_boundary_constraints() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(
        signature_slot_baseline_policy(&external_id, None, &facts),
        SignatureSlotBaselinePolicy {
            java_ty: external_id.clone(),
            boundary_kind: None,
            capability_constraints: vec![SignatureConstraint::RequiredOwnership(
                OwnershipShape::CopyValue,
            )],
            boundary_constraints: Vec::new(),
            constraints: vec![SignatureConstraint::RequiredOwnership(
                OwnershipShape::CopyValue,
            )],
        }
    );
    assert_eq!(
        signature_slot_baseline_policy_with_effective_facts(&external_id, None, &effective_facts),
        signature_slot_baseline_policy(&external_id, None, &facts)
    );

    let boundary_constraints =
        conservative_boundary_constraints(&external_handle, &FactSet::default());
    let default_facts = FactSet::default();
    let default_effective_facts = EffectiveFacts::new(&default_facts);
    assert_eq!(
        conservative_boundary_constraints_with_effective_facts(
            &external_handle,
            &default_effective_facts,
        ),
        boundary_constraints
    );
    assert_eq!(
        signature_slot_baseline_policy(
            &external_handle,
            Some(SignatureBoundaryKind::UnknownApi),
            &FactSet::default(),
        ),
        SignatureSlotBaselinePolicy {
            java_ty: external_handle,
            boundary_kind: Some(SignatureBoundaryKind::UnknownApi),
            capability_constraints: Vec::new(),
            boundary_constraints: boundary_constraints.clone(),
            constraints: boundary_constraints,
        }
    );

    let source = include_str!("../src/signature.rs");
    assert!(source.contains("pub struct SignatureSlotBaselinePolicy"));
    assert!(source.contains("pub fn signature_slot_baseline_policy"));
    assert!(source.contains("pub fn signature_slot_baseline_policy_with_effective_facts"));
    assert!(!source.contains("fn signature_slot_baseline_constraints("));
}

#[test]
fn signature_baseline_constraints_use_copy_capability_facts_for_all_slots() {
    let parsed = parse_java_source(
        r#"
class CopyFactUse {
    ExternalId field;

    private ExternalId echo(ExternalId value) {
        ExternalId local = value;
        this.field = value;
        return local;
    }
}

class ExternalId {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let representations = plan_representations(&resolved.program, &facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = signatures.required_method_signature(ty, method);
    let local = signature
        .locals
        .iter()
        .find(|local| local.name == "local")
        .unwrap();

    assert_eq!(
        signatures
            .required_field_shape("CopyFactUse", "field")
            .ownership,
        OwnershipShape::CopyValue
    );
    assert_eq!(
        signature.params[0].shape.ownership,
        OwnershipShape::CopyValue
    );
    assert_eq!(
        signature.return_shape.as_ref().unwrap().ownership,
        OwnershipShape::CopyValue
    );
    assert_eq!(local.shape.ownership, OwnershipShape::CopyValue);
}

#[test]
fn signature_boundary_slots_keep_capability_baseline_constraints() {
    let parsed = parse_java_source(
        r#"
interface ExternalId {
    int id();
}

class BoundaryCapabilityUse {
    public ExternalId exposed;

    public BoundaryCapabilityUse(ExternalId value) {
        this.exposed = value;
    }

    public ExternalId echo(ExternalId value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let mut facts = resolved.facts.clone();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "BoundaryCapabilityUse")
        .unwrap();
    let field = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "echo" => Some(method),
            _ => None,
        })
        .unwrap();
    let copy_constraint = SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue);

    assert!(
        field_shape_constraints_with_facts(ty, field, &facts).contains(&copy_constraint),
        "{:?}",
        field_shape_constraints_with_facts(ty, field, &facts)
    );
    assert!(
        constructor_param_shape_constraints(ty, ctor, &ctor.params[0], &usage, &facts)
            .contains(&copy_constraint),
        "{:?}",
        constructor_param_shape_constraints(ty, ctor, &ctor.params[0], &usage, &facts)
    );
    assert!(
        method_param_shape_constraints(ty, method, &method.params[0], &usage, &facts)
            .contains(&copy_constraint),
        "{:?}",
        method_param_shape_constraints(ty, method, &method.params[0], &usage, &facts)
    );
    assert!(
        method_return_shape_constraints(ty, method, &usage, &facts).contains(&copy_constraint),
        "{:?}",
        method_return_shape_constraints(ty, method, &usage, &facts)
    );
}

#[test]
fn call_lowering_uses_callee_param_shape_for_argument_coercion() {
    let translation = translate_source(
        r#"
class StringCalls {
    private int emptyScore(String value) {
        return value.isEmpty() ? 1 : 0;
    }

    public int call(String value) {
        return emptyScore(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn empty_score(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.empty_score(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn cross_class_call_lowering_uses_declaring_type_param_shape() {
    let translation = translate_source(
        r#"
class Helper {
    int score(String value) {
        return value.isEmpty() ? 1 : 0;
    }
}

class Caller {
    public int call(String value) {
        Helper helper = new Helper();
        return helper.score(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn score(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("helper.score(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn call_plan_does_not_render_rust_expressions() {
    let source = include_str!("../src/call_plan.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("RustType"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("emit_type"));
    assert!(!source.contains("apply_call_target"));
    assert!(!source.contains("apply_new_target"));
}

#[test]
fn call_lowering_renders_constructor_owner_types_with_facts() {
    let source = include_str!("../src/call_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("pub fn apply_new_target_with_facts"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&target.owner_shape)"));
    assert!(lower_source.contains("apply_new_target_with_facts(&target, rust_args, ctx.facts)"));
}

#[test]
fn external_static_string_constant_support_is_generated_from_field_facts() {
    let source = r#"
class UseExternalConstant {
    String key() {
        return ExternalConstants.NAME;
    }
}
"#;
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "external_constants".to_string(),
            field_name: "NAME".to_string(),
        },
        field_type: Some(JavaType::String),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct ExternalConstants"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub const NAME: &'static str = \"NAME\";"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("ExternalConstants::NAME"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_static_primitive_constant_support_is_generated_from_field_facts() {
    let source = r#"
class UseExternalPrimitiveConstantCaller {
    UseExternalPrimitiveConstant make() {
        return new UseExternalPrimitiveConstant();
    }
}

class UseExternalPrimitiveConstant {
    int count = ExternalCounts.MAX_COUNT;
}
"#;
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "external_counts".to_string(),
            field_name: "MAX_COUNT".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct ExternalCounts"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub const MAX_COUNT: i32 = 0;"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("ExternalCounts::MAX_COUNT"),
        "{}",
        translation.rust
    );

    let builtin_translation = translate_source(
        r#"
import htsjdk.variant.variantcontext.GenotypeLikelihoods;

class BuiltinPrimitiveConstantCaller {
    BuiltinPrimitiveConstant make() {
        return new BuiltinPrimitiveConstant();
    }
}

class BuiltinPrimitiveConstant {
    int count = GenotypeLikelihoods.MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED;
}
"#,
    );
    assert!(
        builtin_translation
            .rust
            .contains("pub struct GenotypeLikelihoods"),
        "{}",
        builtin_translation.rust
    );
    assert!(
        builtin_translation
            .rust
            .contains("pub const MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED: i32 = 0;"),
        "{}",
        builtin_translation.rust
    );
    assert!(
        builtin_translation
            .rust
            .contains("GenotypeLikelihoods::MAX_DIPLOID_ALT_ALLELES_THAT_CAN_BE_GENOTYPED"),
        "{}",
        builtin_translation.rust
    );
}

#[test]
fn static_string_constant_support_extends_existing_opaque_facade_owner() {
    let source = r#"
class ExternalConstantFacadeCollision {
    void accept(ExternalConstants constants) {
    }

    String key() {
        return ExternalConstants.NAME;
    }
}
"#;
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "external_constants".to_string(),
            field_name: "NAME".to_string(),
        },
        field_type: Some(JavaType::String),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert_eq!(
        translation
            .rust
            .matches("pub struct ExternalConstants")
            .count(),
        1,
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("impl ExternalConstants {\n    pub const NAME: &'static str = \"NAME\";"),
        "{}",
        translation.rust
    );
}

#[test]
fn context_static_string_constant_owner_stays_sibling_source_type() {
    let source = r#"
class UsesSiblingStaticConstant {
    SiblingConstants make() {
        return new SiblingConstants();
    }

    String key() {
        return SiblingConstants.NAME;
    }
}
"#;
    let sibling = r#"
class SiblingConstants {
    public static final String NAME = "name";
}
"#
    .to_string();

    let translation =
        translate_source_with_context_sources(source, &[sibling], &FactSet::default());

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("SiblingConstants::NAME"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("pub struct SiblingConstants;"),
        "{}",
        translation.rust
    );
}

#[test]
fn conventional_external_static_string_constants_are_typed_and_supported() {
    let source = r#"
class UseExternalStringConstants {
    String message() {
        return CommandNames.INPUT_LONG_NAME + ExternalDelimiters.FIELD_DELIM;
    }

    void configure(HeaderBuilder builder) {
        builder.addDelimiter(ExternalDelimiters.FIELD_DELIM);
    }
}
"#;

    let translation = translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct CommandNames"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub const INPUT_LONG_NAME: &'static str = \"INPUT_LONG_NAME\";"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub struct ExternalDelimiters"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub const FIELD_DELIM: &'static str = \"FIELD_DELIM\";"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("CommandNames::INPUT_LONG_NAME"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("ExternalDelimiters::FIELD_DELIM"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_support_struct_fields_are_generated_from_field_facts() {
    let source = r#"
class UseExternalRecord {
    void touch(ExternalRecord record) {
    }
}
"#;
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "ExternalRecord".to_string(),
            field_name: "batchCount".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "ExternalRecord".to_string(),
            field_name: "totalBatchCount".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("#[derive(Clone, Debug, Default)]\npub struct ExternalRecord {"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub batch_count: i32,"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub total_batch_count: i32,"),
        "{}",
        translation.rust
    );
}

#[test]
fn external_nested_builder_facade_support_is_generated_from_facts_and_used_calls() {
    let source = r#"
class UseExternalBuilder {
    Object make() {
        ExternalFactory.Product.Builder builder = Product.newBuilder();
        builder.setName("x");
        return builder.build();
    }

    ExternalFactory.Product makeProduct(ExternalFactory.Product.Builder builder) {
        return builder.build();
    }

    Object rebuild(Object value) {
        return value.toBuilder();
    }

    public static ExternalFactory.Product expose(ExternalFactory.Product product) {
        return product.toBuilder().build();
    }

    public static HashMap<String, Integer> indexes(ExternalFactory.Product product) {
        HashMap<String, Integer> result = new HashMap<>();
        for (int i = 0; i < product.getFieldsCount(); ++i) {
            result.put(product.getFields(i).getName(), i);
        }
        return result;
    }
}
"#;
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalFactory.Product".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalFactory.Product".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalFactory.Product.Builder".to_string(),
        strategy: HierarchyStrategy::ErasedObject,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalFactory.Product.Builder".to_string(),
            copy: None,
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FactFieldKey {
            type_name: "ExternalFactory".to_string(),
            field_name: "Product".to_string(),
        },
        field_type: Some(JavaType::Named {
            name: "Product".to_string(),
            args: Vec::new(),
        }),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "newBuilder".to_string(),
        receiver_type: Some("Product".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "Product.Builder".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "toBuilder".to_string(),
        receiver_type: Some("Product".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "Product.Builder".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "build".to_string(),
        receiver_type: Some("ExternalFactory.Product.Builder".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalFactory.Product".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getFieldsCount".to_string(),
        receiver_type: Some("ExternalFactory.Product".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getFieldsCount".to_string(),
        receiver_type: Some("Product".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getFields".to_string(),
        receiver_type: Some("ExternalFactory.Product".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::IndexedGetter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalFactory.FieldInfo".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getFields".to_string(),
        receiver_type: Some("Product".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::IndexedGetter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalFactory.FieldInfo".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "getName".to_string(),
        receiver_type: Some("ExternalFactory.FieldInfo".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::String),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub struct ProductBuilder"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub type ExternalFactoryProduct = Product;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub type ExternalFactoryProductBuilder = ProductBuilder;"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn new_builder() -> ProductBuilder"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("pub fn set_name"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn build(&self) -> ExternalFactoryProduct"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub fn make_product(&self, builder: &ExternalFactoryProductBuilder) -> ExternalFactoryProduct"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return builder.build();"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub trait JavaErasedObjectFacade"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("fn to_builder(&self) -> ProductBuilder"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn expose(product: ExternalFactoryProduct) -> ExternalFactoryProduct"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return product.to_builder().build();"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains(
            "pub fn indexes(product: &ExternalFactoryProduct) -> std::collections::HashMap<String, i32>"
        ),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("product.get_fields(i).get_name()"),
        "{}",
        translation.rust
    );
}

#[test]
fn erased_object_facade_includes_modeled_void_methods() {
    let source = r#"
class ErasedVoidFacadeUse {
    void run(Object handle, Object payload) {
        handle.apply(payload);
        handle.close();
    }
}
"#;
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "apply".to_string(),
        receiver_type: Some("ExternalHandle".to_string()),
        arg_count: Some(1),
        role: ExternalApiRole::Mutator,
        receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "close".to_string(),
        receiver_type: Some("ExternalHandle".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::SideEffect,
        receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
        return_type: None,
        return_nullability: NullabilityState::Unknown,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(source, &facts);

    assert!(
        translation
            .rust
            .contains("pub trait JavaErasedObjectFacade"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("fn apply<T0>(&self, _arg0: T0) -> ();"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("fn close(&self) -> ();"),
        "{}",
        translation.rust
    );
}

#[test]
fn nullable_object_slot_wraps_concrete_return_after_erasure_once() {
    let source = r#"
class NullableObjectFlow {
    Object rebuild(Importer importer) {
        Object value = importer.readMapping();
        if (value == null) {
            return null;
        }
        return value.toBuilder();
    }
}

class Importer {
    VidMappingPB readMapping() {
        return new VidMappingPB();
    }
}

class VidMappingPB {
    VidMappingPBBuilder toBuilder() {
        return new VidMappingPBBuilder();
    }
}

class VidMappingPBBuilder {}
"#;

    let translation = translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("let value: Option<Box<dyn std::any::Any>> = Some(Box::new(importer.read_mapping()) as Box<dyn std::any::Any>);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("Some(Box::new(Box::new(importer.read_mapping())"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("value.as_ref().unwrap().to_builder()"),
        "{}",
        translation.rust
    );
}

#[test]
fn java_object_return_type_lowers_to_erased_any_boundary() {
    let translation = translate_source(
        r#"
class ObjectBoundary {
    Object value() {
        return "x";
    }

    Object fresh() {
        return new Object();
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn value(&self) -> Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("-> Object"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("Box::new(()) as Box<dyn std::any::Any>"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Box<dyn std::any::Any>::new"),
        "{}",
        translation.rust
    );
}

#[test]
fn concrete_external_facade_strategy_preserves_typed_nullable_returns() {
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalImporter".to_string(),
        strategy: HierarchyStrategy::ConcreteStruct,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalMapping".to_string(),
        strategy: HierarchyStrategy::ConcreteStruct,
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "readMapping".to_string(),
        receiver_type: Some("ExternalImporter".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "ExternalMapping".to_string(),
            args: Vec::new(),
        }),
        return_nullability: NullabilityState::MaybeNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "items".to_string(),
        receiver_type: Some("ExternalMapping".to_string()),
        arg_count: Some(0),
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        }),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ExternalLibraryPack,
    });

    let translation = translate_source_with_facts(
        r#"
class ConcreteFacadeNullableReturn {
    void run(ExternalImporter importer) {
        ExternalMapping mapping = importer.readMapping();
        if (mapping == null) {
            return;
        }
        mapping.items();
    }
}
"#,
        &facts,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("importer: &ExternalImporter"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("let mapping: Option<ExternalMapping> = importer.read_mapping();"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("mapping.as_ref().unwrap().items();"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("Option<Box<dyn std::any::Any>>"),
        "{}",
        translation.rust
    );
}

#[test]
fn signature_baseline_marks_declared_object_as_erased_object_boundary() {
    let policy = signature_slot_baseline_policy(
        &JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        },
        None,
        &FactSet::default(),
    );

    assert!(
        policy
            .constraints
            .contains(&SignatureConstraint::RequiredDispatch(
                DispatchShape::ErasedObject
            )),
        "{:?}",
        policy.constraints
    );
    assert!(
        policy
            .constraints
            .contains(&SignatureConstraint::RequiredView(
                ViewShape::ErasedObjectView
            )),
        "{:?}",
        policy.constraints
    );
}

#[test]
fn new_expression_uses_constructor_param_shape_for_argument_coercion() {
    let translation = translate_source(
        r#"
class StringBox {
    int size;
    StringBox(String value) {
        this.size = value.length();
    }
}

class MakesBox {
    public StringBox make(String value) {
        return new StringBox(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn new(value: &str) -> StringBox"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("StringBox::new(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn shape_env_prevents_redundant_string_view_adapters() {
    let translation = translate_source(
        r#"
class StringFlow {
    private int inner(String value) {
        return value.length();
    }

    private int outer(String value) {
        return inner(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn inner(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn outer(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.inner(value)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("self.inner(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn call_return_shape_drives_return_slot_coercion() {
    let translation = translate_source(
        r#"
class StringReturnFlow {
    private static String id(String value) {
        return value;
    }

    public String call(String value) {
        return id(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub fn id(value: &str) -> &str"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return Self::id(value.as_str()).to_string();"),
        "{}",
        translation.rust
    );
}

#[test]
fn usage_analysis_records_body_facts_without_emitting_rust_shapes() {
    let parsed = parse_java_source(
        r#"
class Uses {
    String name;
    public int len(String value) {
        this.name = value;
        return value.length();
    }
}
"#,
    );
    let usage = analyze_usage(&parsed.program);
    let method = usage
        .methods
        .values()
        .find(|method| method.bindings.contains_key("value"))
        .unwrap();
    let value = method.bindings.get("value").unwrap();

    assert!(value.uses.iter().any(|usage| matches!(
        usage,
        BindingUse::StoredInField { field_name } if field_name == "name"
    )));
    assert!(
        value
            .uses
            .iter()
            .any(|usage| matches!(usage, BindingUse::ContentRead))
    );

    let source = include_str!("../src/usage.rs");
    assert!(source.contains("FieldUse::PassedToCall"));
    assert!(source.contains("record_call_arg_use"));
    assert!(source.contains("pub struct CallArgumentBoundaryPolicy"));
    assert!(source.contains("pub fn call_argument_boundary_policy"));
    assert!(
        source
            .contains("call_argument_boundary_policy(receiver.as_deref(), name, internal_methods)")
    );
    assert!(source.contains("pub struct CallResultBoundaryPolicy"));
    assert!(source.contains("pub fn call_result_boundary_policy"));
    assert!(source.contains("pub struct ConstructorInvocationBoundaryPolicy"));
    assert!(source.contains("pub fn constructor_invocation_boundary_policy"));
    assert!(source.contains("constructor_invocation_boundary_policy(*kind)"));
    assert!(source.contains("call_result_boundary_policy(initializer, internal_methods, facts)"));
    assert!(source.contains("call_result_boundary_policy(expr, internal_methods, facts)"));
    assert!(source.contains("receiver_type: policy.receiver_type"));
}

#[test]
fn signature_fixpoint_propagates_private_internal_call_return_shape() {
    let translation = translate_source(
        r#"
class StringReturnFlow {
    private static String id(String value) {
        return value;
    }

    private static String wrapper(String value) {
        return id(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub fn id(value: &str) -> &str"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn wrapper(value: &str) -> &str"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("return Self::id(value);"),
        "{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("return Self::id(value).to_string();"),
        "{}",
        translation.rust
    );
}

#[test]
fn shape_env_records_declared_locals_as_storage_shapes() {
    let mut env = ShapeEnv::new();
    env.insert_local("name", JavaType::String);

    let shape = env.local_shape("name").unwrap();
    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn expr_shape_owns_environment_setup_from_signature_plan() {
    let parsed = parse_java_source(
        r#"
class EnvUse {
    String field;

    EnvUse(String field) {
        this.field = field;
    }

    String pick(String input) {
        String local = input;
        return local;
    }

    static String id(String value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "pick" => Some(method),
            _ => None,
        })
        .unwrap();
    let static_method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "id" => Some(method),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let static_signature = signatures.method_signature(ty, static_method).unwrap();
    let constructor_signature = signatures.constructor_signature(ty, ctor).unwrap();

    let mut env = shape_env_for_type(ty, &signatures);
    assert_eq!(
        env.field_shape("field"),
        signatures.field_shape("EnvUse", "field")
    );
    let constructor_env = shape_env_for_constructor(ty, constructor_signature, &signatures);
    assert_eq!(
        constructor_env.local_shape("field"),
        Some(&constructor_signature.params[0].shape)
    );
    let method_env = shape_env_for_method(ty, method, method_signature, &signatures);
    assert_eq!(
        method_env.local_shape("input"),
        Some(&method_signature.params[0].shape)
    );
    assert_eq!(
        method_env.field_shape("field"),
        signatures.field_shape("EnvUse", "field")
    );
    let static_env = shape_env_for_method(ty, static_method, static_signature, &signatures);
    assert_eq!(
        static_env.local_shape("value"),
        Some(&static_signature.params[0].shape)
    );
    assert_eq!(static_env.field_shape("field"), None);

    let Stmt::VarDecl { .. } = &method.body.as_ref().unwrap().statements[0] else {
        panic!("expected local declaration");
    };
    learn_stmt_shapes(
        &method.body.as_ref().unwrap().statements[0],
        &mut env,
        &method_signature.locals,
        &signatures,
    );

    assert_eq!(
        env.local_shape("local"),
        signatures.local_shape(&method_signature.locals, "local")
    );
}

#[test]
fn indexed_shape_projects_borrowed_slice_elements_without_lowering_rules() {
    let mut env = ShapeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    let projected_shape = indexed_read_shape(&values_shape);
    env.insert_local_shape("values", values_shape);

    let element_shape = infer_expr_shape(&parse_expr("values[0]"), &env);

    assert_eq!(projected_shape, element_shape);
    assert_eq!(element_shape.java_ty, JavaType::String);
    assert_eq!(element_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(element_shape.view, ViewShape::Native);
}

#[test]
fn indexed_shape_projects_owned_container_non_copy_reads_as_borrows() {
    let mut env = ShapeEnv::new();
    let values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    let projected_shape = indexed_read_shape(&values_shape);
    env.insert_local_shape("values", values_shape);

    let element_shape = infer_expr_shape(&parse_expr("values[0]"), &env);

    assert_eq!(projected_shape, element_shape);
    assert_eq!(element_shape.java_ty, JavaType::String);
    assert_eq!(element_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(element_shape.view, ViewShape::Native);
}

#[test]
fn indexed_assignment_target_shape_projects_storage_element_shape() {
    let mut env = ShapeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    let projected_target_shape = indexed_assignment_shape(&values_shape).unwrap();
    env.insert_local_shape("values", values_shape);

    let read_shape = infer_expr_shape(&parse_expr("values[0]"), &env);
    let target_shape = infer_assignment_target_shape(&parse_expr("values[0]"), &env).unwrap();

    assert_eq!(projected_target_shape, target_shape);
    assert_eq!(read_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(target_shape.java_ty, JavaType::String);
    assert_eq!(target_shape.ownership, OwnershipShape::Owned);
    assert_eq!(target_shape.view, ViewShape::Native);
    assert_eq!(target_shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn indexed_shape_keeps_copy_elements_as_values() {
    let mut env = ShapeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(
        JavaType::Primitive(PrimitiveType::Int),
    )));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    env.insert_local_shape("values", values_shape);

    let element_shape = infer_expr_shape(&parse_expr("values[0]"), &env);

    assert_eq!(
        element_shape.java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
    assert_eq!(element_shape.ownership, OwnershipShape::CopyValue);
}

#[test]
fn indexed_shape_uses_type_capability_facts_for_copy_elements() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let mut values_shape =
        RustSlotShape::conservative(JavaType::Array(Box::new(external_id.clone())));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    assert_eq!(
        shape_projection_capability_policy(&external_id, &facts),
        ShapeProjectionCapabilityPolicy {
            java_ty: external_id.clone(),
            value_semantics: true,
            sequence_element: None,
        }
    );
    let effective_facts = EffectiveFacts::new(&facts);
    assert_eq!(
        shape_projection_capability_policy_with_effective_facts(&external_id, &effective_facts),
        shape_projection_capability_policy(&external_id, &facts),
    );
    let projected_shape = indexed_read_shape_with_facts(&values_shape, &facts);
    assert_eq!(
        indexed_read_shape_with_effective_facts(&values_shape, &effective_facts),
        projected_shape,
    );

    let mut env = ShapeEnv::new();
    env.insert_local_shape("values", values_shape);
    let element_shape = infer_expr_shape_with_context(
        &parse_expr("values[0]"),
        &env,
        ExprShapeContext {
            signatures: None,
            representations: None,
            facts: Some(&facts),
        },
    );

    assert_eq!(projected_shape, element_shape);
    assert_eq!(element_shape.java_ty, external_id);
    assert_eq!(element_shape.ownership, OwnershipShape::CopyValue);
}

#[test]
fn indexed_element_projection_policy_owns_copy_and_borrowed_element_axes() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    assert_eq!(
        indexed_element_projection_policy(
            JavaType::String,
            OwnershipShape::Owned,
            &ViewShape::Native,
            &FactSet::default(),
        )
        .ownership,
        OwnershipShape::SharedBorrow
    );
    assert_eq!(
        indexed_element_projection_policy(
            external_id,
            OwnershipShape::SharedBorrow,
            &ViewShape::Slice,
            &facts,
        )
        .ownership,
        OwnershipShape::CopyValue
    );
}

#[test]
fn iteration_element_projection_policy_preserves_value_semantics_for_copy_facts() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    assert_eq!(
        iteration_element_projection_policy(external_id, &facts).ownership,
        OwnershipShape::Owned
    );
    assert_eq!(
        iteration_element_projection_policy(JavaType::String, &FactSet::default()).ownership,
        OwnershipShape::SharedBorrow
    );
}

#[test]
fn indexed_shape_uses_sequence_like_capability_facts() {
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id.clone()),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    assert_eq!(
        shape_projection_capability_policy(&external_ids, &facts),
        ShapeProjectionCapabilityPolicy {
            java_ty: external_ids.clone(),
            value_semantics: false,
            sequence_element: Some(external_id.clone()),
        }
    );
    assert_eq!(
        sequence_element_projection_policy(&external_ids, &facts),
        SequenceElementProjectionPolicy {
            java_ty: external_ids.clone(),
            element_ty: Some(external_id.clone()),
        }
    );
    let mut values_shape = RustSlotShape::conservative(external_ids);
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;

    let read_shape = indexed_read_shape_with_facts(&values_shape, &facts);
    let assignment_shape = indexed_assignment_shape_with_facts(&values_shape, &facts).unwrap();
    let effective_facts = EffectiveFacts::new(&facts);
    assert_eq!(
        indexed_read_shape_with_effective_facts(&values_shape, &effective_facts),
        read_shape,
    );
    assert_eq!(
        indexed_assignment_shape_with_effective_facts(&values_shape, &effective_facts).unwrap(),
        assignment_shape,
    );

    assert_eq!(read_shape.java_ty, external_id);
    assert_eq!(read_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(read_shape.view, ViewShape::Native);
    assert_eq!(assignment_shape.java_ty, read_shape.java_ty);
    assert_eq!(assignment_shape.ownership, OwnershipShape::Owned);
    assert_eq!(assignment_shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn indexed_assignment_target_shape_uses_sequence_like_capability_facts() {
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id.clone()),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let mut env = ShapeEnv::new();
    env.insert_local("values", external_ids);

    assert!(infer_assignment_target_shape(&parse_expr("values[0]"), &env).is_none());

    let target_shape = infer_assignment_target_shape_with_context(
        &parse_expr("values[0]"),
        &env,
        ExprShapeContext {
            signatures: None,
            representations: None,
            facts: Some(&facts),
        },
    )
    .unwrap();

    assert_eq!(target_shape.java_ty, external_id);
    assert_eq!(target_shape.ownership, OwnershipShape::Owned);
    assert_eq!(target_shape.view, ViewShape::Native);
    assert_eq!(target_shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn indexed_assignment_target_shape_consumes_inferred_array_shape() {
    let source = include_str!("../src/expr_shape.rs");

    assert!(source.contains("fn infer_indexed_assignment_target_shape"));
    assert!(source.contains("&infer_expr_shape_with_context(array, env, ctx)"));
    assert!(!source.contains("RustSlotShape::conservative(infer_expr_type(array"));
}

#[test]
fn exception_lowering_policy_owns_result_support_ok_and_err_shapes() {
    let illegal_argument_ty = JavaType::Named {
        name: "IllegalArgumentException".to_string(),
        args: Vec::new(),
    };
    let thrown_new = Expr::New {
        ty: illegal_argument_ty.clone(),
        args: vec![Expr::Var("message".to_string())],
        body: None,
        resolved: None,
    };
    assert_eq!(
        plan_throw_expr(&thrown_new),
        ThrowExprPlan::NewException {
            class_name: "IllegalArgumentException".to_string(),
            args: &[Expr::Var("message".to_string())],
        }
    );
    assert_eq!(
        exception_class_name(&JavaType::Unknown("ExternalException".to_string())),
        "ExternalException"
    );
    assert_eq!(
        exception_class_name(&JavaType::Primitive(PrimitiveType::Int)),
        "JavaException"
    );
    assert_eq!(
        plan_throw_expr(&Expr::Var("err".to_string())),
        ThrowExprPlan::ExistingException(&Expr::Var("err".to_string()))
    );
    let cause_expr = Expr::Call {
        receiver: Some(Box::new(Expr::Var("ce".to_string()))),
        name: "getCause".to_string(),
        args: Vec::new(),
        resolved: None,
    };
    let cause_runtime_rethrow = Expr::Ternary {
        condition: Box::new(Expr::InstanceOf {
            expr: Box::new(cause_expr.clone()),
            ty: JavaType::Named {
                name: "RuntimeException".to_string(),
                args: Vec::new(),
            },
        }),
        then_expr: Box::new(Expr::Cast {
            ty: JavaType::Named {
                name: "RuntimeException".to_string(),
                args: Vec::new(),
            },
            expr: Box::new(cause_expr),
        }),
        else_expr: Box::new(Expr::Var("ce".to_string())),
    };
    assert_eq!(
        plan_throw_expr(&cause_runtime_rethrow),
        ThrowExprPlan::CauseOrSelfIfInstance {
            wrapper: &Expr::Var("ce".to_string()),
            class_names: vec!["RuntimeException".to_string()],
        }
    );
    assert_eq!(
        exception_result_type(RustType::I32),
        RustType::Raw("Result<i32, JavaException>".to_string())
    );
    assert!(matches!(
        java_exception_support_item(),
        new_java2rust::rust_ir::RustItem::Raw(item)
            if item.contains("pub struct JavaException")
                && item.contains("pub fn get_message(&self) -> String")
                && item.contains("impl std::fmt::Display for JavaException")
    ));
    assert_eq!(
        return_stmt(Some(RustExpr::Literal("1".to_string())), true),
        RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw("Ok".to_string())),
            args: vec![RustExpr::Literal("1".to_string())],
        }))
    );
    assert_eq!(
        throw_stmt(new_exception_expr(
            "IllegalArgumentException",
            RustExpr::Literal("\"bad\".to_string()".to_string()),
        )),
        RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::Raw("Err".to_string())),
            args: vec![RustExpr::Call {
                function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
                args: vec![
                    RustExpr::Literal("\"IllegalArgumentException\"".to_string()),
                    RustExpr::Literal("\"bad\".to_string()".to_string()),
                ],
            }],
        }))
    );
    assert_eq!(
        fallback_exception_expr(RustExpr::Var("err".to_string())),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
            args: vec![
                RustExpr::Literal("\"JavaException\"".to_string()),
                RustExpr::Raw("format!(\"{:?}\", err)".to_string()),
            ],
        }
    );
    assert_eq!(
        exception_cause_or_self_if_instance_expr(
            RustExpr::Var("ce".to_string()),
            &["RuntimeException".to_string()],
        ),
        RustExpr::Raw("ce.cause_or_self_if_instance(&[\"RuntimeException\"])".to_string())
    );
    assert_eq!(
        exception_expr_from_new("IllegalStateException", Vec::new(),),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
            args: vec![
                RustExpr::Literal("\"IllegalStateException\"".to_string()),
                RustExpr::Raw("String::new()".to_string()),
            ],
        }
    );
    assert_eq!(
        exception_expr_from_new(
            "IllegalStateException",
            vec![
                RustExpr::Var("message".to_string()),
                RustExpr::Var("ignored".to_string()),
            ],
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
            args: vec![
                RustExpr::Literal("\"IllegalStateException\"".to_string()),
                RustExpr::Raw("format!(\"{}: {}\", message, ignored)".to_string()),
            ],
        }
    );
    assert_eq!(
        exception_expr_from_new_with_cause_hint(
            "IllegalStateException",
            vec![
                RustExpr::Var("message".to_string()),
                RustExpr::Var("cause".to_string()),
            ],
            true,
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::with_cause".to_string())),
            args: vec![
                RustExpr::Literal("\"IllegalStateException\"".to_string()),
                RustExpr::Var("message".to_string()),
                RustExpr::Var("cause".to_string()),
            ],
        }
    );
    assert_eq!(
        exception_expr_from_new_with_cause_hint(
            "MissingArgument",
            vec![
                RustExpr::Var("argument_name".to_string()),
                RustExpr::Var("message".to_string()),
            ],
            false,
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
            args: vec![
                RustExpr::Literal("\"MissingArgument\"".to_string()),
                RustExpr::Raw("format!(\"{}: {}\", argument_name, message)".to_string()),
            ],
        }
    );
    assert_eq!(
        new_exception_expr_with_cause(
            "UserProblem",
            RustExpr::Literal("\"copy failed\".to_string()".to_string()),
            RustExpr::Var("ex".to_string()),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::Raw("JavaException::with_cause".to_string())),
            args: vec![
                RustExpr::Literal("\"UserProblem\"".to_string()),
                RustExpr::Literal("\"copy failed\".to_string()".to_string()),
                RustExpr::Var("ex".to_string()),
            ],
        }
    );
    let resolved_throwing_call = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Reader".to_string(),
        rust_name: "read".to_string(),
        kind: new_java2rust::java_ir::CallKind::Instance,
        return_type: JavaType::Void,
        arg_coercions: Vec::new(),
    };
    let mut call_facts = FactSet::default();
    call_facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "Reader".to_string(),
            method_name: "read".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Unknown("IOException".to_string())],
        provenance: FactProvenance::ParsedSource,
    });
    assert_eq!(
        plan_call_exception_propagation(
            Some(&Expr::Var("reader".to_string())),
            "read",
            &[],
            Some(&resolved_throwing_call),
            &call_facts,
            &new_java2rust::types::TypeEnv::default(),
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    let resolved_camel_throwing_call = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Reader".to_string(),
        rust_name: "read_header_from_path".to_string(),
        kind: new_java2rust::java_ir::CallKind::Instance,
        return_type: JavaType::Named {
            name: "Header".to_string(),
            args: Vec::new(),
        },
        arg_coercions: Vec::new(),
    };
    call_facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "Reader".to_string(),
            method_name: "readHeaderFromPath".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Unknown("IOException".to_string())],
        provenance: FactProvenance::ParsedSource,
    });
    assert_eq!(
        plan_call_exception_propagation(
            Some(&Expr::Var("reader".to_string())),
            "readHeaderFromPath",
            &[],
            Some(&resolved_camel_throwing_call),
            &call_facts,
            &new_java2rust::types::TypeEnv::default(),
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    assert_eq!(
        plan_call_exception_propagation(
            None,
            "readHeaderFromPath",
            &[Expr::Var("path".to_string())],
            None,
            &call_facts,
            &new_java2rust::types::TypeEnv::default(),
            Some("Reader"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    let mut external_call_facts = FactSet::default();
    external_call_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "validate".to_string(),
            receiver_type: Some("Validator".to_string()),
            arg_count: Some(0),
            role: ExternalApiRole::Unknown,
            receiver_mutability: ReceiverMutabilityFact::Unknown,
            return_type: Some(JavaType::Void),
            return_nullability: NullabilityState::Unknown,
            thrown: vec![JavaType::Unknown("ValidationProblem".to_string())],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    external_call_facts
        .external_field_facts
        .push(new_java2rust::facts::ExternalFieldFact {
            field: FactFieldKey {
                type_name: "Registry".to_string(),
                field_name: "DEFAULT_VALIDATOR".to_string(),
            },
            field_type: Some(JavaType::Named {
                name: "Validator".to_string(),
                args: Vec::new(),
            }),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let mut external_types = new_java2rust::types::TypeEnv::new();
    external_types.insert_local(
        "validator",
        JavaType::Named {
            name: "Validator".to_string(),
            args: Vec::new(),
        },
    );
    assert_eq!(
        plan_call_exception_propagation(
            Some(&Expr::Var("validator".to_string())),
            "validate",
            &[],
            None,
            &external_call_facts,
            &external_types,
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    let field_receiver = Expr::FieldAccess(
        Box::new(Expr::Var("Registry".to_string())),
        "DEFAULT_VALIDATOR".to_string(),
    );
    assert_eq!(
        plan_call_exception_propagation(
            Some(&field_receiver),
            "validate",
            &[],
            None,
            &external_call_facts,
            &external_types,
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    external_call_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "nonNull".to_string(),
            receiver_type: None,
            arg_count: Some(1),
            role: ExternalApiRole::BooleanGetter,
            receiver_mutability: ReceiverMutabilityFact::Unknown,
            return_type: Some(JavaType::Primitive(PrimitiveType::Boolean)),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: vec![JavaType::Unknown("NullPolicyException".to_string())],
            provenance: FactProvenance::UserOverride,
        });
    assert_eq!(
        plan_call_exception_propagation(
            None,
            "nonNull",
            &[Expr::Var("value".to_string())],
            None,
            &external_call_facts,
            &external_types,
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::PropagateResult
    );
    assert_eq!(
        plan_call_exception_propagation(
            Some(&Expr::Var("validator".to_string())),
            "validate",
            &[Expr::Literal(Literal::Int(1))],
            None,
            &external_call_facts,
            &external_types,
            Some("Owner"),
        ),
        CallExceptionPropagationPlan::NoPropagation
    );
}

#[test]
fn exception_plan_does_not_render_rust() {
    let source = include_str!("../src/exception_plan.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("RustStmt"));
    assert!(!source.contains("RustType"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("JavaException::new"));
}

#[test]
fn exception_lowering_does_not_infer_java_exception_class_names() {
    let source = include_str!("../src/exception_lowering.rs");

    assert!(!source.contains("JavaType"));
    assert!(!source.contains("exception_class_name"));
    assert!(!source.contains("JavaType::Named"));
    assert!(!source.contains("JavaType::Unknown"));
}

#[test]
fn cast_shape_is_owned_by_type_inference_not_lowering() {
    let mut env = ShapeEnv::new();
    env.insert_local("value", JavaType::Unknown("Object".to_string()));

    let shape = infer_expr_shape(&parse_expr("(String) value"), &env);

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn semantic_facts_own_nullability_and_external_api_heuristics_before_lowering() {
    let lower_source = include_str!("../src/lower.rs");
    let signature_source = include_str!("../src/signature.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");

    assert!(semantic_source.contains("collect_field_nullability_facts"));
    assert!(semantic_source.contains("FiniteSaturationPolicy"));
    assert!(semantic_source.contains("SaturationStep"));
    assert!(semantic_source.contains("collect_external_api_effect_facts"));
    assert!(semantic_source.contains("collect_method_effect_facts"));
    assert!(semantic_source.contains("method_receiver_mutability_effect"));
    assert!(semantic_source.contains("method_return_nullability_effect"));
    assert!(signature_source.contains("method_effect_for"));
    assert!(signature_source.contains("receiver_constraints_with_facts"));
    assert!(signature_source.contains("return_nullability"));
    assert!(signature_source.contains("external_receiver_binding_constraints"));
    assert!(!signature_source.contains("block_mutates_self"));
    assert!(!signature_source.contains("stmt_mutates_self"));
    assert!(!lower_source.contains("field_nullability_for"));
    assert!(!lower_source.contains("external_api_effect_for"));
    assert!(!lower_source.contains("starts_with(\"set\")"));
    assert!(!lower_source.contains("starts_with(\"get\")"));
}

#[test]
fn trivial_accessor_detection_is_not_a_lowering_special_case() {
    let lower_source = include_str!("../src/lower.rs");
    let call_plan_source = include_str!("../src/call_plan.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");

    assert!(semantic_source.contains("trivial_getter_fact"));
    assert!(semantic_source.contains("trivial_setter_fact"));
    assert!(call_plan_source.contains("plan_trivial_accessor_call"));
    assert!(call_plan_source.contains("plan_call_expr"));
    assert!(call_plan_source.contains("plan_call_statement"));
    assert!(!lower_source.contains("plan_trivial_accessor_call"));
    assert!(lower_source.contains("plan_call_expr"));
    assert!(lower_source.contains("plan_call_statement"));
    assert!(!lower_source.contains("trivial_accessor_for"));
    assert!(!lower_source.contains("trivial_getter_field_for"));
    assert!(!lower_source.contains("trivial_setter_field_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_accessor_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_getter_field_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_setter_field_for"));
    assert!(!lower_source.contains("getValue"));
    assert!(!lower_source.contains("setValue"));
}

#[test]
fn semantic_analysis_owns_exception_classification_and_catch_facts() {
    let semantic_source = include_str!("../src/semantic_analysis.rs");
    let exception_lowering_source = include_str!("../src/exception_lowering.rs");
    let exception_plan_source = include_str!("../src/exception_plan.rs");
    let item_assembly_source = include_str!("../src/item_assembly.rs");
    let lower_source = include_str!("../src/lower.rs");
    let representation_lowering_source = include_str!("../src/representation_lowering.rs");
    let parsed = parse_java_source(
        r#"
class ThrowsValue {
    int read() throws IOException {
        return 1;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let mut exception_facts = resolved.facts.clone();
    exception_facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: ty.name.clone(),
            method_name: method.name.clone(),
        },
        receiver_mutability: ReceiverMutabilityFact::Unknown,
        return_nullability: NullabilityState::Unknown,
        thrown: vec![JavaType::Unknown("IOException".to_string())],
        provenance: FactProvenance::ParsedSource,
    });

    assert_eq!(
        plan_java_exception_support(&resolved.program, &exception_facts),
        JavaExceptionSupportPlan {
            emit_support_item: true
        }
    );
    assert_eq!(
        plan_method_exceptions(ty, method, &exception_facts),
        MethodExceptionPlan {
            wrap_returns_in_result: true
        }
    );
    let program_exception_plan = plan_program_exceptions(&resolved.program, &exception_facts);
    assert_eq!(
        program_exception_plan.support,
        JavaExceptionSupportPlan {
            emit_support_item: true
        }
    );
    assert_eq!(
        program_exception_plan.method_plan(ty, method),
        MethodExceptionPlan {
            wrap_returns_in_result: true
        }
    );
    let try_parsed = parse_java_source(
        r#"
class TryCatchValue {
    int parse(boolean bad) {
        try {
            if (bad) {
                throw new IllegalArgumentException("bad");
            }
            return 7;
        } catch (IllegalArgumentException ex) {
            return 1;
        }
    }
}
"#,
    );
    let try_ty = &try_parsed.program.types[0];
    let try_method = try_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = &try_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected try statement");
    };
    assert!(matches!(
        plan_try_stmt(resources, body, catches, finally_block.as_ref()),
        TryStmtPlan::CatchOnly {
            completion: TryBlockCompletion::Terminates,
            ..
        }
    ));
    let fallthrough_try_parsed = parse_java_source(
        r#"
class TryCatchFallthrough {
    void parse(boolean bad) {
        try {
            if (bad) {
                throw new IllegalArgumentException("bad");
            }
        } catch (IllegalArgumentException ex) {
            throw ex;
        }
    }
}
"#,
    );
    let fallthrough_ty = &fallthrough_try_parsed.program.types[0];
    let fallthrough_method = fallthrough_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = &fallthrough_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected try statement");
    };
    assert!(matches!(
        plan_try_stmt(resources, body, catches, finally_block.as_ref()),
        TryStmtPlan::CatchOnly {
            completion: TryBlockCompletion::FallsThroughUnit,
            ..
        }
    ));
    assert!(matches!(
        plan_try_stmt(resources, body, &[], finally_block.as_ref()),
        TryStmtPlan::Unsupported
    ));
    let resources_try_parsed = parse_java_source(
        r#"
class TryWithResourcesUnit {
    void copy(Path source, Path dest, byte[] buffer) {
        try (InputStream input = Files.newInputStream(source);
             OutputStream output = Files.newOutputStream(dest)) {
            int n = input.read(buffer);
            output.write(buffer, 0, n);
        } catch (IOException ex) {
            throw ex;
        }
    }
}
"#,
    );
    let resources_ty = &resources_try_parsed.program.types[0];
    let resources_method = resources_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = &resources_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected try-with-resources statement");
    };
    assert!(matches!(
        plan_try_stmt(resources, body, catches, finally_block.as_ref()),
        TryStmtPlan::WithResources {
            completion: TryBlockCompletion::FallsThroughUnit,
            ..
        }
    ));
    assert_eq!(
        try_resource_names(resources),
        vec!["input".to_string(), "output".to_string()]
    );
    let return_resources_try_parsed = parse_java_source(
        r#"
class TryWithResourcesReturn {
    String read(Path path) {
        try (Reader reader = Files.newBufferedReader(path)) {
            return reader.readLine();
        } catch (IOException ex) {
            return "error";
        }
    }
}
"#,
    );
    let return_resources_ty = &return_resources_try_parsed.program.types[0];
    let return_resources_method = return_resources_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = &return_resources_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected return-bearing try-with-resources statement");
    };
    assert!(matches!(
        plan_try_stmt(resources, body, catches, finally_block.as_ref()),
        TryStmtPlan::WithResources {
            completion: TryBlockCompletion::Terminates,
            ..
        }
    ));
    let finally_try_parsed = parse_java_source(
        r#"
class TryFinallyUnit {
    void run(Resource resource) {
        try {
            resource.read();
        } finally {
            resource.close();
        }
    }
}
"#,
    );
    let finally_ty = &finally_try_parsed.program.types[0];
    let finally_method = finally_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        resources,
        body,
        catches,
        finally_block,
    } = &finally_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected try/finally statement");
    };
    assert!(matches!(
        plan_try_stmt(resources, body, catches, finally_block.as_ref()),
        TryStmtPlan::FinallyOnly { .. }
    ));
    assert_eq!(
        try_catch_stmt(
            RustType::I32,
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::Raw("Ok".to_string())),
                args: vec![RustExpr::Literal("7".to_string())],
            }))],
            vec![CatchRenderArm {
                variable_name: "ex".to_string(),
                class_names: vec!["IllegalArgumentException".to_string()],
                body: vec![RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::Raw("Ok".to_string())),
                    args: vec![RustExpr::Literal("1".to_string())],
                }))],
            }],
            true,
            false,
        ),
        RustStmt::Raw(
            "match (|| -> Result<i32, JavaException> {\n    return Ok(7);\n})() {\n    Ok(value) => return Ok(value),\n    Err(ex) if ex.is_instance(&[\"IllegalArgumentException\"]) => {\n        return Ok(1);\n    }\n    Err(ex) => return Err(ex),\n}".to_string()
        )
    );
    assert_eq!(
        try_catch_stmt(
            RustType::Unit,
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::Raw("Ok".to_string())),
                args: vec![RustExpr::Raw("()".to_string())],
            }))],
            Vec::new(),
            false,
            true,
        ),
        RustStmt::Raw(
            "match (|| -> Result<(), JavaException> {\n    return Ok(());\n})() {\n    Ok(()) => {}\n    Err(ex) => panic!(\"uncaught Java exception: {:?}\", ex),\n}".to_string()
        )
    );
    assert_eq!(
        try_with_resources_scope_stmt(
            RustType::Unit,
            vec![RustStmt::Raw("let input = open_input();".to_string())],
            vec![RustStmt::Raw("copy_chunk(&mut input)?;".to_string())],
            vec![RustStmt::Raw("input.close()?;".to_string())],
            false,
        ),
        RustStmt::Raw(
            "{\n    let input = open_input();\n    let __java_twr_result = (|| -> Result<(), JavaException> {\n        copy_chunk(&mut input)?;\n        return Ok(());\n    })();\n    input.close()?;\n    __java_twr_result?;\n\n}".to_string()
        )
    );
    assert_eq!(
        try_with_resources_scope_stmt(
            RustType::String,
            vec![RustStmt::Raw("let reader = open_reader();".to_string())],
            vec![return_stmt(
                Some(RustExpr::Raw("reader.read_line()".to_string())),
                true,
            )],
            vec![RustStmt::Raw("reader.close()?;".to_string())],
            true,
        ),
        RustStmt::Raw(
            "{\n    let reader = open_reader();\n    let __java_twr_result = (|| -> Result<String, JavaException> {\n        return Ok(reader.read_line());\n    })();\n    reader.close()?;\n    return __java_twr_result;\n\n}".to_string()
        )
    );
    assert_eq!(
        try_finally_scope_stmt(
            vec![RustStmt::Raw("resource.read()?;".to_string())],
            vec![RustStmt::Raw("resource.close();".to_string())],
            true,
        ),
        RustStmt::Raw(
            "{\n    let __java_try_result = (|| -> Result<(), JavaException> {\n        resource.read()?;\n        return Ok(());\n    })();\n    resource.close();\n    __java_try_result?;\n\n}".to_string()
        )
    );
    let twr_close_source = r#"
class TryResourceModeledClose {
    String read() throws IOException {
        try (FeatureReader reader = openReader()) {
            return "ok";
        } catch (IOException ex) {
            throw ex;
        }
    }

    FeatureReader openReader() {
        return null;
    }
}
"#;
    let mut unit_close_facts = FactSet::default();
    unit_close_facts
        .external_api_effect_facts
        .push(ExternalApiEffectFact {
            method_name: "close".to_string(),
            receiver_type: Some("FeatureReader".to_string()),
            arg_count: Some(0),
            role: ExternalApiRole::SideEffect,
            receiver_mutability: ReceiverMutabilityFact::MutatesReceiver,
            return_type: None,
            return_nullability: NullabilityState::Unknown,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let unit_close_translation = translate_source_with_facts(twr_close_source, &unit_close_facts);
    assert!(
        unit_close_translation.rust.contains("reader.close();"),
        "{}",
        unit_close_translation.rust
    );
    assert!(
        !unit_close_translation.rust.contains("reader.close()?;"),
        "{}",
        unit_close_translation.rust
    );

    let mut throwing_close_facts = unit_close_facts.clone();
    throwing_close_facts.external_api_effect_facts[0].thrown = vec![JavaType::Named {
        name: "IOException".to_string(),
        args: Vec::new(),
    }];
    let throwing_close_translation =
        translate_source_with_facts(twr_close_source, &throwing_close_facts);
    assert!(
        throwing_close_translation.rust.contains("reader.close()?;"),
        "{}",
        throwing_close_translation.rust
    );
    let catch_source = r#"
class ExceptionSubclassCatchOrder {
    int parse(Source source) {
        try {
            return source.read();
        } catch (SpecificProblem ex) {
            return 1;
        } catch (BaseProblem ex) {
            return 2;
        }
    }
}
class BaseProblem extends Exception {}
class SpecificProblem extends BaseProblem {}
"#;
    let catch_parsed = parse_java_source(catch_source);
    let catch_resolved = resolve_program(&catch_parsed.program, catch_parsed.diagnostics.clone());
    let catch_semantic = analyze_program(
        &catch_resolved.program,
        &catch_resolved.facts,
        &FactSet::default(),
    );
    let catch_ty = &catch_parsed.program.types[0];
    let catch_method = catch_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Try {
        catches: ordered_catches,
        ..
    } = &catch_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected ordered try/catch");
    };
    assert_eq!(
        catch_match_class_names(
            Some("ExceptionSubclassCatchOrder"),
            Some("parse"),
            1,
            &ordered_catches[1],
            &catch_semantic.facts,
        ),
        vec!["BaseProblem".to_string(), "SpecificProblem".to_string()]
    );

    assert!(semantic_source.contains("collect_exception_facts"));
    assert!(semantic_source.contains("collect_catch_facts"));
    assert!(semantic_source.contains("collect_method_effect_facts"));
    assert!(semantic_source.contains("method_thrown_exception_types"));
    assert!(semantic_source.contains("catch_assignable_from_types"));
    assert!(semantic_source.contains("classify_exception_type"));
    assert!(exception_plan_source.contains("program_uses_java_exception_with_facts"));
    assert!(exception_plan_source.contains("pub struct JavaExceptionSupportPlan"));
    assert!(exception_plan_source.contains("pub struct MethodExceptionPlan"));
    assert!(exception_plan_source.contains("pub struct ProgramExceptionPlan"));
    assert!(exception_plan_source.contains("pub fn plan_java_exception_support"));
    assert!(exception_plan_source.contains("pub fn plan_method_exceptions"));
    assert!(exception_plan_source.contains("pub fn plan_program_exceptions"));
    assert!(exception_plan_source.contains("pub enum TryStmtPlan"));
    assert!(exception_plan_source.contains("pub fn plan_try_stmt"));
    assert!(exception_plan_source.contains("pub fn catch_match_class_names"));
    assert!(exception_lowering_source.contains("pub struct CatchRenderArm"));
    assert!(exception_lowering_source.contains("pub fn try_catch_stmt"));
    assert!(lower_source.contains("let exceptions = plan_program_exceptions(program, facts)"));
    assert!(lower_source.contains("exceptions.support.emit_support_item"));
    assert!(lower_source.contains("method_ctx.exceptions.method_plan"));
    assert!(lower_source.contains("method_function_shell_with_facts_and_exception_plan"));
    assert!(!exception_lowering_source.contains("RuntimeException"));
    assert!(!exception_lowering_source.contains("ExceptionKind"));
    assert!(!exception_plan_source.contains("RuntimeException"));
    assert!(!exception_plan_source.contains("ExceptionKind"));
    assert!(!exception_plan_source.contains("method.throws"));
    assert!(!item_assembly_source.contains("method.throws"));
    assert!(!lower_source.contains("method.throws"));
    assert!(!representation_lowering_source.contains("method.throws"));
    assert!(!lower_source.contains("program_uses_java_exception_with_facts"));
    assert!(!lower_source.contains("method_has_thrown_exceptions"));
    assert!(!lower_source.contains("plan_method_exceptions"));
    assert!(!item_assembly_source.contains("method_has_thrown_exceptions"));
    assert!(!representation_lowering_source.contains("method_has_thrown_exceptions"));
    assert!(item_assembly_source.contains("MethodExceptionPlan"));
    assert!(representation_lowering_source.contains("MethodExceptionPlan"));
    assert!(representation_lowering_source.contains("ProgramExceptionPlan"));
    assert!(!item_assembly_source.contains("plan_method_exceptions"));
    assert!(!representation_lowering_source.contains("plan_method_exceptions"));
    assert!(exception_plan_source.contains("method_has_thrown_exceptions"));
}

#[test]
fn semantic_diagnostics_are_plumbed_through_translation_boundary() {
    let lib_source = include_str!("../src/lib.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");

    assert!(semantic_source.contains("SemanticDiagnostic"));
    assert!(semantic_source.contains("semantic_external_api_effect_budget_exhausted"));
    assert!(lib_source.contains("for diagnostic in semantic.diagnostics"));
    assert!(lib_source.contains(".warning(diagnostic.code, diagnostic.message)"));
}

#[test]
fn control_flow_normalization_plans_are_owned_before_lowering() {
    let semantic_source = include_str!("../src/semantic_analysis.rs");
    let rewrite_source = include_str!("../src/rewrite.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(rewrite_source.contains("pub struct MethodControlFlowPlan"));
    assert!(rewrite_source.contains("AssignmentInConditionPlan"));
    assert!(rewrite_source.contains("TailRecursiveLoopPlan"));
    assert!(semantic_source.contains("collect_control_flow_plans"));
    assert!(semantic_source.contains("assignment_in_condition_plan"));
    assert!(semantic_source.contains("tail_recursive_loop_plan"));
    assert!(semantic_source.contains("collect_recursion_fallback_diagnostics"));
    assert!(semantic_source.contains("recursion_tco_fallback"));
    assert!(!lower_source.contains("AssignmentInConditionPlan"));
    assert!(!lower_source.contains("TailRecursiveLoopPlan"));
    assert!(!lower_source.contains("LoopWithParameterReassignment"));
    assert!(!lower_source.contains("recursion_tco_fallback"));
}

#[test]
fn lowering_diagnostics_only_report_unimplemented_control_flow_rewrites() {
    let lower_source = include_str!("../src/lower.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");
    let rewrite_source = include_str!("../src/rewrite.rs");
    let lowering_diagnostics_source = include_str!("../src/lowering_diagnostics.rs");
    let parsed = parse_java_source("class Parser { int next() { return 1; } }");
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let facts = FactSet::default();
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &analyze_usage(&resolved.program),
        &facts,
    );
    let plans = vec![MethodControlFlowPlan {
        method: FactMethodKey {
            type_name: "Parser".to_string(),
            method_name: "next".to_string(),
        },
        rewrites: vec![
            ControlFlowRewritePlan::AssignmentInCondition(AssignmentInConditionPlan {
                local_name: "token".to_string(),
                source_call: "read()".to_string(),
                null_test: NullTestKind::NotNull,
                normalized_shape: AssignmentConditionNormalizedShape::LoopAssignThenBreak,
            }),
            ControlFlowRewritePlan::TailRecursiveLoop(TailRecursiveLoopPlan {
                method_name: "next".to_string(),
                arg_count: 1,
                normalized_shape: TailRecursiveNormalizedShape::LoopWithParameterReassignment,
            }),
        ],
    }];
    let diagnostics = control_flow_rewrite_diagnostics(&plans);
    let lowering_diagnostics = collect_lowering_diagnostic_plans(
        &resolved.program,
        &facts,
        &representations,
        &signatures,
        &plans,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        lowering_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "control_flow_rewrite_pending")
            .map(|diagnostic| diagnostic.severity)
            .collect::<Vec<_>>(),
        Vec::<Severity>::new()
    );

    assert!(lower_source.contains("lower_program_with_semantic_plans"));
    assert!(lower_source.contains("collect_lowering_diagnostic_plans"));
    assert!(!lower_source.contains("push_control_flow_plan_diagnostics"));
    assert!(!lower_source.contains("control_flow_rewrite_diagnostics"));
    assert!(lowering_diagnostics_source.contains("pub struct LoweringDiagnosticPlan"));
    assert!(lowering_diagnostics_source.contains("ConversionDiagnostic"));
    assert!(lowering_diagnostics_source.contains("lowering_diagnostic_from_conversion_diagnostic"));
    assert!(lowering_diagnostics_source.contains("pub fn collect_lowering_diagnostic_plans"));
    assert!(lowering_diagnostics_source.contains(".diagnostics()"));
    assert!(lowering_diagnostics_source.contains("Severity::Warning"));
    assert!(lowering_diagnostics_source.contains("Severity::Unsupported"));
    assert!(lowering_diagnostics_source.contains("diagnostics_for_program(program)"));
    assert!(lowering_diagnostics_source.contains("signatures"));
    assert!(lowering_diagnostics_source.contains("control_flow_rewrite_diagnostics"));
    assert!(rewrite_source.contains("pub struct ControlFlowRewriteDiagnostic"));
    assert!(rewrite_source.contains("pub fn control_flow_rewrite_diagnostics"));
    assert!(rewrite_source.contains("method_has_tail_recursive_loop_rewrite"));
    assert!(rewrite_source.contains("control_flow_rewrite_pending"));
    assert!(semantic_source.contains("collect_control_flow_plans"));
    assert!(!lower_source.contains("assignment_in_condition_plan("));
    assert!(!lower_source.contains("tail_recursive_loop_plan("));
    assert!(!lower_source.contains("facts.diagnostics()"));
    assert!(!lower_source.contains("diagnostics_for_program(program)"));
    assert!(!lower_source.contains("signatures.diagnostics()"));
    assert!(!lower_source.contains("control_flow_rewrite_pending"));
    assert!(!lower_source.contains("requires assignment-in-condition normalization"));
    assert!(!lower_source.contains("requires tail-recursive loop normalization"));
}

#[test]
fn stream_shape_owns_lambda_body_borrowed_string_views() {
    let default = RustSlotShape::conservative(JavaType::String);
    let shape = infer_stream_lambda_body_shape(&parse_expr("value.trim()"), &default);

    assert_eq!(shape, borrowed_string_content_shape());

    let owned_shape = infer_stream_lambda_body_shape(&parse_expr("value.toLowerCase()"), &default);
    assert_eq!(owned_shape, owned_string_value_shape());
}

#[test]
fn shape_projection_owns_borrowed_string_content_shape() {
    let coercion_source = include_str!("../src/coercion.rs");
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");

    assert!(shape_projection_source.contains("pub fn borrowed_string_content_shape"));
    assert!(shape_projection_source.contains("pub fn owned_string_value_shape"));
    assert!(shape_projection_source.contains("pub fn apply_borrowed_string_content_projection"));
    assert!(shape_projection_source.contains("pub fn borrowed_sequence_element_shape"));
    assert!(shape_projection_source.contains("pub fn apply_borrowed_sequence_element_projection"));
    assert!(slot_policy_source.contains("BorrowedContentPolicy"));
    assert!(slot_policy_source.contains("BorrowedContentKind"));
    assert!(slot_policy_source.contains("BorrowedContentTypePolicy"));
    assert!(slot_policy_source.contains("borrowed_content_type_policy"));
    assert!(!slot_policy_source.contains("borrowed_content_policy_for_type"));
    assert!(!slot_policy_source.contains("fn can_borrow_content"));
    assert!(!slot_policy_source.contains("BorrowedViewPolicy"));
    assert!(!slot_policy_source.contains("BorrowedStringContent"));
    assert!(!stream_shape_source.contains("ViewShape::Str"));
    assert!(!stream_shape_source.contains("JavaType::String"));
    assert!(!slot_policy_source.contains("ViewShape::Str"));
    assert!(!slot_policy_source.contains("OwnershipShape::SharedBorrow"));
    assert!(!coercion_source.contains("element_shape.ownership = OwnershipShape"));
}

#[test]
fn stdlib_owns_known_jdk_content_method_facts() {
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let usage_source = include_str!("../src/usage.rs");

    assert!(is_content_read_method("length"));
    assert!(is_content_read_method("isEmpty"));
    assert!(is_content_read_method("trim"));
    assert!(is_borrowed_string_view_method("trim", 0));
    assert!(!is_borrowed_string_view_method("trim", 1));
    assert!(is_owned_string_result_method("toLowerCase", 0));
    assert!(is_owned_string_result_method("toUpperCase", 0));
    assert!(!is_owned_string_result_method("toLowerCase", 1));
    assert!(!stream_shape_source.contains("name == \"trim\""));
    assert!(!stream_shape_source.contains("toLowerCase"));
    assert!(!usage_source.contains("\"length\" | \"isEmpty\" | \"trim\""));
}

#[test]
fn stream_lambda_emission_owns_java_string_method_mappings() {
    assert_eq!(
        emit_stream_lambda_body(
            &parse_expr("value.trim()"),
            "value",
            StreamLambdaValue::BorrowedItem
        )
        .unwrap(),
        "value.trim()"
    );
    assert_eq!(
        emit_stream_lambda_body(
            &parse_expr("value.isEmpty()"),
            "value",
            StreamLambdaValue::BorrowedItem
        )
        .unwrap(),
        "value.is_empty()"
    );
    assert_eq!(
        emit_stream_lambda_body(
            &parse_expr("value.toLowerCase()"),
            "value",
            StreamLambdaValue::BorrowedItem
        )
        .unwrap(),
        "value.to_lowercase()"
    );
}

#[test]
fn stream_lambda_emission_uses_shape_env_for_bare_fields_and_static_fields() {
    let mut env = new_java2rust::expr_shape::ShapeEnv::new();
    env.insert_field_shape("workspace", RustSlotShape::for_storage(JavaType::String));
    env.insert_static_field_shape(
        "ARRAY_COLUMN_BOUNDS_START",
        RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
    );
    env.insert_local_shape(
        "vid_map_p_b",
        RustSlotShape::for_storage(JavaType::Unknown("VidMapPB".to_string())),
    );
    let ctx = StreamRenderContext::new(&env);

    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("workspace"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        )
        .unwrap(),
        "self.workspace"
    );
    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("ARRAY_COLUMN_BOUNDS_START"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        )
        .unwrap(),
        "Self::ARRAY_COLUMN_BOUNDS_START"
    );
    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("GenomicsDBUtils.getArrayColumnBounds(workspace, partition)"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        )
        .unwrap(),
        "GenomicsDBUtils::get_array_column_bounds(self.workspace, *partition)"
    );
    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("vidMapPB.getContigsList()"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        )
        .unwrap(),
        "vid_map_p_b.get_contigs_list()"
    );
    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("Coordinates.GenomicsDBColumn.newBuilder()"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        )
        .unwrap(),
        "GenomicsDBColumn::new_builder()"
    );
}

#[test]
fn stream_source_element_shape_is_inferred_before_lowering() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));

    let shape = infer_stream_source_element_shape(&parse_expr("values"), &env).unwrap();
    let projected_shape = iteration_element_shape(JavaType::String, &FactSet::default());

    assert_eq!(shape, projected_shape);
    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn types_own_stream_source_element_java_semantics() {
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let boxed_int_list = list_java_type(JavaType::Named {
        name: "Integer".to_string(),
        args: Vec::new(),
    });
    let facts = FactSet::default();
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(
        stream_element_type(&boxed_int_list),
        Some(JavaType::Primitive(PrimitiveType::Int))
    );
    assert_eq!(
        stream_source_capability_policy(&boxed_int_list, &facts),
        StreamSourceCapabilityPolicy {
            source_ty: boxed_int_list.clone(),
            capability_element: Some(JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new(),
            }),
        }
    );
    assert_eq!(
        stream_source_capability_policy_with_effective_facts(&boxed_int_list, &effective_facts),
        stream_source_capability_policy(&boxed_int_list, &facts)
    );
    assert!(!stream_shape_source.contains("sequence_element_type("));
    assert!(stream_shape_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(stream_shape_source.contains("pub struct StreamSourceCapabilityPolicy"));
    assert!(stream_shape_source.contains("pub fn stream_source_capability_policy"));
    assert!(stream_shape_source.contains("stream_source_capability_policy_with_effective_facts"));
    assert!(effective_facts_source.contains("pub fn sequence_element_type_for"));
    assert!(!stream_shape_source.contains("name == \"Integer\""));
    assert!(!stream_shape_source.contains("OwnershipShape"));
    assert!(!stream_shape_source.contains("ownership ="));
    assert!(stream_shape_source.contains("iteration_element_shape_with_effective_facts"));
    assert_eq!(
        stream_shape_source
            .matches("facts.sequence_element_type_for")
            .count(),
        1
    );
    assert!(stream_shape_source.contains("infer_stream_source_element_shape_with_facts"));
    assert!(stream_shape_source.contains("infer_stream_source_element_shape_with_effective_facts"));
    assert!(!stream_shape_source.contains("fn stream_source_element_type("));
}

#[test]
fn stream_source_element_shape_uses_type_capability_facts() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(external_id.clone()));
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let default_shape = infer_stream_source_element_shape(&parse_expr("values"), &env).unwrap();
    let fact_shape =
        infer_stream_source_element_shape_with_facts(&parse_expr("values"), &env, &facts).unwrap();
    let effective_shape = infer_stream_source_element_shape_with_effective_facts(
        &parse_expr("values"),
        &env,
        &EffectiveFacts::new(&facts),
    )
    .unwrap();

    assert_eq!(
        default_shape,
        iteration_element_shape(external_id.clone(), &FactSet::default())
    );
    assert_eq!(
        fact_shape,
        iteration_element_shape(external_id.clone(), &facts)
    );
    assert_eq!(effective_shape, fact_shape);
    assert_eq!(default_shape.java_ty, external_id);
    assert_eq!(default_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(fact_shape.ownership, OwnershipShape::Owned);
    assert_eq!(fact_shape.view, ViewShape::Native);
}

#[test]
fn stream_source_element_shape_uses_sequence_like_capability_facts() {
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut env = TypeEnv::new();
    env.insert_local("values", external_ids.clone());
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id.clone()),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    assert!(infer_stream_source_element_shape(&parse_expr("values"), &env).is_none());
    assert_eq!(
        stream_source_capability_policy(&external_ids, &facts),
        StreamSourceCapabilityPolicy {
            source_ty: external_ids.clone(),
            capability_element: Some(external_id.clone()),
        }
    );

    let shape =
        infer_stream_source_element_shape_with_facts(&parse_expr("values"), &env, &facts).unwrap();

    assert_eq!(shape, iteration_element_shape(external_id, &facts));
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn switch_plan_owns_fallthrough_bindings_and_break_rewrite_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Use {
    int score(int value) {
        switch (value) {
            case 1:
                break;
            default:
                return 2;
        }
        return 0;
    }
}
"#,
    );
    let ty = &parsed.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Switch { selector, cases } = &method.body.as_ref().unwrap().statements[0] else {
        panic!("expected switch");
    };
    let plan = plan_switch(selector, cases);

    assert_eq!(plan.break_label, SWITCH_LABEL);
    assert_eq!(plan.value_binding, SWITCH_VALUE);
    assert_eq!(plan.matched_binding, SWITCH_MATCHED);
    assert!(matches!(
        plan_switch_case(&cases[0]).labels.as_slice(),
        [SwitchCaseLabelPlan::Expr(_)]
    ));
    assert!(matches!(
        plan_switch_case(&cases[1]).labels.as_slice(),
        [SwitchCaseLabelPlan::Default]
    ));
    assert_eq!(
        rewrite_switch_breaks(&cases[0].statements[0], plan.break_label),
        Stmt::Break(Some(SWITCH_LABEL.to_string()))
    );
    assert_eq!(
        switch_case_condition(
            &plan,
            &plan_switch_case(&cases[0]),
            vec![RustExpr::Literal("1".to_string())],
        ),
        "!__java_switch_matched && (__java_switch_value == 1)"
    );
    assert_eq!(
        switch_case_condition(&plan, &plan_switch_case(&cases[1]), Vec::new()),
        "!__java_switch_matched"
    );

    let rendered = render_switch(
        &plan,
        RustExpr::Var("value".to_string()),
        vec![SwitchRenderCase {
            label_condition: "!__java_switch_matched".to_string(),
            statements: vec![RustStmt::Break(Some(SWITCH_LABEL.to_string()))],
        }],
    );
    assert!(rendered.contains("let __java_switch_value = value;"));
    assert!(rendered.contains("let mut __java_switch_matched = false;"));
    assert!(rendered.contains("'java_switch: loop"));
    assert!(rendered.contains("if !__java_switch_matched"));
    assert!(rendered.contains("break 'java_switch;"));

    let simple = parse_java_source(
        r#"
class Use {
    String label(int value) {
        switch (value) {
            case 1:
                return "one";
            case 2:
            case 3:
                return "small";
            default:
                return "other";
        }
    }
}
"#,
    );
    let simple_ty = &simple.program.types[0];
    let simple_method = simple_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Switch {
        selector: simple_selector,
        cases: simple_cases,
    } = &simple_method.body.as_ref().unwrap().statements[0]
    else {
        panic!("expected simple switch");
    };
    let simple_plan = plan_simple_return_switch(simple_selector, simple_cases)
        .expect("label-only groups followed by return cases should be simple match candidates");
    assert_eq!(simple_plan.cases.len(), 4);
    assert!(plan_simple_return_switch_case(&simple_cases[1]).is_none());
    assert!(plan_simple_return_switch_case(&simple_cases[2]).is_some());
    let match_rendered = render_simple_return_match_switch(
        RustExpr::Var("value".to_string()),
        vec![
            SwitchMatchRenderCase {
                patterns: vec!["1".to_string()],
                statements: vec![RustStmt::Return(Some(RustExpr::Literal(
                    "\"one\".to_string()".to_string(),
                )))],
            },
            SwitchMatchRenderCase {
                patterns: vec!["2".to_string(), "3".to_string()],
                statements: vec![RustStmt::Return(Some(RustExpr::Literal(
                    "\"small\".to_string()".to_string(),
                )))],
            },
        ],
        Some(vec![RustStmt::Return(Some(RustExpr::Literal(
            "\"other\".to_string()".to_string(),
        )))]),
    );
    assert!(match_rendered.contains("match value {"));
    assert!(match_rendered.contains("2 | 3 =>"));
    assert!(!match_rendered.contains("__java_switch_matched"));
}

#[test]
fn switch_plan_does_not_render_rust() {
    let source = include_str!("../src/switch_plan.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("RustStmt"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("emit_expr"));
    assert!(!source.contains("emit_statements"));
    assert!(!source.contains("render_switch"));
    assert!(!source.contains("switch_case_condition"));
    assert!(source.contains("pub fn plan_simple_return_switch"));
    assert!(source.contains("pub fn plan_simple_return_switch_case"));
}

#[test]
fn statement_render_owns_raw_block_and_enhanced_for_wrappers() {
    let body = vec![RustStmt::Expr(RustExpr::Call {
        function: Box::new(RustExpr::Var("touch".to_string())),
        args: vec![RustExpr::Var("item".to_string())],
    })];
    let local = LocalPlan {
        name: "count".to_string(),
        java_ty: JavaType::Primitive(PrimitiveType::Int),
        shape: RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int)),
        binding_mutable: true,
    };

    assert_eq!(
        local_let_stmt(&local, Some(RustExpr::Literal("1".to_string()))),
        RustStmt::Let {
            mutable: true,
            name: "count".to_string(),
            ty: Some(RustType::I32),
            value: Some(RustExpr::Literal("1".to_string())),
        }
    );
    let external_ids = JavaType::Named {
        name: "ExternalIds".to_string(),
        args: Vec::new(),
    };
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let external_local = LocalPlan {
        name: "ids".to_string(),
        java_ty: external_ids.clone(),
        shape: RustSlotShape::conservative(external_ids),
        binding_mutable: false,
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalIds".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: Some(external_id),
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    assert_eq!(
        local_let_stmt_with_facts(&external_local, None, &facts),
        RustStmt::Let {
            mutable: false,
            name: "ids".to_string(),
            ty: Some(RustType::Vec(Box::new(RustType::Named(
                "ExternalId".to_string()
            )))),
            value: None,
        }
    );
    assert_eq!(
        assignment_stmt(
            RustExpr::Var("count".to_string()),
            RustExpr::Literal("2".to_string())
        ),
        RustStmt::Assign {
            target: RustExpr::Var("count".to_string()),
            value: RustExpr::Literal("2".to_string()),
        }
    );
    assert_eq!(
        expr_stmt(RustExpr::Var("tick".to_string())),
        RustStmt::Expr(RustExpr::Var("tick".to_string()))
    );
    assert_eq!(
        if_stmt(RustExpr::Var("ready".to_string()), body.clone(), Vec::new()),
        RustStmt::If {
            condition: RustExpr::Var("ready".to_string()),
            then_body: body.clone(),
            else_body: Vec::new(),
        }
    );
    assert_eq!(
        while_stmt(RustExpr::Var("ready".to_string()), body.clone()),
        RustStmt::While {
            condition: RustExpr::Var("ready".to_string()),
            body: body.clone(),
        }
    );
    assert_eq!(
        break_stmt(Some("loop_label".to_string())),
        RustStmt::Break(Some("loop_label".to_string()))
    );
    assert_eq!(continue_stmt(None), RustStmt::Continue(None));
    assert_eq!(
        raw_stmt("unsafe_raw();".to_string()),
        RustStmt::Raw("unsafe_raw();".to_string())
    );
    assert_eq!(
        render_enhanced_for("item", &RustExpr::Var("items".to_string()), &body),
        RustStmt::Raw("for item in items.iter() {\n    touch(item);\n\n}".to_string())
    );
    assert_eq!(
        render_block_stmt(&body),
        RustStmt::Raw("{\n    touch(item);\n\n}".to_string())
    );
}

#[test]
fn statement_render_renders_local_types_with_facts() {
    let source = include_str!("../src/statement_render.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("pub fn local_let_stmt_with_facts"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&local.shape)"));
    assert!(lower_source.contains("local_let_stmt_with_facts"));
}

#[test]
fn loop_lowering_owns_classic_for_default_condition_label_and_continue_updates() {
    let body = vec![RustStmt::Expr(RustExpr::Var("body".to_string()))];
    let update_exprs = vec![parse_expr("i++")];
    let then_block = new_java2rust::java_ir::Block {
        statements: vec![Stmt::Break(None)],
    };
    let no_break_block = new_java2rust::java_ir::Block {
        statements: vec![Stmt::Expr(parse_expr("cursor.advance()"))],
    };
    let else_block = new_java2rust::java_ir::Block {
        statements: vec![Stmt::Continue(Some("outer".to_string()))],
    };
    assert_eq!(
        assemble_for_loop(None, body.clone(), Some("outer".to_string())),
        RustStmt::While {
            condition: RustExpr::Literal("true".to_string()),
            body: vec![RustStmt::Loop {
                label: Some("outer".to_string()),
                body,
            }],
        }
    );
    assert_eq!(
        continue_with_updates(&[RustExpr::Var("i += 1".to_string())]),
        vec![
            RustStmt::Expr(RustExpr::Var("i += 1".to_string())),
            RustStmt::Continue(None),
        ]
    );
    assert_eq!(
        plan_for_body_stmt(&Stmt::Continue(None), &update_exprs),
        ForBodyStmtPlan::ContinueWithUpdates {
            updates: update_exprs.as_slice()
        }
    );
    assert_eq!(
        plan_for_body_stmt(
            &Stmt::If {
                condition: parse_expr("ready"),
                then_block: then_block.clone(),
                else_block: Some(else_block.clone()),
            },
            &update_exprs,
        ),
        ForBodyStmtPlan::If {
            condition: &parse_expr("ready"),
            then_block: &then_block,
            else_block: Some(&else_block),
        }
    );
    assert_eq!(
        plan_for_body_stmt(
            &Stmt::While {
                condition: parse_expr("ready"),
                body: then_block.clone(),
            },
            &update_exprs,
        ),
        ForBodyStmtPlan::While {
            condition: &parse_expr("ready"),
            body: &then_block,
        }
    );
    let assignment_condition = parse_expr("(row = cursor.next()) != null");
    assert!(matches!(
        plan_while_stmt(&assignment_condition, &then_block),
        WhileStmtPlan::AssignmentInCondition {
            local_name: "row",
            value,
            null_test: NullTestKind::NotNull,
            body: _
        } if matches!(value, Expr::Call { name, .. } if name == "next")
    ));
    let null_assignment_condition = parse_expr("(row = cursor.next()) == null");
    let null_assignment_plan = plan_while_stmt(&null_assignment_condition, &no_break_block);
    assert_eq!(
        post_loop_refinement_for_while(&null_assignment_plan),
        PostLoopRefinementPlan::LocalNonNull { local_name: "row" }
    );
    assert_eq!(
        post_loop_refinement_for_while(&plan_while_stmt(&assignment_condition, &then_block)),
        PostLoopRefinementPlan::None
    );
    assert_eq!(
        assignment_condition_loop(
            "row",
            RustExpr::Raw("cursor.next()".to_string()),
            NullTestKind::NotNull,
            vec![RustStmt::Expr(RustExpr::Raw(
                "count += row.size()".to_string()
            ))],
        ),
        RustStmt::Loop {
            label: None,
            body: vec![
                RustStmt::Assign {
                    target: RustExpr::Var("row".to_string()),
                    value: RustExpr::Raw("cursor.next()".to_string()),
                },
                RustStmt::If {
                    condition: RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("row".to_string())),
                        method: "is_none".to_string(),
                        args: Vec::new(),
                    },
                    then_body: vec![RustStmt::Break(None)],
                    else_body: Vec::new(),
                },
                RustStmt::Let {
                    mutable: false,
                    name: "row".to_string(),
                    ty: None,
                    value: Some(RustExpr::Raw("row.as_ref().unwrap()".to_string())),
                },
                RustStmt::Expr(RustExpr::Raw("count += row.size()".to_string())),
            ],
        }
    );

    let plan_source = include_str!("../src/loop_plan.rs");
    let lowering_source = include_str!("../src/loop_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");
    assert!(plan_source.contains("pub enum ForBodyStmtPlan"));
    assert!(plan_source.contains("pub fn plan_for_body_stmt"));
    assert!(plan_source.contains("pub enum WhileStmtPlan"));
    assert!(plan_source.contains("pub fn plan_while_stmt"));
    assert!(plan_source.contains("pub enum PostLoopRefinementPlan"));
    assert!(plan_source.contains("pub fn post_loop_refinement_for_while"));
    assert!(!plan_source.contains("RustStmt"));
    assert!(!plan_source.contains("RustExpr"));
    assert!(lowering_source.contains("continue_with_updates"));
    assert!(lowering_source.contains("assignment_condition_loop"));
    assert!(lower_source.contains("plan_for_body_stmt"));
    assert!(lower_source.contains("plan_while_stmt"));
    assert!(lower_source.contains("ForBodyStmtPlan::ContinueWithUpdates"));
    assert!(!lower_source.contains("Stmt::Continue(None)"));
}

#[test]
fn enhanced_for_over_optional_sequence_unwraps_before_iteration() {
    let translation = translate_source(
        r#"
import java.util.List;

class OptionalSequenceLoop {
    private List<String> paths;

    void visit() {
        if (paths != null && !paths.isEmpty()) {
            for (String path : paths) {
                touch(path);
            }
        }
    }

    void touch(String path) {
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("for path in self.paths.as_ref().expect(\"Java null receiver\").iter()"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("for path in self.paths.iter()"),
        "{}",
        translation.rust
    );
}

#[test]
fn expr_render_owns_basic_rust_expression_shells() {
    assert_eq!(this_expr(), RustExpr::Var("self".to_string()));
    assert_eq!(super_expr(), RustExpr::Raw("super".to_string()));
    assert_eq!(raw_expr("value"), RustExpr::Raw("value".to_string()));
    assert_eq!(
        literal_expr(&Literal::String("value".to_string())),
        RustExpr::Literal("\"value\".to_string()".to_string())
    );
    assert_eq!(
        indexed_expr(
            RustExpr::Var("values".to_string()),
            RustExpr::Literal("0usize".to_string()),
        ),
        RustExpr::Index(
            Box::new(RustExpr::Var("values".to_string())),
            Box::new(RustExpr::Literal("0usize".to_string())),
        )
    );
    assert_eq!(
        binary_expr(
            new_java2rust::java_ir::BinaryOp::Add,
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
        ),
        RustExpr::Binary {
            op: "+".to_string(),
            left: Box::new(RustExpr::Var("left".to_string())),
            right: Box::new(RustExpr::Var("right".to_string())),
        }
    );
    assert_eq!(
        ternary_expr(
            RustExpr::Var("ready".to_string()),
            RustExpr::Var("yes".to_string()),
            RustExpr::Var("no".to_string()),
        ),
        RustExpr::If {
            condition: Box::new(RustExpr::Var("ready".to_string())),
            then_expr: Box::new(RustExpr::Var("yes".to_string())),
            else_expr: Box::new(RustExpr::Var("no".to_string())),
        }
    );
}

#[test]
fn method_reference_plan_owns_generic_function_adapter_shapes() {
    assert_eq!(
        plan_method_reference("IllegalArgumentException", "new"),
        MethodReferencePlan::Constructor {
            type_name: "IllegalArgumentException".to_string()
        }
    );
    assert_eq!(
        render_method_reference(&plan_method_reference("IllegalArgumentException", "new")),
        RustExpr::Raw("IllegalArgumentException::new".to_string())
    );
    assert_eq!(
        render_method_reference(&plan_method_reference("Score", "add")),
        RustExpr::Raw("Score::add".to_string())
    );
    assert_eq!(
        render_method_reference(&plan_method_reference("Tuple2", "_2")),
        RustExpr::Raw("|value| value._2()".to_string())
    );
    assert_eq!(
        render_method_reference(&plan_method_reference("NameHolder", "getName")),
        RustExpr::Raw("|value| value.get_name()".to_string())
    );
    assert_eq!(
        render_method_reference(&plan_method_reference("this", "emit")),
        RustExpr::Raw("|value| self.emit(value)".to_string())
    );
    assert_eq!(
        render_method_reference(&MethodReferencePlan::CurrentReceiverMethod {
            method: "emit".to_string(),
            arity: Some(3),
        }),
        RustExpr::Raw("|arg0, arg1, arg2| self.emit(arg0, arg1, arg2)".to_string())
    );

    let lower_source = include_str!("../src/lower.rs");
    let plan_source = include_str!("../src/method_reference_plan.rs");
    let lowering_source = include_str!("../src/method_reference_lowering.rs");
    assert!(lower_source.contains("plan_method_reference"));
    assert!(lower_source.contains("render_method_reference"));
    assert!(plan_source.contains("pub enum MethodReferencePlan"));
    assert!(!plan_source.contains("RustExpr"));
    assert!(lowering_source.contains("pub fn render_method_reference"));
}

#[test]
fn nullable_external_constructor_callback_slot_wraps_method_reference_branch() {
    let mut facts = FactSet::default();
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "ExternalConfig".to_string(),
            arg_types: vec![
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
                JavaType::Unknown("_".to_string()),
            ],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            receiver_type: Some("ExternalConfig".to_string()),
            method_name: "new".to_string(),
            parameter_index: 1,
            parameter_type: Some(JavaType::Named {
                name: "Function".to_string(),
                args: Vec::new(),
            }),
            parameter_nullability: NullabilityState::MaybeNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
class ExternalConstructorCallbackUse {
    Object readerFactory(Object readers, int begin, int end) {
        return readers;
    }

    Object configure(boolean bypass) {
        return new ExternalConfig("workspace", bypass ? null : this::readerFactory, 7);
    }
}
"#,
        &facts,
    );

    assert!(
        translation.rust.contains("ExternalConfig::new"),
        "constructor call should remain an external constructor boundary:\n{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("if bypass { None } else { Some("),
        "nullable constructor callback slot should wrap the non-null branch once:\n{}",
        translation.rust
    );
    assert!(
        !translation
            .rust
            .contains("if bypass { None } else { |value|"),
        "method reference branch must not bypass optional slot conversion:\n{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("Some(|arg0, arg1, arg2| self.reader_factory(arg0, arg1, arg2))"),
        "current-receiver method reference should use the source method arity:\n{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains(".downcast::<Function>()"),
        "callback slot conversion should not downcast a target-typed method reference:\n{}",
        translation.rust
    );
}

#[test]
fn external_constructor_owned_set_param_clones_optional_self_field_element() {
    let mut facts = FactSet::default();
    facts
        .external_constructor_facts
        .push(ExternalConstructorFact {
            type_name: "ExternalConfig".to_string(),
            arg_types: vec![JavaType::Unknown("_".to_string())],
            provenance: FactProvenance::ExternalLibraryPack,
        });
    facts
        .external_api_argument_effect_facts
        .push(ExternalApiArgumentEffectFact {
            receiver_type: Some("ExternalConfig".to_string()),
            method_name: "new".to_string(),
            parameter_index: 0,
            parameter_type: Some(JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::Named {
                    name: "HeaderLine".to_string(),
                    args: Vec::new(),
                }],
            }),
            parameter_nullability: NullabilityState::DefinitelyNonNull,
            argument_mutability: ReceiverMutabilityFact::ReadOnly,
            alias_policy: ExternalArgumentAliasPolicy::Escapes,
            provenance: FactProvenance::ExternalLibraryPack,
        });

    let translation = translate_source_with_facts(
        r#"
import java.util.Set;

class HeaderLine {}
class ExternalConstructorSetUse {
    Set<HeaderLine> lines;

    Object configure() {
        return new ExternalConfig(lines);
    }
}
"#,
        &facts,
    );

    assert!(
        translation.rust.contains(
            "ExternalConfig::new(self.lines.as_ref().expect(\"Java null receiver\").clone()"
        ) && !translation.rust.contains("self.lines.unwrap()"),
        "typed external constructor set parameter should recover owned value without moving self field:\n{}",
        translation.rust
    );
}

#[test]
fn array_plan_and_lowering_own_java_array_shape_before_orchestrator() {
    let creation_expr = parse_expr("new int[rows][cols]");
    let sized_primitive_expr = parse_expr("new byte[len]");
    let typed_initializer_expr = parse_expr("new String[]{prefix + comment}");
    let initializer_expr = parse_expr("{1, 2}");

    let Some(ArrayExprPlan::Creation {
        element_type,
        dimensions,
    }) = plan_array_expr(&creation_expr)
    else {
        panic!("expected array creation plan");
    };
    assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(dimensions.len(), 2);

    let Some(
        sized_primitive_plan @ ArrayExprPlan::Creation {
            element_type,
            dimensions,
        },
    ) = plan_array_expr(&sized_primitive_expr)
    else {
        panic!("expected sized primitive array creation plan");
    };
    assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Byte));
    assert_eq!(dimensions.len(), 1);

    let Some(
        typed_initializer_plan @ ArrayExprPlan::TypedInitializer {
            element_type,
            elements,
        },
    ) = plan_array_expr(&typed_initializer_expr)
    else {
        panic!("expected typed array initializer plan");
    };
    assert_eq!(element_type, &JavaType::String);
    assert_eq!(elements.len(), 1);
    assert_eq!(typed_initializer_plan.child_exprs(), elements);

    let Some(initializer_plan @ ArrayExprPlan::Initializer { elements }) =
        plan_array_expr(&initializer_expr)
    else {
        panic!("expected array initializer plan");
    };
    assert_eq!(elements.len(), 2);
    assert_eq!(
        initializer_plan.child_exprs(),
        elements,
        "array plan exposes child expressions without rendering them"
    );
    assert_eq!(plan_array_expr(&parse_expr("value")), None);

    assert_eq!(
        render_array_expr(
            &sized_primitive_plan,
            vec![RustExpr::Var("len".to_string())],
        ),
        RustExpr::Raw("vec![0; len as usize]".to_string())
    );
    assert_eq!(
        render_array_expr(
            &typed_initializer_plan,
            vec![RustExpr::Raw(
                "format!(\"{}{}\", prefix, comment)".to_string()
            )],
        ),
        RustExpr::Raw("vec![format!(\"{}{}\", prefix, comment)]".to_string())
    );
    assert_eq!(
        render_array_expr(
            &ArrayExprPlan::Creation {
                element_type: &JavaType::Primitive(PrimitiveType::Int),
                dimensions,
            },
            vec![
                RustExpr::Var("rows".to_string()),
                RustExpr::Var("cols".to_string()),
            ],
        ),
        RustExpr::Raw(
            "/* Java array creation: Primitive(Int)[rows][cols] */ Vec::new()".to_string()
        )
    );
    assert_eq!(
        render_array_expr(
            &initializer_plan,
            vec![
                RustExpr::Literal("1".to_string()),
                RustExpr::Literal("2".to_string()),
            ],
        ),
        RustExpr::Raw("vec![1, 2]".to_string())
    );

    let plan_source = include_str!("../src/array_plan.rs");
    let lowering_source = include_str!("../src/array_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");
    assert!(!plan_source.contains("rust_ir"));
    assert!(!plan_source.contains("RustExpr"));
    assert!(!plan_source.contains("emit_expr"));
    assert!(lowering_source.contains("emit_expr"));
    assert!(lowering_source.contains("Java array creation"));
    assert!(lower_source.contains("plan_array_expr"));
    assert!(lower_source.contains("render_array_expr"));
    assert!(!lower_source.contains("crate::emit::emit_expr"));
    assert!(!lower_source.contains("Java array creation:"));
    assert!(!lower_source.contains("vec![{}]"));
}

#[test]
fn operator_lowering_owns_binary_and_unary_rust_spelling() {
    assert_eq!(
        binary_op_to_rust(new_java2rust::java_ir::BinaryOp::Shl),
        "<<"
    );
    assert_eq!(
        null_comparison_method(new_java2rust::java_ir::BinaryOp::Eq, false),
        Some(NullComparisonMethod::IsNone)
    );
    assert_eq!(
        null_comparison_method(new_java2rust::java_ir::BinaryOp::Ne, true),
        Some(NullComparisonMethod::IsSome)
    );
    assert_eq!(
        lower_null_comparison_expr(
            new_java2rust::java_ir::BinaryOp::Ne,
            RustExpr::Var("value".to_string()),
            false,
        ),
        Some(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "is_some".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(
        plan_numeric_binary_promotion(
            new_java2rust::java_ir::BinaryOp::Mul,
            &JavaType::Primitive(PrimitiveType::Long),
            &JavaType::Primitive(PrimitiveType::Int),
        ),
        Some(NumericBinaryPromotionPlan {
            promoted: PrimitiveType::Long,
            left: Vec::new(),
            right: vec![new_java2rust::coercion::Coercion::WidenPrimitive {
                from: PrimitiveType::Int,
                to: PrimitiveType::Long,
            }],
        })
    );
    assert_eq!(
        lower_numeric_binary_expr(
            new_java2rust::java_ir::BinaryOp::Mul,
            RustExpr::Var("acc".to_string()),
            RustExpr::Var("n".to_string()),
            plan_numeric_binary_promotion(
                new_java2rust::java_ir::BinaryOp::Mul,
                &JavaType::Primitive(PrimitiveType::Long),
                &JavaType::Primitive(PrimitiveType::Int),
            )
            .unwrap(),
        ),
        RustExpr::Binary {
            op: "*".to_string(),
            left: Box::new(RustExpr::Var("acc".to_string())),
            right: Box::new(RustExpr::Cast {
                expr: Box::new(RustExpr::Var("n".to_string())),
                ty: "i64".to_string(),
            }),
        }
    );
    assert_eq!(
        plan_numeric_binary_promotion(
            new_java2rust::java_ir::BinaryOp::Lt,
            &JavaType::Primitive(PrimitiveType::Int),
            &JavaType::Primitive(PrimitiveType::Long),
        ),
        Some(NumericBinaryPromotionPlan {
            promoted: PrimitiveType::Long,
            left: vec![new_java2rust::coercion::Coercion::WidenPrimitive {
                from: PrimitiveType::Int,
                to: PrimitiveType::Long,
            }],
            right: Vec::new(),
        })
    );
    assert_eq!(
        lower_numeric_binary_expr(
            new_java2rust::java_ir::BinaryOp::Lt,
            RustExpr::Var("end".to_string()),
            RustExpr::Var("limit".to_string()),
            plan_numeric_binary_promotion(
                new_java2rust::java_ir::BinaryOp::Lt,
                &JavaType::Primitive(PrimitiveType::Int),
                &JavaType::Primitive(PrimitiveType::Long),
            )
            .unwrap(),
        ),
        RustExpr::Binary {
            op: "<".to_string(),
            left: Box::new(RustExpr::Cast {
                expr: Box::new(RustExpr::Var("end".to_string())),
                ty: "i64".to_string(),
            }),
            right: Box::new(RustExpr::Var("limit".to_string())),
        }
    );
    let string_shape = RustSlotShape::conservative(JavaType::String);
    let optional_string_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Optional".to_string(),
        args: vec![JavaType::String],
    });
    assert_eq!(
        plan_equality_comparison(&string_shape, &string_shape),
        EqualityComparisonPlan::Direct
    );
    assert_eq!(
        plan_equality_comparison(&optional_string_shape, &optional_string_shape),
        EqualityComparisonPlan::BothOptionalByRef
    );
    assert_eq!(
        plan_equality_comparison(&optional_string_shape, &string_shape),
        EqualityComparisonPlan::LeftOptionalByRef
    );
    assert_eq!(
        plan_equality_comparison(&string_shape, &optional_string_shape),
        EqualityComparisonPlan::RightOptionalByRef
    );
    assert_eq!(
        lower_equality_comparison_expr(
            EqualityComparisonPlan::BothOptionalByRef,
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
        ),
        RustExpr::Binary {
            op: "==".to_string(),
            left: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("left".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
            right: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("right".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
        }
    );
    assert_eq!(
        lower_equality_comparison_expr_for_op(
            new_java2rust::java_ir::BinaryOp::Ne,
            EqualityComparisonPlan::RightOptionalByRef,
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
        ),
        RustExpr::Raw("Some(&left) != right.as_ref()".to_string())
    );
    let mut borrowed_str_shape = RustSlotShape::conservative(JavaType::String);
    borrowed_str_shape.ownership = OwnershipShape::SharedBorrow;
    borrowed_str_shape.view = ViewShape::Str;
    assert_eq!(
        plan_equality_comparison(&borrowed_str_shape, &optional_string_shape),
        EqualityComparisonPlan::RightOptionalStringView
    );
    assert_eq!(
        plan_equality_comparison(&optional_string_shape, &borrowed_str_shape),
        EqualityComparisonPlan::LeftOptionalStringView
    );
    assert_eq!(
        lower_equality_comparison_expr(
            EqualityComparisonPlan::RightOptionalStringView,
            RustExpr::Raw("name.trim()".to_string()),
            RustExpr::Var("sample_name".to_string()),
        ),
        RustExpr::Raw("Some(name.trim()) == sample_name.as_deref()".to_string())
    );
    let nullable_value_ty = JavaType::Named {
        name: "NullableValue".to_string(),
        args: Vec::new(),
    };
    let optional_value_shape = RustSlotShape::conservative(optional_java_type(nullable_value_ty));
    let mut equality_facts = FactSet::default();
    equality_facts.method_effect_facts.push(MethodEffectFact {
        method: FactMethodKey {
            type_name: "NullableValue".to_string(),
            method_name: "equals".to_string(),
        },
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: Vec::new(),
        provenance: FactProvenance::ParsedSource,
    });
    assert_eq!(
        plan_equality_comparison_with_facts(
            &optional_value_shape,
            &optional_value_shape,
            &equality_facts,
        ),
        EqualityComparisonPlan::BothOptionalCustomEquals {
            method_name: "equals".to_string(),
        }
    );
    assert_eq!(
        lower_equality_comparison_expr(
            EqualityComparisonPlan::BothOptionalCustomEquals {
                method_name: "equals".to_string(),
            },
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
        ),
        RustExpr::Raw(
            "match (left, right) { (Some(left), Some(right)) => left.equals(right), (None, None) => true, _ => false }"
                .to_string(),
        )
    );
    assert_eq!(
        lower_unary_expr(
            new_java2rust::java_ir::UnaryOp::PostInc,
            RustExpr::Var("i".to_string()),
        ),
        RustExpr::Raw("i += 1".to_string())
    );
    assert_eq!(
        lower_unary_expr(
            new_java2rust::java_ir::UnaryOp::Not,
            RustExpr::Var("ready".to_string()),
        ),
        RustExpr::Unary {
            op: "!".to_string(),
            expr: Box::new(RustExpr::Var("ready".to_string())),
        }
    );
    assert_eq!(
        stream_unary_prefix(new_java2rust::java_ir::UnaryOp::Neg),
        Some("-")
    );
    assert_eq!(
        stream_unary_prefix(new_java2rust::java_ir::UnaryOp::PostDec),
        None
    );
    assert_eq!(
        plan_ternary_null_refinement(&parse_expr("old == null")),
        TernaryNullRefinementPlan::LocalNullTest {
            local_name: "old",
            then_non_null: false,
            else_non_null: true,
        }
    );
    assert_eq!(
        plan_ternary_null_refinement(&parse_expr("old != null")),
        TernaryNullRefinementPlan::LocalNullTest {
            local_name: "old",
            then_non_null: true,
            else_non_null: false,
        }
    );
    assert_eq!(
        plan_ternary_null_refinement(&parse_expr("ready")),
        TernaryNullRefinementPlan::None
    );

    let nullability_plan_source = include_str!("../src/nullability_plan.rs");
    assert!(nullability_plan_source.contains("pub enum TernaryNullRefinementPlan"));
    assert!(nullability_plan_source.contains("pub fn plan_ternary_null_refinement"));
}

#[test]
fn literal_lowering_owns_java_literal_rust_spelling() {
    assert_eq!(literal_to_rust(&Literal::Null), "None");
    assert_eq!(literal_to_rust(&Literal::Bool(true)), "true");
    assert_eq!(literal_to_rust(&Literal::Int(7)), "7");
    assert_eq!(literal_to_rust(&Literal::Long(7)), "7i64");
    assert_eq!(literal_to_rust(&Literal::Float(1.5)), "1.5f32");
    assert_eq!(literal_to_rust(&Literal::Double(1.5)), "1.5f64");
    assert_eq!(
        literal_to_rust(&Literal::String("name".to_string())),
        "\"name\".to_string()"
    );
    assert_eq!(literal_to_rust(&Literal::Char('x')), "'x'");
}

#[test]
fn access_lowering_owns_bare_field_and_member_access_fallbacks() {
    let mut env = ShapeEnv::new();
    env.insert_field("count", JavaType::Primitive(PrimitiveType::Int));
    env.insert_static_field_shape(
        "LIMIT",
        RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
    );
    env.insert_local("local", JavaType::Primitive(PrimitiveType::Int));

    assert_eq!(
        lower_bare_name("count", &env),
        RustExpr::Field(
            Box::new(RustExpr::Var("self".to_string())),
            "count".to_string()
        )
    );
    assert_eq!(
        lower_bare_name("local", &env),
        RustExpr::Var("local".to_string())
    );
    assert_eq!(
        lower_bare_name("LIMIT", &env),
        RustExpr::Raw("Self::LIMIT".to_string())
    );

    let mut type_env = TypeEnv::new();
    type_env.insert_local("values", JavaType::Array(Box::new(JavaType::String)));
    type_env.insert_local(
        "callback",
        JavaType::Named {
            name: "BatchCompletionCallbackFunctionArgument".to_string(),
            args: Vec::new(),
        },
    );
    assert_eq!(
        lower_field_access(
            &parse_expr("values"),
            "length",
            &type_env,
            RustExpr::Var("values".to_string()),
        ),
        RustExpr::Raw("values.len() as i32".to_string())
    );
    assert_eq!(
        lower_field_access(
            &parse_expr("callback"),
            "batchCount",
            &type_env,
            RustExpr::Var("callback".to_string()),
        ),
        RustExpr::Field(
            Box::new(RustExpr::Var("callback".to_string())),
            "batch_count".to_string()
        )
    );
    assert_eq!(
        lower_field_access(
            &parse_expr("helper"),
            "name",
            &type_env,
            RustExpr::Var("helper".to_string()),
        ),
        RustExpr::Field(
            Box::new(RustExpr::Var("helper".to_string())),
            "name".to_string()
        )
    );
    assert_eq!(
        lower_field_access(
            &parse_expr("Owner"),
            "CONSTANT",
            &type_env,
            RustExpr::Var("owner".to_string()),
        ),
        RustExpr::Raw("Owner::CONSTANT".to_string())
    );

    let access_lowering_source = include_str!("../src/access_lowering.rs");
    let member_plan_source = include_str!("../src/member_access_plan.rs");
    assert!(member_plan_source.contains("pub struct JavaMemberAccessPolicy"));
    assert!(member_plan_source.contains("pub enum JavaMemberAccessKind"));
    assert!(member_plan_source.contains("pub fn java_member_access_policy"));
    assert!(member_plan_source.contains("MemberAccessOp::LenAsI32"));
    assert!(member_plan_source.contains("MemberAccessOp::StaticConstant"));
    assert!(member_plan_source.contains("MemberAccessOp::RustField"));
    assert!(!access_lowering_source.contains("\"length\""));
    assert!(!access_lowering_source.contains("infer_member_access_type"));
}

#[test]
fn source_instance_field_access_uses_rust_field_names() {
    let translation = translate_source(
        r#"
class FieldNameFlow {
    int batchCount;
    int totalBatchCount;

    int read(FieldNameFlow other) {
        return other.batchCount + this.totalBatchCount;
    }

    void write(FieldNameFlow other) {
        this.batchCount = other.totalBatchCount;
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub batch_count: i32")
            || translation.rust.contains("batch_count: i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("other.batch_count"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.total_batch_count"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.batch_count = other.total_batch_count"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains(".batchCount"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains(".totalBatchCount"),
        "{}",
        translation.rust
    );
}

#[test]
fn stream_map_to_int_shape_is_inferred_before_lowering() {
    let shape = infer_stream_map_to_int_shape(&parse_expr("value.length()"));

    assert_eq!(shape.java_ty, JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(shape.ownership, OwnershipShape::CopyValue);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn stream_collect_lowered_type_is_shape_driven() {
    let mut element = RustSlotShape::conservative(JavaType::String);
    element.ownership = OwnershipShape::SharedBorrow;
    element.view = ViewShape::Str;

    let lowered = stream_collect_actual(element);

    assert!(matches!(
        lowered,
        LoweredType::VecElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
                && element_shape.ownership == OwnershipShape::SharedBorrow
                && element_shape.view == ViewShape::Str
    ));
}

#[test]
fn stream_plan_owns_terminal_actual_type_before_lowering() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr("values.stream().map(value -> value.trim()).findFirst()");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();

    assert!(matches!(
        plan.actual,
        LoweredType::OptionalElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
                && element_shape.ownership == OwnershipShape::SharedBorrow
                && element_shape.view == ViewShape::Str
    ));
}

#[test]
fn stream_plan_accepts_simple_captureful_lambdas_without_global_capture_veto() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        list_java_type(JavaType::Primitive(PrimitiveType::Int)),
    );
    env.insert_local("base", JavaType::Primitive(PrimitiveType::Int));
    let expr = parse_expr("values.stream().mapToInt(value -> value + base).sum()");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert_eq!(
        lowered.expr,
        RustExpr::Raw("values.iter().map(|value| (*value + base)).sum::<i32>()".to_string())
    );
}

#[test]
fn stream_plan_owns_flat_map_lambda_collection_stream_shape() {
    let mut env = TypeEnv::new();
    env.insert_local("groups", list_java_type(list_java_type(JavaType::String)));
    let expr =
        parse_expr("groups.stream().flatMap(group -> group.stream()).collect(Collectors.toList())");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("groups".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [StreamOp::FlatMap(FlatMapPlan::LambdaCollectionStream(flat_map))]
            if flat_map.lambda.param_name == "group"
                && flat_map.iteration == StreamSourceIteration::Borrowed
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "groups.iter().flat_map(|group| group.iter()).collect::<Vec<_>>()".to_string()
        )
    );
}

#[test]
fn stream_plan_infers_flat_map_lambda_projection_stream_shape() {
    let parsed = parse_java_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class StreamFlatMapProjection {
    List<String> flatten(List<NameGroup> groups) {
        return groups.stream()
                .flatMap(group -> group.names().stream())
                .collect(Collectors.toList());
    }
}

class NameGroup {
    List<String> names() {
        return null;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "flatten" => Some(method),
            _ => None,
        })
        .unwrap();
    let Some(Stmt::Return(Some(expr))) = method.body.as_ref().unwrap().statements.first() else {
        panic!("expected stream return");
    };
    let mut env = TypeEnv::new();
    env.insert_local(
        "groups",
        list_java_type(JavaType::Named {
            name: "NameGroup".to_string(),
            args: Vec::new(),
        }),
    );

    let plan = build_stream_plan(expr, &env, &resolved.facts).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("groups".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [StreamOp::FlatMap(FlatMapPlan::LambdaCollectionStream(flat_map))]
            if flat_map.lambda.param_name == "group"
                && flat_map.iteration == StreamSourceIteration::OwnedValue
    ));
    assert!(matches!(
        plan.actual,
        LoweredType::VecElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "groups.iter().flat_map(|group| group.names().into_iter()).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_plan_lowers_flat_map_block_returning_stream_pipeline() {
    let parsed = parse_java_source(
        r#"
import java.util.List;
import java.util.stream.Collectors;

class StreamFlatMapBlockPipeline {
    List<String> cleanGroups(List<Group> groups) {
        return groups.stream().flatMap(group -> {
            List<String> names = group.names();
            return names.stream()
                    .filter(name -> !name.isEmpty())
                    .map(name -> name.trim());
        }).collect(Collectors.toList());
    }
}

class Group {
    List<String> names() {
        return null;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "cleanGroups" => Some(method),
            _ => None,
        })
        .unwrap();
    let Some(Stmt::Return(Some(expr))) = method.body.as_ref().unwrap().statements.first() else {
        panic!("expected stream return");
    };
    let mut env = TypeEnv::new();
    env.insert_local(
        "groups",
        list_java_type(JavaType::Named {
            name: "Group".to_string(),
            args: Vec::new(),
        }),
    );

    let plan = build_stream_plan(expr, &env, &resolved.facts).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("groups".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [StreamOp::FlatMap(FlatMapPlan::LambdaStreamChain { lambda, pipeline })]
            if lambda.param_name == "group"
                && pipeline.ops.len() == 2
                && pipeline.output_shape.java_ty == JavaType::String
    ));
    assert!(matches!(
        plan.actual,
        LoweredType::VecElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "groups.iter().flat_map(|group| { let names = group.names(); names.iter().filter(|name| !name.is_empty()).map(|name| name.trim()) }).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_plan_accepts_value_returning_block_lambdas() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr(
        "values.stream().map(value -> { String trimmed = value.trim(); return trimmed.toLowerCase(); }).collect(Collectors.toList())",
    );

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert!(
        matches!(plan.ops.as_slice(), [StreamOp::Map { lambda, .. }] if lambda.body.prefix_statements().len() == 1)
    );
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().map(|value| { let trimmed = value.trim(); trimmed.to_lowercase() }).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_plan_keeps_mut_for_assigned_block_lambda_locals() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr(
        "values.stream().map(value -> { String trimmed = value.trim(); trimmed = trimmed.toLowerCase(); return trimmed; }).collect(Collectors.toList())",
    );

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().map(|value| { let mut trimmed = value.trim(); trimmed = trimmed.to_lowercase(); trimmed }).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_plan_unwraps_optional_sequence_sources_before_iteration() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        optional_java_type(list_java_type(JavaType::String)),
    );
    let expr =
        parse_expr("values.stream().map(value -> value.trim()).collect(Collectors.toList())");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert!(plan.source.optional_sequence);
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.as_ref().unwrap().iter().map(|value| value.trim()).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_plan_refines_non_null_filter_elements() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        list_java_type(optional_java_type(JavaType::String)),
    );
    let expr = parse_expr("values.stream().filter(Objects::nonNull).collect(Collectors.toList())");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [StreamOp::Filter(FilterPlan::NonNull { output_shape, .. })]
            if output_shape.java_ty == JavaType::String
    ));
    assert!(matches!(
        plan.actual,
        LoweredType::VecElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().filter_map(|item| item.as_ref()).collect::<Vec<_>>()".to_string()
        )
    );
}

#[test]
fn stream_plan_fuses_nullable_map_with_non_null_filter_shape() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr(
        "values.stream().map(value -> { if (value.isEmpty()) { return null; } return value.toLowerCase(); }).filter(Objects::nonNull).collect(Collectors.toList())",
    );

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [
            StreamOp::Map { output_shape, .. },
            StreamOp::Filter(FilterPlan::NonNull { output_shape: refined_shape, .. })
        ] if output_shape.optional_element_type() == Some(JavaType::String)
            && refined_shape.java_ty == JavaType::String
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().map(|value| { if value.is_empty() { None } else { Some(value.to_lowercase()) } }).filter_map(|item| item).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn stream_lambda_lowers_split_length_and_integer_parse_in_nullable_map() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr(
        "values.stream().map(value -> { String[] pieces = value.split(\",\"); if (pieces.length != 3) { return null; } return Integer.parseInt(pieces[1]); }).filter(Objects::nonNull).collect(Collectors.toList())",
    );

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert!(matches!(
        plan.ops.as_slice(),
        [
            StreamOp::Map { output_shape, .. },
            StreamOp::Filter(FilterPlan::NonNull { output_shape: refined_shape, .. })
        ] if output_shape.optional_element_type() == Some(JavaType::Primitive(PrimitiveType::Int))
            && refined_shape.java_ty == JavaType::Primitive(PrimitiveType::Int)
    ));
    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().map(|value| { let pieces = value.split(\",\").collect::<Vec<_>>(); if (pieces.len() as i32) != 3 { None } else { Some(pieces[1 as usize].parse::<i32>().unwrap()) } }).filter_map(|item| item).collect::<Vec<_>>()"
                .to_string()
        )
    );
    let mut env = ShapeEnv::new();
    env.insert_local_shape(
        "vid_map_p_b",
        RustSlotShape::conservative(JavaType::Named {
            name: "VidMapPB".to_string(),
            args: Vec::new(),
        }),
    );
    assert_eq!(
        emit_stream_lambda_body_with_context(
            &parse_expr("vid_map_p_b.getContigsList()"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            StreamRenderContext::new(&env),
        ),
        Some("vid_map_p_b.get_contigs_list()".to_string())
    );
    assert_eq!(
        emit_stream_lambda_body(
            &parse_expr("constants.DELIMITER"),
            "partition",
            StreamLambdaValue::BorrowedItem,
        ),
        Some("Constants::DELIMITER".to_string())
    );
}

#[test]
fn stream_plan_lowers_collectors_to_map_terminal() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr = parse_expr("values.stream().collect(Collectors.toMap(v -> v, v -> v.length()))");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert_eq!(
        lowered.expr,
        RustExpr::Raw(
            "values.iter().map(|v| ((*v).clone(), v.len() as i32)).collect::<std::collections::HashMap<_, _>>()"
                .to_string()
        )
    );
    let LoweredType::Slot(shape) = lowered.actual else {
        panic!("Collectors.toMap should lower as a concrete map slot");
    };
    assert_eq!(
        shape.java_ty,
        JavaType::Named {
            name: "Map".to_string(),
            args: vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int)],
        }
    );
}

#[test]
fn stream_plan_does_not_render_rust_iterator_chains() {
    let source = include_str!("../src/stream.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("emit_expr"));
    assert!(!source.contains("render_stream_plan"));
    assert!(!source.contains("emit_stream_lambda_body"));
    assert!(!source.contains(".collect::<Vec<_>>()"));
    assert!(!source.contains(".next()"));
    assert!(!source.contains(".cloned()"));
    assert!(!source.contains("stream_lambdas_are_capture_free"));
}

#[test]
fn conversion_plan_owns_stream_collect_actual_shape_selection() {
    let stream_source = include_str!("../src/stream.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");

    assert!(conversion_source.contains("collected_sequence_from_element_shape"));
    assert!(!coercion_source.contains("collected_sequence_from_element_shape"));
    assert!(!stream_source.contains("OwnershipShape"));
    assert!(!stream_source.contains("list_java_type"));
    assert!(stream_source.contains("use crate::conversion_plan::LoweredType"));
    assert!(stream_source.contains("LoweredType::collected_sequence_from_element_shape"));
}

#[test]
fn stream_shape_does_not_own_lowered_actual_types() {
    let source = include_str!("../src/stream_shape.rs");

    assert!(!source.contains("LoweredType"));
    assert!(!source.contains("stream_terminal_actual"));
    assert!(!source.contains("stream_collect_actual"));
}

#[test]
fn stream_lowering_owns_iterator_chain_rendering_from_plan() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));
    let expr =
        parse_expr("values.stream().map(value -> value.trim()).collect(Collectors.toList())");

    let plan = build_stream_plan(&expr, &env, &FactSet::default()).unwrap();
    let lowered = render_stream_plan(&plan, RustExpr::Var("values".to_string())).unwrap();

    assert_eq!(
        lowered.expr,
        RustExpr::Raw("values.iter().map(|value| value.trim()).collect::<Vec<_>>()".to_string())
    );
    assert!(matches!(
        lowered.actual,
        LoweredType::VecElementBorrow { .. }
    ));
}

#[test]
fn resolved_call_return_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class StringReturnFlow {
    private static String id(String value) {
        return value;
    }

    public String call(String value) {
        return id(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let call_method = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "call" => Some(method),
            _ => None,
        })
        .unwrap();
    let return_expr = call_method
        .body
        .as_ref()
        .unwrap()
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        return_expr,
        Expr::Call {
            resolved: Some(_),
            ..
        }
    ));
    let solved_id_return = signatures
        .method_return_shape_by_rust_name("StringReturnFlow", "id")
        .expect("id should have a solved return shape");
    assert_eq!(solved_id_return.ownership, OwnershipShape::SharedBorrow);
    assert!(
        signatures
            .method_return_shape_by_rust_name("StringReturnFlow", "missing")
            .is_none()
    );

    let shape = infer_expr_shape_with_context(
        return_expr,
        &ShapeEnv::new(),
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
            facts: None,
        },
    );

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Str);
}

#[test]
fn typed_receiver_field_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Helper {
    String name;
}

class Caller {
    public String read(Helper helper) {
        return helper.name;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let caller = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Caller")
        .unwrap();
    let read_method = caller
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "read" => Some(method),
            _ => None,
        })
        .unwrap();
    let return_expr = read_method
        .body
        .as_ref()
        .unwrap()
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap();
    assert!(matches!(return_expr, Expr::FieldAccess(_, field) if field == "name"));

    let mut env = ShapeEnv::new();
    env.insert_local(
        "helper",
        JavaType::Named {
            name: "Helper".to_string(),
            args: Vec::new(),
        },
    );
    let shape = infer_expr_shape_with_context(
        return_expr,
        &env,
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
            facts: None,
        },
    );

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn typed_receiver_field_assignment_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Helper {
    String name;
}

class Caller {
    public void write(Helper helper, String value) {
        helper.name = value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);

    let mut env = ShapeEnv::new();
    env.insert_local(
        "helper",
        JavaType::Named {
            name: "Helper".to_string(),
            args: Vec::new(),
        },
    );
    let shape = infer_assignment_target_shape_with_context(
        &parse_expr("helper.name"),
        &env,
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
            facts: None,
        },
    )
    .unwrap();

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn named_lattice_descriptors_define_core_domains() {
    use new_java2rust::lattice::{
        DispatchLatticeValue, EscapeLatticeValue, InitializationLatticeValue, dispatch_lattice,
        dispatch_lattice_value, escape_lattice, field_nullability_join, initialization_lattice,
        nullability_lattice, ownership_lattice, receiver_mutability_lattice, view_lattice,
    };

    let nullability = nullability_lattice();
    assert_eq!(nullability.bottom, NullabilityState::DefinitelyNonNull);
    assert_eq!(nullability.top, NullabilityState::Unknown);
    assert!(nullability.values.contains(&NullabilityState::MaybeNull));
    assert!(nullability.merge_rule.contains("MaybeNull"));
    assert!(nullability.precedence_rule.contains("DefinitelyNonNull"));
    assert!(nullability.diagnostic_rule.contains("diagnostics"));

    let ownership = ownership_lattice();
    assert_eq!(ownership.bottom, OwnershipShape::SharedBorrow);
    assert_eq!(ownership.top, OwnershipShape::Owned);
    assert!(ownership.precedence_rule.contains("Owned"));

    let view = view_lattice();
    assert_eq!(view.bottom, ViewShape::Str);
    assert_eq!(view.top, ViewShape::Native);
    assert!(view.values.contains(&ViewShape::ErasedObjectView));

    let dispatch = dispatch_lattice();
    assert_eq!(dispatch.bottom, DispatchLatticeValue::Concrete);
    assert_eq!(dispatch.top, DispatchLatticeValue::RejectTraitObject);
    assert_eq!(
        dispatch_lattice_value(DispatchRequirementKind::NeedsOwnedPolymorphicStorage),
        DispatchLatticeValue::OwnedPolymorphicStorage
    );
    assert_eq!(
        dispatch_lattice_value(DispatchRequirementKind::NeedsUnknownRuntimeObject),
        DispatchLatticeValue::UnknownRuntimeObject
    );

    let mutability = receiver_mutability_lattice();
    assert_eq!(mutability.bottom, ReceiverMutabilityFact::ReadOnly);
    assert_eq!(mutability.top, ReceiverMutabilityFact::Unknown);

    let escape = escape_lattice();
    assert_eq!(escape.bottom, EscapeLatticeValue::LocalOnly);
    assert_eq!(escape.top, EscapeLatticeValue::UnknownBoundary);

    let initialization = initialization_lattice();
    assert_eq!(
        initialization.bottom,
        InitializationLatticeValue::Uninitialized
    );
    assert_eq!(initialization.top, InitializationLatticeValue::MaybeNull);

    assert_eq!(
        field_nullability_join(
            NullabilityState::DefinitelyNull,
            NullabilityState::DefinitelyNonNull,
        ),
        NullabilityState::MaybeNull
    );
    assert_eq!(
        field_nullability_join(
            NullabilityState::Unknown,
            NullabilityState::DefinitelyNonNull
        ),
        NullabilityState::Unknown
    );
    assert_eq!(
        field_nullability_join(NullabilityState::Unknown, NullabilityState::DefinitelyNull),
        NullabilityState::MaybeNull
    );
}

#[test]
fn core_signature_lattice_merges_satisfy_join_laws() {
    fn assert_join_laws<T>(values: &[T], join: fn(T, T) -> T)
    where
        T: Clone + Eq + std::fmt::Debug,
    {
        for value in values {
            assert_eq!(join(value.clone(), value.clone()), *value);
        }

        for left in values {
            for right in values {
                assert_eq!(
                    join(left.clone(), right.clone()),
                    join(right.clone(), left.clone())
                );
                let merged = join(left.clone(), right.clone());
                assert_eq!(join(merged.clone(), left.clone()), merged);
                assert_eq!(join(merged.clone(), right.clone()), merged);
            }
        }

        for left in values {
            for middle in values {
                for right in values {
                    assert_eq!(
                        join(join(left.clone(), middle.clone()), right.clone()),
                        join(left.clone(), join(middle.clone(), right.clone()))
                    );
                }
            }
        }
    }

    assert_join_laws(
        &[
            OwnershipShape::SharedBorrow,
            OwnershipShape::MutableBorrow,
            OwnershipShape::CopyValue,
            OwnershipShape::Owned,
        ],
        |left, right| merge_ownership_requirement_policy(Some(left), right).kept,
    );
    assert_join_laws(
        &[
            ViewShape::Str,
            ViewShape::DynTraitView,
            ViewShape::ErasedObjectView,
            ViewShape::Native,
        ],
        |left, right| merge_view_requirement_policy(Some(left), right).kept,
    );
    assert_join_laws(
        &[
            DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            DispatchShape::ErasedObject,
            DispatchShape::Concrete,
        ],
        |left, right| merge_dispatch_requirement_policy(Some(left), right).kept,
    );
    assert_join_laws(
        &[
            NullabilityState::DefinitelyNonNull,
            NullabilityState::Unknown,
            NullabilityState::DefinitelyNull,
            NullabilityState::MaybeNull,
        ],
        |left, right| merge_storage_nullability_requirement_policy(Some(left), right).kept,
    );
}
