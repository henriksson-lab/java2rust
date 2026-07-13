use std::collections::HashSet;

use new_java2rust::access_lowering::{lower_bare_name, lower_field_access};
use new_java2rust::array_lowering::render_array_expr;
use new_java2rust::array_plan::{ArrayExprPlan, plan_array_expr};
use new_java2rust::call_lowering::{
    apply_call_target, apply_new_target, apply_new_target_with_facts,
};
use new_java2rust::call_plan::{
    ArgumentLoweringPlan, CallExprPlan, CallStatementPlan, CallTargetPlan, TrivialAccessorCallPlan,
    plan_call_arguments, plan_call_expr, plan_call_statement, plan_call_target,
    plan_constructor_arguments, plan_new_target, plan_trivial_accessor_call,
    resolved_call_arg_shapes, resolved_constructor_arg_shapes,
};
use new_java2rust::coercion::Coercion;
use new_java2rust::constructor_plan::{
    ConstructorDelegationPlan, ConstructorFieldValuePlan, plan_constructor_delegation,
    plan_constructor_field_values,
};
use new_java2rust::conversion_lowering::{apply_cast_conversion, apply_instanceof};
use new_java2rust::conversion_plan::{
    InstanceOfCheck, LoweredType, plan_cast_conversion, plan_instanceof,
};
use new_java2rust::diagnostics::Severity;
use new_java2rust::effective_facts::EffectiveFacts;
use new_java2rust::exception_lowering::{
    exception_expr_from_new, exception_result_type, fallback_exception_expr,
    java_exception_support_item, new_exception_expr, return_stmt, throw_stmt,
};
use new_java2rust::exception_plan::{
    JavaExceptionSupportPlan, MethodExceptionPlan, ThrowExprPlan, exception_class_name,
    plan_java_exception_support, plan_method_exceptions, plan_program_exceptions, plan_throw_expr,
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
use new_java2rust::facts::{
    AccessorInlinePolicy, ClosedHierarchyFact, DispatchRequirementFact, DispatchRequirementKind,
    ExternalApiRole, FactProvenance, FactSet, FieldKey as FactFieldKey, FieldNullabilityFact,
    FieldNullabilityProvenance, HierarchyEdgeFact, HierarchyOverride, HierarchyStrategy,
    MethodEffectFact, MethodKey as FactMethodKey, NullabilityState, ObjectSafetyViolation,
    OwnedRecoveryCapability, ReceiverMutabilityFact, TrivialAccessorFact, TrivialAccessorKind,
    TypeCapabilityOverride,
};
use new_java2rust::item_assembly::{
    constructor_default_field_value, constructor_function_shell, constructor_return_body,
    delegating_constructor_return_body, impl_item_for_type, method_function_shell,
    method_function_shell_with_facts, method_function_shell_with_facts_and_exception_plan,
    raw_item, struct_item_for_type,
};
use new_java2rust::java_ir::{
    ConstructorInvocationKind, Expr, JavaType, Literal, Member, Modifier, PrimitiveType, Stmt,
    WildcardBound,
};
use new_java2rust::literal_lowering::literal_to_rust;
use new_java2rust::loop_lowering::{assemble_for_loop, continue_with_updates};
use new_java2rust::loop_plan::{ForBodyStmtPlan, plan_for_body_stmt};
use new_java2rust::lower::{lower_program_with_facts, lower_program_with_plans};
use new_java2rust::lowering_diagnostics::collect_lowering_diagnostic_plans;
use new_java2rust::operator_lowering::{binary_op_to_rust, lower_unary_expr, stream_unary_prefix};
use new_java2rust::parser::{parse_expr, parse_java_source};
use new_java2rust::representation::{
    ClosedEnumFactPolicy, HierarchyEdgeDiagnosticOutcome, ObjectRepresentation,
    ObjectRepresentationRequest, ObjectSafetyFactPolicy, ObjectSafetyPolicy,
    RepresentationDispatchPolicy, RepresentationHierarchyFactPolicy, RepresentationLookupFallback,
    RepresentationProvenance, TypeRepresentation, anonymous_class_diagnostic_policy,
    closed_enum_fact_policy, closed_enum_inference_policy, hierarchy_edge_diagnostic_policy,
    hierarchy_override_application_policy, hierarchy_override_validation_policy,
    object_hierarchy_context, object_representation_policy, object_safety_fact_policy,
    plan_representations, representation_dispatch_policy, representation_hierarchy_fact_policy,
};
use new_java2rust::representation_lowering::{
    RepresentationTypeEmission, TraitImplMethodCompatibilityError, TraitImplMethodFallbackPolicy,
    TraitImplMethodMessagePolicy, raw_type_item_for_with_facts, trait_impl_emissions_for_type,
    trait_impl_emissions_for_type_with_facts, trait_impl_method_message_policy, type_emission_for,
    type_emission_for_with_facts,
};
use new_java2rust::resolve::resolve_program;
use new_java2rust::rewrite::{
    AssignmentConditionNormalizedShape, AssignmentInConditionPlan, ControlFlowRewritePlan,
    MethodControlFlowPlan, NullTestKind, TailRecursiveLoopPlan, TailRecursiveNormalizedShape,
    control_flow_rewrite_diagnostics,
};
use new_java2rust::rust_ir::{RustExpr, RustItem, RustReceiver, RustStmt, RustType};
use new_java2rust::shape_projection::{
    DispatchProjectionPolicy, DispatchRequirementProjectionPolicy,
    DispatchRequirementProjectionRole, EscapingRoleDispatchPolicy, SequenceElementProjectionPolicy,
    ShapeProjectionCapabilityPolicy, borrowed_string_content_shape, dispatch_projection_policy,
    dispatch_requirement_projection_policy, escaping_role_dispatch_policy,
    indexed_assignment_shape, indexed_assignment_shape_with_effective_facts,
    indexed_assignment_shape_with_facts, indexed_element_projection_policy, indexed_read_shape,
    indexed_read_shape_with_effective_facts, indexed_read_shape_with_facts,
    iteration_element_projection_policy, iteration_element_shape, param_shape_for_java_type,
    project_java_value_shape, return_shape_for_java_type, sequence_element_projection_policy,
    shape_projection_capability_policy, shape_projection_capability_policy_with_effective_facts,
    storage_shape_for_java_type, value_shape_for_java_type,
};
use new_java2rust::signature::{
    BindingBoundaryPolicy, ConstructorKey, ConstructorParamBoundaryPolicy, FieldBoundaryPolicy,
    FieldKey, LocalPlan, MergedSignatureConstraints, MethodConstraintSet, MethodKey,
    MethodParamBoundaryPolicy, MethodReturnBoundaryPolicy, ReceiverConstraint, ReceiverPlan,
    SignatureBoundaryFallback, SignatureBoundaryKind, SignatureBoundaryKindMerge,
    SignatureBoundaryRepresentationFactPolicy, SignatureBoundaryRepresentationPolicy,
    SignatureConstraint, SignatureConstraintAxis, SignatureConstraintPropagationKind,
    SignatureConstraintPropagationPolicy, SignatureConstraintState, SignatureDiagnostic,
    SignatureDispatchRequirementPriorityPolicy, SignatureMemberApiBoundaryPolicy,
    SignatureNativeBoundaryPolicy, SignatureOwnedRecoveryCapabilityPolicy,
    SignatureOwnedRecoveryPolicy, SignatureOwnershipRequirementPriorityPolicy,
    SignatureRequirementMerge, SignatureShapeRequest, SignatureSlotBaselinePolicy,
    SignatureSlotRole, SignatureViewRequirementPriorityPolicy, apply_signature_constraints,
    binding_boundary_policy, capability_diagnostics_for_signature_constraints,
    collect_signature_constraints, conservative_boundary_constraints,
    conservative_boundary_constraints_with_effective_facts, constrained_receiver_plan,
    constructor_param_boundary_policy, constructor_param_shape_constraints, field_boundary_policy,
    field_nullability_shape_policy, field_shape_constraints, field_shape_constraints_with_facts,
    field_shape_constraints_with_usage, local_shape_constraints, merge_dispatch_requirement_policy,
    merge_ownership_requirement_policy, merge_signature_boundary_kind_policy,
    merge_storage_nullability_requirement_policy, merge_view_requirement_policy,
    method_param_boundary_policy, method_param_shape_constraints, method_return_boundary_policy,
    method_return_shape_constraints, receiver_constraints, receiver_constraints_with_facts,
    receiver_constraints_with_usage, signature_boundary_constraints,
    signature_boundary_constraints_with_effective_facts, signature_boundary_policy,
    signature_boundary_policy_with_effective_facts, signature_boundary_representation_fact_policy,
    signature_boundary_representation_fact_policy_with_effective_facts,
    signature_boundary_representation_policy, signature_capability_policy,
    signature_capability_policy_with_effective_facts,
    signature_constraints_for_borrowed_content_policy,
    signature_dispatch_requirement_priority_policy, signature_fixpoint_iteration_budget,
    signature_fixpoint_saturation_policy, signature_member_api_boundary_policy,
    signature_native_boundary_policy, signature_native_boundary_policy_with_effective_facts,
    signature_owned_recovery_capability_policy,
    signature_owned_recovery_capability_policy_with_effective_facts,
    signature_owned_recovery_policy, signature_owned_recovery_policy_from_capability,
    signature_owned_recovery_policy_with_effective_facts,
    signature_ownership_requirement_priority_policy, signature_shape_constraint_propagation_policy,
    signature_slot_baseline_policy, signature_slot_baseline_policy_with_effective_facts,
    signature_slot_constraint_propagation_policy, signature_slot_dispatch_requirement_policy,
    signature_slot_dispatch_requirement_policy_with_effective_facts,
    signature_view_requirement_priority_policy, solve_signature_constraints_to_fixpoint,
    solve_signature_constraints_to_fixpoint_result, solve_signature_shape,
    solve_signature_shape_with_facts, solve_signatures, solve_signatures_with_facts,
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
    NativeJavaRustTypePolicy, NativeNamedJavaRustTypePolicy, SlotContentCapabilityPolicy,
    SlotDefaultPolicy, SlotDefaultValuePolicy, SlotNoDefaultMessagePolicy, SlotRustTypePolicy,
    SlotValueRustTypePolicy, default_expr_for_slot, default_expr_for_slot_with_facts,
    native_java_rust_type_policy, native_java_rust_type_policy_with_effective_facts,
    native_named_java_rust_type_policy, rust_type_for_slot, rust_type_for_slot_with_facts,
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
use new_java2rust::stdlib::{is_borrowed_string_view_method, is_content_read_method};
use new_java2rust::stream::{build_stream_plan, stream_collect_actual};
use new_java2rust::stream_lowering::{
    StreamLambdaValue, emit_stream_lambda_body, render_stream_plan,
};
use new_java2rust::stream_shape::{
    StreamSourceCapabilityPolicy, infer_stream_lambda_body_shape, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape, infer_stream_source_element_shape_with_effective_facts,
    infer_stream_source_element_shape_with_facts, stream_source_capability_policy,
    stream_source_capability_policy_with_effective_facts,
};
use new_java2rust::switch_lowering::{SwitchRenderCase, render_switch, switch_case_condition};
use new_java2rust::switch_plan::{
    SWITCH_LABEL, SWITCH_MATCHED, SWITCH_VALUE, SwitchCaseLabelPlan, plan_switch, plan_switch_case,
    rewrite_switch_breaks,
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
    MethodApiBoundaryPolicy, analyze_usage, analyze_usage_with_facts,
    call_argument_boundary_policy, call_result_boundary_policy,
    constructor_invocation_boundary_policy, method_api_boundary_policy,
};
use new_java2rust::{emit, translate_source, translate_source_with_facts};

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
        OwnedRecoveryCapability::ConservativeClone
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

    let reachable = facts.reachable_hierarchy_edges_for_type(&sub_ty);
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

    let reachable = facts.reachable_hierarchy_edges_for_type(&a_ty);

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

interface HasConstant {
    int VERSION = 1;
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
    let has_constant_ty = JavaType::Named {
        name: "HasConstant".to_string(),
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
        !resolved
            .facts
            .type_capabilities_for(&combiner_ty)
            .object_safe_trait
    );
    assert!(
        !resolved
            .facts
            .type_capabilities_for(&has_constant_ty)
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
                && fact.object_safe_trait == Some(false))
    );
    assert!(
        resolved
            .facts
            .type_capability_overrides_for(&has_constant_ty)
            .any(|fact| fact.provenance == FactProvenance::ParsedSource
                && fact.object_safe_trait == Some(false))
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
    assert_eq!(
        combiner_safety.violations,
        vec![ObjectSafetyViolation::SelfReferentialParameter {
            method_name: "merge".to_string(),
            param_name: "other".to_string(),
        }]
    );
    let has_constant_safety = resolved
        .facts
        .object_safety_fact_for_name("HasConstant")
        .unwrap();
    assert_eq!(
        has_constant_safety.violations,
        vec![ObjectSafetyViolation::FieldMember {
            field_name: "v_e_r_s_i_o_n".to_string(),
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

interface HasConstant {
    int VERSION = 1;
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
    let has_constant_ty = JavaType::Named {
        name: "HasConstant".to_string(),
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
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&combiner_ty),
        DispatchShape::ErasedObject
    );
    assert_eq!(
        representations.representation_for(&has_constant_ty),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&has_constant_ty),
        DispatchShape::ErasedObject
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
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic.message.contains("interface Combiner")
                    && diagnostic
                        .message
                        .contains("method merge parameter other mentions the interface type")
            ),
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .any(
                |diagnostic| diagnostic.code == "non_object_safe_trait_fallback"
                    && diagnostic.message.contains("interface HasConstant")
                    && diagnostic.message.contains("interface field v_e_r_s_i_o_n")
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
    assert_eq!(
        object_model_source
            .matches("closed_hierarchy_variants_for")
            .count(),
        1
    );
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

    assert_eq!(
        representations.representation_for(&unknown),
        ObjectRepresentation::ErasedObject
    );
    assert_eq!(
        representations.dispatch_for(&unknown),
        DispatchShape::ErasedObject
    );

    let shape = value_shape_for_java_type(&representations, unknown);

    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
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
        Some(UnsupportedExpressionLowering::ClassLiteral)
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
    assert!(!lower_source.contains("UnsupportedExpressionLowering::ClassLiteral"));
    assert!(!lower_source.contains("UnsupportedMemberLowering::ParserUnsupported"));
    assert!(!lower_source.contains("Member::Unsupported"));
    assert!(!lower_source.contains("\"unsupported_member\""));
    assert!(!lower_source.contains("UnsupportedStatementLowering::TryCatchFinally"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::LocalTypeDeclaration"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ConstructorDelegation"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ParserUnsupported"));
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
    assert!(source.contains("raw_type_item_for_with_facts"));
    assert!(source.contains("trait_impl_emissions_for_type_with_facts"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&"));
    assert!(lower_source.contains("raw_type_item_for_with_facts"));
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
            && diagnostic.message.contains("direct subtype")
    }));
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && diagnostic.message.contains("unknown variant Missing")
    }));
    assert!(policy.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "representation_override_conflict"
            && diagnostic.message.contains("omits source subtype Num")
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
                && diagnostic.message.contains("omits source subtype Num")
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

    assert!(iterations >= 3, "{iterations}");
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
    let usage = analyze_usage(&resolved.program);
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

    assert!(iterations >= 3, "{iterations}");
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
    Delegation() {
        this(1);
    }
    Delegation(int value) {}
}

class Child extends Delegation {
    Child() {
        super();
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
    };
    assert!(matches!(
        plan_call_target(None, "make", Some(&resolved_static)),
        CallTargetPlan::StaticSelf { rust_name: "make" }
    ));

    let resolved_instance = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Helper".to_string(),
        rust_name: "read_name".to_string(),
        return_type: JavaType::String,
        kind: new_java2rust::java_ir::CallKind::Instance,
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
    };
    let constructor = new_java2rust::java_ir::ResolvedConstructor {
        declaring_type: "Uses".to_string(),
        rust_name: ctor.rust_name.clone().unwrap(),
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
    assert!(call_plan_source.contains("pub fn plan_constructor_arguments"));
    assert!(call_plan_source.contains("pub fn receiver_expr"));
    assert!(!lower_source.contains("resolved_call_arg_shapes"));
    assert!(!lower_source.contains("resolved_constructor_arg_shapes"));
    assert!(!lower_source.contains("lower_args_for_shapes"));
    assert!(!lower_source.contains("CallTargetPlan::InstanceReceiver"));
    assert!(!lower_source.contains("CallTargetPlan::FallbackReceiver"));
    assert!(lower_source.contains("lower_args_with_plan"));
    assert!(lower_source.contains(".receiver_expr()"));
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
    };
    let resolved_setter = new_java2rust::java_ir::ResolvedCall {
        declaring_type: "Bean".to_string(),
        rust_name: "set_value".to_string(),
        return_type: JavaType::Void,
        kind: new_java2rust::java_ir::CallKind::Instance,
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
        inline_policy: AccessorInlinePolicy::PreserveApi,
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
        inline_policy: AccessorInlinePolicy::PreserveApi,
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
    assert!(!source.contains("pub fn return_slot"));
    assert!(!source.contains("pub enum ReturnValuePlan"));
    assert!(!source.contains("pub fn return_value_plan"));
    assert!(lower_source.contains("return_value_plan"));
    assert!(!lower_source.contains("return_slot"));
    assert!(lower_source.contains("assignment_value_plan_for_shape"));
    assert!(lower_source.contains("assignment_value_plan_for_target"));
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
    assert!(!source.contains("pub enum DispatchCoercionPolicy"));
    assert!(!source.contains("pub fn dispatch_coercion_policy"));
    assert!(!source.contains("pub fn dispatch_coercion_policy_with_facts"));
    assert!(!source.contains("pub enum BorrowedDynToOwnedBoxedDynRecoveryPolicy"));
    assert!(source.contains("CloneBoxAdapterRequired"));
    assert!(source.contains("pub struct UnsupportedCoercionMessagePolicy"));
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
    assert!(!source.contains("pub enum InvalidCastPolicy"));
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
}

#[test]
fn coercion_plan_owns_instanceof_fallback_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");

    assert!(source.contains("pub struct CoercionDiagnostic"));
    assert!(conversion_source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(!source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(source.contains("ErasedObjectToDynTrait"));
    assert!(source.contains("ErasedObjectToBoxedDynTrait"));
    assert!(conversion_source.contains("pub enum InstanceOfCheck"));
    assert!(!source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("BoxedDynTraitType"));
    assert!(conversion_source.contains("policy: InstanceOfFallbackPolicy"));
    assert!(conversion_source.contains("pub fn plan_instanceof_check"));
    assert!(!source.contains("pub fn plan_instanceof_check"));
    assert!(source.contains("unsupported_instanceof_shape"));
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
    assert!(!source.contains("pub fn ownership_coercion_policy"));
    assert!(!source.contains("pub fn ownership_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub struct CoercionOwnedRecoveryCapabilityPolicy"));
    assert!(!source.contains("pub fn coercion_owned_recovery_capability_policy"));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedCopyValue"));
    assert!(source.contains("Coercion::CopyBorrowedValue"));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedWithClone"));
    assert!(conversion_source.contains("BorrowedToOwnedNonCloneable"));
    assert!(source.contains("ownership_coercion_fallback_message_policy"));
    assert!(conversion_source.contains("policy: CloneRecoveryPolicy"));
    assert!(conversion_source.contains("ProvenCloneCapability"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_for"));
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
fn external_facts_parser_ingests_closed_world_and_representation_overrides() {
    let parsed = parse_external_facts(
        r#"
# one closed-world fact and two hierarchy overrides
closed Expr
closed-enum Expr ExprValue Add Num
boxed-dyn-trait Sink SinkTrait
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
fn external_facts_parser_ingests_external_api_effect_overrides() {
    let parsed = parse_external_facts(
        r#"
external-api getValue * getter read-only
external-api setValue ExternalCounter setter mutates-receiver
"#,
    );

    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let getter = parsed.facts.external_api_effect_for("getValue").unwrap();
    assert_eq!(getter.method_name, "getValue");
    assert_eq!(getter.receiver_type, None);
    assert_eq!(getter.role, ExternalApiRole::Getter);
    assert_eq!(getter.receiver_mutability, ReceiverMutabilityFact::ReadOnly);
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
    assert_eq!(setter.provenance, FactProvenance::UserOverride);
}

#[test]
fn external_facts_parser_ingests_field_nullability_overrides() {
    let parsed = parse_external_facts(
        r#"
field-nullability User name definitely-non-null
field-nullability Cache.value maybe-null
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
    let parsed = parse_external_facts("closed Expr maybe\ncopy ExternalId maybe\nunknown Fact\n");

    assert!(parsed.diagnostics.has_code("external_fact_parse_error"));
    assert_eq!(parsed.diagnostics.len(), 3);
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
    let storage = SignatureConstraint::StorageSlot;
    let mutable = SignatureConstraint::MutableBinding;

    assert!(signature_shape_constraint_propagation_policy().allows(&ownership));
    assert!(!signature_shape_constraint_propagation_policy().allows(&storage));
    assert!(!signature_shape_constraint_propagation_policy().allows(&mutable));
    assert!(signature_slot_constraint_propagation_policy().allows(&ownership));
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
    assert!(source.contains("pub fn merge_ownership_requirement_policy"));
    assert!(source.contains("pub fn merge_view_requirement_policy"));
    assert!(source.contains("pub fn merge_dispatch_requirement_policy"));
    assert!(!source.contains("fn ownership_requirement_rank"));
    assert!(!source.contains("fn view_requirement_rank"));
    assert!(!source.contains("fn dispatch_requirement_rank"));
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
        call_result_boundary_policy(&parse_expr("localFetch()"), &internal_methods),
        Some(CallResultBoundaryPolicy {
            name: "localFetch".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        })
    );
    assert_eq!(
        call_result_boundary_policy(&parse_expr("local_fetch()"), &internal_methods),
        Some(CallResultBoundaryPolicy {
            name: "local_fetch".to_string(),
            boundary: CallArgumentBoundary::InternalSameType,
        })
    );
    assert_eq!(
        call_result_boundary_policy(&parse_expr("helper.fetch()"), &internal_methods),
        Some(CallResultBoundaryPolicy {
            name: "fetch".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        })
    );
    assert_eq!(
        call_result_boundary_policy(&parse_expr("new ExternalName()"), &internal_methods),
        Some(CallResultBoundaryPolicy {
            name: "new".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        })
    );
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
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "ExternalExpr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExternalExprValue".to_string(),
            variants: vec!["ExternalLiteral".to_string()],
        },
        provenance: FactProvenance::UserOverride,
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
            &new_java2rust::usage::UsageFacts::default()
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
        method_return_boundary_policy(unknown_ty, unknown_method, &usage),
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
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    assert!(!source.contains("fn member_is_api_boundary"));
    assert!(source.contains("pub struct SignatureBoundaryKindMerge"));
    assert!(source.contains("pub fn merge_signature_boundary_kind_policy"));
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
    assert!(object_model_source.contains("closed_hierarchy_variants_for"));
    assert!(!representation_source.contains("type_capabilities_for"));
    assert!(!representation_source.contains("type_capability_overrides"));
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
    assert!(effective_facts_source.contains("FactProvenance::UserOverride => 3"));
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
        1,
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
    assert!(source.contains("call_result_boundary_policy(initializer, internal_methods)"));
    assert!(source.contains("call_result_boundary_policy(expr, internal_methods)"));
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
            OwnershipShape::SharedBorrow,
            &ViewShape::Slice,
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
        ThrowExprPlan::FallbackValue(&Expr::Var("err".to_string()))
    );
    assert_eq!(
        exception_result_type(RustType::I32),
        RustType::Raw("Result<i32, JavaException>".to_string())
    );
    assert!(matches!(
        java_exception_support_item(),
        new_java2rust::rust_ir::RustItem::Raw(item) if item.contains("pub struct JavaException")
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
                RustExpr::Var("message".to_string()),
            ],
        }
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
    assert!(!lower_source.contains("AssignmentInConditionPlan"));
    assert!(!lower_source.contains("TailRecursiveLoopPlan"));
    assert!(!lower_source.contains("LoopWithParameterReassignment"));
}

#[test]
fn lowering_consumes_control_flow_plans_only_as_pending_diagnostics() {
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

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, "control_flow_rewrite_pending");
    assert_eq!(
        diagnostics[0].message,
        "Parser::next requires assignment-in-condition normalization before idiomatic lowering"
    );
    assert_eq!(diagnostics[1].code, "control_flow_rewrite_pending");
    assert_eq!(
        diagnostics[1].message,
        "Parser::next requires tail-recursive loop normalization before idiomatic lowering"
    );
    assert_eq!(
        lowering_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "control_flow_rewrite_pending")
            .map(|diagnostic| diagnostic.severity)
            .collect::<Vec<_>>(),
        vec![Severity::Unsupported, Severity::Unsupported]
    );

    assert!(lower_source.contains("lower_program_with_semantic_plans"));
    assert!(lower_source.contains("collect_lowering_diagnostic_plans"));
    assert!(!lower_source.contains("push_control_flow_plan_diagnostics"));
    assert!(!lower_source.contains("control_flow_rewrite_diagnostics"));
    assert!(lowering_diagnostics_source.contains("pub struct LoweringDiagnosticPlan"));
    assert!(lowering_diagnostics_source.contains("pub fn collect_lowering_diagnostic_plans"));
    assert!(lowering_diagnostics_source.contains(".diagnostics()"));
    assert!(lowering_diagnostics_source.contains("Severity::Warning"));
    assert!(lowering_diagnostics_source.contains("Severity::Unsupported"));
    assert!(lowering_diagnostics_source.contains("diagnostics_for_program(program)"));
    assert!(lowering_diagnostics_source.contains("signatures"));
    assert!(lowering_diagnostics_source.contains("control_flow_rewrite_diagnostics"));
    assert!(rewrite_source.contains("pub struct ControlFlowRewriteDiagnostic"));
    assert!(rewrite_source.contains("pub fn control_flow_rewrite_diagnostics"));
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
}

#[test]
fn shape_projection_owns_borrowed_string_content_shape() {
    let coercion_source = include_str!("../src/coercion.rs");
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");

    assert!(shape_projection_source.contains("pub fn borrowed_string_content_shape"));
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
    assert!(!stream_shape_source.contains("name == \"trim\""));
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

    let plan_source = include_str!("../src/loop_plan.rs");
    let lowering_source = include_str!("../src/loop_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");
    assert!(plan_source.contains("pub enum ForBodyStmtPlan"));
    assert!(plan_source.contains("pub fn plan_for_body_stmt"));
    assert!(!plan_source.contains("RustStmt"));
    assert!(!plan_source.contains("RustExpr"));
    assert!(lowering_source.contains("continue_with_updates"));
    assert!(lower_source.contains("plan_for_body_stmt"));
    assert!(lower_source.contains("ForBodyStmtPlan::ContinueWithUpdates"));
    assert!(!lower_source.contains("Stmt::Continue(None)"));
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
fn array_plan_and_lowering_own_java_array_fallbacks_before_orchestrator() {
    let creation_expr = parse_expr("new int[rows][cols]");
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

    let mut type_env = TypeEnv::new();
    type_env.insert_local("values", JavaType::Array(Box::new(JavaType::String)));
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

    let access_lowering_source = include_str!("../src/access_lowering.rs");
    let member_plan_source = include_str!("../src/member_access_plan.rs");
    assert!(member_plan_source.contains("pub struct JavaMemberAccessPolicy"));
    assert!(member_plan_source.contains("pub enum JavaMemberAccessKind"));
    assert!(member_plan_source.contains("pub fn java_member_access_policy"));
    assert!(member_plan_source.contains("MemberAccessOp::LenAsI32"));
    assert!(!access_lowering_source.contains("\"length\""));
    assert!(!access_lowering_source.contains("infer_member_access_type"));
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
