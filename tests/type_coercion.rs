use new_java2rust::coercion::{
    BorrowedStringViewProjection, Coercion, CoercionDiagnostic, IndexToUsizeProjection,
    JavaDisplayPlan, LiteralSlotPlan, LoweredExpr, RustPrimitiveTypeNamePolicy,
    UnsupportedCoercionMessagePolicy, apply_borrowed_string_view_projection, apply_coercions,
    apply_expr_to_slot, apply_expr_to_slot_with_facts, apply_instanceof_check,
    apply_java_display_plan, apply_literal_slot_plan, apply_lowered_expr_to_slot,
    apply_lowered_expr_to_slot_with_facts, apply_shape_coercion_plan, apply_slot_conversion_plan,
    apply_syntax_coercion_plan, borrowed_element_recovery_fallback_message_policy,
    borrowed_string_view_projection, dispatch_coercion_fallback_message_policy,
    instanceof_fallback_message_policy, java_integral_literal_char, literal_slot_plan,
    normalize_map_key_borrow_projection, ownership_coercion_fallback_message_policy, plan_coercion,
    plan_coercion_with_facts, plan_shape_coercion, plan_shape_coercion_with_facts,
    render_index_to_usize_projection, render_instanceof_check,
    rust_expr_is_constructed_optional_value, rust_primitive_type_name_policy,
};
use new_java2rust::conversion_lowering::{
    adapt_optional_assignment_value_for_target, apply_lowered_slot_expr_conversion,
    apply_slot_expr_conversion,
};
use new_java2rust::conversion_plan::{
    AssignmentValuePlan, BorrowedDynToOwnedBoxedDynRecoveryPolicy, BorrowedElementAdapterPolicy,
    BorrowedElementCarrier, BorrowedElementRecoveryFallbackPolicy, BorrowedElementRecoveryPolicy,
    BorrowedElementSlotAdapterPolicy, CloneRecoveryPolicy, CoercionContentPolicy,
    CoercionOwnedRecoveryCapabilityPolicy, CoercionShapeContentPolicy, ConversionDiagnostic,
    DispatchCoercionFallbackPolicy, DispatchCoercionPolicy, DynCloneBoxAdapterAvailability,
    DynCloneBoxAdapterDeferralReason, DynCloneBoxAdapterGenerationPolicy,
    DynCloneBoxAdapterRequirement, InstanceOfCheck, InstanceOfFallbackPolicy,
    InstanceOfTargetPolicy, InvalidCastPolicy, JavaCastSemanticsPolicy, JavaTypeConversionKind,
    LoweredSlotExprConversionPlan, LoweredType, OptionalElementTargetMatchPolicy,
    OptionalPresentConstructionSource, OptionalShapeCoercionPolicy,
    OwnershipCoercionFallbackPolicy, OwnershipCoercionPolicy, PrimitiveRankPolicy,
    PrimitiveWideningPolicy, ReturnValuePlan, RuntimeTypeTargetPolicy, RustNominalTypeNamePolicy,
    ShapeCoercionPlan, Slot, SlotConversionPlan, SlotExprConversionPlan, SyntaxCoercionPlan,
    SyntaxSlotCoercionPolicy, VecElementTargetMatchPolicy, ViewCoercionPolicy,
    actual_shape_for_slot_expr, argument_slot, assignment_slot, assignment_slot_for_target,
    assignment_value_plan_for_shape, assignment_value_plan_for_target,
    borrowed_element_adapter_policy, borrowed_element_recovery_policy,
    borrowed_element_recovery_policy_with_effective_facts, borrowed_element_slot_adapter_policy,
    coercion_content_policy, coercion_content_policy_with_effective_facts,
    coercion_owned_recovery_capability_policy,
    coercion_owned_recovery_capability_policy_with_effective_facts, coercion_shape_content_policy,
    coercion_shape_content_policy_with_effective_facts, condition_slot, dispatch_coercion_policy,
    dispatch_coercion_policy_with_facts, dyn_clone_box_adapter_generation_policy,
    dyn_clone_box_adapter_generation_policy_with_availability, dyn_clone_box_adapter_requirement,
    dyn_clone_box_adapter_requirement_with_availability, erased_object_runtime_contract,
    hierarchy_instanceof_policy, index_slot, instanceof_target_policy, java_cast_semantics_policy,
    java_cast_type_conversion_plan, java_type_conversion_plan_from_evidence,
    lowered_slot_conversion_plan, lowered_slot_conversion_plan_with_facts,
    optional_element_target_match_policy, optional_present_construction_plan_for_actual,
    optional_shape_coercion_policy, optional_shape_coercion_policy_with_facts,
    optional_slot_construction_plan, optional_slot_construction_plan_for_actual,
    ownership_coercion_policy, ownership_coercion_policy_with_effective_facts,
    plan_instanceof_check, plan_instanceof_check_with_facts, plan_lowered_slot_conversion,
    plan_slot_conversion, preconverted_optional_slot_construction_plan, primitive_rank_policy,
    primitive_widening_policy, return_slot, return_value_plan, runtime_type_target_policy,
    rust_nominal_type_name_policy, shape_coercion_plan, shape_coercion_plan_with_facts,
    slot_conversion_plan, slot_conversion_plan_with_facts, syntax_coercion_plan,
    syntax_coercion_plan_with_facts, syntax_slot_coercion_policy, vec_element_target_match_policy,
    view_coercion_policy, view_coercion_policy_with_effective_facts,
};
use new_java2rust::diagnostics::Severity;
use new_java2rust::effective_facts::{
    CastTypeCapabilityPolicy, EffectiveFacts, InstanceOfTargetCapabilityPolicy,
};
use new_java2rust::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape};
use new_java2rust::external_packs::{hdf5_pack_facts, parse_external_pack_manifest};
use new_java2rust::facts::{
    ExternalConversionAdapterFact, ExternalConversionAdapterKind, FactProvenance, FactSet,
    OwnedRecoveryCapability, OwnedRecoveryCapabilityFact, TypeCapabilityOverride,
};
use new_java2rust::java_ir::{
    JavaCoercionEvidence, JavaCoercionKind, JavaType, Literal, PrimitiveType,
};
use new_java2rust::lowering_diagnostics::lowering_diagnostic_from_conversion_diagnostic;
use new_java2rust::member_access_plan::{
    JavaMemberAccessKind, JavaMemberAccessPolicy, MemberAccessOp, java_member_access_policy,
    plan_member_access,
};
use new_java2rust::parser::{parse_expr, parse_type};
use new_java2rust::rust_ir::RustExpr;
use new_java2rust::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use new_java2rust::string_concat::is_string_concat;
use new_java2rust::string_concat_plan::build_string_concat_plan;
use new_java2rust::string_concat_render::render_string_concat;
use new_java2rust::types::{
    TypeEnv, infer_expr_type, list_java_type, optional_java_type, set_java_type,
};
use new_java2rust::unsupported;

fn simple_interval_locatable_facts() -> FactSet {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "SimpleInterval".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![JavaType::Named {
                name: "Locatable".to_string(),
                args: Vec::new(),
            }],
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    facts
}

#[test]
fn rust_get_projection_counts_as_existing_optional_carrier() {
    assert!(rust_expr_is_constructed_optional_value(&RustExpr::Raw(
        "fields.get(&\"ID\".to_string()).cloned()".to_string()
    )));
    assert!(rust_expr_is_constructed_optional_value(&RustExpr::Raw(
        "values.get(index).copied()".to_string()
    )));
    assert!(rust_expr_is_constructed_optional_value(
        &RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("fields".to_string())),
                method: "get".to_string(),
                args: vec![RustExpr::Literal("\"ID\"".to_string())],
            }),
            method: "cloned".to_string(),
            args: Vec::new(),
        }
    ));
}

#[test]
fn runtime_type_target_policy_names_nominal_runtime_targets() {
    let named = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };

    assert_eq!(
        rust_nominal_type_name_policy(&named),
        RustNominalTypeNamePolicy {
            java_ty: named.clone(),
            target_type: Some("Dog".to_string()),
        }
    );
    assert_eq!(
        runtime_type_target_policy(&named),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "Dog".to_string(),
        }
    );
    let nested = JavaType::Named {
        name: "VCFCompoundHeaderLine.SupportedHeaderLineType".to_string(),
        args: Vec::new(),
    };
    assert_eq!(
        runtime_type_target_policy(&nested),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "VCFCompoundHeaderLineSupportedHeaderLineType".to_string(),
        }
    );
    assert_eq!(
        runtime_type_target_policy(&JavaType::String),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "String".to_string(),
        }
    );
    let boxed_boolean = JavaType::Named {
        name: "java.lang.Boolean".to_string(),
        args: Vec::new(),
    };
    assert_eq!(
        rust_nominal_type_name_policy(&boxed_boolean),
        RustNominalTypeNamePolicy {
            java_ty: boxed_boolean.clone(),
            target_type: Some("bool".to_string()),
        }
    );
    assert_eq!(
        runtime_type_target_policy(&boxed_boolean),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "bool".to_string(),
        }
    );
    assert_eq!(
        rust_nominal_type_name_policy(&JavaType::Primitive(PrimitiveType::Int)),
        RustNominalTypeNamePolicy {
            java_ty: JavaType::Primitive(PrimitiveType::Int),
            target_type: None,
        }
    );
    assert_eq!(
        runtime_type_target_policy(&JavaType::Primitive(PrimitiveType::Int)),
        RuntimeTypeTargetPolicy::MissingNominalTarget {
            java_ty: JavaType::Primitive(PrimitiveType::Int),
        }
    );
}

#[test]
fn primitive_widening_policy_names_numeric_widening_rank() {
    assert_eq!(
        primitive_rank_policy(PrimitiveType::Int),
        PrimitiveRankPolicy {
            primitive: PrimitiveType::Int,
            rank: 3,
        }
    );
    assert_eq!(
        rust_primitive_type_name_policy(PrimitiveType::Long),
        RustPrimitiveTypeNamePolicy {
            primitive: PrimitiveType::Long,
            rust_name: "i64",
        }
    );
    assert_eq!(
        primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Int),
            &JavaType::Primitive(PrimitiveType::Long),
        ),
        Some(PrimitiveWideningPolicy {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
            from_rank: 3,
            to_rank: 4,
            widens: true,
        })
    );
    assert_eq!(
        primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Long),
            &JavaType::Primitive(PrimitiveType::Int),
        )
        .unwrap()
        .widens,
        false
    );
    assert_eq!(
        primitive_widening_policy(&JavaType::String, &JavaType::Primitive(PrimitiveType::Int)),
        None
    );
}

#[test]
fn cast_semantics_policy_names_primitive_and_impossible_casts() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let long_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    let boolean_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Boolean));
    let string_shape = RustSlotShape::conservative(JavaType::String);

    let numeric = shape_coercion_plan(&int_shape, &long_shape);
    assert_eq!(
        java_cast_semantics_policy(&int_shape, &long_shape, &numeric),
        JavaCastSemanticsPolicy::PrimitiveNumericCast {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
            widening: primitive_widening_policy(&int_shape.java_ty, &long_shape.java_ty),
        }
    );

    let boolean_to_int = shape_coercion_plan(&boolean_shape, &int_shape);
    assert_eq!(
        java_cast_semantics_policy(&boolean_shape, &int_shape, &boolean_to_int),
        JavaCastSemanticsPolicy::ImpossibleCast {
            actual: JavaType::Primitive(PrimitiveType::Boolean),
            target: JavaType::Primitive(PrimitiveType::Int),
        }
    );

    let primitive_to_reference = shape_coercion_plan(&int_shape, &string_shape);
    assert_eq!(
        java_cast_semantics_policy(&int_shape, &string_shape, &primitive_to_reference),
        JavaCastSemanticsPolicy::ImpossibleCast {
            actual: JavaType::Primitive(PrimitiveType::Int),
            target: JavaType::String,
        }
    );
}

#[test]
fn cast_semantics_policy_names_reference_upcasts_checked_downcasts_and_fallbacks() {
    let dog_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut animal_enum_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    animal_enum_shape.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let upcast = shape_coercion_plan(&dog_shape, &animal_enum_shape);
    assert_eq!(
        java_cast_semantics_policy(&dog_shape, &animal_enum_shape, &upcast),
        JavaCastSemanticsPolicy::ReferenceUpcast
    );

    let downcast = shape_coercion_plan(&animal_enum_shape, &dog_shape);
    assert_eq!(
        java_cast_semantics_policy(&animal_enum_shape, &dog_shape, &downcast),
        JavaCastSemanticsPolicy::CheckedReferenceDowncast {
            invalid_cast: InvalidCastPolicy::Panic,
        }
    );

    let mut dyn_shape = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_shape.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let fallback = shape_coercion_plan(&animal_enum_shape, &dyn_shape);
    assert_eq!(
        java_cast_semantics_policy(&animal_enum_shape, &dyn_shape, &fallback),
        JavaCastSemanticsPolicy::DiagnosticOnlyFallback {
            diagnostics: fallback.unsupported_diagnostics(),
        }
    );
}

#[test]
fn cast_semantics_policy_names_generic_raw_wildcard_and_optional_result_casts() {
    let list_string = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::String],
    });
    let list_integer = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Primitive(PrimitiveType::Int)],
    });
    let generic_cast = shape_coercion_plan(&list_string, &list_integer);
    assert_eq!(
        java_cast_semantics_policy(&list_string, &list_integer, &generic_cast),
        JavaCastSemanticsPolicy::UncheckedGenericCast {
            actual: list_string.java_ty.clone(),
            target: list_integer.java_ty.clone(),
        }
    );

    let raw_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: Vec::new(),
    });
    let raw_cast = shape_coercion_plan(&raw_list, &list_string);
    assert_eq!(
        java_cast_semantics_policy(&raw_list, &list_string, &raw_cast),
        JavaCastSemanticsPolicy::RawOrWildcardCast {
            actual: raw_list.java_ty.clone(),
            target: list_string.java_ty.clone(),
        }
    );

    let wildcard_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Wildcard(None)],
    });
    let wildcard_cast = shape_coercion_plan(&wildcard_list, &list_string);
    assert_eq!(
        java_cast_semantics_policy(&wildcard_list, &list_string, &wildcard_cast),
        JavaCastSemanticsPolicy::RawOrWildcardCast {
            actual: wildcard_list.java_ty.clone(),
            target: list_string.java_ty.clone(),
        }
    );

    let optional_string = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let result_string = RustSlotShape::conservative(JavaType::Named {
        name: "Result".to_string(),
        args: vec![JavaType::String],
    });
    let optional_result_cast = shape_coercion_plan(&optional_string, &result_string);
    assert_eq!(
        java_cast_semantics_policy(&optional_string, &result_string, &optional_result_cast),
        JavaCastSemanticsPolicy::OptionalResultCast {
            actual: optional_string.java_ty.clone(),
            target: result_string.java_ty.clone(),
        }
    );
}

#[test]
fn conversion_plan_classifies_resolver_coercion_evidence_and_casts() {
    let widening = JavaCoercionEvidence {
        arg_index: 0,
        actual: JavaType::Primitive(PrimitiveType::Int),
        target: JavaType::Primitive(PrimitiveType::Long),
        kind: JavaCoercionKind::PrimitiveWidening,
    };
    assert_eq!(
        java_type_conversion_plan_from_evidence(&widening).kind,
        JavaTypeConversionKind::PrimitiveWidening {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
        }
    );

    let boxing = JavaCoercionEvidence {
        arg_index: 1,
        actual: JavaType::Primitive(PrimitiveType::Int),
        target: JavaType::Named {
            name: "Integer".to_string(),
            args: Vec::new(),
        },
        kind: JavaCoercionKind::Boxing,
    };
    assert_eq!(
        java_type_conversion_plan_from_evidence(&boxing).kind,
        JavaTypeConversionKind::Boxing
    );

    let unboxing = JavaCoercionEvidence {
        arg_index: 2,
        actual: JavaType::Named {
            name: "Integer".to_string(),
            args: Vec::new(),
        },
        target: JavaType::Primitive(PrimitiveType::Int),
        kind: JavaCoercionKind::Unboxing,
    };
    assert_eq!(
        java_type_conversion_plan_from_evidence(&unboxing).kind,
        JavaTypeConversionKind::Unboxing
    );

    let lambda = JavaCoercionEvidence {
        arg_index: 3,
        actual: JavaType::Unknown("lambda".to_string()),
        target: JavaType::Named {
            name: "Predicate".to_string(),
            args: Vec::new(),
        },
        kind: JavaCoercionKind::LambdaAdaptation,
    };
    assert_eq!(
        java_type_conversion_plan_from_evidence(&lambda).kind,
        JavaTypeConversionKind::LambdaAdaptation
    );

    let actual_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::String],
    });
    let target_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Named {
            name: "Integer".to_string(),
            args: Vec::new(),
        }],
    });
    assert!(matches!(
        java_cast_type_conversion_plan(&actual_list, &target_list, &FactSet::default()).kind,
        JavaTypeConversionKind::Cast(JavaCastSemanticsPolicy::UncheckedGenericCast { .. })
    ));
}

#[test]
fn cast_and_instanceof_type_capability_policy_centralizes_nonprimitive_type_checks() {
    let list_string = JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::String],
    };
    let list_int = JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Primitive(PrimitiveType::Int)],
    };
    let raw_list = JavaType::Named {
        name: "List".to_string(),
        args: Vec::new(),
    };
    let optional_string = optional_java_type(JavaType::String);
    let result_string = JavaType::Named {
        name: "Result".to_string(),
        args: vec![JavaType::String],
    };
    let facts = FactSet::default();
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(
        effective_facts.cast_type_capability_policy(&list_string, &list_int),
        CastTypeCapabilityPolicy {
            actual: list_string.clone(),
            target: list_int.clone(),
            optional_or_result: false,
            raw_or_wildcard: false,
            unchecked_generic: true,
        }
    );
    assert_eq!(
        facts.cast_type_capability_policy(&raw_list, &list_string),
        CastTypeCapabilityPolicy {
            actual: raw_list.clone(),
            target: list_string.clone(),
            optional_or_result: false,
            raw_or_wildcard: true,
            unchecked_generic: false,
        }
    );
    assert_eq!(
        facts.cast_type_capability_policy(&optional_string, &result_string),
        effective_facts.cast_type_capability_policy(&optional_string, &result_string)
    );
    assert_eq!(
        facts.cast_type_capability_policy(&optional_string, &result_string),
        CastTypeCapabilityPolicy {
            actual: optional_string,
            target: result_string,
            optional_or_result: true,
            raw_or_wildcard: false,
            unchecked_generic: false,
        }
    );

    assert_eq!(
        facts.instanceof_target_capability_policy(&raw_list),
        InstanceOfTargetCapabilityPolicy {
            target: raw_list.clone(),
            primitive_target: false,
            raw_or_wildcard_target: true,
        }
    );
    assert_eq!(
        facts.instanceof_target_capability_policy(&JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Wildcard(None)],
        }),
        InstanceOfTargetCapabilityPolicy {
            target: JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Wildcard(None)],
            },
            primitive_target: false,
            raw_or_wildcard_target: true,
        }
    );
}

#[test]
fn type_inference_owns_binary_numeric_promotion() {
    let mut env = TypeEnv::new();
    env.insert_local("b", JavaType::Primitive(PrimitiveType::Byte));
    env.insert_local("s", JavaType::Primitive(PrimitiveType::Short));
    env.insert_local("i", JavaType::Primitive(PrimitiveType::Int));

    let ty = infer_expr_type(&parse_expr("b + s + i"), &env);
    assert_eq!(ty, JavaType::Primitive(PrimitiveType::Int));
}

#[test]
fn type_inference_owns_complex_java_numeric_coercion_chain() {
    let mut env = TypeEnv::new();
    env.insert_local("b", JavaType::Primitive(PrimitiveType::Byte));
    env.insert_local("s", JavaType::Primitive(PrimitiveType::Short));
    env.insert_local("c", JavaType::Primitive(PrimitiveType::Char));
    env.insert_local("i", JavaType::Primitive(PrimitiveType::Int));
    env.insert_local("l", JavaType::Primitive(PrimitiveType::Long));
    env.insert_local("f", JavaType::Primitive(PrimitiveType::Float));
    env.insert_local("d", JavaType::Primitive(PrimitiveType::Double));

    let promoted = parse_expr("((((b + s) * (i - c)) + (l / 3L)) / (f + 2.0f)) + d");
    assert_eq!(
        infer_expr_type(&promoted, &env),
        JavaType::Primitive(PrimitiveType::Double)
    );

    let narrowed = parse_expr("(byte) ((((b + s) * (i - c)) + (l / 3L)) / (f + 2.0f))");
    assert_eq!(
        infer_expr_type(&narrowed, &env),
        JavaType::Primitive(PrimitiveType::Byte)
    );
}

#[test]
fn type_inference_owns_java_length_type() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
    );

    let ty = infer_expr_type(&parse_expr("values.length"), &env);
    assert_eq!(ty, JavaType::Primitive(PrimitiveType::Int));
    let expr = parse_expr("values");
    let policy = java_member_access_policy(&expr, "length", &env).unwrap();
    let plan = plan_member_access(&expr, "length", &env).unwrap();

    assert_eq!(
        policy,
        JavaMemberAccessPolicy {
            member: "length".to_string(),
            java_ty: JavaType::Primitive(PrimitiveType::Int),
            kind: JavaMemberAccessKind::ArrayLength,
        }
    );
    assert_eq!(plan.op, MemberAccessOp::LenAsI32);
    assert_eq!(plan.shape.java_ty, JavaType::Primitive(PrimitiveType::Int));
}

#[test]
fn type_inference_owns_optional_array_access_element_type() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "values",
        optional_java_type(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double,
        )))),
    );

    assert_eq!(
        infer_expr_type(&parse_expr("values[0]"), &env),
        JavaType::Primitive(PrimitiveType::Double)
    );
    assert_eq!(
        infer_expr_type(&parse_expr("values[0] * 1073741824L"), &env),
        JavaType::Primitive(PrimitiveType::Double)
    );
}

#[test]
fn member_access_plan_snake_cases_proved_instance_fields() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "callback",
        JavaType::Named {
            name: "BatchCompletionCallbackFunctionArgument".to_string(),
            args: Vec::new(),
        },
    );

    let receiver = parse_expr("callback");
    let policy = java_member_access_policy(&receiver, "batchCount", &env).unwrap();
    let plan = plan_member_access(&receiver, "batchCount", &env).unwrap();

    assert_eq!(
        policy,
        JavaMemberAccessPolicy {
            member: "batchCount".to_string(),
            java_ty: JavaType::Primitive(PrimitiveType::Int),
            kind: JavaMemberAccessKind::InstanceField,
        }
    );
    assert_eq!(
        plan.op,
        MemberAccessOp::RustField {
            rust_name: "batch_count".to_string(),
        }
    );
    assert_eq!(plan.shape.java_ty, JavaType::Primitive(PrimitiveType::Int));
}

#[test]
fn type_inference_owns_known_external_static_string_constants() {
    let env = TypeEnv::new();

    assert_eq!(
        infer_expr_type(
            &parse_expr("GATKVCFConstants.RAW_MAPPING_QUALITY_WITH_DEPTH_KEY"),
            &env
        ),
        JavaType::String
    );
    assert_eq!(
        infer_expr_type(&parse_expr("GATKVCFConstants.TREE_SCORE"), &env),
        JavaType::String
    );

    let receiver = parse_expr("GATKVCFConstants");
    let policy =
        java_member_access_policy(&receiver, "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY", &env).unwrap();
    let plan = plan_member_access(&receiver, "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY", &env).unwrap();

    assert_eq!(
        policy,
        JavaMemberAccessPolicy {
            member: "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY".to_string(),
            java_ty: JavaType::String,
            kind: JavaMemberAccessKind::StaticConstant,
        }
    );
    assert_eq!(
        plan.op,
        MemberAccessOp::StaticConstant {
            owner: "GATKVCFConstants".to_string()
        }
    );
    assert_eq!(plan.shape.java_ty, JavaType::String);
    assert_eq!(plan.shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(plan.shape.view, ViewShape::Str);
}

#[test]
fn expr_shape_projects_known_external_static_string_constants() {
    let env = ShapeEnv::new();

    let shape = infer_expr_shape(
        &parse_expr("GATKVCFConstants.RAW_MAPPING_QUALITY_WITH_DEPTH_KEY"),
        &env,
    );

    assert_eq!(shape.java_ty, JavaType::String);
}

#[test]
fn type_inference_consumes_external_pack_return_type_facts() {
    let mut env = TypeEnv::new();
    env.insert_local(
        "hdf5",
        JavaType::Named {
            name: "HDF5File".to_string(),
            args: Vec::new(),
        },
    );
    env.insert_local(
        "variant",
        JavaType::Named {
            name: "VariantContext".to_string(),
            args: Vec::new(),
        },
    );

    assert_eq!(
        infer_expr_type(&parse_expr(r#"hdf5.readDoubleArray("/metrics")"#), &env),
        JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Double)))
    );
    assert_eq!(
        infer_expr_type(&parse_expr("variant.getAlleles()"), &env),
        JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Allele".to_string(),
                args: Vec::new(),
            }],
        }
    );
}

#[test]
fn member_access_plan_uses_type_inference_and_shape_projection() {
    let source = include_str!("../src/member_access_plan.rs");

    assert!(source.contains("infer_member_access_type"));
    assert!(source.contains("pub struct JavaMemberAccessPolicy"));
    assert!(source.contains("pub enum JavaMemberAccessKind"));
    assert!(source.contains("pub fn java_member_access_policy"));
    assert!(source.contains("let policy = java_member_access_policy"));
    assert!(source.contains("project_java_value_shape"));
    assert!(!source.contains("infer_expr_type"));
    assert!(!source.contains("PrimitiveType"));
    assert!(!source.contains("JavaType::Primitive"));
    assert!(!source.contains("RustSlotShape::conservative"));
    assert!(!source.contains("is_length_receiver"));
}

#[test]
fn type_inference_identifies_string_concat() {
    let mut env = TypeEnv::new();
    env.insert_local("name", JavaType::String);
    env.insert_local("count", JavaType::Primitive(PrimitiveType::Int));

    assert!(is_string_concat(
        &parse_expr("\"name=\" + name + count"),
        &env
    ));
    let expr = parse_expr("\"name=\" + name + count");
    let plan = build_string_concat_plan(&expr, &env).unwrap();

    assert_eq!(plan.format_string, "name={}{}");
    assert_eq!(plan.args.len(), 2);
    assert_eq!(
        render_string_concat(
            &plan,
            vec![
                RustExpr::Var("name".to_string()),
                RustExpr::Var("count".to_string()),
            ],
        ),
        RustExpr::Raw("format!(\"name={}{}\", name, count)".to_string())
    );
}

#[test]
fn string_concat_semantics_do_not_render_rust() {
    let semantic_source = include_str!("../src/string_concat.rs");
    let plan_source = include_str!("../src/string_concat_plan.rs");
    let render_source = include_str!("../src/string_concat_render.rs");

    assert!(!semantic_source.contains("rust_ir"));
    assert!(!semantic_source.contains("emit::"));
    assert!(!semantic_source.contains("format!("));
    assert!(!plan_source.contains("rust_ir"));
    assert!(!plan_source.contains("emit::"));
    assert!(!plan_source.contains("format!("));
    assert!(!render_source.contains("analyze_string_concat"));
    assert!(!render_source.contains("build_string_concat_plan"));
    assert!(render_source.contains("format!("));
    assert!(render_source.contains("render_java_display_operand"));
    assert!(!render_source.contains("strip_arithmetic_display_conversion"));
}

#[test]
fn coercion_planner_owns_index_and_shape_widening() {
    assert_eq!(
        plan_coercion(&JavaType::Primitive(PrimitiveType::Int), &Slot::Index),
        vec![Coercion::IndexToUsize]
    );

    let actual = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::WidenPrimitive {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long
        }]
    );
}

#[test]
fn borrowed_native_string_can_coerce_to_string_view() {
    let mut actual = RustSlotShape::conservative(JavaType::String);
    actual.ownership = OwnershipShape::SharedBorrow;
    actual.view = ViewShape::Native;
    let mut target = RustSlotShape::conservative(JavaType::String);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Str;

    let plan = shape_coercion_plan(&actual, &target);

    assert_eq!(plan.coercions(), vec![Coercion::AsStr]);
    assert_eq!(
        apply_shape_coercion_plan(
            RustExpr::Raw("parts[0]".to_string()),
            &shape_coercion_plan_with_facts(&actual, &target, &FactSet::default())
        ),
        RustExpr::Raw("parts[0].as_str()".to_string())
    );
}

#[test]
fn map_key_borrow_projection_is_centralized_and_idempotent() {
    let owned = RustSlotShape::conservative(JavaType::String);
    assert_eq!(
        normalize_map_key_borrow_projection(
            RustExpr::Var("key".to_string()),
            Some("key"),
            Some(&owned)
        ),
        RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::Var("key".to_string())),
        }
    );

    let mut shared = RustSlotShape::conservative(JavaType::String);
    shared.ownership = OwnershipShape::SharedBorrow;
    assert_eq!(
        normalize_map_key_borrow_projection(
            RustExpr::Raw("&key".to_string()),
            Some("key"),
            Some(&shared)
        ),
        RustExpr::Var("key".to_string())
    );

    assert_eq!(
        normalize_map_key_borrow_projection(
            RustExpr::Raw("java_string_substring(name.as_str(), 0, 1)".to_string()),
            None,
            None,
        ),
        RustExpr::Raw("java_string_substring(name.as_str(), 0, 1)".to_string())
    );
}

#[test]
fn literal_slot_plan_owns_borrowed_string_and_numeric_char_adaptation() {
    let mut borrowed_str = RustSlotShape::conservative(JavaType::String);
    borrowed_str.ownership = OwnershipShape::SharedBorrow;
    borrowed_str.view = ViewShape::Str;
    let borrowed_plan = literal_slot_plan(
        &Literal::String("id".to_string()),
        &argument_slot(borrowed_str),
    )
    .unwrap();
    assert_eq!(
        borrowed_plan,
        LiteralSlotPlan::BorrowedStringView {
            value: "id".to_string()
        }
    );
    assert_eq!(
        apply_literal_slot_plan(&borrowed_plan),
        RustExpr::Literal("\"id\"".to_string())
    );

    let char_slot = argument_slot(RustSlotShape::conservative(JavaType::Primitive(
        PrimitiveType::Char,
    )));
    let char_plan = literal_slot_plan(&Literal::Int(9), &char_slot).unwrap();
    assert_eq!(char_plan, LiteralSlotPlan::NumericCharCode { ch: '\t' });
    assert_eq!(
        apply_literal_slot_plan(&char_plan),
        RustExpr::Literal("'\\t'".to_string())
    );
    assert_eq!(java_integral_literal_char(&Literal::Long(32)), Some(' '));
    assert_eq!(java_integral_literal_char(&Literal::Int(0xd800)), None);
}

#[test]
fn borrowed_string_view_projection_owns_rendered_view_compatibility() {
    let existing = borrowed_string_view_projection("*value.as_str()", Some("value")).unwrap();
    assert_eq!(existing, BorrowedStringViewProjection::AlreadyStringView);
    assert_eq!(
        apply_borrowed_string_view_projection("*value.as_str()", &existing),
        "value.as_str()"
    );

    let local = borrowed_string_view_projection("*value", Some("value")).unwrap();
    assert_eq!(
        local,
        BorrowedStringViewProjection::LocalStringValue {
            rust_name: "value".to_string()
        }
    );
    assert_eq!(
        apply_borrowed_string_view_projection("*value", &local),
        "value.as_str()"
    );

    assert_eq!(borrowed_string_view_projection("*value", None), None);
}

#[test]
fn shape_coercion_plan_keeps_axis_policy_decisions_inspectable() {
    let actual = RustSlotShape::conservative(JavaType::String);
    let mut target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    target.ownership = OwnershipShape::SharedBorrow;
    let mut target_element = target.clone();
    target_element.java_ty = JavaType::String;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(
        plan,
        ShapeCoercionPlan {
            actual: actual.clone(),
            target: target.clone(),
            dispatch: DispatchCoercionPolicy::NoDispatchChange,
            view: ViewCoercionPolicy::NoViewChange,
            optional: OptionalShapeCoercionPolicy::Wrap {
                element_ty: JavaType::String,
                element_plan: Box::new(ShapeCoercionPlan {
                    actual: actual.clone(),
                    target: target_element,
                    dispatch: DispatchCoercionPolicy::NoDispatchChange,
                    view: ViewCoercionPolicy::NoViewChange,
                    optional: OptionalShapeCoercionPolicy::NoChange,
                    ownership: OwnershipCoercionPolicy::BorrowOwnedValue,
                    primitive_widening: None,
                }),
            },
            ownership: OwnershipCoercionPolicy::BorrowOwnedValue,
            primitive_widening: None,
        }
    );
    assert_eq!(
        plan.coercions(),
        vec![Coercion::Borrow, Coercion::WrapOption]
    );
    assert_eq!(
        apply_shape_coercion_plan(RustExpr::Var("name".to_string()), &plan),
        RustExpr::Raw("Some(&name)".to_string())
    );

    let int_actual = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let long_target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    let widening = shape_coercion_plan(&int_actual, &long_target);

    assert_eq!(
        widening.primitive_widening,
        Some(PrimitiveWideningPolicy {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
            from_rank: 3,
            to_rank: 4,
            widens: true,
        })
    );
    assert_eq!(
        widening.coercions(),
        vec![Coercion::WidenPrimitive {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
        }]
    );
}

#[test]
fn owned_to_mutable_borrow_is_an_ownership_coercion() {
    let external = JavaType::Named {
        name: "ExternalMutableConfig".to_string(),
        args: Vec::new(),
    };
    let actual = RustSlotShape::conservative(external.clone());
    let mut target = RustSlotShape::conservative(external);
    target.ownership = OwnershipShape::MutableBorrow;

    let plan = shape_coercion_plan(&actual, &target);

    assert_eq!(plan.ownership, OwnershipCoercionPolicy::BorrowOwnedMutably);
    assert_eq!(plan.coercions(), vec![Coercion::BorrowMut]);
    assert_eq!(
        apply_shape_coercion_plan(RustExpr::Var("config".to_string()), &plan),
        RustExpr::Raw("&mut config".to_string())
    );
}

#[test]
fn optional_owned_to_mutable_borrow_uses_mutable_optional_projection() {
    let config_ty = JavaType::Named {
        name: "ExternalMutableConfig".to_string(),
        args: Vec::new(),
    };
    let actual = RustSlotShape::conservative(optional_java_type(config_ty.clone()));
    let mut target = RustSlotShape::conservative(config_ty);
    target.ownership = OwnershipShape::MutableBorrow;

    let plan = shape_coercion_plan(&actual, &target);

    assert_eq!(plan.coercions(), vec![Coercion::UnwrapOptionMutBorrowed]);
    assert_eq!(
        apply_shape_coercion_plan(RustExpr::Var("config".to_string()), &plan),
        RustExpr::Raw("config.as_mut().expect(\"Java null receiver\")".to_string())
    );
}

#[test]
fn syntax_slot_coercion_policy_owns_index_condition_and_concat_segments() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
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

    assert_eq!(
        syntax_slot_coercion_policy(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &facts
        ),
        SyntaxSlotCoercionPolicy::IndexToUsize
    );
    assert_eq!(
        syntax_slot_coercion_policy(
            &JavaType::Primitive(PrimitiveType::Boolean),
            &Slot::Condition,
            &facts
        ),
        SyntaxSlotCoercionPolicy::Identity
    );
    assert_eq!(
        syntax_slot_coercion_policy(
            &external_id,
            &Slot::StringConcatSegment,
            &FactSet::default()
        ),
        SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString
    );
    assert_eq!(
        syntax_slot_coercion_policy(&external_id, &Slot::StringConcatSegment, &facts),
        SyntaxSlotCoercionPolicy::Identity
    );
    assert_eq!(
        syntax_coercion_plan(&JavaType::Primitive(PrimitiveType::Int), &Slot::Index,),
        SyntaxCoercionPlan {
            actual: JavaType::Primitive(PrimitiveType::Int),
            slot: Slot::Index,
            content: CoercionContentPolicy {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                string_content: false,
            },
            policy: SyntaxSlotCoercionPolicy::IndexToUsize,
        }
    );
    assert_eq!(
        apply_syntax_coercion_plan(
            RustExpr::Var("i".to_string()),
            &syntax_coercion_plan(&JavaType::Primitive(PrimitiveType::Int), &Slot::Index),
        ),
        RustExpr::Raw("i as usize".to_string())
    );
    assert_eq!(
        apply_syntax_coercion_plan(
            RustExpr::Raw("(bits & 4499201580859392i64) >> 42".to_string()),
            &syntax_coercion_plan(&JavaType::Primitive(PrimitiveType::Long), &Slot::Index),
        ),
        RustExpr::Raw("((bits & 4499201580859392i64) >> 42) as usize".to_string())
    );
    assert_eq!(
        render_index_to_usize_projection("0", IndexToUsizeProjection::JavaStringCharAt),
        "0 as usize"
    );
    assert_eq!(
        render_index_to_usize_projection("index", IndexToUsizeProjection::JavaStringCharAt),
        "(index) as usize"
    );
    assert_eq!(
        render_index_to_usize_projection("index", IndexToUsizeProjection::StreamLambdaArgument),
        "index as usize"
    );
    assert_eq!(
        render_index_to_usize_projection("*value", IndexToUsizeProjection::StreamLambdaArgument),
        "(*value) as usize"
    );
    assert_eq!(
        apply_coercions(
            RustExpr::Raw("value >> 12".to_string()),
            &[Coercion::WidenPrimitive {
                from: PrimitiveType::Long,
                to: PrimitiveType::Double,
            }],
        ),
        RustExpr::Raw("(value >> 12) as f64".to_string())
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        SyntaxCoercionPlan {
            actual: external_id.clone(),
            slot: Slot::StringConcatSegment,
            content: CoercionContentPolicy {
                java_ty: external_id.clone(),
                string_content: true,
            },
            policy: SyntaxSlotCoercionPolicy::Identity,
        }
    );
    assert_eq!(
        apply_syntax_coercion_plan(
            RustExpr::Var("id".to_string()),
            &syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        ),
        RustExpr::Var("id".to_string())
    );
    assert_eq!(
        syntax_coercion_plan(&external_id, &Slot::StringConcatSegment).policy,
        SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString,
    );
    assert_eq!(
        coercion_content_policy(&external_id, &facts),
        CoercionContentPolicy {
            java_ty: external_id.clone(),
            string_content: true,
        }
    );
    assert_eq!(
        coercion_shape_content_policy(&RustSlotShape::conservative(external_id.clone()), &facts),
        CoercionShapeContentPolicy {
            java_ty: external_id,
            string_content: true,
        }
    );
}

#[test]
fn slot_conversion_plan_selects_shape_syntax_and_identity_boundaries() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let long_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));

    assert_eq!(
        slot_conversion_plan(&int_shape, &Slot::Index),
        SlotConversionPlan::Syntax(SyntaxCoercionPlan {
            actual: JavaType::Primitive(PrimitiveType::Int),
            slot: Slot::Index,
            content: CoercionContentPolicy {
                java_ty: JavaType::Primitive(PrimitiveType::Int),
                string_content: false,
            },
            policy: SyntaxSlotCoercionPolicy::IndexToUsize,
        })
    );
    assert_eq!(
        slot_conversion_plan_with_facts(
            &int_shape,
            &Slot::Return(long_shape.clone()),
            &FactSet::default(),
        ),
        SlotConversionPlan::Shape(ShapeCoercionPlan {
            actual: int_shape,
            target: long_shape,
            dispatch: DispatchCoercionPolicy::NoDispatchChange,
            view: ViewCoercionPolicy::NoViewChange,
            optional: OptionalShapeCoercionPolicy::NoChange,
            ownership: OwnershipCoercionPolicy::NoOwnershipChange,
            primitive_widening: Some(PrimitiveWideningPolicy {
                from: PrimitiveType::Int,
                to: PrimitiveType::Long,
                from_rank: 3,
                to_rank: 4,
                widens: true,
            }),
        })
    );

    let lowered = LoweredType::optional_element_borrow(JavaType::String);
    assert_eq!(
        lowered_slot_conversion_plan(&lowered, &Slot::Index),
        SlotConversionPlan::Identity {
            actual: lowered,
            slot: Slot::Index,
        }
    );
}

#[test]
fn conversion_plan_wraps_slot_conversion_decisions_before_application() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let slot_plan = plan_slot_conversion(int_shape.clone(), Slot::Index, &FactSet::default());

    assert_eq!(
        slot_plan,
        SlotExprConversionPlan {
            actual: int_shape,
            slot: Slot::Index,
            conversion: SlotConversionPlan::Syntax(SyntaxCoercionPlan {
                actual: JavaType::Primitive(PrimitiveType::Int),
                slot: Slot::Index,
                content: CoercionContentPolicy {
                    java_ty: JavaType::Primitive(PrimitiveType::Int),
                    string_content: false,
                },
                policy: SyntaxSlotCoercionPolicy::IndexToUsize,
            }),
        }
    );
    assert_eq!(
        apply_slot_expr_conversion(RustExpr::Var("i".to_string()), &slot_plan),
        RustExpr::Raw("i as usize".to_string())
    );

    let lowered_actual = LoweredType::optional_element_borrow(JavaType::String);
    let lowered_plan =
        plan_lowered_slot_conversion(&lowered_actual, Slot::Index, &FactSet::default());
    assert_eq!(
        lowered_plan,
        LoweredSlotExprConversionPlan {
            actual: lowered_actual.clone(),
            slot: Slot::Index,
            conversion: SlotConversionPlan::Identity {
                actual: lowered_actual,
                slot: Slot::Index,
            },
        }
    );
    assert_eq!(
        apply_lowered_slot_expr_conversion(
            LoweredExpr {
                expr: RustExpr::Var("value".to_string()),
                actual: LoweredType::optional_element_borrow(JavaType::String),
            },
            &lowered_plan,
        ),
        RustExpr::Var("value".to_string())
    );
}

#[test]
fn crate_mapped_handle_and_source_helper_use_same_slot_conversion_policy() {
    let crate_mapped_ty = JavaType::Named {
        name: "OpenMode".to_string(),
        args: Vec::new(),
    };
    let source_helper_ty = JavaType::Named {
        name: "SourceOpenMode".to_string(),
        args: Vec::new(),
    };

    let mut source_facts = FactSet::default();
    source_facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "SourceOpenMode".to_string(),
            copy: Some(false),
            clone: Some(true),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });

    let crate_mapped_actual = RustSlotShape {
        java_ty: crate_mapped_ty.clone(),
        ownership: OwnershipShape::SharedBorrow,
        view: ViewShape::Native,
        dispatch: DispatchShape::Concrete,
        mutability: new_java2rust::slot_shape::MutabilityShape::Immutable,
        type_identity: new_java2rust::slot_shape::TypeIdentity::from_java_type(
            crate_mapped_ty.clone(),
        ),
    };
    let crate_mapped_target = RustSlotShape::conservative(crate_mapped_ty);
    let source_actual = RustSlotShape {
        java_ty: source_helper_ty.clone(),
        ownership: OwnershipShape::SharedBorrow,
        view: ViewShape::Native,
        dispatch: DispatchShape::Concrete,
        mutability: new_java2rust::slot_shape::MutabilityShape::Immutable,
        type_identity: new_java2rust::slot_shape::TypeIdentity::from_java_type(
            source_helper_ty.clone(),
        ),
    };
    let source_target = RustSlotShape::conservative(source_helper_ty);

    let crate_mapped_plan = slot_conversion_plan_with_facts(
        &crate_mapped_actual,
        &Slot::Return(crate_mapped_target),
        &hdf5_pack_facts(),
    );
    let source_plan = slot_conversion_plan_with_facts(
        &source_actual,
        &Slot::Return(source_target),
        &source_facts,
    );

    let crate_mapped_coercions = match &crate_mapped_plan {
        SlotConversionPlan::Shape(plan) => plan.coercions(),
        other => panic!("expected crate-mapped shape conversion plan, got {other:?}"),
    };
    let source_coercions = match &source_plan {
        SlotConversionPlan::Shape(plan) => plan.coercions(),
        other => panic!("expected source helper shape conversion plan, got {other:?}"),
    };

    assert_eq!(crate_mapped_coercions.len(), 1);
    assert_eq!(source_coercions.len(), 1);
    assert!(matches!(
        crate_mapped_coercions.as_slice(),
        [Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { .. }
        }]
    ));
    assert!(matches!(
        source_coercions.as_slice(),
        [Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { .. }
        }]
    ));
    assert!(matches!(
        crate_mapped_plan,
        SlotConversionPlan::Shape(ShapeCoercionPlan {
            ownership: OwnershipCoercionPolicy::RecoverOwnedWithClone { .. },
            ..
        })
    ));
    assert!(matches!(
        source_plan,
        SlotConversionPlan::Shape(ShapeCoercionPlan {
            ownership: OwnershipCoercionPolicy::RecoverOwnedWithClone { .. },
            ..
        })
    ));
}

#[test]
fn borrowed_whole_optional_string_recovery_uses_borrowed_carrier_policy() {
    let option_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;
    element_shape.view = ViewShape::Str;
    let target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let actual = LoweredType::OptionalElementBorrow {
        option_shape: Box::new(option_shape),
        element_shape: Box::new(element_shape),
        recovery: None,
    };

    let plan = lowered_slot_conversion_plan_with_facts(
        &actual,
        &Slot::Return(target),
        &FactSet::default(),
    );

    assert!(matches!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            adapter: Some(BorrowedElementAdapterPolicy {
                recovery: BorrowedElementRecoveryPolicy::StringViewToOwned,
                ..
            }),
            ..
        })
    ));
}

#[test]
fn optional_owned_recovery_capability_follows_element_capability() {
    let facts = FactSet::default();
    let optional_string = optional_java_type(JavaType::String);

    assert!(matches!(
        facts.owned_recovery_capability_for(&optional_string),
        OwnedRecoveryCapability::ExplicitClone { java_ty } if java_ty == optional_string
    ));
}

#[test]
fn optional_array_carrier_to_optional_slice_target_uses_view_projection_policy() {
    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
    let option_shape = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    let element_shape = RustSlotShape::conservative(array_ty.clone());
    let mut target = RustSlotShape::conservative(optional_java_type(array_ty));
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape.clone()),
            element_shape: Box::new(element_shape.clone()),
            recovery: None,
        },
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );

    assert_eq!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape: option_shape,
            element_shape: element_shape.clone(),
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: Some(BorrowedElementAdapterPolicy {
                carrier: BorrowedElementCarrier::Option,
                element_shape,
                recovery: BorrowedElementRecoveryPolicy::BorrowElement,
                unwrap_carrier: false,
            }),
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("values".to_string()), &plan),
        RustExpr::Raw("values.as_deref()".to_string())
    );
}

#[test]
fn optional_array_carrier_to_required_slice_target_borrows_before_unwrap() {
    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
    let option_shape = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    let element_shape = RustSlotShape::conservative(array_ty.clone());
    let mut target = RustSlotShape::conservative(array_ty);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        },
        &Slot::Argument(target),
        &FactSet::default(),
    );

    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("values".to_string()), &plan),
        RustExpr::Raw("values.as_ref().expect(\"Java null receiver\").as_slice()".to_string())
    );
}

#[test]
fn lowered_slot_conversion_plan_records_borrowed_element_adapter_before_rendering() {
    let element_ty = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let actual = LoweredType::optional_element_borrow(element_ty.clone());
    let target = RustSlotShape::conservative(optional_java_type(element_ty));
    let LoweredType::OptionalElementBorrow {
        option_shape,
        element_shape,
        ..
    } = &actual
    else {
        panic!("expected optional element borrow");
    };

    let plan = lowered_slot_conversion_plan_with_facts(
        &actual,
        &Slot::Return(target.clone()),
        &FactSet::default(),
    );

    assert_eq!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape: (**option_shape).clone(),
            element_shape: (**element_shape).clone(),
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: Some(BorrowedElementAdapterPolicy {
                carrier: BorrowedElementCarrier::Option,
                element_shape: (**element_shape).clone(),
                recovery: BorrowedElementRecoveryPolicy::CloneElements {
                    policy: CloneRecoveryPolicy::AuditRequired,
                },
                unwrap_carrier: false,
            }),
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("values.iter().next()".to_string()), &plan),
        RustExpr::Raw(
            "values.iter().next().cloned()/* TODO(translation): validate added clone */"
                .to_string()
        )
    );
}

#[test]
fn clone_recovery_policy_is_explicit_in_coercion_adapter() {
    assert_eq!(
        apply_coercions(
            RustExpr::Var("value".to_string()),
            &[Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            }],
        ),
        RustExpr::Raw("value.clone()/* TODO(translation): validate added clone */".to_string())
    );
    assert_eq!(
        apply_coercions(
            RustExpr::Var("value".to_string()),
            &[Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: JavaType::Named {
                        name: "ExternalValue".to_string(),
                        args: Vec::new(),
                    },
                },
            }],
        ),
        RustExpr::Raw("value.clone()".to_string())
    );
}

#[test]
fn shape_coercion_uses_capability_facts_for_borrowed_to_owned_clone() {
    let external = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let mut actual = RustSlotShape::conservative(external.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(external);

    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::AuditRequired,
        }]
    );
}

#[test]
fn shape_coercion_uses_explicit_clone_capability_as_recovery_proof() {
    let external = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalValue".to_string(),
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
    let mut actual = RustSlotShape::conservative(external.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(external.clone());

    assert_eq!(
        coercion_owned_recovery_capability_policy(&external, &facts),
        CoercionOwnedRecoveryCapabilityPolicy {
            java_ty: external.clone(),
            recovery: OwnedRecoveryCapability::ExplicitClone {
                java_ty: external.clone(),
            },
        }
    );
    let effective_facts = EffectiveFacts::new(&facts);
    assert_eq!(
        coercion_owned_recovery_capability_policy_with_effective_facts(&external, &effective_facts,),
        coercion_owned_recovery_capability_policy(&external, &facts)
    );
    let policy = ownership_coercion_policy(&actual, &target, &facts);
    assert_eq!(
        ownership_coercion_policy_with_effective_facts(&actual, &target, &effective_facts),
        policy
    );

    assert_eq!(
        policy,
        OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: external.clone(),
            },
        }
    );
    assert!(policy.diagnostics().is_empty());
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &facts),
        vec![Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty: external },
        }]
    );
}

#[test]
fn shape_coercion_uses_copy_capability_for_borrowed_to_owned_recovery() {
    let external = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
            copy: Some(true),
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let mut actual = RustSlotShape::conservative(external.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(external);

    assert_eq!(
        ownership_coercion_policy(&actual, &target, &facts),
        OwnershipCoercionPolicy::RecoverOwnedCopyValue
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &facts),
        vec![Coercion::CopyBorrowedValue]
    );
    assert_eq!(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("external_id".to_string()),
            actual,
            &Slot::Argument(target),
            &facts,
        ),
        RustExpr::Raw("*external_id".to_string())
    );
}

#[test]
fn ownership_coercion_policy_owns_borrow_clone_and_unsupported_recovery() {
    let external = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: None,
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let mut owned = RustSlotShape::conservative(external.clone());
    owned.ownership = OwnershipShape::Owned;
    let mut borrowed = RustSlotShape::conservative(external.clone());
    borrowed.ownership = OwnershipShape::SharedBorrow;
    let mut borrowed_handle = RustSlotShape::conservative(handle.clone());
    borrowed_handle.ownership = OwnershipShape::SharedBorrow;
    let owned_handle = RustSlotShape::conservative(handle);

    assert_eq!(
        ownership_coercion_policy(&owned, &borrowed, &facts),
        OwnershipCoercionPolicy::BorrowOwnedValue
    );
    let clone_recovery = ownership_coercion_policy(&borrowed, &owned, &facts);
    assert_eq!(
        clone_recovery,
        OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::AuditRequired,
        }
    );
    assert_eq!(
        clone_recovery.diagnostics(),
        vec![CoercionDiagnostic {
            code: "clone_audit_required",
            message:
                "borrowed-to-owned Java value recovery inserted a clone that requires ownership audit"
                    .to_string(),
        }]
    );
    assert_eq!(
        ownership_coercion_policy(&borrowed_handle, &owned_handle, &facts),
        OwnershipCoercionPolicy::UnsupportedRecovery {
            actual_ownership: OwnershipShape::SharedBorrow,
            target_ownership: OwnershipShape::Owned,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable {
                java_ty: JavaType::Named {
                    name: "ExternalHandle".to_string(),
                    args: Vec::new(),
                },
            },
        }
    );
    assert_eq!(
        ownership_coercion_policy(&borrowed_handle, &owned_handle, &facts).diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_to_owned_non_cloneable",
            message:
                "unsupported Java borrowed-to-owned recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
                    .to_string(),
        }]
    );
    let rendered = apply_coercions(
        RustExpr::Var("handle".to_string()),
        &ownership_coercion_policy(&borrowed_handle, &owned_handle, &facts).coercions(),
    );
    assert!(matches!(
        rendered,
        RustExpr::Raw(ref raw)
            if raw.contains("unsupported Java borrowed-to-owned recovery for non-cloneable type")
                && raw.contains("ExternalHandle")
    ));
}

#[test]
fn owned_recovery_capability_facts_drive_distinct_coercion_outcomes() {
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
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: type_name.to_string(),
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
    }

    assert_eq!(
        facts
            .owned_recovery_capability_fact_for(&clone_box)
            .unwrap()
            .capability,
        OwnedRecoveryCapability::CloneBoxAdapter {
            java_ty: clone_box.clone(),
        }
    );
    assert_eq!(
        coercion_owned_recovery_capability_policy(&clone_box, &facts).recovery,
        OwnedRecoveryCapability::CloneBoxAdapter {
            java_ty: clone_box.clone(),
        }
    );

    let borrowed_to_owned = |java_ty: &JavaType| {
        let mut actual = RustSlotShape::conservative(java_ty.clone());
        actual.ownership = OwnershipShape::SharedBorrow;
        let target = RustSlotShape::conservative(java_ty.clone());
        (actual, target)
    };
    let (actual_clone_box, target_clone_box) = borrowed_to_owned(&clone_box);
    assert_eq!(
        ownership_coercion_policy(&actual_clone_box, &target_clone_box, &facts),
        OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: clone_box.clone(),
            },
        }
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual_clone_box, &target_clone_box, &facts),
        vec![Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty: clone_box },
        }]
    );

    let (actual_factory, target_factory) = borrowed_to_owned(&factory);
    assert_eq!(
        ownership_coercion_policy(&actual_factory, &target_factory, &facts),
        OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: factory.clone(),
            },
        }
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual_factory, &target_factory, &facts),
        vec![Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty: factory },
        }]
    );

    let (actual_immutable, target_immutable) = borrowed_to_owned(&immutable);
    assert_eq!(
        ownership_coercion_policy(&actual_immutable, &target_immutable, &facts),
        OwnershipCoercionPolicy::RecoverOwnedCopyValue
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual_immutable, &target_immutable, &facts),
        vec![Coercion::CopyBorrowedValue]
    );

    let (actual_facade, target_facade) = borrowed_to_owned(&facade);
    assert_eq!(
        ownership_coercion_policy(&actual_facade, &target_facade, &facts),
        OwnershipCoercionPolicy::UnsupportedRecovery {
            actual_ownership: OwnershipShape::SharedBorrow,
            target_ownership: OwnershipShape::Owned,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle {
                java_ty: facade,
            },
        }
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual_facade, &target_facade, &facts),
        vec![Coercion::UnsupportedOwnership {
            actual_ownership: OwnershipShape::SharedBorrow,
            target_ownership: OwnershipShape::Owned,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle {
                java_ty: actual_facade.java_ty.clone(),
            },
        }]
    );

    let (actual_ffi, target_ffi) = borrowed_to_owned(&ffi);
    assert_eq!(
        ownership_coercion_policy(&actual_ffi, &target_ffi, &facts).diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_to_ffi_owned_handle",
            message:
                "unsupported Java borrowed-to-owned recovery for FFI-owned handle Named { name: \"FfiHandle\", args: [] }; use an explicit FFI ownership transfer"
                    .to_string(),
        }]
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual_ffi, &target_ffi, &facts),
        vec![Coercion::UnsupportedOwnership {
            actual_ownership: OwnershipShape::SharedBorrow,
            target_ownership: OwnershipShape::Owned,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToFfiOwnedHandle { java_ty: ffi },
        }]
    );
}

#[test]
fn external_pack_manifest_owned_recovery_facts_drive_coercion() {
    let parsed = parse_external_pack_manifest(
        r#"
        external-pack neutral.gatk neutral.gatk external-fact-model semantic
        external-pack-owned-recovery neutral.gatk neutral.gatk.VariantContext factory-copy
        external-pack-owned-recovery neutral.gatk neutral.gatk.Allele immutable-shared-handle
        external-pack-owned-recovery neutral.gatk neutral.gatk.NativeReader ffi-owned-handle
        "#,
    );
    assert!(
        parsed.diagnostics.is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed.diagnostics
    );

    let variant = JavaType::Named {
        name: "neutral.gatk.VariantContext".to_string(),
        args: Vec::new(),
    };
    let allele = JavaType::Named {
        name: "neutral.gatk.Allele".to_string(),
        args: Vec::new(),
    };
    let reader = JavaType::Named {
        name: "neutral.gatk.NativeReader".to_string(),
        args: Vec::new(),
    };
    let borrowed_to_owned = |java_ty: &JavaType| {
        let mut actual = RustSlotShape::conservative(java_ty.clone());
        actual.ownership = OwnershipShape::SharedBorrow;
        let target = RustSlotShape::conservative(java_ty.clone());
        (actual, target)
    };

    assert_eq!(
        parsed
            .facts
            .owned_recovery_capability_fact_for(&variant)
            .unwrap()
            .capability,
        OwnedRecoveryCapability::FactoryCopy {
            java_ty: JavaType::Named {
                name: "VariantContext".to_string(),
                args: Vec::new(),
            },
        }
    );
    let (actual_variant, target_variant) = borrowed_to_owned(&variant);
    assert_eq!(
        ownership_coercion_policy(&actual_variant, &target_variant, &parsed.facts),
        OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Named {
                    name: "VariantContext".to_string(),
                    args: Vec::new(),
                },
            },
        }
    );

    let (actual_allele, target_allele) = borrowed_to_owned(&allele);
    assert_eq!(
        ownership_coercion_policy(&actual_allele, &target_allele, &parsed.facts),
        OwnershipCoercionPolicy::RecoverOwnedCopyValue
    );

    let (actual_reader, target_reader) = borrowed_to_owned(&reader);
    assert_eq!(
        ownership_coercion_policy(&actual_reader, &target_reader, &parsed.facts).diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_to_ffi_owned_handle",
            message:
                "unsupported Java borrowed-to-owned recovery for FFI-owned handle Named { name: \"neutral.gatk.NativeReader\", args: [] }; use an explicit FFI ownership transfer"
                    .to_string(),
        }]
    );
}

#[test]
fn external_pack_manifest_owned_recovery_rejects_unowned_or_unknown_entries() {
    let unowned = parse_external_pack_manifest(
        r#"
        external-pack neutral.gatk neutral.gatk external-fact-model semantic
        external-pack-owned-recovery neutral.gatk other.runtime.Handle clone-box-adapter
        "#,
    );
    assert!(
        unowned.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "external_pack_manifest_unowned_owned_recovery_capability_fact"
        }),
        "{:?}",
        unowned.diagnostics
    );

    let unknown_kind = parse_external_pack_manifest(
        r#"
        external-pack neutral.gatk neutral.gatk external-fact-model semantic
        external-pack-owned-recovery neutral.gatk neutral.gatk.Handle magical-copy
        "#,
    );
    assert!(
        unknown_kind.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "external_pack_manifest_parse_error"
                && diagnostic
                    .message
                    .contains("invalid external-pack-owned-recovery capability kind")
        }),
        "{:?}",
        unknown_kind.diagnostics
    );
}

#[test]
fn shape_coercion_rejects_borrowed_to_owned_for_non_cloneable_types() {
    let external = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalHandle".to_string(),
            copy: None,
            clone: Some(false),
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: None,
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });
    let mut actual = RustSlotShape::conservative(external.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(external);
    let conversion_plan = shape_coercion_plan_with_facts(&actual, &target, &facts);

    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &facts),
        vec![Coercion::UnsupportedOwnership {
            actual_ownership: OwnershipShape::SharedBorrow,
            target_ownership: OwnershipShape::Owned,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable {
                java_ty: JavaType::Named {
                    name: "ExternalHandle".to_string(),
                    args: Vec::new(),
                },
            },
        }]
    );
    assert_eq!(
        conversion_plan.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_borrowed_to_owned_non_cloneable",
            message:
                "unsupported Java borrowed-to-owned recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
                    .to_string(),
        }]
    );
    assert_eq!(
        plan_slot_conversion(actual.clone(), Slot::Argument(target.clone()), &facts)
            .unsupported_diagnostics(),
        conversion_plan.unsupported_diagnostics()
    );
    let rendered = apply_expr_to_slot_with_facts(
        RustExpr::Var("handle".to_string()),
        actual,
        &Slot::Argument(target),
        &facts,
    );
    assert!(matches!(
        rendered,
        RustExpr::Raw(ref raw)
            if raw.contains("unsupported Java borrowed-to-owned recovery for non-cloneable type")
                && raw.contains("ExternalHandle")
    ));
}

#[test]
fn optional_shape_coercion_policy_records_structural_element_coercions() {
    let actual = RustSlotShape::conservative(JavaType::String);
    let mut target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    target.ownership = OwnershipShape::SharedBorrow;
    let mut target_element = target.clone();
    target_element.java_ty = JavaType::String;
    let facts = FactSet::default();

    let wrap_policy = optional_shape_coercion_policy_with_facts(&actual, &target, &facts);
    assert_eq!(
        wrap_policy,
        OptionalShapeCoercionPolicy::Wrap {
            element_ty: JavaType::String,
            element_plan: Box::new(ShapeCoercionPlan {
                actual: actual.clone(),
                target: target_element,
                dispatch: DispatchCoercionPolicy::NoDispatchChange,
                view: ViewCoercionPolicy::NoViewChange,
                optional: OptionalShapeCoercionPolicy::NoChange,
                ownership: OwnershipCoercionPolicy::BorrowOwnedValue,
                primitive_widening: None,
            }),
        }
    );
    assert_eq!(
        wrap_policy.coercions(),
        vec![Coercion::Borrow, Coercion::WrapOption]
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &facts),
        vec![Coercion::Borrow, Coercion::WrapOption]
    );

    let construction = optional_slot_construction_plan(&target, JavaType::String);
    assert_eq!(construction.wrapper_ownership, OwnershipShape::Owned);
    assert_eq!(construction.target, target);
    assert_eq!(construction.element_shape.java_ty, JavaType::String);
    assert_eq!(
        construction.element_shape.ownership,
        OwnershipShape::SharedBorrow
    );
    let actual_owned = RustSlotShape::conservative(JavaType::String);
    let construction = optional_slot_construction_plan_for_actual(&actual_owned, &target, &facts);
    assert_eq!(construction.wrapper_ownership, OwnershipShape::Owned);
    assert_eq!(construction.target, target);
    assert_eq!(construction.element_shape, actual_owned);
    assert_eq!(
        construction.element_plan.coercions(),
        vec![Coercion::Borrow]
    );
    let optional_construction = SlotConversionPlan::OptionalConstruction(construction);
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Var("name".to_string()), &optional_construction),
        RustExpr::Call {
            function: Box::new(RustExpr::Var("Some".to_string())),
            args: vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("name".to_string())),
            }],
        }
    );
    let preconverted =
        preconverted_optional_slot_construction_plan(&target, JavaType::String, &facts);
    assert_eq!(preconverted.wrapper_ownership, OwnershipShape::Owned);
    assert_eq!(preconverted.target, target);
    assert_eq!(preconverted.element_shape.java_ty, JavaType::String);
    assert_eq!(
        preconverted.element_shape.ownership,
        OwnershipShape::SharedBorrow
    );
    assert_eq!(
        preconverted.element_plan.coercions(),
        vec![Coercion::Identity]
    );
    let optional_preconverted = SlotConversionPlan::OptionalConstruction(preconverted);
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("&name".to_string()), &optional_preconverted),
        RustExpr::Call {
            function: Box::new(RustExpr::Var("Some".to_string())),
            args: vec![RustExpr::Raw("&name".to_string())],
        }
    );

    let unwrap_actual = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let unwrap_target = RustSlotShape::conservative(JavaType::String);
    let mut unwrap_actual_element = unwrap_actual.clone();
    unwrap_actual_element.java_ty = JavaType::String;
    let unwrap_policy = optional_shape_coercion_policy(&unwrap_actual, &unwrap_target);
    assert_eq!(
        unwrap_policy,
        OptionalShapeCoercionPolicy::Unwrap {
            element_ty: JavaType::String,
            element_plan: Box::new(ShapeCoercionPlan {
                actual: unwrap_actual_element,
                target: unwrap_target,
                dispatch: DispatchCoercionPolicy::NoDispatchChange,
                view: ViewCoercionPolicy::NoViewChange,
                optional: OptionalShapeCoercionPolicy::NoChange,
                ownership: OwnershipCoercionPolicy::NoOwnershipChange,
                primitive_widening: None,
            }),
        }
    );
    assert_eq!(unwrap_policy.coercions(), vec![Coercion::UnwrapOption]);
}

#[test]
fn optional_present_construction_policy_records_indexed_borrow_source() {
    let facts = FactSet::default();
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Optional".to_string(),
        args: vec![JavaType::String],
    });
    target.ownership = OwnershipShape::Owned;

    let mut actual_indexed = RustSlotShape::conservative(JavaType::String);
    actual_indexed.ownership = OwnershipShape::SharedBorrow;
    let indexed = optional_present_construction_plan_for_actual(
        &actual_indexed,
        &target,
        JavaType::String,
        OptionalPresentConstructionSource::IndexedBorrowProjection,
        &facts,
    );
    assert_eq!(indexed.element_lowering_slot, Slot::Value);
    assert_eq!(
        indexed.construction.element_shape.ownership,
        OwnershipShape::Owned
    );
    assert_eq!(
        indexed.construction.element_plan.coercions(),
        vec![Coercion::Identity]
    );

    let plain = optional_present_construction_plan_for_actual(
        &actual_indexed,
        &target,
        JavaType::String,
        OptionalPresentConstructionSource::Plain,
        &facts,
    );
    assert_eq!(
        plain.element_lowering_slot,
        Slot::Assignment(plain.construction.element_shape.clone())
    );
    assert_eq!(
        plain.construction.element_shape.ownership,
        OwnershipShape::Owned
    );
}

#[test]
fn optional_assignment_adapter_preserves_structured_none_without_rendering() {
    let facts = FactSet::default();
    let actual = RustSlotShape::conservative(JavaType::String);
    let target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let conversion_plan = SlotExprConversionPlan {
        actual: actual.clone(),
        slot: Slot::Assignment(target.clone()),
        conversion: SlotConversionPlan::OptionalConstruction(
            optional_slot_construction_plan_for_actual(&actual, &target, &facts),
        ),
    };
    let preconverted =
        preconverted_optional_slot_construction_plan(&target, JavaType::String, &facts);

    assert_eq!(
        adapt_optional_assignment_value_for_target(
            RustExpr::Var("None".to_string()),
            actual,
            conversion_plan,
            preconverted,
            false,
        ),
        RustExpr::Var("None".to_string())
    );
}

#[test]
fn external_into_adapter_matches_unresolved_external_type_names() {
    let mut facts = FactSet::default();
    facts
        .external_conversion_adapter_facts
        .push(ExternalConversionAdapterFact {
            from_type: "ReadsFormatWriteOption".to_string(),
            to_type: "WriteOption".to_string(),
            kind: ExternalConversionAdapterKind::Into,
            provenance: FactProvenance::ExternalLibraryPack,
        });
    let actual =
        RustSlotShape::conservative(JavaType::Unknown("ReadsFormatWriteOption".to_string()));
    let target = RustSlotShape::conservative(JavaType::Unknown("WriteOption".to_string()));

    let plan = dispatch_coercion_policy_with_facts(&actual, &target, &facts);

    assert!(matches!(plan, DispatchCoercionPolicy::ExternalInto { .. }));
}

#[test]
fn java_platform_zip_output_stream_converts_to_output_stream_slot() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "ZipOutputStream".to_string(),
        args: Vec::new(),
    });
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "OutputStream".to_string(),
        args: Vec::new(),
    });

    let plan = dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default());

    assert!(matches!(plan, DispatchCoercionPolicy::ExternalInto { .. }));
}

#[test]
fn java_platform_file_input_stream_converts_to_input_stream_slot() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "FileInputStream".to_string(),
        args: Vec::new(),
    });
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "InputStream".to_string(),
        args: Vec::new(),
    });

    let plan = dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default());

    assert!(matches!(plan, DispatchCoercionPolicy::ExternalInto { .. }));
}

#[test]
fn java_platform_file_input_stream_converts_before_optional_wrap() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "FileInputStream".to_string(),
        args: Vec::new(),
    });
    let target = RustSlotShape::conservative(optional_java_type(JavaType::Named {
        name: "InputStream".to_string(),
        args: Vec::new(),
    }));

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    let coercions = plan.coercions();
    assert!(
        matches!(
            coercions.as_slice(),
            [Coercion::ExternalInto { .. }, Coercion::WrapOption]
        ),
        "{coercions:?}"
    );
}

#[test]
fn optional_shape_coercion_wraps_after_erased_element_conversion() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "VidMappingPB".to_string(),
        args: Vec::new(),
    });
    let mut target = RustSlotShape::conservative(optional_java_type(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }));
    target.dispatch = new_java2rust::slot_shape::DispatchShape::ErasedObject;
    target.view = new_java2rust::slot_shape::ViewShape::ErasedObjectView;
    let facts = FactSet::default();

    let policy = optional_shape_coercion_policy_with_facts(&actual, &target, &facts);
    assert!(matches!(
        policy,
        OptionalShapeCoercionPolicy::Wrap { ref element_plan, .. }
            if matches!(element_plan.dispatch, DispatchCoercionPolicy::EraseObject)
    ));
    assert_eq!(
        policy.coercions(),
        vec![Coercion::EraseObject, Coercion::WrapOption]
    );
    assert_eq!(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("vid_map".to_string()),
            actual,
            &Slot::Assignment(target),
            &facts,
        ),
        RustExpr::Raw("Some(Box::new(vid_map) as Box<dyn std::any::Any>)".to_string())
    );
}

#[test]
fn coercion_uses_type_capability_facts_for_concat_segments() {
    let external_id = JavaType::Named {
        name: "ExternalId".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "ExternalId".to_string(),
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

    assert_eq!(
        plan_coercion(&external_id, &Slot::StringConcatSegment),
        vec![Coercion::ErasedObjectToJavaDisplayString]
    );
    assert_eq!(
        plan_coercion_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        vec![Coercion::Identity]
    );
}

#[test]
fn optional_display_segments_use_java_null_string_policy_before_rendering() {
    let optional_string = RustSlotShape::conservative(optional_java_type(JavaType::String));

    assert_eq!(
        slot_conversion_plan(&optional_string, &Slot::DisplaySegment),
        SlotConversionPlan::Syntax(SyntaxCoercionPlan {
            actual: optional_java_type(JavaType::String),
            slot: Slot::DisplaySegment,
            content: CoercionContentPolicy {
                java_ty: optional_java_type(JavaType::String),
                string_content: false,
            },
            policy: SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString,
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(
            RustExpr::Var("value".to_string()),
            &slot_conversion_plan(&optional_string, &Slot::StringConcatSegment),
        ),
        RustExpr::Raw("value.as_deref().unwrap_or(\"null\")".to_string())
    );
}

#[test]
fn optional_platform_facade_display_segments_use_display_value_policy() {
    let optional_uri = RustSlotShape::conservative(optional_java_type(JavaType::Named {
        name: "URI".to_string(),
        args: Vec::new(),
    }));

    assert_eq!(
        slot_conversion_plan(&optional_uri, &Slot::DisplaySegment),
        SlotConversionPlan::Syntax(SyntaxCoercionPlan {
            actual: optional_java_type(JavaType::Named {
                name: "URI".to_string(),
                args: Vec::new(),
            }),
            slot: Slot::DisplaySegment,
            content: CoercionContentPolicy {
                java_ty: optional_java_type(JavaType::Named {
                    name: "URI".to_string(),
                    args: Vec::new(),
                }),
                string_content: false,
            },
            policy: SyntaxSlotCoercionPolicy::OptionalToJavaDisplayValue,
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(
            RustExpr::Var("uri".to_string()),
            &slot_conversion_plan(&optional_uri, &Slot::StringConcatSegment),
        ),
        RustExpr::Raw(
            "uri.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
                .to_string(),
        )
    );
    assert_eq!(
        apply_java_display_plan(
            RustExpr::Raw("values.get(key.as_str())".to_string()),
            JavaDisplayPlan::OptionalBorrowedValue,
        ),
        RustExpr::Raw(
            "values.get(key.as_str()).map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
                .to_string(),
        )
    );
}

#[test]
fn optional_element_borrow_display_segments_use_lowered_slot_policy() {
    let actual =
        LoweredType::optional_element_borrow_shape(RustSlotShape::conservative(JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        }));

    assert_eq!(
        lowered_slot_conversion_plan(&actual, &Slot::DisplaySegment),
        SlotConversionPlan::Syntax(SyntaxCoercionPlan {
            actual: optional_java_type(JavaType::Named {
                name: "URI".to_string(),
                args: Vec::new(),
            }),
            slot: Slot::DisplaySegment,
            content: CoercionContentPolicy {
                java_ty: JavaType::Named {
                    name: "URI".to_string(),
                    args: Vec::new(),
                },
                string_content: false,
            },
            policy: SyntaxSlotCoercionPolicy::OptionalBorrowedToJavaDisplayValue,
        })
    );
    assert_eq!(
        apply_lowered_slot_expr_conversion(
            LoweredExpr {
                expr: RustExpr::Raw("values.get(key.as_str())".to_string()),
                actual,
            },
            &plan_lowered_slot_conversion(
                &LoweredType::optional_element_borrow_shape(RustSlotShape::conservative(
                    JavaType::Named {
                        name: "URI".to_string(),
                        args: Vec::new(),
                    },
                )),
                Slot::StringConcatSegment,
                &FactSet::default(),
            ),
        ),
        RustExpr::Raw(
            "values.get(key.as_str()).map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
                .to_string(),
        )
    );
}

#[test]
fn conversion_plan_owns_return_slot_construction() {
    let shape = RustSlotShape::conservative(JavaType::String);

    assert_eq!(return_slot(Some(&shape)), Some(Slot::Return(shape.clone())));
    assert_eq!(return_slot(None), None);
    assert_eq!(
        return_value_plan(Some(&shape)),
        ReturnValuePlan::Slot(Slot::Return(shape))
    );
    assert_eq!(return_value_plan(None), ReturnValuePlan::Raw);
}

#[test]
fn coercion_owns_syntax_role_slot_construction() {
    let shape = RustSlotShape::conservative(JavaType::String);

    assert_eq!(
        assignment_slot(shape.clone()),
        Slot::Assignment(shape.clone())
    );
    assert_eq!(
        assignment_value_plan_for_shape(&shape),
        AssignmentValuePlan {
            slot: Slot::Assignment(shape.clone())
        }
    );
    assert_eq!(
        argument_slot(shape),
        Slot::Argument(RustSlotShape::conservative(JavaType::String))
    );
    assert_eq!(condition_slot(), Slot::Condition);
    assert_eq!(index_slot(), Slot::Index);
}

#[test]
fn conversion_plan_owns_assignment_target_slot_selection() {
    let mut env = ShapeEnv::new();
    env.insert_local("name", JavaType::String);

    assert_eq!(
        assignment_slot_for_target(&parse_expr("name"), &env, ExprShapeContext::default(),),
        Slot::Assignment(RustSlotShape::for_storage(JavaType::String))
    );
    assert_eq!(
        assignment_value_plan_for_target(&parse_expr("name"), &env, ExprShapeContext::default(),),
        AssignmentValuePlan {
            slot: Slot::Assignment(RustSlotShape::for_storage(JavaType::String))
        }
    );
    assert_eq!(
        assignment_slot_for_target(
            &parse_expr("unknown.field"),
            &env,
            ExprShapeContext::default(),
        ),
        Slot::Value
    );
    assert_eq!(
        assignment_value_plan_for_target(
            &parse_expr("unknown.field"),
            &env,
            ExprShapeContext::default(),
        ),
        AssignmentValuePlan { slot: Slot::Value }
    );
}

#[test]
fn conversion_plan_owns_slot_actual_shape_selection_for_casts() {
    let mut env = ShapeEnv::new();
    env.insert_local("value", JavaType::Unknown("Object".to_string()));

    let shape = actual_shape_for_slot_expr(
        &parse_expr("(String) value"),
        &env,
        ExprShapeContext::default(),
    );

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn shape_coercion_owns_string_view_adapters() {
    let actual = RustSlotShape::conservative(JavaType::String);
    let mut target = RustSlotShape::conservative(JavaType::String);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Str;

    assert_eq!(plan_shape_coercion(&actual, &target), vec![Coercion::AsStr]);

    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("name".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw("name.as_str()".to_string())
    );
}

#[test]
fn view_coercion_policy_owns_string_view_adapters() {
    let owned = RustSlotShape::conservative(JavaType::String);
    let mut borrowed_str = RustSlotShape::conservative(JavaType::String);
    borrowed_str.ownership = OwnershipShape::SharedBorrow;
    borrowed_str.view = ViewShape::Str;
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));

    assert_eq!(
        view_coercion_policy(&owned, &borrowed_str, &FactSet::default()),
        ViewCoercionPolicy::BorrowStringView
    );
    assert_eq!(
        view_coercion_policy(&borrowed_str, &owned, &FactSet::default()),
        ViewCoercionPolicy::OwnStringFromView
    );
    assert_eq!(
        view_coercion_policy(&int_shape, &int_shape, &FactSet::default()),
        ViewCoercionPolicy::NoViewChange
    );
}

#[test]
fn view_coercion_policy_owns_sequence_slice_adapters() {
    let element_ty = JavaType::Named {
        name: "SimpleInterval".to_string(),
        args: Vec::new(),
    };
    let list_ty = JavaType::Named {
        name: "List".to_string(),
        args: vec![element_ty.clone()],
    };
    let array_ty = JavaType::Array(Box::new(element_ty));
    let actual = RustSlotShape::conservative(list_ty);
    let mut target = RustSlotShape::conservative(array_ty);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(plan.view, ViewCoercionPolicy::BorrowSequenceSliceView);
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
        vec![Coercion::AsSlice]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("intervals".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw("intervals.as_slice()".to_string())
    );
}

#[test]
fn set_to_borrowed_slice_collects_temporary_vec_view() {
    let element_ty = JavaType::Named {
        name: "SimpleInterval".to_string(),
        args: Vec::new(),
    };
    let set_ty = JavaType::Named {
        name: "Set".to_string(),
        args: vec![element_ty.clone()],
    };
    let array_ty = JavaType::Array(Box::new(element_ty));
    let actual = RustSlotShape::conservative(set_ty);
    let mut target = RustSlotShape::conservative(array_ty);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(plan.view, ViewCoercionPolicy::BorrowSequenceSliceView);
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Raw("self.lines".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw("&(self.lines).iter().cloned().collect::<Vec<_>>()".to_string())
    );
}

#[test]
fn optional_sequence_to_slice_uses_borrowed_unwrap_then_slice_view() {
    let element_ty = JavaType::Named {
        name: "SimpleInterval".to_string(),
        args: Vec::new(),
    };
    let list_ty = JavaType::Named {
        name: "List".to_string(),
        args: vec![element_ty.clone()],
    };
    let actual = RustSlotShape::conservative(optional_java_type(list_ty));
    let mut target = RustSlotShape::conservative(JavaType::Array(Box::new(element_ty)));
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
        vec![Coercion::UnwrapOptionBorrowed, Coercion::AsSlice]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Raw("self.intervals".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw(
            "self.intervals.as_ref().expect(\"Java null receiver\").as_slice()".to_string()
        )
    );
}

#[test]
fn shape_coercion_uses_type_capability_facts_for_string_views() {
    let external_name = JavaType::Named {
        name: "ExternalName".to_string(),
        args: Vec::new(),
    };
    let actual = RustSlotShape::conservative(external_name.clone());
    let mut target = RustSlotShape::conservative(external_name.clone());
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Str;
    let mut facts = FactSet::default();
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
    let effective_facts = EffectiveFacts::new(&facts);

    assert_eq!(
        coercion_content_policy_with_effective_facts(&external_name, &effective_facts),
        coercion_content_policy(&external_name, &facts)
    );
    assert_eq!(
        coercion_shape_content_policy_with_effective_facts(&actual, &effective_facts),
        coercion_shape_content_policy(&actual, &facts)
    );
    assert_eq!(
        view_coercion_policy_with_effective_facts(&actual, &target, &effective_facts),
        view_coercion_policy(&actual, &target, &facts)
    );

    assert_eq!(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("name".to_string()),
            actual,
            &Slot::Argument(target),
            &facts,
        ),
        RustExpr::Raw("name.as_str()".to_string())
    );
}

#[test]
fn shape_coercion_owns_closed_enum_upcast_and_downcast_adapters() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("dog".to_string()),
            actual,
            &Slot::Assignment(target.clone())
        ),
        RustExpr::Raw("AnimalValue::Dog(dog)".to_string())
    );

    let mut enum_actual = target;
    enum_actual.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let downcast_target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });

    assert_eq!(
        plan_shape_coercion(&enum_actual, &downcast_target),
        vec![Coercion::DowncastClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
}

#[test]
fn coercion_owns_closed_enum_instanceof_predicates() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });

    let check = plan_instanceof_check(&actual, &target);
    assert_eq!(
        check,
        InstanceOfCheck::ClosedEnumVariant {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("animal".to_string()), &check),
        RustExpr::Raw("matches!(animal, AnimalValue::Dog(_))".to_string())
    );
    assert_eq!(
        apply_instanceof_check(RustExpr::Var("animal".to_string()), &actual, &target),
        RustExpr::Raw("matches!(animal, AnimalValue::Dog(_))".to_string())
    );
}

#[test]
fn coercion_owns_dyn_boxed_dyn_and_erased_instanceof_predicates() {
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut dyn_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut boxed_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut erased_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_actual.dispatch = DispatchShape::ErasedObject;
    let mut optional_erased_actual =
        RustSlotShape::conservative(optional_java_type(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        }));
    optional_erased_actual.dispatch = DispatchShape::ErasedObject;

    let dyn_check = plan_instanceof_check(&dyn_actual, &target);
    let boxed_check = plan_instanceof_check(&boxed_actual, &target);
    let erased_check = plan_instanceof_check(&erased_actual, &target);
    let optional_erased_check = plan_instanceof_check(&optional_erased_actual, &target);

    assert_eq!(
        dyn_check,
        InstanceOfCheck::DynTraitType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("animal".to_string()), &dyn_check),
        RustExpr::Raw("animal.as_any().is::<Dog>()".to_string())
    );
    assert_eq!(
        boxed_check,
        InstanceOfCheck::BoxedDynTraitType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("animal".to_string()), &boxed_check),
        RustExpr::Raw("animal.as_ref().as_any().is::<Dog>()".to_string())
    );
    assert_eq!(
        erased_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("object".to_string()), &erased_check),
        RustExpr::Raw("object.is::<Dog>()".to_string())
    );
    assert_eq!(
        optional_erased_check,
        InstanceOfCheck::OptionalErasedObjectType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("object".to_string()), &optional_erased_check),
        RustExpr::Raw("object.as_ref().is_some_and(|value| value.is::<Dog>())".to_string())
    );
}

#[test]
fn instanceof_target_policy_names_supported_runtime_target_shapes() {
    let concrete = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    assert_eq!(
        instanceof_target_policy(&concrete, &FactSet::default()),
        InstanceOfTargetPolicy::ConcreteRuntimeTarget {
            target_type: "Dog".to_string(),
        }
    );

    let mut closed = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    closed.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    assert_eq!(
        instanceof_target_policy(&closed, &FactSet::default()),
        InstanceOfTargetPolicy::ClosedEnumTarget {
            enum_name: "AnimalValue".to_string(),
        }
    );

    let mut dyn_target = closed.clone();
    dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        instanceof_target_policy(&dyn_target, &FactSet::default()),
        InstanceOfTargetPolicy::DynTraitTarget {
            trait_name: "Animal".to_string(),
        }
    );

    let mut boxed_target = closed.clone();
    boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        instanceof_target_policy(&boxed_target, &FactSet::default()),
        InstanceOfTargetPolicy::BoxedDynTraitTarget {
            trait_name: "Animal".to_string(),
        }
    );

    let mut erased = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    erased.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        instanceof_target_policy(&erased, &FactSet::default()),
        InstanceOfTargetPolicy::ErasedObjectTarget
    );
}

#[test]
fn instanceof_rejects_primitive_raw_wildcard_and_missing_runtime_targets() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let primitive = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let primitive_check = plan_instanceof_check(&actual, &primitive);
    assert_eq!(
        primitive_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_instanceof_primitive_target",
            message: "unsupported Java instanceof check: primitive target type Primitive(Int)"
                .to_string(),
        }]
    );

    let raw_comparator = RustSlotShape::conservative(JavaType::Named {
        name: "Comparator".to_string(),
        args: Vec::new(),
    });
    let raw_check = plan_instanceof_check(&actual, &raw_comparator);
    assert_eq!(
        raw_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_instanceof_raw_or_wildcard_target",
            message:
                "unsupported Java instanceof check: raw or wildcard target type Named { name: \"Comparator\", args: [] }"
                    .to_string(),
        }]
    );

    let wildcard = RustSlotShape::conservative(JavaType::Named {
        name: "Comparator".to_string(),
        args: vec![JavaType::Wildcard(None)],
    });
    let wildcard_check = plan_instanceof_check(&actual, &wildcard);
    assert_eq!(
        wildcard_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_instanceof_raw_or_wildcard_target",
            message:
                "unsupported Java instanceof check: raw or wildcard target type Named { name: \"Comparator\", args: [Wildcard(None)] }"
                    .to_string(),
        }]
    );

    let array_target = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    let missing_check = plan_instanceof_check(&actual, &array_target);
    assert_eq!(
        missing_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_instanceof_missing_runtime_target",
            message:
                "unsupported Java instanceof check: target type Array(String) has no concrete runtime type"
                    .to_string(),
        }]
    );
}

#[test]
fn instanceof_allows_reifiable_raw_list_runtime_target() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::ErasedObject;
    let raw_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: Vec::new(),
    });
    let wildcard_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Wildcard(None)],
    });

    let raw_check = plan_instanceof_check(&actual, &raw_list);
    assert_eq!(
        raw_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Vec<Box<dyn std::any::Any>>".to_string(),
        }
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("value".to_string()), &raw_check),
        RustExpr::Raw("value.is::<Vec<Box<dyn std::any::Any>>>()".to_string())
    );

    let wildcard_check = plan_instanceof_check(&actual, &wildcard_list);
    assert_eq!(
        wildcard_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Vec<Box<dyn std::any::Any>>".to_string(),
        }
    );
}

#[test]
fn instanceof_uses_hierarchy_facts_for_interface_targets_with_closed_implementors() {
    let dog = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };
    let animal = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: vec![animal.clone()],
            extends: None,
            final_type: Some(true),
            object_safe_trait: None,
            provenance: FactProvenance::UserOverride,
        });

    let dog_shape = RustSlotShape::conservative(dog);
    let mut animal_closed_shape = RustSlotShape::conservative(animal.clone());
    animal_closed_shape.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    assert_eq!(
        hierarchy_instanceof_policy(&dog_shape, &animal_closed_shape, &facts).always_true,
        true
    );
    assert_eq!(
        plan_instanceof_check_with_facts(&dog_shape, &animal_closed_shape, &facts),
        InstanceOfCheck::AlwaysTrue
    );

    let mut animal_dyn_shape = RustSlotShape::conservative(animal);
    animal_dyn_shape.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_instanceof_check_with_facts(&dog_shape, &animal_dyn_shape, &facts),
        InstanceOfCheck::AlwaysTrue
    );
}

#[test]
fn coercion_owns_instanceof_fallback_policy() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };

    let check = plan_instanceof_check(&actual, &target);
    assert_eq!(
        check,
        InstanceOfCheck::Unsupported {
            actual_dispatch: DispatchShape::Concrete,
            target_dispatch: DispatchShape::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
            },
            policy: InstanceOfFallbackPolicy::CompileError,
        }
    );
    assert_eq!(
        check.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_instanceof_shape",
            message:
                "unsupported Java instanceof shape: actual=Concrete, target=ClosedEnum { enum_name: \"AnimalValue\" }"
                    .to_string(),
        }]
    );
    assert_eq!(
        check.unsupported_diagnostics(),
        vec![ConversionDiagnostic {
            code: "unsupported_instanceof_shape",
            message:
                "unsupported Java instanceof shape: actual=Concrete, target=ClosedEnum { enum_name: \"AnimalValue\" }"
                    .to_string(),
        }]
    );
}

#[test]
fn coercion_names_missing_runtime_targets_for_dispatch_downcasts() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let target = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));

    let policy = dispatch_coercion_policy(&actual, &target);

    assert_eq!(
        policy,
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::Concrete,
            policy: DispatchCoercionFallbackPolicy::MissingRuntimeTarget {
                java_ty: JavaType::Array(Box::new(JavaType::String)),
            },
        }
    );
    assert_eq!(
        policy.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_dispatch_missing_runtime_target",
            message:
                "unsupported Java dispatch coercion: target type Array(String) has no concrete runtime type"
                    .to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw(
            "{ let _ = animal; compile_error!(\"unsupported Java dispatch coercion: target type Array(String) has no concrete runtime type\") }"
                .to_string()
        )
    );
}

#[test]
fn instanceof_names_missing_runtime_targets() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let target = RustSlotShape::conservative(JavaType::Wildcard(None));

    let check = plan_instanceof_check(&actual, &target);

    assert_eq!(
        check,
        InstanceOfCheck::Unsupported {
            actual_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::Concrete,
            policy: InstanceOfFallbackPolicy::RawOrWildcardTarget {
                java_ty: JavaType::Wildcard(None),
            },
        }
    );
    assert_eq!(
        check.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_instanceof_raw_or_wildcard_target",
            message:
                "unsupported Java instanceof check: raw or wildcard target type Wildcard(None)"
                    .to_string(),
        }]
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("animal".to_string()), &check),
        RustExpr::Raw(
            "compile_error!(\"unsupported Java instanceof check: raw or wildcard target type Wildcard(None)\")"
                .to_string()
        )
    );
}

#[test]
fn unsupported_coercion_message_policy_feeds_diagnostics_and_compile_errors() {
    let instance_message = instanceof_fallback_message_policy(
        &InstanceOfFallbackPolicy::ErasedObjectToDynTrait {
            trait_name: "Animal".to_string(),
        },
        &DispatchShape::ErasedObject,
        &DispatchShape::DynTrait {
            trait_name: "Animal".to_string(),
        },
    );
    assert_eq!(
        instance_message.code,
        "unsupported_erased_object_instanceof_dyn_trait"
    );
    assert_eq!(
        instance_message.clone().into_diagnostic(),
        CoercionDiagnostic {
            code: "unsupported_erased_object_instanceof_dyn_trait",
            message: instance_message.message.clone(),
        }
    );
    assert_eq!(
        instance_message.compile_error_expr(),
        RustExpr::Raw(format!("compile_error!({:?})", instance_message.message))
    );
    let quoted_message = UnsupportedCoercionMessagePolicy {
        code: "quoted_message",
        message: "unsupported coercion for \"pkg\\Type\"".to_string(),
    };
    assert_eq!(
        quoted_message.compile_error_expr(),
        RustExpr::Raw(
            "compile_error!(\"unsupported coercion for \\\"pkg\\\\Type\\\"\")".to_string()
        )
    );

    let dispatch_message = dispatch_coercion_fallback_message_policy(
        &DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait {
            trait_name: "Animal".to_string(),
        },
        &DispatchShape::ErasedObject,
        &DispatchShape::BoxedDynTrait {
            trait_name: "Animal".to_string(),
        },
    );
    assert_eq!(
        dispatch_message.code,
        "unsupported_erased_object_to_boxed_dyn_trait"
    );
    assert_eq!(
        dispatch_message.compile_error_expr_consuming(&RustExpr::Var("object".to_string())),
        RustExpr::Raw(format!(
            "{{ let _ = object; compile_error!({:?}) }}",
            dispatch_message.message
        ))
    );

    let ownership_message = ownership_coercion_fallback_message_policy(
        &OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable {
            java_ty: JavaType::Named {
                name: "ExternalHandle".to_string(),
                args: Vec::new(),
            },
        },
        OwnershipShape::SharedBorrow,
        OwnershipShape::Owned,
    );
    assert_eq!(
        ownership_message.code,
        "unsupported_borrowed_to_owned_non_cloneable"
    );
    assert_eq!(
        lowering_diagnostic_from_conversion_diagnostic(ConversionDiagnostic {
            code: ownership_message.code,
            message: ownership_message.message.clone(),
        })
        .severity,
        Severity::Unsupported
    );
    assert_eq!(
        ownership_message.compile_error_expr_consuming(&RustExpr::Var("value".to_string())),
        RustExpr::Raw(format!(
            "{{ let _ = value; compile_error!({:?}) }}",
            ownership_message.message
        ))
    );

    let element_shape = RustSlotShape::conservative(JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    });
    let element_message = borrowed_element_recovery_fallback_message_policy(
        &BorrowedElementRecoveryFallbackPolicy::NonCloneableElement {
            java_ty: element_shape.java_ty.clone(),
        },
        &element_shape,
    );
    assert_eq!(
        element_message.code,
        "unsupported_borrowed_element_non_cloneable"
    );
    assert_eq!(
        element_message.compile_error_expr_consuming(&RustExpr::Var("item".to_string())),
        RustExpr::Raw(format!(
            "{{ let _ = item; compile_error!({:?}) }}",
            element_message.message
        ))
    );
}

#[test]
fn dynamic_compile_error_helpers_escape_diagnostic_text() {
    let message = "unsupported \"quoted\" Java path \\pkg\\Type with {braces} and \"#";
    assert_eq!(
        unsupported::compile_error_expr(message),
        RustExpr::Raw(format!("compile_error!({message:?})"))
    );
    assert_eq!(
        unsupported::compile_error_stmt(message),
        new_java2rust::rust_ir::RustStmt::Raw(format!("compile_error!({message:?});"))
    );
    assert_eq!(
        unsupported::compile_error_expr_consuming(&RustExpr::Var("value".to_string()), message),
        RustExpr::Raw(format!("{{ let _ = value; compile_error!({message:?}) }}"))
    );
}

#[test]
fn instanceof_policy_names_erased_object_interface_target_fallbacks() {
    let mut actual = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    actual.dispatch = DispatchShape::ErasedObject;
    let mut dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut boxed_target = dyn_target.clone();
    boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };

    let dyn_check = plan_instanceof_check(&actual, &dyn_target);
    let boxed_check = plan_instanceof_check(&actual, &boxed_target);

    assert_eq!(
        dyn_check,
        InstanceOfCheck::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: InstanceOfFallbackPolicy::ErasedObjectToDynTrait {
                trait_name: "Animal".to_string(),
            },
        }
    );
    assert_eq!(
        dyn_check.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_erased_object_instanceof_dyn_trait",
            message:
                "unsupported Java erased object instanceof borrowed dyn trait check for Animal; a concrete runtime target is required"
                    .to_string(),
        }]
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("object".to_string()), &dyn_check),
        RustExpr::Raw(
            "compile_error!(\"unsupported Java erased object instanceof borrowed dyn trait check for Animal; a concrete runtime target is required\")"
                .to_string()
        )
    );

    assert_eq!(
        boxed_check,
        InstanceOfCheck::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
        }
    );
    assert_eq!(
        boxed_check.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_erased_object_instanceof_boxed_dyn_trait",
            message:
                "unsupported Java erased object instanceof owned boxed dyn trait check for Animal; a concrete runtime target is required"
                    .to_string(),
        }]
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("object".to_string()), &boxed_check),
        RustExpr::Raw(
            "compile_error!(\"unsupported Java erased object instanceof owned boxed dyn trait check for Animal; a concrete runtime target is required\")"
                .to_string()
        )
    );
}

#[test]
fn dispatch_coercion_does_not_downcast_same_typed_native_values() {
    let mut dyn_int = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    dyn_int.dispatch = DispatchShape::DynTrait {
        trait_name: "ExternalGetterResult".to_string(),
    };
    let int_target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));

    assert_eq!(
        dispatch_coercion_policy(&dyn_int, &int_target),
        DispatchCoercionPolicy::NoDispatchChange
    );
    dyn_int.java_ty = JavaType::Unknown("external_getter".to_string());
    assert_eq!(
        dispatch_coercion_policy(&dyn_int, &int_target),
        DispatchCoercionPolicy::NoDispatchChange
    );

    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)));
    let mut erased_array = RustSlotShape::conservative(array_ty.clone());
    erased_array.dispatch = DispatchShape::ErasedObject;
    let array_target = RustSlotShape::conservative(array_ty);

    assert_eq!(
        dispatch_coercion_policy(&erased_array, &array_target),
        DispatchCoercionPolicy::NoDispatchChange
    );
    erased_array.java_ty = JavaType::Unknown("external_byte_array_getter".to_string());
    assert_eq!(
        dispatch_coercion_policy(&erased_array, &array_target),
        DispatchCoercionPolicy::NoDispatchChange
    );
}

#[test]
fn dispatch_coercion_policy_owns_wrapping_trait_downcast_and_unsupported_cases() {
    let dog = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut animal_enum = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    animal_enum.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let mut animal_boxed = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    animal_boxed.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut animal_dyn = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    animal_dyn.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };

    assert_eq!(
        dispatch_coercion_policy(&dog, &animal_enum),
        DispatchCoercionPolicy::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }
    );
    assert_eq!(
        dispatch_coercion_policy(&dog, &animal_boxed),
        DispatchCoercionPolicy::BoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        dispatch_coercion_policy(&animal_dyn, &dog),
        DispatchCoercionPolicy::DowncastDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }
    );
    assert_eq!(
        dispatch_coercion_policy(&animal_boxed, &dog),
        DispatchCoercionPolicy::DowncastBoxedDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }
    );
    let unsupported = dispatch_coercion_policy(&animal_enum, &animal_dyn);
    assert_eq!(
        unsupported,
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
            },
            target_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::CompileError,
        }
    );
    assert_eq!(
        unsupported.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_dispatch_coercion",
            message:
                "unsupported Java dispatch coercion: actual=ClosedEnum { enum_name: \"AnimalValue\" }, target=DynTrait { trait_name: \"Animal\" }"
                    .to_string(),
        }]
    );
}

#[test]
fn shape_coercion_owns_dyn_trait_and_erased_object_adapters() {
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&actual, &dyn_target),
        vec![Coercion::BorrowDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("dog".to_string()),
            actual.clone(),
            &Slot::Argument(dyn_target)
        ),
        RustExpr::Raw("&dog as &dyn Animal".to_string())
    );

    let mut boxed_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion(&actual, &boxed_target),
        vec![Coercion::BoxDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );

    let mut erased_target = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_target.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        plan_shape_coercion(&actual, &erased_target),
        vec![Coercion::EraseObject]
    );

    let mut dyn_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let downcast_target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    assert_eq!(
        plan_shape_coercion(&dyn_actual, &downcast_target),
        vec![Coercion::DowncastDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );

    let mut boxed_downcast_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_downcast_actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion(&boxed_downcast_actual, &downcast_target),
        vec![Coercion::DowncastBoxedDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            boxed_downcast_actual.clone(),
            &Slot::Argument(downcast_target.clone())
        ),
        RustExpr::Raw(
            "animal.as_ref().as_any().downcast_ref::<Dog>().expect(\"invalid Java cast\")"
                .to_string()
        )
    );

    let mut erased_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_actual.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        plan_shape_coercion(&erased_actual, &downcast_target),
        vec![Coercion::DowncastErasedObject {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("object".to_string()),
            erased_actual,
            &Slot::Argument(downcast_target)
        ),
        RustExpr::Raw("*object.downcast::<Dog>().expect(\"invalid Java cast\")".to_string())
    );

    let mut boxed_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut borrowed_dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    borrowed_dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion(&boxed_actual, &borrowed_dyn_target),
        vec![Coercion::BorrowBoxedDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            boxed_actual,
            &Slot::Argument(borrowed_dyn_target)
        ),
        RustExpr::Raw("animal.as_ref() as &dyn Animal".to_string())
    );
    assert_eq!(
        apply_coercions(
            RustExpr::Var("writer".to_string()),
            &[
                Coercion::BoxDynTrait {
                    trait_name: "Animal".to_string(),
                },
                Coercion::CloneRecovery {
                    policy: CloneRecoveryPolicy::AuditRequired,
                },
            ],
        ),
        RustExpr::Raw(
            "(Box::new(writer) as Box<dyn Animal>).clone()/* TODO(translation): validate added clone */"
                .to_string()
        )
    );

    let mut borrowed_dyn_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    borrowed_dyn_actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut owned_boxed_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    owned_boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let borrowed_dyn_to_owned_box =
        dispatch_coercion_policy(&borrowed_dyn_actual, &owned_boxed_target);
    assert_eq!(
        borrowed_dyn_to_owned_box,
        DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_requirement(
            "Animal",
            &BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
        ),
        DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter {
            trait_name: "Animal".to_string(),
            policy: CloneRecoveryPolicy::AuditRequired,
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_generation_policy(
            &DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter {
                trait_name: "Animal".to_string(),
                policy: CloneRecoveryPolicy::AuditRequired,
            }
        ),
        DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported {
            trait_name: "Animal".to_string(),
            reason: DynCloneBoxAdapterDeferralReason::GenerationNotImplemented {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_requirement_with_availability(
            "Animal",
            &BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
            DynCloneBoxAdapterAvailability::Available,
        ),
        DynCloneBoxAdapterRequirement::ProvenAdapter {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(borrowed_dyn_to_owned_box.diagnostics(), Vec::new());
    assert_eq!(
        plan_shape_coercion(&borrowed_dyn_actual, &owned_boxed_target),
        vec![Coercion::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            borrowed_dyn_actual,
            &Slot::Argument(owned_boxed_target)
        ),
        RustExpr::Raw("animal.clone_box()".to_string())
    );
}

#[test]
fn dispatch_coercion_uses_clone_facts_for_borrowed_dyn_to_boxed_dyn_fallback() {
    let animal = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let mut borrowed_dyn_actual = RustSlotShape::conservative(animal.clone());
    borrowed_dyn_actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut owned_boxed_target = RustSlotShape::conservative(animal.clone());
    owned_boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Animal".to_string(),
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

    let policy =
        dispatch_coercion_policy_with_facts(&borrowed_dyn_actual, &owned_boxed_target, &facts);

    assert_eq!(
        policy,
        DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_requirement(
            "Animal",
            &BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: animal.clone(),
                },
            },
        ),
        DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter {
            trait_name: "Animal".to_string(),
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: animal.clone(),
            },
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_generation_policy_with_availability(
            "Animal",
            &BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: animal.clone(),
                },
            },
            DynCloneBoxAdapterAvailability::Available,
        ),
        DynCloneBoxAdapterGenerationPolicy::AlreadyAvailable {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        dyn_clone_box_adapter_generation_policy(
            &DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter {
                trait_name: "Animal".to_string(),
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: animal.clone(),
                },
            }
        ),
        DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported {
            trait_name: "Animal".to_string(),
            reason: DynCloneBoxAdapterDeferralReason::GenerationNotImplemented {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: animal.clone(),
                },
            },
        }
    );
    assert_eq!(policy.diagnostics(), Vec::new());
    assert_eq!(
        plan_shape_coercion_with_facts(&borrowed_dyn_actual, &owned_boxed_target, &facts),
        vec![Coercion::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );

    let mut non_clone_facts = FactSet::default();
    non_clone_facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Animal".to_string(),
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
        coercion_owned_recovery_capability_policy(
            &JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            },
            &non_clone_facts,
        ),
        CoercionOwnedRecoveryCapabilityPolicy {
            java_ty: JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            },
            recovery: OwnedRecoveryCapability::NonCloneable,
        }
    );
    assert_eq!(
        dispatch_coercion_policy_with_facts(
            &borrowed_dyn_actual,
            &owned_boxed_target,
            &non_clone_facts
        )
        .diagnostics(),
        Vec::new()
    );
    assert_eq!(
        dyn_clone_box_adapter_requirement(
            "Animal",
            &BorrowedDynToOwnedBoxedDynRecoveryPolicy::NoOwnershipProof,
        ),
        DynCloneBoxAdapterRequirement::UnsupportedNonCloneable {
            trait_name: "Animal".to_string(),
        }
    );
}

#[test]
fn borrowed_dyn_recovery_uses_clone_box_capability_fact_once() {
    let animal = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let mut borrowed_dyn_actual = RustSlotShape::conservative(animal.clone());
    borrowed_dyn_actual.ownership = OwnershipShape::SharedBorrow;
    borrowed_dyn_actual.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut owned_boxed_target = RustSlotShape::conservative(animal.clone());
    owned_boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut facts = FactSet::default();
    facts
        .owned_recovery_capability_facts
        .push(OwnedRecoveryCapabilityFact {
            type_name: "Animal".to_string(),
            capability: OwnedRecoveryCapability::CloneBoxAdapter {
                java_ty: animal.clone(),
            },
            provenance: FactProvenance::ExternalLibraryPack,
        });

    assert_eq!(
        dispatch_coercion_policy_with_facts(&borrowed_dyn_actual, &owned_boxed_target, &facts),
        DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
}

#[test]
fn shape_coercion_owns_unsupported_dispatch_fallback_policy() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&actual, &target),
        vec![Coercion::UnsupportedDispatch {
            actual_dispatch: DispatchShape::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
            },
            target_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::CompileError,
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            actual,
            &Slot::Argument(target)
        ),
        RustExpr::Raw(format!(
            "{{ let _ = animal; compile_error!({:?}) }}",
            "unsupported Java dispatch coercion: actual=ClosedEnum { enum_name: \"AnimalValue\" }, target=DynTrait { trait_name: \"Animal\" }"
        ))
    );
}

#[test]
fn erased_object_runtime_contract_covers_carrier_downcast_instanceof_and_recovery() {
    let contract = erased_object_runtime_contract("Animal");
    assert_eq!(contract.rust_type, "Box<dyn std::any::Any>");
    assert_eq!(
        contract.erase_operation,
        "Box::new(value) as Box<dyn std::any::Any>"
    );
    assert_eq!(
        contract.concrete_downcast_operation,
        "*value.downcast::<Target>().expect(\"invalid Java cast\")"
    );
    assert_eq!(
        contract.concrete_instanceof_operation,
        "value.is::<Target>()"
    );

    let mut erased = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    erased.dispatch = DispatchShape::ErasedObject;
    let mut erased_object_decl = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_object_decl.dispatch = DispatchShape::ErasedObject;
    let concrete = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let mut boxed_dyn_target = dyn_target.clone();
    boxed_dyn_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };

    assert_eq!(
        plan_shape_coercion(&concrete, &erased),
        vec![Coercion::EraseObject]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("dog".to_string()),
            concrete.clone(),
            &Slot::Argument(erased.clone()),
        ),
        RustExpr::Raw("Box::new(dog) as Box<dyn std::any::Any>".to_string())
    );
    assert_eq!(
        plan_shape_coercion(&erased, &concrete),
        vec![Coercion::DowncastErasedObject {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    assert_eq!(
        render_instanceof_check(
            RustExpr::Var("object".to_string()),
            &plan_instanceof_check(&erased, &concrete),
        ),
        RustExpr::Raw("object.is::<Dog>()".to_string())
    );
    assert_eq!(
        dispatch_coercion_policy(&erased, &dyn_target),
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: contract.dyn_trait_recovery,
        }
    );
    assert_eq!(
        dispatch_coercion_policy(&erased, &boxed_dyn_target),
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: contract.boxed_dyn_trait_recovery,
        }
    );

    for slot in [
        Slot::Assignment(erased_object_decl.clone()),
        Slot::Argument(erased_object_decl.clone()),
        Slot::Return(erased_object_decl.clone()),
    ] {
        assert_eq!(
            slot_conversion_plan(&erased, &slot),
            SlotConversionPlan::Shape(ShapeCoercionPlan {
                actual: erased.clone(),
                target: erased_object_decl.clone(),
                dispatch: DispatchCoercionPolicy::NoDispatchChange,
                view: ViewCoercionPolicy::NoViewChange,
                optional: OptionalShapeCoercionPolicy::NoChange,
                ownership: OwnershipCoercionPolicy::NoOwnershipChange,
                primitive_widening: None,
            })
        );
        assert_eq!(
            apply_expr_to_slot(RustExpr::Var("object".to_string()), erased.clone(), &slot),
            RustExpr::Var("object".to_string())
        );
    }
}

#[test]
fn dispatch_coercion_names_erased_object_trait_recovery_fallbacks() {
    let mut erased_actual = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    erased_actual.dispatch = DispatchShape::ErasedObject;

    let mut borrowed_dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    borrowed_dyn_target.dispatch = DispatchShape::DynTrait {
        trait_name: "Animal".to_string(),
    };
    let erased_to_borrowed_dyn = dispatch_coercion_policy(&erased_actual, &borrowed_dyn_target);

    assert_eq!(
        erased_to_borrowed_dyn,
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait {
                trait_name: "Animal".to_string(),
            },
        }
    );
    assert_eq!(
        erased_to_borrowed_dyn.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_erased_object_to_dyn_trait",
            message:
                "unsupported Java erased object to borrowed dyn trait recovery for Animal; a concrete downcast target is required"
                    .to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("object".to_string()),
            erased_actual.clone(),
            &Slot::Argument(borrowed_dyn_target)
        ),
        RustExpr::Raw(
            "{ let _ = object; compile_error!(\"unsupported Java erased object to borrowed dyn trait recovery for Animal; a concrete downcast target is required\") }"
                .to_string()
        )
    );

    let mut boxed_dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_dyn_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let erased_to_boxed_dyn = dispatch_coercion_policy(&erased_actual, &boxed_dyn_target);

    assert_eq!(
        erased_to_boxed_dyn,
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::ErasedObject,
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
        }
    );
    assert_eq!(
        erased_to_boxed_dyn.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_erased_object_to_boxed_dyn_trait",
            message:
                "unsupported Java erased object to owned boxed dyn trait recovery for Animal; a concrete downcast target and ownership proof are required"
                    .to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("object".to_string()),
            erased_actual,
            &Slot::Argument(boxed_dyn_target)
        ),
        RustExpr::Raw(
            "{ let _ = object; compile_error!(\"unsupported Java erased object to owned boxed dyn trait recovery for Animal; a concrete downcast target and ownership proof are required\") }"
                .to_string()
        )
    );
}

#[test]
fn coercion_owns_borrowed_optional_to_owned_optional_slot() {
    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(JavaType::String),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::String,
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("values.iter().next()".to_string())),
            method: "cloned".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn borrowed_element_target_match_policies_name_carrier_and_element_matches() {
    let string_element = RustSlotShape::conservative(JavaType::String);
    let option_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let option_target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let vec_shape = RustSlotShape::conservative(list_java_type(JavaType::String));
    let vec_target = RustSlotShape::conservative(list_java_type(JavaType::String));
    let int_option_target =
        RustSlotShape::conservative(optional_java_type(JavaType::Primitive(PrimitiveType::Int)));
    let int_vec_target =
        RustSlotShape::conservative(list_java_type(JavaType::Primitive(PrimitiveType::Int)));

    assert_eq!(
        optional_element_target_match_policy(&option_target, &option_shape, &string_element),
        OptionalElementTargetMatchPolicy {
            target: option_target.clone(),
            option_shape: option_shape.clone(),
            element_shape: string_element.clone(),
            matches: true,
        }
    );
    assert_eq!(
        optional_element_target_match_policy(&int_option_target, &option_shape, &string_element),
        OptionalElementTargetMatchPolicy {
            target: int_option_target,
            option_shape,
            element_shape: string_element.clone(),
            matches: false,
        }
    );
    assert_eq!(
        vec_element_target_match_policy(&vec_target, &vec_shape, &string_element),
        VecElementTargetMatchPolicy {
            target: vec_target.clone(),
            vec_shape: vec_shape.clone(),
            element_shape: string_element.clone(),
            matches: true,
        }
    );
    assert_eq!(
        vec_element_target_match_policy(&int_vec_target, &vec_shape, &string_element),
        VecElementTargetMatchPolicy {
            target: int_vec_target,
            vec_shape,
            element_shape: string_element,
            matches: false,
        }
    );
}

#[test]
fn coercion_uses_copy_capability_for_borrowed_optional_elements() {
    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(JavaType::Primitive(PrimitiveType::Int)),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::Primitive(PrimitiveType::Int),
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("values.iter().next()".to_string())),
            method: "copied".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn coercion_uses_type_capability_facts_for_borrowed_optional_elements() {
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
    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(external_id.clone()),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(external_id))),
        &facts,
    );

    assert_eq!(
        expr,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("values.iter().next()".to_string())),
            method: "copied".to_string(),
            args: Vec::new(),
        }
    );
}

#[test]
fn borrowed_element_recovery_policy_owns_string_copy_clone_and_unsupported_cases() {
    let mut string_view = RustSlotShape::conservative(JavaType::String);
    string_view.ownership = OwnershipShape::SharedBorrow;
    string_view.view = ViewShape::Str;
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let cloneable = JavaType::Named {
        name: "CloneableId".to_string(),
        args: Vec::new(),
    };
    let handle = JavaType::Named {
        name: "ExternalHandle".to_string(),
        args: Vec::new(),
    };
    let cloneable_shape = RustSlotShape::conservative(cloneable.clone());
    let handle_shape = RustSlotShape::conservative(handle.clone());
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "CloneableId".to_string(),
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
        borrowed_element_recovery_policy(&string_view, &facts),
        BorrowedElementRecoveryPolicy::StringViewToOwned
    );
    let effective_facts = EffectiveFacts::new(&facts);
    assert_eq!(
        borrowed_element_recovery_policy_with_effective_facts(&string_view, &effective_facts),
        borrowed_element_recovery_policy(&string_view, &facts)
    );
    assert_eq!(
        borrowed_element_recovery_policy(&int_shape, &facts),
        BorrowedElementRecoveryPolicy::CopyElements
    );
    assert_eq!(
        borrowed_element_recovery_policy(&cloneable_shape, &facts),
        BorrowedElementRecoveryPolicy::CloneElements {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Named {
                    name: "CloneableId".to_string(),
                    args: Vec::new(),
                },
            },
        }
    );
    assert!(
        borrowed_element_recovery_policy(&cloneable_shape, &facts)
            .diagnostics(&cloneable_shape)
            .is_empty()
    );
    let unsupported = borrowed_element_recovery_policy(&handle_shape, &facts);
    assert_eq!(
        unsupported,
        BorrowedElementRecoveryPolicy::Unsupported {
            policy: BorrowedElementRecoveryFallbackPolicy::NonCloneableElement {
                java_ty: JavaType::Named {
                    name: "ExternalHandle".to_string(),
                    args: Vec::new(),
                },
            },
        }
    );
    assert_eq!(
        unsupported.diagnostics(&handle_shape),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_element_non_cloneable",
            message:
                "unsupported Java borrowed element ownership recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
                    .to_string(),
        }]
    );
    assert_eq!(
        unsupported.unsupported_diagnostics(&handle_shape),
        vec![ConversionDiagnostic {
            code: "unsupported_borrowed_element_non_cloneable",
            message:
                "unsupported Java borrowed element ownership recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
                    .to_string(),
        }]
    );
}

#[test]
fn borrowed_element_clone_recovery_audits_conservative_clone_capability() {
    let element_ty = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let element_shape = RustSlotShape::conservative(element_ty.clone());
    let policy = borrowed_element_recovery_policy(&element_shape, &FactSet::default());

    assert_eq!(
        policy,
        BorrowedElementRecoveryPolicy::CloneElements {
            policy: CloneRecoveryPolicy::AuditRequired,
        }
    );
    assert_eq!(
        policy.diagnostics(&element_shape),
        vec![CoercionDiagnostic {
            code: "borrowed_element_clone_audit_required",
            message:
                "borrowed Java element recovery inserted cloned() and requires ownership audit"
                    .to_string(),
        }]
    );
    assert_eq!(
        apply_lowered_expr_to_slot(
            LoweredExpr {
                expr: RustExpr::Raw("values.iter().next()".to_string()),
                actual: LoweredType::optional_element_borrow(element_ty.clone()),
            },
            &Slot::Return(RustSlotShape::conservative(optional_java_type(element_ty))),
        ),
        RustExpr::Raw(
            "values.iter().next().cloned()/* TODO(translation): validate added clone */"
                .to_string()
        )
    );
}

#[test]
fn borrowed_sequence_ownership_recovery_uses_to_vec() {
    let java_array = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Double)));
    let mut actual = RustSlotShape::conservative(java_array.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(java_array);

    let plan = shape_coercion_plan(&actual, &target);

    assert!(matches!(
        plan.ownership,
        OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { .. }
    ));
    assert_eq!(
        plan.coercions(),
        vec![Coercion::ToOwnedSequence {
            policy: CloneRecoveryPolicy::AuditRequired,
        }]
    );
    assert_eq!(
        apply_lowered_expr_to_slot(
            LoweredExpr {
                expr: RustExpr::Raw("Table::VALUES[index as usize]".to_string()),
                actual: LoweredType::Slot(Box::new(actual)),
            },
            &Slot::Return(target),
        ),
        RustExpr::Raw(
            "(Table::VALUES[index as usize]).to_vec()/* TODO(translation): validate added clone */"
                .to_string()
        )
    );
}

#[test]
fn borrowed_element_recovery_uses_richer_owned_recovery_facts() {
    let immutable = JavaType::Named {
        name: "ImmutableSharedHandle".to_string(),
        args: Vec::new(),
    };
    let factory = JavaType::Named {
        name: "FactoryCopyHandle".to_string(),
        args: Vec::new(),
    };
    let facade = JavaType::Named {
        name: "FacadeHandle".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    for (type_name, capability) in [
        (
            "ImmutableSharedHandle",
            OwnedRecoveryCapability::ImmutableSharedHandle {
                java_ty: immutable.clone(),
            },
        ),
        (
            "FactoryCopyHandle",
            OwnedRecoveryCapability::FactoryCopy {
                java_ty: factory.clone(),
            },
        ),
        (
            "FacadeHandle",
            OwnedRecoveryCapability::FacadeOwnedExternalHandle {
                java_ty: facade.clone(),
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

    assert_eq!(
        borrowed_element_recovery_policy(&RustSlotShape::conservative(immutable), &facts),
        BorrowedElementRecoveryPolicy::CopyElements
    );
    assert_eq!(
        borrowed_element_recovery_policy(&RustSlotShape::conservative(factory.clone()), &facts),
        BorrowedElementRecoveryPolicy::CloneElements {
            policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty: factory },
        }
    );
    assert_eq!(
        borrowed_element_recovery_policy(&RustSlotShape::conservative(facade.clone()), &facts),
        BorrowedElementRecoveryPolicy::Unsupported {
            policy: BorrowedElementRecoveryFallbackPolicy::NonCloneableElement { java_ty: facade },
        }
    );
}

#[test]
fn borrowed_element_adapter_policy_shares_recovery_across_option_and_vec_carriers() {
    let mut string_view = RustSlotShape::conservative(JavaType::String);
    string_view.ownership = OwnershipShape::SharedBorrow;
    string_view.view = ViewShape::Str;
    let int_element = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let external_value = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let external_element = RustSlotShape::conservative(external_value);
    let facts = FactSet::default();

    assert_eq!(
        borrowed_element_adapter_policy(BorrowedElementCarrier::Option, &string_view, &facts),
        BorrowedElementAdapterPolicy {
            carrier: BorrowedElementCarrier::Option,
            element_shape: string_view.clone(),
            recovery: BorrowedElementRecoveryPolicy::StringViewToOwned,
            unwrap_carrier: false,
        }
    );
    assert_eq!(
        borrowed_element_adapter_policy(BorrowedElementCarrier::Vec, &string_view, &facts),
        BorrowedElementAdapterPolicy {
            carrier: BorrowedElementCarrier::Vec,
            element_shape: string_view,
            recovery: BorrowedElementRecoveryPolicy::StringViewToOwned,
            unwrap_carrier: false,
        }
    );
    assert_eq!(
        borrowed_element_adapter_policy(BorrowedElementCarrier::Option, &int_element, &facts)
            .recovery,
        BorrowedElementRecoveryPolicy::CopyElements
    );
    assert_eq!(
        borrowed_element_adapter_policy(BorrowedElementCarrier::Vec, &external_element, &facts)
            .recovery,
        BorrowedElementRecoveryPolicy::CloneElements {
            policy: CloneRecoveryPolicy::AuditRequired,
        }
    );
}

#[test]
fn borrowed_element_slot_adapter_policy_combines_carrier_match_and_recovery() {
    let string_element = RustSlotShape::conservative(JavaType::String);
    let option_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let option_target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let vec_shape = RustSlotShape::conservative(list_java_type(JavaType::String));
    let vec_target = RustSlotShape::conservative(list_java_type(JavaType::String));
    let set_shape = RustSlotShape::conservative(set_java_type(JavaType::String));
    let set_target = RustSlotShape::conservative(set_java_type(JavaType::String));
    let facts = FactSet::default();

    let option_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Option,
        &option_target,
        &option_shape,
        &string_element,
        &facts,
    );
    assert_eq!(
        option_policy,
        BorrowedElementSlotAdapterPolicy {
            target: option_target.clone(),
            carrier_shape: option_shape,
            element_shape: string_element.clone(),
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: None,
        }
    );

    let concrete_option_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Option,
        &string_element,
        &RustSlotShape::conservative(optional_java_type(JavaType::String)),
        &string_element,
        &facts,
    );
    assert_eq!(concrete_option_policy.matches, true);
    assert_eq!(
        concrete_option_policy
            .adapter
            .as_ref()
            .map(|adapter| adapter.unwrap_carrier),
        Some(true)
    );

    let vec_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Vec,
        &vec_target,
        &vec_shape,
        &string_element,
        &facts,
    );
    assert_eq!(vec_policy.matches, true);
    assert_eq!(
        vec_policy.adapter.as_ref().unwrap().carrier,
        BorrowedElementCarrier::Vec
    );

    let set_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Set,
        &set_target,
        &set_shape,
        &string_element,
        &facts,
    );
    assert_eq!(set_policy.matches, true);
    assert_eq!(
        set_policy.adapter.as_ref().unwrap().carrier,
        BorrowedElementCarrier::Set
    );

    let mismatch = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Vec,
        &option_target,
        &vec_shape,
        &string_element,
        &facts,
    );
    assert_eq!(mismatch.matches, false);
    assert_eq!(mismatch.adapter, None);
}

#[test]
fn optional_borrowed_element_adapter_unwraps_to_concrete_copy_target() {
    let int_ty = JavaType::Primitive(PrimitiveType::Int);
    let target = RustSlotShape::conservative(int_ty.clone());
    let integer_ty = JavaType::Named {
        name: "Integer".to_string(),
        args: Vec::new(),
    };
    let option_shape = RustSlotShape::conservative(optional_java_type(integer_ty.clone()));
    let mut element_shape = RustSlotShape::conservative(integer_ty);
    element_shape.ownership = OwnershipShape::SharedBorrow;

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape.clone()),
            element_shape: Box::new(element_shape.clone()),
            recovery: None,
        },
        &Slot::Assignment(target.clone()),
        &FactSet::default(),
    );

    assert_eq!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape: option_shape,
            element_shape: element_shape.clone(),
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: Some(BorrowedElementAdapterPolicy {
                carrier: BorrowedElementCarrier::Option,
                element_shape,
                recovery: BorrowedElementRecoveryPolicy::CopyElements,
                unwrap_carrier: true,
            }),
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("values.get(&key)".to_string()), &plan),
        RustExpr::Raw("values.get(&key).copied().expect(\"Java null receiver\")".to_string())
    );
}

#[test]
fn optional_borrowed_concrete_element_unwraps_to_dyn_trait_target() {
    let simple_interval = JavaType::Named {
        name: "SimpleInterval".to_string(),
        args: Vec::new(),
    };
    let locatable = JavaType::Named {
        name: "Locatable".to_string(),
        args: Vec::new(),
    };
    let mut element_shape = RustSlotShape::conservative(simple_interval.clone());
    element_shape.ownership = OwnershipShape::SharedBorrow;
    let option_shape = RustSlotShape::conservative(optional_java_type(simple_interval));
    let mut target = RustSlotShape::conservative(locatable);
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::DynTraitView;
    target.dispatch = DispatchShape::DynTrait {
        trait_name: "Locatable".to_string(),
    };
    let expected_element_shape = element_shape.clone();
    let facts = simple_interval_locatable_facts();

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape.clone()),
            element_shape: Box::new(element_shape.clone()),
            recovery: None,
        },
        &Slot::Argument(target.clone()),
        &facts,
    );

    assert_eq!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape: option_shape,
            element_shape,
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: Some(BorrowedElementAdapterPolicy {
                carrier: BorrowedElementCarrier::Option,
                element_shape: expected_element_shape,
                recovery: BorrowedElementRecoveryPolicy::BorrowElement,
                unwrap_carrier: true,
            }),
        })
    );
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("intervals.get(0)".to_string()), &plan),
        RustExpr::Raw(
            "intervals.get(0).expect(\"Java null receiver\") as &dyn Locatable".to_string()
        )
    );
}

#[test]
fn optional_borrowed_concrete_element_unwraps_to_concrete_facade_target() {
    let simple_interval = JavaType::Named {
        name: "SimpleInterval".to_string(),
        args: Vec::new(),
    };
    let locatable = JavaType::Named {
        name: "Locatable".to_string(),
        args: Vec::new(),
    };
    let mut element_shape = RustSlotShape::conservative(simple_interval.clone());
    element_shape.ownership = OwnershipShape::SharedBorrow;
    let option_shape = RustSlotShape::conservative(optional_java_type(simple_interval));
    let mut target = RustSlotShape::conservative(locatable);
    target.ownership = OwnershipShape::SharedBorrow;
    let facts = simple_interval_locatable_facts();

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        },
        &Slot::Argument(target),
        &facts,
    );

    assert!(matches!(
        plan,
        SlotConversionPlan::BorrowedElement(BorrowedElementSlotAdapterPolicy {
            matches: true,
            adapter: Some(BorrowedElementAdapterPolicy {
                recovery: BorrowedElementRecoveryPolicy::BorrowElement,
                unwrap_carrier: true,
                ..
            }),
            ..
        })
    ));
    assert_eq!(
        apply_slot_conversion_plan(RustExpr::Raw("intervals.get(0)".to_string()), &plan),
        RustExpr::Raw(
            "{ let _java_value = intervals.get(0).expect(\"Java null receiver\"); &Locatable }"
                .to_string()
        )
    );
}

#[test]
fn coercion_rejects_non_cloneable_borrowed_optional_elements() {
    let external_handle = JavaType::Named {
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
    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr {
            expr: RustExpr::Raw("handles.iter().next()".to_string()),
            actual: LoweredType::optional_element_borrow(external_handle.clone()),
        },
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            external_handle,
        ))),
        &facts,
    );

    assert!(matches!(
        expr,
        RustExpr::Raw(ref raw)
            if raw.contains(
                "unsupported Java borrowed element ownership recovery for non-cloneable type"
            ) && raw.contains("ExternalHandle")
    ));
}

#[test]
fn coercion_uses_element_shape_for_borrowed_vec_string_view() {
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;
    element_shape.view = ViewShape::Str;

    let expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().map(|v| v.trim()).collect::<Vec<_>>()".to_string()),
            actual: LoweredType::vec_element_borrow(element_shape),
        },
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
    );

    assert_eq!(
        expr,
        RustExpr::Raw(
            "values.iter().map(|v| v.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
                .to_string()
        )
    );
}

#[test]
fn coercion_fuses_borrowed_collection_clone_recovery_into_terminal_collect() {
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;

    let vec_expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw("values.iter().collect::<Vec<_>>()".to_string()),
            actual: LoweredType::vec_element_borrow(element_shape.clone()),
        },
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
    );
    assert_eq!(
        vec_expr,
        RustExpr::Raw("values.iter().cloned().collect::<Vec<_>>()".to_string())
    );

    let set_expr = apply_lowered_expr_to_slot(
        LoweredExpr {
            expr: RustExpr::Raw(
                "groups.iter().flat_map(|item| item.iter()).collect::<std::collections::HashSet<_>>()"
                    .to_string(),
            ),
            actual: LoweredType::set_element_borrow(element_shape),
        },
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
    );
    assert_eq!(
        set_expr,
        RustExpr::Raw(
            "groups.iter().flat_map(|item| item.iter()).cloned().collect::<std::collections::HashSet<_>>()"
                .to_string()
        )
    );
}

#[test]
fn coercion_rejects_non_cloneable_borrowed_vec_elements() {
    let external_handle = JavaType::Named {
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
    let mut element_shape = RustSlotShape::conservative(external_handle.clone());
    element_shape.ownership = OwnershipShape::SharedBorrow;

    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr {
            expr: RustExpr::Raw("handles.iter().collect::<Vec<_>>()".to_string()),
            actual: LoweredType::vec_element_borrow(element_shape),
        },
        &Slot::Return(RustSlotShape::conservative(list_java_type(external_handle))),
        &facts,
    );

    assert!(matches!(
        expr,
        RustExpr::Raw(ref raw)
            if raw.contains(
                "unsupported Java borrowed element ownership recovery for non-cloneable type"
            ) && raw.contains("ExternalHandle")
    ));
}

#[test]
fn parser_keeps_generic_type_shape_for_type_pass() {
    assert_eq!(
        parse_type("List<Integer>"),
        JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "Integer".to_string(),
                args: Vec::new()
            }]
        }
    );
}
