use new_java2rust::coercion::{
    Coercion, CoercionDiagnostic, JavaDisplayPlan, LoweredExpr, UnsupportedCoercionMessagePolicy,
    apply_coercions, apply_expr_to_slot_with_facts, apply_java_display_plan,
    apply_lowered_expr_to_slot_with_facts, apply_shape_coercion_plan, apply_slot_conversion_plan,
    apply_syntax_coercion_plan, plan_coercion_with_facts, plan_shape_coercion_with_facts,
};
use new_java2rust::conversion_plan::{
    BorrowedElementAdapterPolicy, BorrowedElementCarrier, BorrowedElementRecoveryFallbackPolicy,
    BorrowedElementRecoveryPolicy, CloneRecoveryPolicy, ConversionDiagnostic,
    DispatchCoercionFallbackPolicy, DispatchCoercionPolicy, InstanceOfCheck,
    InstanceOfFallbackPolicy, InstanceOfTargetPolicy, InvalidCastPolicy, JavaCastSemanticsPolicy,
    JavaTypeConversionKind, LoweredSlotExprConversionPlan, LoweredType,
    OptionalPresentConstructionSource, OptionalShapeCoercionPolicy,
    OwnershipCoercionFallbackPolicy, OwnershipCoercionPolicy, ReturnValuePlan,
    RuntimeTypeTargetPolicy, ShapeCoercionPlan, Slot, SlotConversionPlan, SlotExprConversionPlan,
    SyntaxSlotCoercionPolicy, ViewCoercionPolicy, actual_shape_for_slot_expr, argument_slot,
    assignment_slot, assignment_slot_for_target, assignment_value_plan_for_shape,
    assignment_value_plan_for_target, borrowed_element_recovery_policy,
    borrowed_element_recovery_policy_with_effective_facts, borrowed_element_slot_adapter_policy,
    coercion_content_policy, coercion_content_policy_with_effective_facts,
    coercion_owned_recovery_capability_policy_with_effective_facts,
    coercion_shape_content_policy_with_effective_facts, condition_slot,
    dispatch_coercion_policy_with_facts, hierarchy_instanceof_policy, index_slot,
    instanceof_target_policy, java_cast_semantics_policy_with_facts,
    java_cast_type_conversion_plan, java_type_conversion_plan_from_evidence,
    lowered_slot_conversion_plan_with_facts, optional_element_target_match_policy,
    optional_present_construction_plan_for_actual, optional_shape_coercion_policy_with_facts,
    optional_slot_construction_plan_for_actual, optional_slot_construction_plan_with_facts,
    ownership_coercion_policy, ownership_coercion_policy_with_effective_facts,
    plan_instanceof_check_with_facts, plan_lowered_slot_conversion, plan_slot_conversion,
    preconverted_optional_slot_construction_plan, primitive_widening_policy, return_slot,
    return_value_plan, runtime_type_target_policy, rust_nominal_type_name_policy,
    shape_coercion_plan_with_facts, slot_conversion_plan_with_facts,
    syntax_coercion_plan_with_facts, vec_element_target_match_policy, view_coercion_policy,
    view_coercion_policy_with_effective_facts,
};
use new_java2rust::effective_facts::{
    CastTypeCapabilityPolicy, EffectiveFacts, InstanceOfTargetCapabilityPolicy,
};
use new_java2rust::emit;
use new_java2rust::emit::emit_expr;
use new_java2rust::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape};
use new_java2rust::external_packs::{hdf5_pack_facts, parse_external_pack_manifest};
use new_java2rust::facts::{
    ExternalApiCoverage, ExternalConversionAdapterFact, ExternalConversionAdapterKind,
    ExternalFieldFact, ExternalFieldKind, ExternalLibraryPackFact, FactProvenance, FactSet,
    FieldKey, OwnedRecoveryCapability, OwnedRecoveryCapabilityFact, TypeCapabilityOverride,
};
use new_java2rust::java_ir::{
    Expr, JavaCoercionEvidence, JavaCoercionKind, JavaType, PrimitiveType,
};
use new_java2rust::member_access_plan::{
    JavaMemberAccessKind, MemberAccessOp, java_member_access_policy_with_facts,
    plan_member_access_with_facts,
};
use new_java2rust::parser::{parse_expr, parse_type};
use new_java2rust::rust_ir::{
    RustClosureBody, RustClosureParam, RustExpr, RustExprShape, RustMatchArm, RustStmt, RustType,
};
use new_java2rust::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use new_java2rust::storage_strategy::{Escape, StoragePlan, TypeStorageFacts};
use new_java2rust::types::{
    TypeEnv, infer_expr_type, list_java_type, optional_java_type, set_java_type,
};

fn assert_compile_error_block_consuming(
    expr: RustExpr,
    consumed_rendered: &str,
    message_fragment: &str,
) {
    let RustExpr::Block { stmts, tail } = expr else {
        panic!("expected compile_error block");
    };
    assert_eq!(stmts.len(), 1);
    assert!(matches!(
        &stmts[0],
        RustStmt::Let {
            mutable: false,
            name,
            ty: None,
            value: Some(value),
        } if name == "_" && emit_expr(value) == consumed_rendered
    ));
    assert!(matches!(
        tail.as_deref(),
        Some(RustExpr::MacroCall { macro_name, args })
            if macro_name == "compile_error"
                && matches!(args.as_slice(), [RustExpr::Literal(message)]
                    if message.replace("\\\"", "\"").contains(message_fragment))
    ));
}

fn single_param_closure(pattern: &str, body: RustExpr) -> RustExpr {
    RustExpr::Closure {
        is_move: false,
        return_type: None,
        params: vec![RustClosureParam {
            pattern: pattern.to_string(),
            ty: None,
        }],
        body: RustClosureBody::Expr(Box::new(body)),
    }
}

fn values_iter_next_expr() -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "iter".to_string(),
            args: Vec::new(),
        }),
        method: "next".to_string(),
        args: Vec::new(),
    }
}

fn temporary_as_slice_expr() -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var("temporary".to_string())),
        method: "as_slice".to_string(),
        args: Vec::new(),
    }
}

fn values_get_key_as_str_expr() -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var("values".to_string())),
        method: "get".to_string(),
        args: vec![RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("key".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        }],
    }
}

fn intervals_get_zero_expr() -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var("intervals".to_string())),
        method: "get".to_string(),
        args: vec![RustExpr::Literal("0".to_string())],
    }
}

fn intervals_get_zero_expect_expr() -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(intervals_get_zero_expr()),
        method: "expect".to_string(),
        args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
    }
}

fn expr_unshaped_eq(expr: &RustExpr, expected: &RustExpr) -> bool {
    expr.unshaped() == expected
}

fn value_as_str_closure() -> RustExpr {
    single_param_closure(
        "value",
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        },
    )
}

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
fn runtime_type_target_policy_names_nominal_runtime_targets() {
    let named = JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    };

    let named_policy = rust_nominal_type_name_policy(&named);
    assert_eq!(named_policy.java_ty(), &named);
    assert_eq!(named_policy.target_type(), Some("Dog"));
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
            target_type: "VCFCompoundHeaderLine_SupportedHeaderLineType".to_string(),
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
    let boxed_boolean_policy = rust_nominal_type_name_policy(&boxed_boolean);
    assert_eq!(boxed_boolean_policy.java_ty(), &boxed_boolean);
    assert_eq!(boxed_boolean_policy.target_type(), Some("bool"));
    assert_eq!(
        runtime_type_target_policy(&boxed_boolean),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "bool".to_string(),
        }
    );
    let int_policy = rust_nominal_type_name_policy(&JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(
        int_policy.java_ty(),
        &JavaType::Primitive(PrimitiveType::Int)
    );
    assert_eq!(int_policy.target_type(), None);
    assert_eq!(
        runtime_type_target_policy(&JavaType::Primitive(PrimitiveType::Int)),
        RuntimeTypeTargetPolicy::MissingNominalTarget {
            java_ty: JavaType::Primitive(PrimitiveType::Int),
        }
    );
}

#[test]
fn primitive_widening_policy_names_numeric_widening_rank() {
    let int_to_long = primitive_widening_policy(
        &JavaType::Primitive(PrimitiveType::Int),
        &JavaType::Primitive(PrimitiveType::Long),
    )
    .unwrap();
    assert_eq!(int_to_long.from(), PrimitiveType::Int);
    assert_eq!(int_to_long.to(), PrimitiveType::Long);
    assert!(int_to_long.widens());
    assert_eq!(
        primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Long),
            &JavaType::Primitive(PrimitiveType::Int),
        )
        .unwrap()
        .widens(),
        false
    );
    assert!(
        !primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Byte),
            &JavaType::Primitive(PrimitiveType::Char),
        )
        .unwrap()
        .widens()
    );
    assert!(
        !primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Boolean),
            &JavaType::Primitive(PrimitiveType::Int),
        )
        .unwrap()
        .widens()
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

    let numeric = shape_coercion_plan_with_facts(&int_shape, &long_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &int_shape,
            &long_shape,
            &numeric,
            &FactSet::default(),
        ),
        JavaCastSemanticsPolicy::PrimitiveNumericCast {
            from: PrimitiveType::Int,
            to: PrimitiveType::Long,
            widening: primitive_widening_policy(&int_shape.java_ty, &long_shape.java_ty),
        }
    );

    let boolean_to_int =
        shape_coercion_plan_with_facts(&boolean_shape, &int_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &boolean_shape,
            &int_shape,
            &boolean_to_int,
            &FactSet::default(),
        ),
        JavaCastSemanticsPolicy::ImpossibleCast {
            actual: JavaType::Primitive(PrimitiveType::Boolean),
            target: JavaType::Primitive(PrimitiveType::Int),
        }
    );

    let primitive_to_reference =
        shape_coercion_plan_with_facts(&int_shape, &string_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &int_shape,
            &string_shape,
            &primitive_to_reference,
            &FactSet::default(),
        ),
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
    let upcast =
        shape_coercion_plan_with_facts(&dog_shape, &animal_enum_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &dog_shape,
            &animal_enum_shape,
            &upcast,
            &FactSet::default(),
        ),
        JavaCastSemanticsPolicy::ReferenceUpcast
    );

    let downcast =
        shape_coercion_plan_with_facts(&animal_enum_shape, &dog_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &animal_enum_shape,
            &dog_shape,
            &downcast,
            &FactSet::default(),
        ),
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
    let fallback =
        shape_coercion_plan_with_facts(&animal_enum_shape, &dyn_shape, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &animal_enum_shape,
            &dyn_shape,
            &fallback,
            &FactSet::default(),
        ),
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
    let generic_cast =
        shape_coercion_plan_with_facts(&list_string, &list_integer, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &list_string,
            &list_integer,
            &generic_cast,
            &FactSet::default(),
        ),
        JavaCastSemanticsPolicy::UncheckedGenericCast {
            actual: list_string.java_ty.clone(),
            target: list_integer.java_ty.clone(),
        }
    );

    let raw_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: Vec::new(),
    });
    let raw_cast = shape_coercion_plan_with_facts(&raw_list, &list_string, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &raw_list,
            &list_string,
            &raw_cast,
            &FactSet::default(),
        ),
        JavaCastSemanticsPolicy::RawOrWildcardCast {
            actual: raw_list.java_ty.clone(),
            target: list_string.java_ty.clone(),
        }
    );

    let wildcard_list = RustSlotShape::conservative(JavaType::Named {
        name: "List".to_string(),
        args: vec![JavaType::Wildcard(None)],
    });
    let wildcard_cast =
        shape_coercion_plan_with_facts(&wildcard_list, &list_string, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &wildcard_list,
            &list_string,
            &wildcard_cast,
            &FactSet::default(),
        ),
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
    let optional_result_cast =
        shape_coercion_plan_with_facts(&optional_string, &result_string, &FactSet::default());
    assert_eq!(
        java_cast_semantics_policy_with_facts(
            &optional_string,
            &result_string,
            &optional_result_cast,
            &FactSet::default(),
        ),
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
        java_type_conversion_plan_from_evidence(&widening).kind(),
        &JavaTypeConversionKind::PrimitiveWidening {
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
        java_type_conversion_plan_from_evidence(&boxing).kind(),
        &JavaTypeConversionKind::Boxing
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
        java_type_conversion_plan_from_evidence(&unboxing).kind(),
        &JavaTypeConversionKind::Unboxing
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
        java_type_conversion_plan_from_evidence(&lambda).kind(),
        &JavaTypeConversionKind::LambdaAdaptation
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
        java_cast_type_conversion_plan(&actual_list, &target_list, &FactSet::default()).kind(),
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
        CastTypeCapabilityPolicy::new(list_string.clone(), list_int.clone(), false, false, true)
    );
    assert_eq!(
        facts.cast_type_capability_policy(&raw_list, &list_string),
        CastTypeCapabilityPolicy::new(raw_list.clone(), list_string.clone(), false, true, false)
    );
    assert_eq!(
        facts.cast_type_capability_policy(&optional_string, &result_string),
        effective_facts.cast_type_capability_policy(&optional_string, &result_string)
    );
    assert_eq!(
        facts.cast_type_capability_policy(&optional_string, &result_string),
        CastTypeCapabilityPolicy::new(optional_string, result_string, true, false, false)
    );

    assert_eq!(
        facts.instanceof_target_capability_policy(&raw_list),
        InstanceOfTargetCapabilityPolicy::new(raw_list.clone(), false, true)
    );
    assert_eq!(
        facts.instanceof_target_capability_policy(&JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Wildcard(None)],
        }),
        InstanceOfTargetCapabilityPolicy::new(
            JavaType::Named {
                name: "List".to_string(),
                args: vec![JavaType::Wildcard(None)],
            },
            false,
            true,
        )
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
    let policy =
        java_member_access_policy_with_facts(&expr, "length", &env, &FactSet::default()).unwrap();
    let plan = plan_member_access_with_facts(&expr, "length", &env, &FactSet::default()).unwrap();

    assert_eq!(policy.member(), "length");
    assert_eq!(policy.java_ty(), &JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(policy.kind(), JavaMemberAccessKind::ArrayLength);
    assert_eq!(plan.op(), &MemberAccessOp::LenAsI32);
    assert_eq!(
        plan.shape().java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
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
    let policy =
        java_member_access_policy_with_facts(&receiver, "batchCount", &env, &FactSet::default())
            .unwrap();
    let plan =
        plan_member_access_with_facts(&receiver, "batchCount", &env, &FactSet::default()).unwrap();

    assert_eq!(policy.member(), "batchCount");
    assert_eq!(policy.java_ty(), &JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(policy.kind(), JavaMemberAccessKind::InstanceField);
    assert_eq!(
        plan.op(),
        &MemberAccessOp::RustField {
            rust_name: "batch_count".to_string(),
        }
    );
    assert_eq!(
        plan.shape().java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
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
    let policy = java_member_access_policy_with_facts(
        &receiver,
        "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY",
        &env,
        &FactSet::default(),
    )
    .unwrap();
    let plan = plan_member_access_with_facts(
        &receiver,
        "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY",
        &env,
        &FactSet::default(),
    )
    .unwrap();

    assert_eq!(policy.member(), "RAW_MAPPING_QUALITY_WITH_DEPTH_KEY");
    assert_eq!(policy.java_ty(), &JavaType::String);
    assert_eq!(policy.kind(), JavaMemberAccessKind::StaticConstant);
    assert_eq!(
        plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "GATKVCFConstants".to_string()
        }
    );
    assert_eq!(plan.shape().java_ty, JavaType::String);
    assert_eq!(plan.shape().ownership, OwnershipShape::SharedBorrow);
    assert_eq!(plan.shape().view, ViewShape::Str);
}

#[test]
fn member_access_plan_preserves_package_qualified_static_constant_owner() {
    let env = TypeEnv::new();
    let receiver = parse_expr("a.Item");
    let plan = plan_member_access_with_facts(&receiver, "KIND", &env, &FactSet::default()).unwrap();

    assert_eq!(
        plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "AItem".to_string(),
        }
    );
}

#[test]
fn member_access_plan_preserves_package_qualified_external_static_constant_owner() {
    let env = TypeEnv::new();
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey::named("a.Item", "KIND"),
        field_type: Some(JavaType::String),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey::named("b.Item", "KIND"),
        field_type: Some(JavaType::String),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let a_plan = plan_member_access_with_facts(&parse_expr("a.Item"), "KIND", &env, &facts)
        .expect("exact external fact should plan as a static constant");
    assert_eq!(
        a_plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "AItem".to_string(),
        }
    );

    let b_plan = plan_member_access_with_facts(&parse_expr("b.Item"), "KIND", &env, &facts)
        .expect("exact external peer fact should plan as a static constant");
    assert_eq!(
        b_plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "BItem".to_string(),
        }
    );
}

#[test]
fn member_access_plan_does_not_use_raw_mapped_field_type_as_static_constant_owner() {
    let env = TypeEnv::new();
    let mut facts = FactSet::default();
    facts.external_library_packs.push(ExternalLibraryPackFact {
        pack_name: "mapped".to_string(),
        package_prefix: "mapped.Box".to_string(),
        mode: new_java2rust::facts::ExternalLibraryMode::RustCrateMapping,
        coverage: ExternalApiCoverage::Mapped,
        rust_target: Some("mapped_facade".to_string()),
        provenance: FactProvenance::ExternalLibraryPack,
    });
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey::named("MappedOwner", "VALUE"),
        field_type: Some(JavaType::Named {
            name: "mapped.Box".to_string(),
            args: vec![JavaType::String],
        }),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let plan = plan_member_access_with_facts(&parse_expr("MappedOwner"), "VALUE", &env, &facts)
        .expect("exact external static field fact should still plan as a static constant");

    assert_eq!(
        plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "MappedOwner".to_string(),
        }
    );
}

#[test]
fn member_access_plan_does_not_invent_unknown_uppercase_static_constant_owner() {
    let mut env = TypeEnv::new();
    env.insert_type_field("Item", "KIND", JavaType::String);
    env.insert_static_type_alias("Cfg", "a.Config");
    env.insert_type_field("a.Config", "LIMIT", JavaType::Primitive(PrimitiveType::Int));

    let source_plan = plan_member_access_with_facts(
        &Expr::Var("Item".to_string()),
        "KIND",
        &env,
        &FactSet::default(),
    )
    .expect("known source type field should still plan as a static constant");
    assert_eq!(
        source_plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "Item".to_string(),
        }
    );

    let alias_plan = plan_member_access_with_facts(
        &Expr::Var("Cfg".to_string()),
        "LIMIT",
        &env,
        &FactSet::default(),
    )
    .expect("import/static alias should still prove the static owner");
    assert_eq!(
        alias_plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "AConfig".to_string(),
        }
    );

    assert!(
        plan_member_access_with_facts(
            &Expr::Var("Other".to_string()),
            "VALUE",
            &TypeEnv::new(),
            &FactSet::default(),
        )
        .is_none(),
        "capitalization alone must not prove an unknown static constant owner"
    );
}

#[test]
fn source_static_owner_keeps_emitted_simple_identity_over_qualified_import_alias() {
    let mut env = TypeEnv::new();
    env.insert_type_field("Matrix", "SIZE", JavaType::Primitive(PrimitiveType::Int));
    env.insert_static_type_alias("Matrix", "jaligner.matrix.Matrix");

    let plan = plan_member_access_with_facts(
        &Expr::Var("Matrix".to_string()),
        "SIZE",
        &env,
        &FactSet::default(),
    )
    .expect("source static constant should resolve through the source owner");

    assert_eq!(
        plan.op(),
        &MemberAccessOp::StaticConstant {
            owner: "Matrix".to_string(),
        }
    );
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
fn coercion_planner_owns_index_and_shape_widening() {
    assert_eq!(
        plan_coercion_with_facts(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &FactSet::default(),
        ),
        vec![Coercion::IndexToUsize]
    );

    let actual = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
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

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(plan.coercions(), vec![Coercion::AsStr]);
    let projected = apply_shape_coercion_plan(
        RustExpr::Raw("parts[0]".to_string()),
        &shape_coercion_plan_with_facts(&actual, &target, &FactSet::default()),
    );
    assert_eq!(emit_expr(&projected), "parts[0].as_str()");
    assert!(matches!(
        projected.into_unshaped(),
        RustExpr::MethodCall { receiver, method, .. }
            if method == "as_str"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::Index(array, index)
                        if matches!(array.as_ref(), RustExpr::Var(name) if name == "parts")
                            && matches!(index.as_ref(), RustExpr::Literal(value) if value == "0")
                )
    ));

    let parenthesized = apply_shape_coercion_plan(
        RustExpr::Raw("left + right".to_string()),
        &shape_coercion_plan_with_facts(&actual, &target, &FactSet::default()),
    );
    assert_eq!(emit_expr(&parenthesized), "(left + right).as_str()");
    let is_left_plus_right = |expr: &RustExpr| {
        matches!(
            expr,
            RustExpr::Binary { op, left, right }
                if op == "+"
                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                    && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
        )
    };
    assert!(matches!(
        parenthesized.into_unshaped(),
        RustExpr::MethodCall { receiver, method, args }
            if method == "as_str"
                && args.is_empty()
                && (is_left_plus_right(receiver.as_ref())
                    || matches!(receiver.as_ref(), RustExpr::Grouped(inner) if is_left_plus_right(inner)))
    ));
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
        ShapeCoercionPlan::new(
            actual.clone(),
            target.clone(),
            DispatchCoercionPolicy::NoDispatchChange,
            ViewCoercionPolicy::NoViewChange,
            OptionalShapeCoercionPolicy::Wrap {
                element_ty: JavaType::String,
                element_plan: Box::new(ShapeCoercionPlan::new(
                    actual.clone(),
                    target_element,
                    DispatchCoercionPolicy::NoDispatchChange,
                    ViewCoercionPolicy::NoViewChange,
                    OptionalShapeCoercionPolicy::NoChange,
                    OwnershipCoercionPolicy::BorrowOwnedValue,
                    None,
                )),
            },
            OwnershipCoercionPolicy::BorrowOwnedValue,
            None,
        )
    );
    assert_eq!(
        plan.coercions(),
        vec![Coercion::Borrow, Coercion::WrapOption]
    );
    let applied = apply_shape_coercion_plan(RustExpr::Var("name".to_string()), &plan);
    assert_eq!(new_java2rust::emit::emit_expr(&applied), "Some(&name)");
    assert_eq!(
        applied.recorded_shape(),
        Some(RustExprShape::OptionalConstructed)
    );

    let int_actual = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let long_target = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));
    let widening = shape_coercion_plan_with_facts(&int_actual, &long_target, &FactSet::default());

    assert_eq!(
        widening.primitive_widening(),
        primitive_widening_policy(
            &JavaType::Primitive(PrimitiveType::Int),
            &JavaType::Primitive(PrimitiveType::Long),
        )
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

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(
        plan.ownership(),
        &OwnershipCoercionPolicy::BorrowOwnedMutably
    );
    assert_eq!(plan.coercions(), vec![Coercion::BorrowMut]);
    let borrowed = apply_shape_coercion_plan(RustExpr::Var("config".to_string()), &plan);
    assert_eq!(emit_expr(&borrowed), "&mut config");
    assert_eq!(
        borrowed.clone().into_unshaped(),
        RustExpr::Unary {
            op: "&mut ".to_string(),
            expr: Box::new(RustExpr::Var("config".to_string())),
        }
    );
    assert_eq!(borrowed.recorded_shape(), Some(RustExprShape::MutBorrowed));
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

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert_eq!(plan.coercions(), vec![Coercion::UnwrapOptionMutBorrowed]);
    let borrowed = apply_shape_coercion_plan(RustExpr::Var("config".to_string()), &plan);
    assert_eq!(
        borrowed.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("config".to_string())),
                method: "as_mut".to_string(),
                args: Vec::new(),
            }),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        }
    );
    assert_eq!(
        emit_expr(&borrowed),
        "config.as_mut().expect(\"Java null receiver\")"
    );
    assert_eq!(borrowed.recorded_shape(), Some(RustExprShape::MutBorrowed));
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
        syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &facts
        )
        .policy(),
        &SyntaxSlotCoercionPolicy::IndexToUsize
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Boolean),
            &Slot::Condition,
            &facts
        )
        .policy(),
        &SyntaxSlotCoercionPolicy::Identity
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(
            &external_id,
            &Slot::StringConcatSegment,
            &FactSet::default()
        )
        .policy(),
        &SyntaxSlotCoercionPolicy::ToStringForConcat
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts).policy(),
        &SyntaxSlotCoercionPolicy::Identity
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &FactSet::default(),
        ),
        syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &FactSet::default(),
        )
    );
    assert_eq!(
        apply_syntax_coercion_plan(
            RustExpr::Var("i".to_string()),
            &syntax_coercion_plan_with_facts(
                &JavaType::Primitive(PrimitiveType::Int),
                &Slot::Index,
                &FactSet::default(),
            ),
        ),
        RustExpr::Cast {
            expr: Box::new(RustExpr::Var("i".to_string())),
            ty: "usize".to_string(),
        }
    );
    let raw_index = apply_syntax_coercion_plan(
        RustExpr::Raw("(bits & 4499201580859392i64) >> 42".to_string()),
        &syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Long),
            &Slot::Index,
            &FactSet::default(),
        ),
    );
    assert_eq!(
        emit_expr(&raw_index),
        "((bits & 4499201580859392i64) >> 42) as usize"
    );
    assert_eq!(
        raw_index,
        RustExpr::Cast {
            expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: ">>".to_string(),
                left: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "&".to_string(),
                    left: Box::new(RustExpr::Var("bits".to_string())),
                    right: Box::new(RustExpr::Literal("4499201580859392i64".to_string())),
                }))),
                right: Box::new(RustExpr::Literal("42".to_string())),
            }))),
            ty: "usize".to_string(),
        }
    );
    assert_eq!(
        apply_coercions(
            RustExpr::Raw("value >> 12".to_string()),
            &[Coercion::WidenPrimitive {
                from: PrimitiveType::Long,
                to: PrimitiveType::Double,
            }],
        ),
        RustExpr::Cast {
            expr: Box::new(RustExpr::Raw("value >> 12".to_string())),
            ty: "f64".to_string(),
        }
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts)
    );
    assert_eq!(
        apply_syntax_coercion_plan(
            RustExpr::Var("id".to_string()),
            &syntax_coercion_plan_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        ),
        RustExpr::Var("id".to_string())
    );
    assert_eq!(
        syntax_coercion_plan_with_facts(
            &external_id,
            &Slot::StringConcatSegment,
            &FactSet::default(),
        )
        .policy(),
        &SyntaxSlotCoercionPolicy::ToStringForConcat,
    );
    let content_policy = coercion_content_policy(&external_id, &facts);
    assert_eq!(content_policy.java_ty(), &external_id);
    assert!(content_policy.string_content());
    let effective_facts = EffectiveFacts::new(&facts);
    let shape_content_policy = coercion_shape_content_policy_with_effective_facts(
        &RustSlotShape::conservative(external_id.clone()),
        &effective_facts,
    );
    assert_eq!(shape_content_policy.java_ty(), &external_id);
    assert!(shape_content_policy.string_content());
}

#[test]
fn slot_conversion_plan_selects_shape_syntax_and_identity_boundaries() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let long_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Long));

    assert_eq!(
        slot_conversion_plan_with_facts(&int_shape, &Slot::Index, &FactSet::default()),
        SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts(
            &JavaType::Primitive(PrimitiveType::Int),
            &Slot::Index,
            &FactSet::default(),
        ))
    );
    assert_eq!(
        slot_conversion_plan_with_facts(
            &int_shape,
            &Slot::Return(long_shape.clone()),
            &FactSet::default(),
        ),
        SlotConversionPlan::Shape(ShapeCoercionPlan::new(
            int_shape,
            long_shape,
            DispatchCoercionPolicy::NoDispatchChange,
            ViewCoercionPolicy::NoViewChange,
            OptionalShapeCoercionPolicy::NoChange,
            OwnershipCoercionPolicy::NoOwnershipChange,
            primitive_widening_policy(
                &JavaType::Primitive(PrimitiveType::Int),
                &JavaType::Primitive(PrimitiveType::Long),
            ),
        ))
    );

    let lowered = LoweredType::optional_element_borrow(JavaType::String);
    assert_eq!(
        lowered_slot_conversion_plan_with_facts(&lowered, &Slot::Index, &FactSet::default()),
        SlotConversionPlan::Identity {
            actual: lowered,
            slot: Slot::Index,
        }
    );
}

#[test]
fn rust_unsigned_byte_array_return_composes_byte_conversion_pipeline() {
    let byte_array = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)));
    let target = RustSlotShape::conservative(byte_array.clone());
    let lowered = LoweredType::rust_unsigned_byte_array_return(byte_array.clone(), false);
    let plan =
        plan_lowered_slot_conversion(&lowered, Slot::Return(target.clone()), &FactSet::default());
    assert!(matches!(
        plan.conversion(),
        SlotConversionPlan::RustUnsignedByteArrayReturn(_)
    ));

    let optional_target = RustSlotShape::conservative(optional_java_type(byte_array.clone()));
    let optional_lowered = LoweredType::rust_unsigned_byte_array_return(byte_array, true);
    let optional_plan = plan_lowered_slot_conversion(
        &optional_lowered,
        Slot::Return(optional_target),
        &FactSet::default(),
    );
    assert!(matches!(
        optional_plan.conversion(),
        SlotConversionPlan::RustUnsignedByteArrayReturn(_)
    ));
}

#[test]
fn conversion_plan_wraps_slot_conversion_decisions_before_application() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    let slot_plan = plan_slot_conversion(int_shape.clone(), Slot::Index, &FactSet::default());

    assert_eq!(
        slot_plan,
        SlotExprConversionPlan::new(
            int_shape,
            Slot::Index,
            SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts(
                &JavaType::Primitive(PrimitiveType::Int),
                &Slot::Index,
                &FactSet::default(),
            )),
        )
    );
    let lowered_actual = LoweredType::optional_element_borrow(JavaType::String);
    let lowered_plan =
        plan_lowered_slot_conversion(&lowered_actual, Slot::Index, &FactSet::default());
    assert_eq!(
        lowered_plan,
        LoweredSlotExprConversionPlan::new(
            lowered_actual.clone(),
            Slot::Index,
            SlotConversionPlan::Identity {
                actual: lowered_actual,
                slot: Slot::Index,
            },
        )
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
        nested_collection_views: Vec::new(),
        ownership: OwnershipShape::SharedBorrow,
        view: ViewShape::Native,
        dispatch: DispatchShape::Concrete,
        mutability: new_java2rust::slot_shape::MutabilityShape::Immutable,
    };
    let crate_mapped_target = RustSlotShape::conservative(crate_mapped_ty);
    let source_actual = RustSlotShape {
        java_ty: source_helper_ty.clone(),
        nested_collection_views: Vec::new(),
        ownership: OwnershipShape::SharedBorrow,
        view: ViewShape::Native,
        dispatch: DispatchShape::Concrete,
        mutability: new_java2rust::slot_shape::MutabilityShape::Immutable,
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
        SlotConversionPlan::Shape(ref shape)
            if matches!(shape.ownership(), OwnershipCoercionPolicy::RecoverOwnedWithClone { .. })
    ));
    assert!(matches!(
        source_plan,
        SlotConversionPlan::Shape(ref shape)
            if matches!(shape.ownership(), OwnershipCoercionPolicy::RecoverOwnedWithClone { .. })
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

    let SlotConversionPlan::BorrowedElement(policy) = plan else {
        panic!("expected borrowed element policy");
    };
    assert_eq!(
        policy.adapter().map(BorrowedElementAdapterPolicy::recovery),
        Some(&BorrowedElementRecoveryPolicy::StringViewToOwned)
    );
}

#[test]
fn optional_element_borrow_display_projects_the_option_carrier() {
    let option_shape =
        RustSlotShape::conservative(optional_java_type(JavaType::Primitive(PrimitiveType::Int)));
    let mut element_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    element_shape.ownership = OwnershipShape::SharedBorrow;
    let actual = LoweredType::OptionalElementBorrow {
        option_shape: Box::new(option_shape),
        element_shape: Box::new(element_shape),
        recovery: None,
    };
    let plan =
        plan_lowered_slot_conversion(&actual, Slot::StringConcatSegment, &FactSet::default());

    let SlotConversionPlan::Syntax(syntax_plan) = plan.conversion() else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        syntax_plan.policy(),
        &SyntaxSlotCoercionPolicy::OptionalBorrowedToJavaDisplayValue
    );
}

#[test]
fn optional_element_borrow_nullable_string_display_flattens_nested_nulls() {
    let option_shape =
        RustSlotShape::conservative(optional_java_type(optional_java_type(JavaType::String)));
    let mut element_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));
    element_shape.ownership = OwnershipShape::SharedBorrow;
    let actual = LoweredType::OptionalElementBorrow {
        option_shape: Box::new(option_shape),
        element_shape: Box::new(element_shape),
        recovery: None,
    };
    let plan =
        plan_lowered_slot_conversion(&actual, Slot::StringConcatSegment, &FactSet::default());

    let SlotConversionPlan::Syntax(syntax_plan) = plan.conversion() else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        syntax_plan.policy(),
        &SyntaxSlotCoercionPolicy::OptionalBorrowedOptionalStringToJavaDisplayString
    );

    let rendered = emit_expr(&apply_coercions(
        RustExpr::Var("value".to_string()),
        &[Coercion::OptionalBorrowedOptionalStringToJavaDisplayString],
    ));
    assert_eq!(
        rendered,
        "value.and_then(|value| value.as_ref()).as_ref().map(|value| std::convert::AsRef::<str>::as_ref(value)).unwrap_or(\"null\")"
    );
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

    let SlotConversionPlan::BorrowedElement(policy) = &plan else {
        panic!("expected borrowed element policy");
    };
    assert_eq!(policy.target(), &target);
    assert_eq!(policy.carrier_shape(), &option_shape);
    assert_eq!(policy.element_shape(), &element_shape);
    assert_eq!(policy.carrier(), BorrowedElementCarrier::Option);
    assert!(policy.matches());
    let adapter = policy.adapter().expect("expected borrowed element adapter");
    assert_eq!(adapter.carrier(), BorrowedElementCarrier::Option);
    assert_eq!(adapter.element_shape(), &element_shape);
    assert_eq!(
        adapter.recovery(),
        &BorrowedElementRecoveryPolicy::BorrowElement
    );
    assert!(!adapter.unwrap_carrier());
    // Compared through `into_unshaped()`: the adapter now records the slot
    // shape it produced, and the tag is invisible to the emitter by design — the
    // generated Rust is byte-identical. This asserts the emitted expression,
    // which is what it was always about (claude_audits/0007).
    let projected = apply_slot_conversion_plan(RustExpr::Raw("values".to_string()), &plan);
    assert_eq!(emit_expr(&projected), "values.as_deref()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        apply_slot_conversion_plan(projected, &plan).into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        },
        "a recorded optional-element projection is already handled and must not \
         fall through to other borrowed-element adapters"
    );

    let mut cloned_element_actual =
        RustSlotShape::conservative(target.optional_element_type().unwrap());
    cloned_element_actual.ownership = OwnershipShape::SharedBorrow;
    cloned_element_actual.view = ViewShape::Slice;
    let cloned_element_projected = RustExpr::shaped_as(
        RustExprShape::OptionalProjected,
        Some(cloned_element_actual),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        },
    );
    let cloned_element_result = apply_slot_conversion_plan(cloned_element_projected, &plan);
    assert_eq!(
        cloned_element_result.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        },
        "idempotence must compare the produced slot semantics; derived type \
         identity makes stale metadata unrepresentable"
    );
    assert!(!matches!(
        cloned_element_result.into_unshaped(),
        RustExpr::Raw(_)
    ));
}

#[test]
fn optional_string_carrier_to_optional_string_view_records_projection_shape() {
    let actual = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let mut target = RustSlotShape::conservative(optional_java_type(JavaType::String));
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Str;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
    assert!(matches!(
        plan.optional(),
        OptionalShapeCoercionPolicy::MapElement { .. }
    ));

    let projected = apply_shape_coercion_plan(RustExpr::Var("name".to_string()), &plan);
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("name".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
            method: "map".to_string(),
            args: vec![value_as_str_closure()],
        }
    );
    assert_eq!(
        emit_expr(&projected),
        "name.as_ref().map(|value| value.as_str())"
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional string-view projection should record its produced element shape");
    assert_eq!(produced.java_ty, JavaType::String);
    assert_eq!(produced.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(produced.view, ViewShape::Str);
    assert_eq!(
        apply_shape_coercion_plan(projected, &plan).into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("name".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
            method: "map".to_string(),
            args: vec![value_as_str_closure()],
        },
        "a recorded optional string projection is already handled and must not \
         depend on recognising the rendered adapter suffix"
    );
}

#[test]
fn optional_shape_map_to_optional_shared_native_records_projection_shape() {
    let element_ty = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let actual = RustSlotShape::conservative(optional_java_type(element_ty.clone()));
    let mut target = RustSlotShape::conservative(optional_java_type(element_ty.clone()));
    target.ownership = OwnershipShape::SharedBorrow;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
    assert!(matches!(
        plan.optional(),
        OptionalShapeCoercionPolicy::MapElement { .. }
    ));

    let projected = apply_shape_coercion_plan(RustExpr::Var("value".to_string()), &plan);
    assert_eq!(emit_expr(&projected), "value.as_ref()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "as_ref".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional shared-native projection should record its produced element shape");
    assert_eq!(produced.java_ty, element_ty);
    assert_eq!(produced.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(produced.view, ViewShape::Native);
}

#[test]
fn optional_shape_map_to_optional_slice_records_projection_shape() {
    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
    let actual = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    let mut target = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    target.ownership = OwnershipShape::SharedBorrow;
    target.view = ViewShape::Slice;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
    assert!(matches!(
        plan.optional(),
        OptionalShapeCoercionPolicy::MapElement { .. }
    ));

    let projected = apply_shape_coercion_plan(RustExpr::Var("values".to_string()), &plan);
    assert_eq!(emit_expr(&projected), "values.as_deref()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional slice projection should record its produced element shape");
    assert_eq!(produced.java_ty, array_ty);
    assert_eq!(produced.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(produced.view, ViewShape::Slice);
    assert_eq!(
        emit_expr(&apply_shape_coercion_plan(projected.clone(), &plan)),
        "values.as_deref()",
        "a recorded optional slice projection is already handled and must not \
         depend on recognising the rendered adapter suffix"
    );
    assert_eq!(
        apply_shape_coercion_plan(projected, &plan).into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref".to_string(),
            args: Vec::new(),
        },
        "a recorded optional slice projection is already handled and must not \
         depend on recognising the rendered adapter suffix"
    );
}

#[test]
fn optional_shape_map_to_optional_mut_slice_records_projection_shape() {
    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
    let actual = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    let mut target = RustSlotShape::conservative(optional_java_type(array_ty.clone()));
    target.ownership = OwnershipShape::MutableBorrow;
    target.view = ViewShape::Slice;

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
    assert!(matches!(
        plan.optional(),
        OptionalShapeCoercionPolicy::MapElement { .. }
    ));

    let projected = apply_shape_coercion_plan(RustExpr::Var("values".to_string()), &plan);
    assert_eq!(emit_expr(&projected), "values.as_deref_mut()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref_mut".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional mutable-slice projection should record its produced element shape");
    assert_eq!(produced.java_ty, array_ty);
    assert_eq!(produced.ownership, OwnershipShape::MutableBorrow);
    assert_eq!(produced.view, ViewShape::Slice);
    assert_eq!(
        emit_expr(&apply_shape_coercion_plan(projected.clone(), &plan)),
        "values.as_deref_mut()",
        "a recorded optional mutable-slice projection is already handled and must \
         not depend on recognising the rendered adapter suffix"
    );
    assert_eq!(
        apply_shape_coercion_plan(projected, &plan).into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "as_deref_mut".to_string(),
            args: Vec::new(),
        },
        "a recorded optional mutable-slice projection is already handled and must \
         not depend on recognising the rendered adapter suffix"
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
    let mut optional_projected_element = target.clone();
    optional_projected_element.view = ViewShape::Native;

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        },
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );

    let projected = apply_slot_conversion_plan(RustExpr::Raw("values".to_string()), &plan);
    assert_eq!(
        emit_expr(&projected),
        "values.as_ref().expect(\"Java null receiver\").as_slice()"
    );
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::shaped_as(
                RustExprShape::OptionalProjected,
                Some(optional_projected_element.clone()),
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "expect".to_string(),
                    args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
                },
            )),
            method: "as_slice".to_string(),
            args: Vec::new(),
        }
    );
    let actual = projected
        .recorded_slot_shape()
        .expect("required borrowed-element projection should record its produced slot shape");
    assert_eq!(actual.java_ty, target.java_ty);
    assert_eq!(actual.ownership, target.ownership);
    assert_eq!(actual.view, target.view);
    assert_eq!(actual.dispatch, target.dispatch);
    assert_eq!(
        apply_slot_conversion_plan(projected, &plan).into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::shaped_as(
                RustExprShape::OptionalProjected,
                Some(optional_projected_element),
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "expect".to_string(),
                    args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
                },
            )),
            method: "as_slice".to_string(),
            args: Vec::new(),
        },
        "a recorded required-element projection is already handled and must not \
         depend on recognising the rendered adapter suffix"
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

    let SlotConversionPlan::BorrowedElement(policy) = &plan else {
        panic!("expected borrowed element policy");
    };
    assert_eq!(policy.target(), &target);
    assert_eq!(policy.carrier_shape(), option_shape.as_ref());
    assert_eq!(policy.element_shape(), element_shape.as_ref());
    assert_eq!(policy.carrier(), BorrowedElementCarrier::Option);
    assert!(policy.matches());
    let adapter = policy.adapter().expect("expected borrowed element adapter");
    assert_eq!(adapter.carrier(), BorrowedElementCarrier::Option);
    assert_eq!(adapter.element_shape(), element_shape.as_ref());
    assert_eq!(
        adapter.recovery(),
        &BorrowedElementRecoveryPolicy::CloneElements {
            policy: CloneRecoveryPolicy::AuditRequired,
        }
    );
    assert!(!adapter.unwrap_carrier());
    let adapted =
        apply_slot_conversion_plan(RustExpr::Raw("values.iter().next()".to_string()), &plan);
    assert_eq!(
        emit::emit_expr(&adapted),
        "values.iter().next().cloned()/* TODO(translation): validate added clone */"
    );
    assert!(
        matches!(
            adapted.into_unshaped(),
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { method, .. } if method == "cloned"
                )
        ),
        "borrowed optional clone recovery should keep cloned() structured"
    );
}

#[test]
fn clone_recovery_policy_is_explicit_in_coercion_adapter() {
    let clone_recovery = apply_coercions(
        RustExpr::Var("value".to_string()),
        &[Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::AuditRequired,
        }],
    )
    .into_unshaped();
    assert_eq!(
        emit_expr(&clone_recovery),
        "std::clone::Clone::clone(&value)/* TODO(translation): validate added clone */"
    );
    assert!(matches!(
        clone_recovery,
        RustExpr::PostfixComment { expr, .. }
            if matches!(
                expr.as_ref(),
                RustExpr::Call { function, .. }
                    if emit_expr(function) == "std::clone::Clone::clone"
            )
    ));
    let proven = apply_coercions(
        RustExpr::Var("value".to_string()),
        &[Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Named {
                    name: "ExternalValue".to_string(),
                    args: Vec::new(),
                },
            },
        }],
    );
    assert_eq!(emit_expr(&proven), "std::clone::Clone::clone(&value)");
    assert!(matches!(
        proven.clone().into_unshaped(),
        RustExpr::Call { function, .. }
            if emit_expr(&function) == "std::clone::Clone::clone"
    ));
    assert_eq!(proven.recorded_shape(), Some(RustExprShape::OwnedRecovery));

    let already_recovered = apply_coercions(
        RustExpr::Raw("value.clone()".to_string()),
        &[Coercion::CloneRecovery {
            policy: CloneRecoveryPolicy::AuditRequired,
        }],
    );
    assert_eq!(emit_expr(&already_recovered), "value.clone()");
    assert_eq!(
        already_recovered.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
    assert_eq!(
        already_recovered.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
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
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
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
    let effective_facts = EffectiveFacts::new(&facts);

    let recovery_policy =
        coercion_owned_recovery_capability_policy_with_effective_facts(&external, &effective_facts);
    assert_eq!(recovery_policy.java_ty(), &external);
    assert_eq!(
        recovery_policy.recovery(),
        OwnedRecoveryCapability::ExplicitClone {
            java_ty: external.clone(),
        }
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
    let copied = apply_expr_to_slot_with_facts(
        RustExpr::Var("external_id".to_string()),
        actual,
        &Slot::Argument(target),
        &facts,
    );
    assert_eq!(
        copied,
        RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(RustExpr::Var("external_id".to_string())),
        }
    );
    assert_eq!(emit_expr(&copied), "*external_id");
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
        vec![CoercionDiagnostic::new(
            "clone_audit_required",
            "borrowed-to-owned Java value recovery inserted a clone that requires ownership audit"
                .to_string()
        )]
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
        vec![CoercionDiagnostic::new(
            "unsupported_borrowed_to_owned_non_cloneable",
            "unsupported Java borrowed-to-owned recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }",
        )]
    );
    let rendered = apply_coercions(
        RustExpr::Var("handle".to_string()),
        &ownership_coercion_policy(&borrowed_handle, &owned_handle, &facts).coercions(),
    );
    assert_compile_error_block_consuming(
        rendered,
        "handle",
        "unsupported Java borrowed-to-owned recovery for non-cloneable type",
    );
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
        coercion_owned_recovery_capability_policy_with_effective_facts(
            &clone_box,
            &EffectiveFacts::new(&facts),
        )
        .recovery(),
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
        vec![CoercionDiagnostic::new(
            "unsupported_borrowed_to_ffi_owned_handle",
            "unsupported Java borrowed-to-owned recovery for FFI-owned handle Named { name: \"FfiHandle\", args: [] }; use an explicit FFI ownership transfer",
        )]
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
        parsed.diagnostics().is_empty(),
        "manifest should parse cleanly: {:?}",
        parsed.diagnostics()
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
            .facts()
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
        ownership_coercion_policy(&actual_variant, &target_variant, parsed.facts()),
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
        ownership_coercion_policy(&actual_allele, &target_allele, parsed.facts()),
        OwnershipCoercionPolicy::RecoverOwnedCopyValue
    );

    let (actual_reader, target_reader) = borrowed_to_owned(&reader);
    assert_eq!(
        ownership_coercion_policy(&actual_reader, &target_reader, parsed.facts()).diagnostics(),
        vec![CoercionDiagnostic::new(
            "unsupported_borrowed_to_ffi_owned_handle",
            "unsupported Java borrowed-to-owned recovery for FFI-owned handle Named { name: \"neutral.gatk.NativeReader\", args: [] }; use an explicit FFI ownership transfer",
        )]
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
        unowned.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "external_pack_manifest_unowned_owned_recovery_capability_fact"
        }),
        "{:?}",
        unowned.diagnostics()
    );

    let unknown_kind = parse_external_pack_manifest(
        r#"
        external-pack neutral.gatk neutral.gatk external-fact-model semantic
        external-pack-owned-recovery neutral.gatk neutral.gatk.Handle magical-copy
        "#,
    );
    assert!(
        unknown_kind.diagnostics().iter().any(|diagnostic| {
            diagnostic.code() == "external_pack_manifest_parse_error"
                && diagnostic
                    .message()
                    .contains("invalid external-pack-owned-recovery capability kind")
        }),
        "{:?}",
        unknown_kind.diagnostics()
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
        vec![ConversionDiagnostic::new(
            "unsupported_borrowed_to_owned_non_cloneable",
            "unsupported Java borrowed-to-owned recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
        )]
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
    assert_compile_error_block_consuming(
        rendered,
        "handle",
        "unsupported Java borrowed-to-owned recovery for non-cloneable type",
    );
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
            element_plan: Box::new(ShapeCoercionPlan::new(
                actual.clone(),
                target_element,
                DispatchCoercionPolicy::NoDispatchChange,
                ViewCoercionPolicy::NoViewChange,
                OptionalShapeCoercionPolicy::NoChange,
                OwnershipCoercionPolicy::BorrowOwnedValue,
                None,
            )),
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

    let construction =
        optional_slot_construction_plan_with_facts(&target, JavaType::String, &FactSet::default());
    assert_eq!(construction.wrapper_ownership(), OwnershipShape::Owned);
    assert_eq!(construction.target(), &target);
    assert_eq!(construction.element_shape().java_ty, JavaType::String);
    assert_eq!(
        construction.element_shape().ownership,
        OwnershipShape::SharedBorrow
    );
    let actual_owned = RustSlotShape::conservative(JavaType::String);
    let construction = optional_slot_construction_plan_for_actual(&actual_owned, &target, &facts);
    assert_eq!(construction.wrapper_ownership(), OwnershipShape::Owned);
    assert_eq!(construction.target(), &target);
    assert_eq!(construction.element_shape(), &actual_owned);
    assert_eq!(
        construction.element_plan().coercions(),
        vec![Coercion::Borrow]
    );
    let optional_construction = SlotConversionPlan::OptionalConstruction(construction);
    let applied =
        apply_slot_conversion_plan(RustExpr::Var("name".to_string()), &optional_construction);
    assert_eq!(new_java2rust::emit::emit_expr(&applied), "Some(&name)");
    assert_eq!(
        applied.recorded_shape(),
        Some(RustExprShape::OptionalConstructed)
    );
    let RustExpr::Call { args, .. } = applied.unshaped() else {
        panic!("optional construction should stay a Some call");
    };
    assert_eq!(args[0].recorded_shape(), Some(RustExprShape::Borrowed));
    let preconverted =
        preconverted_optional_slot_construction_plan(&target, JavaType::String, &facts);
    assert_eq!(preconverted.wrapper_ownership(), OwnershipShape::Owned);
    assert_eq!(preconverted.target(), &target);
    assert_eq!(preconverted.element_shape().java_ty, JavaType::String);
    assert_eq!(
        preconverted.element_shape().ownership,
        OwnershipShape::SharedBorrow
    );
    assert_eq!(
        preconverted.element_plan().coercions(),
        vec![Coercion::Identity]
    );
    let optional_preconverted = SlotConversionPlan::OptionalConstruction(preconverted);
    let applied =
        apply_slot_conversion_plan(RustExpr::Raw("&name".to_string()), &optional_preconverted);
    assert_eq!(
        applied.recorded_shape(),
        Some(RustExprShape::OptionalConstructed)
    );
    assert_eq!(
        applied.unshaped(),
        &RustExpr::Call {
            function: Box::new(RustExpr::path(["Some"])),
            args: vec![RustExpr::Raw("&name".to_string())],
        }
    );

    let unwrap_actual = RustSlotShape::conservative(optional_java_type(JavaType::String));
    let unwrap_target = RustSlotShape::conservative(JavaType::String);
    let mut unwrap_actual_element = unwrap_actual.clone();
    unwrap_actual_element.java_ty = JavaType::String;
    let unwrap_policy = optional_shape_coercion_policy_with_facts(
        &unwrap_actual,
        &unwrap_target,
        &FactSet::default(),
    );
    assert_eq!(
        unwrap_policy,
        OptionalShapeCoercionPolicy::Unwrap {
            element_ty: JavaType::String,
            element_plan: Box::new(ShapeCoercionPlan::new(
                unwrap_actual_element,
                unwrap_target,
                DispatchCoercionPolicy::NoDispatchChange,
                ViewCoercionPolicy::NoViewChange,
                OptionalShapeCoercionPolicy::NoChange,
                OwnershipCoercionPolicy::NoOwnershipChange,
                None,
            )),
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
    assert_eq!(indexed.element_lowering_slot(), &Slot::Value);
    assert_eq!(
        indexed.construction().element_shape().ownership,
        OwnershipShape::Owned
    );
    assert_eq!(
        indexed.construction().element_plan().coercions(),
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
        plain.element_lowering_slot(),
        &Slot::Assignment(plain.construction().element_shape().clone())
    );
    assert_eq!(
        plain.construction().element_shape().ownership,
        OwnershipShape::Owned
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
fn external_upcast_coercion_uses_adapter_into_projection_at_use_site() {
    let rendered = apply_coercions(
        RustExpr::Var("zip".to_string()),
        &[Coercion::ExternalInto {
            from_type: JavaType::Named {
                name: "ZipOutputStream".to_string(),
                args: Vec::new(),
            },
            to_type: JavaType::Named {
                name: "OutputStream".to_string(),
                args: Vec::new(),
            },
        }],
    );

    assert_eq!(
        emit_expr(&rendered),
        "std::convert::Into::<OutputStream>::into(zip)"
    );
    assert!(!matches!(rendered.into_unshaped(), RustExpr::Raw(_)));
}

#[test]
fn external_into_clones_source_before_consuming_conversion() {
    let rendered = apply_coercions(
        RustExpr::Var("stream".to_string()),
        &[
            Coercion::ExternalInto {
                from_type: JavaType::Named {
                    name: "ByteArrayOutputStream".to_string(),
                    args: Vec::new(),
                },
                to_type: JavaType::Named {
                    name: "OutputStream".to_string(),
                    args: Vec::new(),
                },
            },
            Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
        ],
    );

    assert_eq!(
        emit_expr(&rendered),
        "std::convert::Into::<OutputStream>::into(std::clone::Clone::clone(&stream)/* TODO(translation): validate added clone */)"
    );
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
fn insertion_ordered_map_converts_to_plain_map_slot() {
    let args = vec![
        JavaType::String,
        JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        },
    ];
    let actual = RustSlotShape::conservative(JavaType::Named {
        name: "LinkedHashMap".to_string(),
        args: args.clone(),
    });
    let target = RustSlotShape::conservative(JavaType::Named {
        name: "Map".to_string(),
        args,
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
            if matches!(element_plan.dispatch(), DispatchCoercionPolicy::EraseObject)
    ));
    assert_eq!(
        policy.coercions(),
        vec![Coercion::EraseObject, Coercion::WrapOption]
    );
    let applied = apply_expr_to_slot_with_facts(
        RustExpr::Var("vid_map".to_string()),
        actual,
        &Slot::Assignment(target),
        &facts,
    );
    assert_eq!(
        new_java2rust::emit::emit_expr(&applied),
        "Some(Box::new(vid_map) as Box<dyn std::any::Any>)"
    );
    assert_eq!(
        applied.recorded_shape(),
        Some(RustExprShape::OptionalConstructed)
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
        plan_coercion_with_facts(
            &external_id,
            &Slot::StringConcatSegment,
            &FactSet::default(),
        ),
        vec![Coercion::ToStringForConcat]
    );
    assert_eq!(
        plan_coercion_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        vec![Coercion::Identity]
    );
}

#[test]
fn optional_display_segments_use_java_null_string_policy_before_rendering() {
    let optional_string = RustSlotShape::conservative(optional_java_type(JavaType::String));

    let optional_string_conversion = slot_conversion_plan_with_facts(
        &optional_string,
        &Slot::DisplaySegment,
        &FactSet::default(),
    );
    let SlotConversionPlan::Syntax(optional_string_syntax) = optional_string_conversion else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        optional_string_syntax.actual(),
        &optional_java_type(JavaType::String)
    );
    assert_eq!(optional_string_syntax.slot(), &Slot::DisplaySegment);
    assert_eq!(
        optional_string_syntax.policy(),
        &SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString
    );
    assert_eq!(
        apply_slot_conversion_plan(
            RustExpr::Var("value".to_string()),
            &slot_conversion_plan_with_facts(
                &optional_string,
                &Slot::StringConcatSegment,
                &FactSet::default(),
            ),
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    params: vec![RustClosureParam {
                        pattern: "value".to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: RustClosureBody::Expr(Box::new(RustExpr::Call {
                        function: Box::new(RustExpr::GenericPath {
                            base: vec![
                                "std".to_string(),
                                "convert".to_string(),
                                "AsRef".to_string(),
                            ],
                            args: vec![RustType::Named("str".to_string())],
                            member: Some("as_ref".to_string()),
                        }),
                        args: vec![RustExpr::Var("value".to_string())],
                    })),
                }],
            }),
            method: "unwrap_or".to_string(),
            args: vec![RustExpr::Literal("\"null\"".to_string())],
        }
    );
}

#[test]
fn optional_platform_facade_display_segments_use_display_value_policy() {
    let optional_uri = RustSlotShape::conservative(optional_java_type(JavaType::Named {
        name: "URI".to_string(),
        args: Vec::new(),
    }));

    let optional_uri_conversion =
        slot_conversion_plan_with_facts(&optional_uri, &Slot::DisplaySegment, &FactSet::default());
    let SlotConversionPlan::Syntax(optional_uri_syntax) = optional_uri_conversion else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        optional_uri_syntax.actual(),
        &optional_java_type(JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(optional_uri_syntax.slot(), &Slot::DisplaySegment);
    assert_eq!(
        optional_uri_syntax.policy(),
        &SyntaxSlotCoercionPolicy::OptionalToDisplayValue
    );
    assert_eq!(
        apply_slot_conversion_plan(
            RustExpr::Var("uri".to_string()),
            &slot_conversion_plan_with_facts(
                &optional_uri,
                &Slot::StringConcatSegment,
                &FactSet::default(),
            ),
        ),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("uri".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "value".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("value".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    })),
                }],
            }),
            method: "unwrap_or_else".to_string(),
            args: vec![RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: Vec::new(),
                body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Literal("\"null\"".to_string())),
                    method: "to_string".to_string(),
                    args: Vec::new(),
                })),
            }],
        }
    );
    let raw_borrowed_display = apply_java_display_plan(
        RustExpr::Raw("values.get(key.as_str())".to_string()),
        JavaDisplayPlan::OptionalBorrowedValue,
    );
    assert_eq!(
        emit_expr(&raw_borrowed_display),
        "values.get(key.as_str()).map(|value| value.java_arrays_deep_to_string()).unwrap_or_else(|| \"null\".to_string())"
    );
    assert!(matches!(
        raw_borrowed_display,
        RustExpr::MethodCall { method, receiver, .. }
            if method == "unwrap_or_else"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { receiver, method, .. }
                        if method == "map"
                            && receiver.as_ref() == &values_get_key_as_str_expr()
                )
    ));
}

#[test]
fn optional_element_borrow_display_segments_use_lowered_slot_policy() {
    let actual =
        LoweredType::optional_element_borrow_shape(RustSlotShape::conservative(JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        }));

    let borrowed_display_conversion = lowered_slot_conversion_plan_with_facts(
        &actual,
        &Slot::DisplaySegment,
        &FactSet::default(),
    );
    let SlotConversionPlan::Syntax(borrowed_display_syntax) = borrowed_display_conversion else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        borrowed_display_syntax.actual(),
        &optional_java_type(JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        })
    );
    assert_eq!(borrowed_display_syntax.slot(), &Slot::DisplaySegment);
    assert_eq!(
        borrowed_display_syntax.policy(),
        &SyntaxSlotCoercionPolicy::OptionalBorrowedToDisplayValue
    );
    let borrowed_string_concat_plan = plan_lowered_slot_conversion(
        &LoweredType::optional_element_borrow_shape(RustSlotShape::conservative(JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        })),
        Slot::StringConcatSegment,
        &FactSet::default(),
    );
    let borrowed_string_concat = borrowed_string_concat_plan.conversion();
    let SlotConversionPlan::Syntax(borrowed_string_concat_syntax) = borrowed_string_concat else {
        panic!("expected syntax conversion");
    };
    assert_eq!(
        borrowed_string_concat_syntax.policy(),
        &SyntaxSlotCoercionPolicy::OptionalBorrowedToDisplayValue
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
    let assignment_value = assignment_value_plan_for_shape(&shape);
    assert_eq!(assignment_value.slot(), &Slot::Assignment(shape.clone()));
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
    let assignment_value =
        assignment_value_plan_for_target(&parse_expr("name"), &env, ExprShapeContext::default());
    assert_eq!(
        assignment_value.slot(),
        &Slot::Assignment(RustSlotShape::for_storage(JavaType::String))
    );
    assert_eq!(
        assignment_slot_for_target(
            &parse_expr("unknown.field"),
            &env,
            ExprShapeContext::default(),
        ),
        Slot::Value
    );
    let unknown_assignment_value = assignment_value_plan_for_target(
        &parse_expr("unknown.field"),
        &env,
        ExprShapeContext::default(),
    );
    assert_eq!(unknown_assignment_value.slot(), &Slot::Value);
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

    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
        vec![Coercion::AsStr]
    );

    let projected = apply_expr_to_slot_with_facts(
        RustExpr::Var("name".to_string()),
        actual,
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&projected), "name.as_str()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("string view projection should record its target shape");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
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

    let recovered = apply_expr_to_slot_with_facts(
        RustExpr::Raw("name.as_str()".to_string()),
        borrowed_str.clone(),
        &Slot::Argument(owned.clone()),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&recovered), "name.as_str().to_string()");
    assert_eq!(
        recovered.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("name".to_string())),
                method: "as_str".to_string(),
                args: Vec::new(),
            }),
            method: "to_string".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        recovered.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
    assert_eq!(recovered.recorded_slot_shape(), Some(&owned));

    let already_owned = apply_expr_to_slot_with_facts(
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        },
        borrowed_str.clone(),
        &Slot::Argument(owned.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        already_owned.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        already_owned.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
    assert_eq!(already_owned.recorded_slot_shape(), Some(&owned));

    let compound_recovered = apply_expr_to_slot_with_facts(
        RustExpr::Raw("left + right".to_string()),
        borrowed_str,
        &Slot::Argument(owned.clone()),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&compound_recovered), "(left + right).to_string()");
    assert!(matches!(
        compound_recovered.into_unshaped(),
        RustExpr::MethodCall { method, .. } if method == "to_string"
    ));
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

    assert_eq!(plan.view(), &ViewCoercionPolicy::BorrowSequenceSliceView);
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
        vec![Coercion::AsSlice]
    );
    let projected = apply_expr_to_slot_with_facts(
        RustExpr::Var("intervals".to_string()),
        actual.clone(),
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&projected), "intervals.as_slice()");
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("intervals".to_string())),
            method: "as_slice".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("slice view projection should record its target shape");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);

    let mut mutable_target = target.clone();
    mutable_target.ownership = OwnershipShape::MutableBorrow;
    let mutable_plan =
        shape_coercion_plan_with_facts(&actual, &mutable_target, &FactSet::default());
    assert_eq!(
        *mutable_plan.view(),
        ViewCoercionPolicy::BorrowSequenceMutSliceView
    );
    let mutable_projected = apply_coercions(
        RustExpr::Var("intervals".to_string()),
        &[Coercion::AsMutSlice],
    );
    assert_eq!(emit_expr(&mutable_projected), "intervals.as_mut_slice()");
    assert_eq!(
        mutable_projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("intervals".to_string())),
            method: "as_mut_slice".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        mutable_projected.recorded_shape(),
        Some(RustExprShape::MutableSliceView)
    );

    let raw_slice_projected = apply_coercions(
        RustExpr::Raw("make_values()".to_string()),
        &[Coercion::AsSlice],
    );
    assert_eq!(emit_expr(&raw_slice_projected), "make_values().as_slice()");
    assert!(matches!(
        raw_slice_projected.into_unshaped(),
        RustExpr::MethodCall { method, .. } if method == "as_slice"
    ));

    let raw_mutable_projected = apply_coercions(
        RustExpr::Raw("left + right".to_string()),
        &[Coercion::AsMutSlice],
    );
    assert_eq!(
        emit_expr(&raw_mutable_projected),
        "(left + right).as_mut_slice()"
    );
    assert!(matches!(
        raw_mutable_projected.into_unshaped(),
        RustExpr::MethodCall { method, .. } if method == "as_mut_slice"
    ));
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

    assert_eq!(plan.view(), &ViewCoercionPolicy::BorrowSequenceSliceView);
    let projected = apply_expr_to_slot_with_facts(
        RustExpr::Raw("self.lines".to_string()),
        actual.clone(),
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&projected),
        "&self.lines.iter().cloned().collect::<Vec<_>>()"
    );
    assert!(
        !matches!(projected.clone().into_unshaped(), RustExpr::Raw(_)),
        "set-to-slice temporary adapter should compose borrow/iter/cloned/collect structurally"
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("set-to-slice projection should record its produced slot shape");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
    assert_eq!(
        emit_expr(&apply_expr_to_slot_with_facts(
            projected,
            actual,
            &Slot::Argument(target.clone()),
            &FactSet::default(),
        )),
        "&self.lines.iter().cloned().collect::<Vec<_>>()",
        "a recorded set-to-slice projection is already handled and must not depend \
         on recognising the rendered adapter suffix"
    );

    let tagged_slice = RustExpr::shaped_as(
        RustExprShape::SliceView,
        Some(target.clone()),
        temporary_as_slice_expr(),
    );
    assert_eq!(
        apply_expr_to_slot_with_facts(
            tagged_slice,
            RustSlotShape::conservative(JavaType::Named {
                name: "Set".to_string(),
                args: vec![JavaType::Named {
                    name: "SimpleInterval".to_string(),
                    args: Vec::new(),
                }],
            }),
            &Slot::Argument(target.clone()),
            &FactSet::default(),
        )
        .into_unshaped(),
        temporary_as_slice_expr(),
        "a recorded target shape, not the rendered suffix, proves idempotence"
    );

    let untagged_suffix = apply_expr_to_slot_with_facts(
        RustExpr::Raw("temporary.as_slice()".to_string()),
        RustSlotShape::conservative(JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Named {
                name: "SimpleInterval".to_string(),
                args: Vec::new(),
            }],
        }),
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&untagged_suffix),
        "&temporary.as_slice().iter().cloned().collect::<Vec<_>>()"
    );
    assert!(
        !matches!(untagged_suffix.clone().into_unshaped(), RustExpr::Raw(_)),
        "untagged set-to-slice temporary adapter should also be structured"
    );
    let produced = untagged_suffix
        .recorded_slot_shape()
        .expect("set-to-slice adaptation should record the produced shape");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
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
    let projected = apply_expr_to_slot_with_facts(
        RustExpr::Raw("self.intervals".to_string()),
        actual,
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&projected),
        "self.intervals.as_ref().expect(\"Java null receiver\").as_slice()"
    );
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::shaped(
                RustExprShape::Borrowed,
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Field(
                            Box::new(RustExpr::Var("self".to_string())),
                            "intervals".to_string(),
                        )),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "expect".to_string(),
                    args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
                },
            )),
            method: "as_slice".to_string(),
            args: Vec::new(),
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional unwrap plus slice view should record its target shape");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
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
    let shape_content_policy =
        coercion_shape_content_policy_with_effective_facts(&actual, &effective_facts);
    assert_eq!(shape_content_policy.java_ty(), &external_name);
    assert!(shape_content_policy.string_content());
    assert_eq!(
        view_coercion_policy_with_effective_facts(&actual, &target, &effective_facts),
        view_coercion_policy(&actual, &target, &facts)
    );

    let projected = apply_expr_to_slot_with_facts(
        RustExpr::Var("name".to_string()),
        actual,
        &Slot::Argument(target),
        &facts,
    );
    assert_eq!(emit_expr(&projected), "name.as_str()");
    assert_eq!(
        projected.into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        }
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
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
        vec![Coercion::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }]
    );
    assert_eq!(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("dog".to_string()),
            actual,
            &Slot::Assignment(target.clone()),
            &FactSet::default(),
        ),
        RustExpr::Call {
            function: Box::new(RustExpr::path(["AnimalValue", "Dog"])),
            args: vec![RustExpr::Var("dog".to_string())],
        }
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
        plan_shape_coercion_with_facts(&enum_actual, &downcast_target, &FactSet::default()),
        vec![Coercion::DowncastClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
            borrowed_payload: false,
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    let downcast = apply_expr_to_slot_with_facts(
        RustExpr::Var("animal".to_string()),
        enum_actual,
        &Slot::Assignment(downcast_target),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&downcast),
        "match animal { AnimalValue::Dog(value) => value, _ => panic!(\"invalid Java cast\") }"
    );
    let RustExpr::Match { arms, .. } = downcast.unshaped() else {
        panic!("expected a checked carrier downcast: {downcast:?}");
    };
    let [success, failure] = arms.as_slice() else {
        panic!("expected success and checked-failure arms: {arms:?}");
    };
    assert_eq!(success.pattern, "AnimalValue::Dog(value)");
    assert!(matches!(success.body.unshaped(), RustExpr::Var(value) if value == "value"));
    assert_eq!(failure.pattern, "_");
    assert_eq!(
        failure.body.recorded_shape(),
        Some(RustExprShape::CheckedCastFailure)
    );
    assert!(
        matches!(failure.body.unshaped(), RustExpr::MacroCall { macro_name, .. } if macro_name == "panic")
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

    let check = plan_instanceof_check_with_facts(&actual, &target, &FactSet::default());
    assert_eq!(
        check,
        InstanceOfCheck::ClosedEnumVariant {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }
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

    let dyn_check = plan_instanceof_check_with_facts(&dyn_actual, &target, &FactSet::default());
    let boxed_check = plan_instanceof_check_with_facts(&boxed_actual, &target, &FactSet::default());
    let erased_check =
        plan_instanceof_check_with_facts(&erased_actual, &target, &FactSet::default());
    let optional_erased_check =
        plan_instanceof_check_with_facts(&optional_erased_actual, &target, &FactSet::default());

    assert_eq!(
        dyn_check,
        InstanceOfCheck::DynTraitType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        boxed_check,
        InstanceOfCheck::BoxedDynTraitType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        erased_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Dog".to_string(),
        }
    );
    assert_eq!(
        optional_erased_check,
        InstanceOfCheck::OptionalErasedObjectType {
            target_type: "Dog".to_string(),
        }
    );
}

#[test]
fn optional_erased_instanceof_uses_shared_handle_carrier_for_shared_targets() {
    let mut optional_erased_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Optional".to_string(),
        args: vec![JavaType::Named {
            name: "Dog".to_string(),
            args: Vec::new(),
        }],
    });
    optional_erased_actual.dispatch = DispatchShape::ErasedObject;

    let target = RustSlotShape::conservative(JavaType::Named {
        name: "Dog".to_string(),
        args: Vec::new(),
    });
    let mut storage_plan = StoragePlan::default();
    storage_plan.insert(
        "Dog".to_string(),
        TypeStorageFacts::default()
            .with_escape(Escape::Stored)
            .with_locally_aliased(true)
            .with_mutated(false)
            .with_mutation_provably_absent(true)
            .with_threaded(false)
            .with_static_reachable(false)
            .with_surface_forwardable(true),
    );
    let facts = FactSet {
        storage_strategies: storage_plan,
        ..FactSet::default()
    };

    let check = plan_instanceof_check_with_facts(&optional_erased_actual, &target, &facts);
    assert_eq!(
        check,
        InstanceOfCheck::OptionalErasedObjectType {
            target_type: "std::rc::Rc<Dog>".to_string(),
        }
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
    let primitive_check =
        plan_instanceof_check_with_facts(&actual, &primitive, &FactSet::default());
    assert_eq!(
        primitive_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_primitive_target",
            "unsupported Java instanceof check: primitive target type Primitive(Int)",
        )]
    );

    let raw_comparator = RustSlotShape::conservative(JavaType::Named {
        name: "Comparator".to_string(),
        args: Vec::new(),
    });
    let raw_check = plan_instanceof_check_with_facts(&actual, &raw_comparator, &FactSet::default());
    assert_eq!(
        raw_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_raw_or_wildcard_target",
            "unsupported Java instanceof check: raw or wildcard target type Named { name: \"Comparator\", args: [] }",
        )]
    );

    let wildcard = RustSlotShape::conservative(JavaType::Named {
        name: "Comparator".to_string(),
        args: vec![JavaType::Wildcard(None)],
    });
    let wildcard_check = plan_instanceof_check_with_facts(&actual, &wildcard, &FactSet::default());
    assert_eq!(
        wildcard_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_raw_or_wildcard_target",
            "unsupported Java instanceof check: raw or wildcard target type Named { name: \"Comparator\", args: [Wildcard(None)] }",
        )]
    );

    let array_target = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    let missing_check =
        plan_instanceof_check_with_facts(&actual, &array_target, &FactSet::default());
    assert_eq!(
        missing_check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_missing_runtime_target",
            "unsupported Java instanceof check: target type Array(String) has no concrete runtime type",
        )]
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

    let raw_check = plan_instanceof_check_with_facts(&actual, &raw_list, &FactSet::default());
    assert_eq!(
        raw_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Vec<Box<dyn std::any::Any>>".to_string(),
        }
    );
    let wildcard_check =
        plan_instanceof_check_with_facts(&actual, &wildcard_list, &FactSet::default());
    assert_eq!(
        wildcard_check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Vec<Box<dyn std::any::Any>>".to_string(),
        }
    );
}

#[test]
fn instanceof_allows_erased_object_array_runtime_target() {
    let mut actual = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    actual.dispatch = DispatchShape::ErasedObject;
    let object_array = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    })));

    let check = plan_instanceof_check_with_facts(&actual, &object_array, &FactSet::default());

    assert_eq!(
        check,
        InstanceOfCheck::ErasedObjectType {
            target_type: "Vec<Option<Box<dyn std::any::Any>>>".to_string(),
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
        hierarchy_instanceof_policy(&dog_shape, &animal_closed_shape, &facts).always_true(),
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

    let check = plan_instanceof_check_with_facts(&actual, &target, &FactSet::default());
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
        vec![CoercionDiagnostic::new(
            "unsupported_instanceof_shape",
            "unsupported Java instanceof shape: actual=Concrete, target=ClosedEnum { enum_name: \"AnimalValue\" }",
        )]
    );
    assert_eq!(
        check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_shape",
            "unsupported Java instanceof shape: actual=Concrete, target=ClosedEnum { enum_name: \"AnimalValue\" }",
        )]
    );
}

#[test]
fn concrete_source_supertype_instanceof_subtype_requires_runtime_carrier() {
    let actual_ty = JavaType::Named {
        name: "HeaderLine".to_string(),
        args: Vec::new(),
    };
    let target_ty = JavaType::Named {
        name: "FormatLine".to_string(),
        args: Vec::new(),
    };
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "FormatLine".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "HeaderLine".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    let actual = RustSlotShape::conservative(actual_ty.clone());
    let target = RustSlotShape::conservative(target_ty.clone());

    let check = plan_instanceof_check_with_facts(&actual, &target, &facts);

    assert_eq!(
        check,
        InstanceOfCheck::Unsupported {
            actual_dispatch: DispatchShape::Concrete,
            target_dispatch: DispatchShape::Concrete,
            policy: InstanceOfFallbackPolicy::SourceHierarchyRuntimeCarrierRequired {
                actual: actual_ty,
                target: target_ty,
            },
        }
    );
    assert_eq!(
        check.unsupported_diagnostics(),
        vec![ConversionDiagnostic::new(
            "unsupported_instanceof_source_hierarchy_runtime_carrier",
            "unsupported Java instanceof check from source supertype Named { name: \"HeaderLine\", args: [] } to subtype Named { name: \"FormatLine\", args: [] }: a member-preserving runtime carrier is required"
        )]
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

    let policy = dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default());

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
        vec![CoercionDiagnostic::new("unsupported_dispatch_missing_runtime_target", "unsupported Java dispatch coercion: target type Array(String) has no concrete runtime type"
                    .to_string())]
    );
    assert_compile_error_block_consuming(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("animal".to_string()),
            actual,
            &Slot::Argument(target),
            &FactSet::default(),
        ),
        "animal",
        "unsupported Java dispatch coercion: target type Array(String) has no concrete runtime type",
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

    let check = plan_instanceof_check_with_facts(&actual, &target, &FactSet::default());

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
        vec![CoercionDiagnostic::new(
            "unsupported_instanceof_raw_or_wildcard_target",
            "unsupported Java instanceof check: raw or wildcard target type Wildcard(None)"
                .to_string()
        )]
    );
}

#[test]
fn unsupported_coercion_message_policy_escapes_compile_error_text() {
    let quoted_message = UnsupportedCoercionMessagePolicy::new(
        "quoted_message",
        "unsupported coercion for \"pkg\\Type\"",
    );
    assert_eq!(
        quoted_message.compile_error_expr(),
        RustExpr::MacroCall {
            macro_name: "compile_error".to_string(),
            args: vec![RustExpr::Literal(
                "\"unsupported coercion for \\\"pkg\\\\Type\\\"\"".to_string()
            )],
        }
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

    let dyn_check = plan_instanceof_check_with_facts(&actual, &dyn_target, &FactSet::default());
    let boxed_check = plan_instanceof_check_with_facts(&actual, &boxed_target, &FactSet::default());

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
        vec![CoercionDiagnostic::new("unsupported_erased_object_instanceof_dyn_trait", "unsupported Java erased object instanceof borrowed dyn trait check for Animal; a concrete runtime target is required"
                    .to_string())]
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
        vec![CoercionDiagnostic::new("unsupported_erased_object_instanceof_boxed_dyn_trait", "unsupported Java erased object instanceof owned boxed dyn trait check for Animal; a concrete runtime target is required"
                    .to_string())]
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
        dispatch_coercion_policy_with_facts(&dyn_int, &int_target, &FactSet::default()),
        DispatchCoercionPolicy::NoDispatchChange
    );
    dyn_int.java_ty = JavaType::Unknown("external_getter".to_string());
    assert_eq!(
        dispatch_coercion_policy_with_facts(&dyn_int, &int_target, &FactSet::default()),
        DispatchCoercionPolicy::NoDispatchChange
    );

    let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)));
    let mut erased_array = RustSlotShape::conservative(array_ty.clone());
    erased_array.dispatch = DispatchShape::ErasedObject;
    let array_target = RustSlotShape::conservative(array_ty);

    assert_eq!(
        dispatch_coercion_policy_with_facts(&erased_array, &array_target, &FactSet::default()),
        DispatchCoercionPolicy::NoDispatchChange
    );
    erased_array.java_ty = JavaType::Unknown("external_byte_array_getter".to_string());
    assert_eq!(
        dispatch_coercion_policy_with_facts(&erased_array, &array_target, &FactSet::default()),
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
        dispatch_coercion_policy_with_facts(&dog, &animal_enum, &FactSet::default()),
        DispatchCoercionPolicy::WrapClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variant_name: "Dog".to_string(),
        }
    );
    assert_eq!(
        dispatch_coercion_policy_with_facts(&dog, &animal_boxed, &FactSet::default()),
        DispatchCoercionPolicy::BoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(
        dispatch_coercion_policy_with_facts(&animal_dyn, &dog, &FactSet::default()),
        DispatchCoercionPolicy::DowncastDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }
    );
    assert_eq!(
        dispatch_coercion_policy_with_facts(&animal_boxed, &dog, &FactSet::default()),
        DispatchCoercionPolicy::DowncastBoxedDynTrait {
            target_type: "Dog".to_string(),
            invalid_cast: InvalidCastPolicy::Panic,
        }
    );
    let unsupported =
        dispatch_coercion_policy_with_facts(&animal_enum, &animal_dyn, &FactSet::default());
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
        vec![CoercionDiagnostic::new(
            "unsupported_dispatch_coercion",
            "unsupported Java dispatch coercion: actual=ClosedEnum { enum_name: \"AnimalValue\" }, target=DynTrait { trait_name: \"Animal\" }",
        )]
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
        plan_shape_coercion_with_facts(&actual, &dyn_target, &FactSet::default()),
        vec![Coercion::BorrowDynTrait {
            trait_name: "Animal".to_string(),
            mutable: false,
        }]
    );
    let dyn_borrow = apply_expr_to_slot_with_facts(
        RustExpr::Var("dog".to_string()),
        actual.clone(),
        &Slot::Argument(dyn_target),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&dyn_borrow), "(&dog) as &dyn Animal");
    assert!(matches!(
        dyn_borrow.clone().into_unshaped(),
        RustExpr::Cast { expr, ty }
            if ty == "&dyn Animal"
                && matches!(
                    expr.unshaped(),
                    RustExpr::Unary { op, expr } if op == "&" && matches!(expr.unshaped(), RustExpr::Var(name) if name == "dog")
                )
    ));
    assert_eq!(dyn_borrow.recorded_shape(), Some(RustExprShape::Borrowed));

    let mut boxed_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &boxed_target, &FactSet::default()),
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
        plan_shape_coercion_with_facts(&actual, &erased_target, &FactSet::default()),
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
        plan_shape_coercion_with_facts(&dyn_actual, &downcast_target, &FactSet::default()),
        vec![
            Coercion::DowncastDynTrait {
                target_type: "Dog".to_string(),
                invalid_cast: InvalidCastPolicy::Panic,
            },
            Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: JavaType::Named {
                        name: "Dog".to_string(),
                        args: Vec::new(),
                    },
                },
            },
        ]
    );
    let dyn_downcast = apply_expr_to_slot_with_facts(
        RustExpr::Var("animal".to_string()),
        dyn_actual.clone(),
        &Slot::Argument(downcast_target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&dyn_downcast),
        "std::clone::Clone::clone(animal.as_any().downcast_ref::<Dog>().expect(\"invalid Java cast\"))"
    );
    assert!(matches!(dyn_downcast, RustExpr::Shaped { .. }));

    let mut boxed_downcast_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_downcast_actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    assert_eq!(
        plan_shape_coercion_with_facts(
            &boxed_downcast_actual,
            &downcast_target,
            &FactSet::default()
        ),
        vec![
            Coercion::DowncastBoxedDynTrait {
                target_type: "Dog".to_string(),
                invalid_cast: InvalidCastPolicy::Panic,
            },
            Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: JavaType::Named {
                        name: "Dog".to_string(),
                        args: Vec::new(),
                    },
                },
            },
        ]
    );
    let boxed_downcast = apply_expr_to_slot_with_facts(
        RustExpr::Var("animal".to_string()),
        boxed_downcast_actual.clone(),
        &Slot::Argument(downcast_target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&boxed_downcast),
        "std::clone::Clone::clone(animal.as_ref().as_any().downcast_ref::<Dog>().expect(\"invalid Java cast\"))"
    );
    assert!(matches!(boxed_downcast, RustExpr::Shaped { .. }));

    let mut erased_actual = RustSlotShape::conservative(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    });
    erased_actual.dispatch = DispatchShape::ErasedObject;
    assert_eq!(
        plan_shape_coercion_with_facts(&erased_actual, &downcast_target, &FactSet::default()),
        vec![Coercion::DowncastErasedObject {
            target_type: "Dog".to_string(),
            borrowed: false,
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    let object_downcast = apply_expr_to_slot_with_facts(
        RustExpr::Var("object".to_string()),
        erased_actual,
        &Slot::Argument(downcast_target),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&object_downcast),
        "(*object.downcast::<Dog>().expect(\"invalid Java cast\"))"
    );
    assert!(
        matches!(
            object_downcast,
            RustExpr::Grouped(grouped)
                if matches!(
                    grouped.as_ref(),
                    RustExpr::Unary { op, expr }
                        if op == "*"
                            && matches!(
                                expr.as_ref(),
                                RustExpr::MethodCall { method, .. } if method == "expect"
                            )
                )
        ),
        "erased object downcast should group the unary deref structurally"
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
        plan_shape_coercion_with_facts(&boxed_actual, &borrowed_dyn_target, &FactSet::default()),
        vec![Coercion::BorrowBoxedDynTrait {
            trait_name: "Animal".to_string(),
            mutable: false,
        }]
    );
    let boxed_dyn_borrow = apply_expr_to_slot_with_facts(
        RustExpr::Var("animal".to_string()),
        boxed_actual,
        &Slot::Argument(borrowed_dyn_target),
        &FactSet::default(),
    );
    assert_eq!(
        new_java2rust::emit::emit_expr(&boxed_dyn_borrow),
        "animal.as_ref() as &dyn Animal"
    );
    assert_eq!(
        boxed_dyn_borrow.recorded_shape(),
        Some(RustExprShape::Borrowed)
    );
    let boxed_dyn_clone_recovery = apply_coercions(
        RustExpr::Var("writer".to_string()),
        &[
            Coercion::BoxDynTrait {
                trait_name: "Animal".to_string(),
            },
            Coercion::CloneRecovery {
                policy: CloneRecoveryPolicy::AuditRequired,
            },
        ],
    )
    .into_unshaped();
    assert_eq!(
        emit_expr(&boxed_dyn_clone_recovery),
        "std::clone::Clone::clone(&(Box::new(writer) as Box<dyn Animal>))/* TODO(translation): validate added clone */"
    );
    assert!(matches!(
        boxed_dyn_clone_recovery,
        RustExpr::PostfixComment { expr, .. }
            if matches!(
                expr.as_ref(),
                RustExpr::Call { function, .. }
                    if emit_expr(function) == "std::clone::Clone::clone"
            )
    ));

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
    let borrowed_dyn_to_owned_box = dispatch_coercion_policy_with_facts(
        &borrowed_dyn_actual,
        &owned_boxed_target,
        &FactSet::default(),
    );
    assert_eq!(
        borrowed_dyn_to_owned_box,
        DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }
    );
    assert_eq!(borrowed_dyn_to_owned_box.diagnostics(), Vec::new());
    assert_eq!(
        plan_shape_coercion_with_facts(
            &borrowed_dyn_actual,
            &owned_boxed_target,
            &FactSet::default()
        ),
        vec![Coercion::CloneBoxDynTrait {
            trait_name: "Animal".to_string(),
        }]
    );
    let cloned_box = apply_expr_to_slot_with_facts(
        RustExpr::Var("animal".to_string()),
        borrowed_dyn_actual,
        &Slot::Argument(owned_boxed_target),
        &FactSet::default(),
    );
    assert_eq!(emit_expr(&cloned_box), "animal.clone_box()");
    assert!(matches!(
        cloned_box.clone().into_unshaped(),
        RustExpr::MethodCall { receiver, method, .. }
            if method == "clone_box"
                && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "animal")
    ));
    assert_eq!(
        cloned_box.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
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
    let animal_ty = JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    };
    let recovery_policy = coercion_owned_recovery_capability_policy_with_effective_facts(
        &animal_ty,
        &EffectiveFacts::new(&non_clone_facts),
    );
    assert_eq!(recovery_policy.java_ty(), &animal_ty);
    assert_eq!(
        recovery_policy.recovery(),
        OwnedRecoveryCapability::NonCloneable
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
fn shape_coercion_recovers_erased_object_into_source_hierarchy_carrier() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    let mut actual = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    actual.dispatch = DispatchShape::ErasedObject;
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };

    assert_eq!(
        plan_shape_coercion_with_facts(&actual, &target, &facts),
        vec![Coercion::DowncastErasedObjectToClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Animal".to_string(), "Dog".to_string()],
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    let recovered = apply_expr_to_slot_with_facts(
        RustExpr::Var("object".to_string()),
        actual,
        &Slot::Assignment(target),
        &facts,
    );
    let rendered = emit_expr(&recovered);
    assert!(
        rendered.contains("if __java_erased_object.is::<Animal>()"),
        "{rendered}"
    );
    assert!(
        rendered.contains("AnimalValue::Animal(std::clone::Clone::clone(__java_erased_object.downcast_ref::<Animal>().expect(\"invalid Java cast\")))"),
        "{rendered}"
    );
    assert!(
        rendered.contains("AnimalValue::Dog(std::clone::Clone::clone(__java_erased_object.downcast_ref::<Dog>().expect(\"invalid Java cast\")))"),
        "{rendered}"
    );
}

#[test]
fn instanceof_recovers_erased_object_against_source_hierarchy_carrier() {
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "Dog".to_string(),
            copy: None,
            clone: None,
            string_like: None,
            sequence_like: None,
            implements: Vec::new(),
            extends: Some(JavaType::Named {
                name: "Animal".to_string(),
                args: Vec::new(),
            }),
            final_type: None,
            object_safe_trait: None,
            provenance: FactProvenance::ParsedSource,
        });
    let mut actual = RustSlotShape::conservative(JavaType::Unknown("Object".to_string()));
    actual.dispatch = DispatchShape::ErasedObject;
    let mut target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    target.dispatch = DispatchShape::ClosedEnum {
        enum_name: "AnimalValue".to_string(),
    };

    let check = plan_instanceof_check_with_facts(&actual, &target, &facts);
    assert_eq!(
        check,
        InstanceOfCheck::ErasedObjectClosedEnum {
            enum_name: "AnimalValue".to_string(),
            carrier_type: "AnimalValue".to_string(),
            variants: vec!["Animal".to_string(), "Dog".to_string()],
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
        plan_shape_coercion_with_facts(&actual, &target, &FactSet::default()),
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
    assert_compile_error_block_consuming(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("animal".to_string()),
            actual,
            &Slot::Argument(target),
            &FactSet::default(),
        ),
        "animal",
        "unsupported Java dispatch coercion: actual=ClosedEnum { enum_name: \"AnimalValue\" }, target=DynTrait { trait_name: \"Animal\" }",
    );
}

#[test]
fn erased_object_runtime_contract_covers_carrier_downcast_instanceof_and_recovery() {
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
        plan_shape_coercion_with_facts(&concrete, &erased, &FactSet::default()),
        vec![Coercion::EraseObject]
    );
    let erased_dog = apply_expr_to_slot_with_facts(
        RustExpr::Var("dog".to_string()),
        concrete.clone(),
        &Slot::Argument(erased.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&erased_dog),
        "Box::new(dog) as Box<dyn std::any::Any>"
    );
    assert_eq!(
        erased_dog,
        RustExpr::Cast {
            expr: Box::new(RustExpr::Call {
                function: Box::new(RustExpr::path(["Box", "new"])),
                args: vec![RustExpr::Var("dog".to_string())],
            }),
            ty: "Box<dyn std::any::Any>".to_string(),
        }
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&erased, &concrete, &FactSet::default()),
        vec![Coercion::DowncastErasedObject {
            target_type: "Dog".to_string(),
            borrowed: false,
            invalid_cast: InvalidCastPolicy::Panic,
        }]
    );
    assert_eq!(
        dispatch_coercion_policy_with_facts(&erased, &dyn_target, &FactSet::default()),
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
        dispatch_coercion_policy_with_facts(&erased, &boxed_dyn_target, &FactSet::default()),
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

    for slot in [
        Slot::Assignment(erased_object_decl.clone()),
        Slot::Argument(erased_object_decl.clone()),
        Slot::Return(erased_object_decl.clone()),
    ] {
        assert_eq!(
            slot_conversion_plan_with_facts(&erased, &slot, &FactSet::default()),
            SlotConversionPlan::Shape(ShapeCoercionPlan::new(
                erased.clone(),
                erased_object_decl.clone(),
                DispatchCoercionPolicy::NoDispatchChange,
                ViewCoercionPolicy::NoViewChange,
                OptionalShapeCoercionPolicy::NoChange,
                OwnershipCoercionPolicy::NoOwnershipChange,
                None,
            ))
        );
        assert_eq!(
            apply_expr_to_slot_with_facts(
                RustExpr::Var("object".to_string()),
                erased.clone(),
                &slot,
                &FactSet::default(),
            ),
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
    let erased_to_borrowed_dyn = dispatch_coercion_policy_with_facts(
        &erased_actual,
        &borrowed_dyn_target,
        &FactSet::default(),
    );

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
        vec![CoercionDiagnostic::new("unsupported_erased_object_to_dyn_trait", "unsupported Java erased object to borrowed dyn trait recovery for Animal; a concrete downcast target is required"
                    .to_string())]
    );
    assert_compile_error_block_consuming(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("object".to_string()),
            erased_actual.clone(),
            &Slot::Argument(borrowed_dyn_target),
            &FactSet::default(),
        ),
        "object",
        "unsupported Java erased object to borrowed dyn trait recovery for Animal; a concrete downcast target is required",
    );

    let mut boxed_dyn_target = RustSlotShape::conservative(JavaType::Named {
        name: "Animal".to_string(),
        args: Vec::new(),
    });
    boxed_dyn_target.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "Animal".to_string(),
    };
    let erased_to_boxed_dyn =
        dispatch_coercion_policy_with_facts(&erased_actual, &boxed_dyn_target, &FactSet::default());

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
        vec![CoercionDiagnostic::new("unsupported_erased_object_to_boxed_dyn_trait", "unsupported Java erased object to owned boxed dyn trait recovery for Animal; a concrete downcast target and ownership proof are required"
                    .to_string())]
    );
    assert_compile_error_block_consuming(
        apply_expr_to_slot_with_facts(
            RustExpr::Var("object".to_string()),
            erased_actual,
            &Slot::Argument(boxed_dyn_target),
            &FactSet::default(),
        ),
        "object",
        "unsupported Java erased object to owned boxed dyn trait recovery for Animal; a concrete downcast target and ownership proof are required",
    );
}

#[test]
fn coercion_owns_borrowed_optional_to_owned_optional_slot() {
    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.iter().next()".to_string()),
            LoweredType::optional_element_borrow(JavaType::String),
        ),
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::String,
        ))),
        &FactSet::default(),
    );

    assert_eq!(
        expr.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(values_iter_next_expr()),
            method: "cloned".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
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

    assert!(
        optional_element_target_match_policy(&option_target, &option_shape, &string_element)
            .matches()
    );
    assert!(
        !optional_element_target_match_policy(&int_option_target, &option_shape, &string_element)
            .matches()
    );
    assert!(vec_element_target_match_policy(&vec_target, &vec_shape, &string_element).matches());
    assert!(
        !vec_element_target_match_policy(&int_vec_target, &vec_shape, &string_element).matches()
    );
}

#[test]
fn coercion_uses_copy_capability_for_borrowed_optional_elements() {
    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.iter().next()".to_string()),
            LoweredType::optional_element_borrow(JavaType::Primitive(PrimitiveType::Int)),
        ),
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            JavaType::Primitive(PrimitiveType::Int),
        ))),
        &FactSet::default(),
    );

    assert_eq!(
        expr.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(values_iter_next_expr()),
            method: "copied".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
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
        LoweredExpr::new(
            RustExpr::Raw("values.iter().next()".to_string()),
            LoweredType::optional_element_borrow(external_id.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(optional_java_type(external_id))),
        &facts,
    );

    assert_eq!(
        expr.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(values_iter_next_expr()),
            method: "copied".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
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
        vec![CoercionDiagnostic::new(
            "unsupported_borrowed_element_non_cloneable",
            "unsupported Java borrowed element ownership recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }",
        )]
    );
    assert_eq!(
        unsupported.unsupported_diagnostics(&handle_shape),
        vec![ConversionDiagnostic::new(
            "unsupported_borrowed_element_non_cloneable",
            "unsupported Java borrowed element ownership recovery for non-cloneable type Named { name: \"ExternalHandle\", args: [] }"
        )]
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
        vec![CoercionDiagnostic::new(
            "borrowed_element_clone_audit_required",
            "borrowed Java element recovery inserted cloned() and requires ownership audit"
                .to_string()
        )]
    );
    // Compared through `into_unshaped`: the producer now records
    // `RustExprShape::OwnedRecovery` so consumers can ask the tag instead of
    // matching the emitted comment. The tag is invisible to the emitter, so the
    // generated Rust asserted here is unchanged.
    let adapted = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.iter().next()".to_string()),
            LoweredType::optional_element_borrow(element_ty.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(optional_java_type(element_ty))),
        &FactSet::default(),
    );
    assert_eq!(
        emit::emit_expr(&adapted),
        "values.iter().next().cloned()/* TODO(translation): validate added clone */"
    );
    assert!(
        matches!(
            adapted.into_unshaped(),
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { method, .. } if method == "cloned"
                )
        ),
        "borrowed optional clone recovery should keep cloned() structured"
    );
}

#[test]
fn borrowed_sequence_ownership_recovery_uses_to_vec() {
    let java_array = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Double)));
    let mut actual = RustSlotShape::conservative(java_array.clone());
    actual.ownership = OwnershipShape::SharedBorrow;
    let target = RustSlotShape::conservative(java_array);

    let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());

    assert!(matches!(
        plan.ownership(),
        OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { .. }
    ));
    assert_eq!(
        plan.coercions(),
        vec![Coercion::ToOwnedSequence {
            policy: CloneRecoveryPolicy::AuditRequired,
        }]
    );
    let recovered = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("Table::VALUES[index as usize]".to_string()),
            LoweredType::Slot(Box::new(actual.clone())),
        ),
        &Slot::Return(target.clone()),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&recovered),
        "(Table::VALUES[index as usize]).to_vec()/* TODO(translation): validate added clone */"
    );
    assert!(matches!(
        recovered.clone().into_unshaped(),
        RustExpr::PostfixComment { expr, .. }
            if matches!(
                expr.as_ref(),
                RustExpr::MethodCall { receiver, method, .. }
                    if method == "to_vec"
                        && matches!(
                            receiver.as_ref(),
                            RustExpr::Grouped(grouped)
                                if matches!(
                                    grouped.as_ref(),
                                    RustExpr::Index(array, index)
                                        if emit_expr(array) == "Table::VALUES"
                                            && matches!(
                                                index.as_ref(),
                                                RustExpr::Cast { expr, ty }
                                                    if ty == "usize"
                                                        && matches!(
                                                            expr.as_ref(),
                                                            RustExpr::Var(name) if name == "index"
                                                        )
                                            )
                                )
                        )
            )
    ));
    assert!(
        !matches!(recovered.clone().into_unshaped(), RustExpr::Raw(_)),
        "borrowed sequence to_vec recovery should keep audited method call structured"
    );
    assert_eq!(
        recovered.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );

    let already_recovered = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.to_vec()".to_string()),
            LoweredType::Slot(Box::new(actual)),
        ),
        &Slot::Return(target),
        &FactSet::default(),
    );
    assert_eq!(
        already_recovered.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "to_vec".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        already_recovered.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
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
    assert_eq!(option_policy.target(), &option_target);
    assert_eq!(option_policy.carrier_shape(), &option_shape);
    assert_eq!(option_policy.element_shape(), &string_element);
    assert_eq!(option_policy.carrier(), BorrowedElementCarrier::Option);
    assert!(option_policy.matches());
    assert!(option_policy.adapter().is_none());

    let concrete_option_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Option,
        &string_element,
        &RustSlotShape::conservative(optional_java_type(JavaType::String)),
        &string_element,
        &facts,
    );
    assert_eq!(concrete_option_policy.matches(), true);
    assert_eq!(
        concrete_option_policy
            .adapter()
            .map(BorrowedElementAdapterPolicy::unwrap_carrier),
        Some(true)
    );

    let vec_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Vec,
        &vec_target,
        &vec_shape,
        &string_element,
        &facts,
    );
    assert_eq!(vec_policy.matches(), true);
    assert_eq!(
        vec_policy.adapter().unwrap().carrier(),
        BorrowedElementCarrier::Vec
    );

    let set_policy = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Set {
            kind: new_java2rust::stdlib::JavaCollectionKind::Set,
            semantic_equality: false,
        },
        &set_target,
        &set_shape,
        &string_element,
        &facts,
    );
    assert_eq!(set_policy.matches(), true);
    assert_eq!(
        set_policy.adapter().unwrap().carrier(),
        BorrowedElementCarrier::Set {
            kind: new_java2rust::stdlib::JavaCollectionKind::Set,
            semantic_equality: false,
        }
    );

    let mismatch = borrowed_element_slot_adapter_policy(
        BorrowedElementCarrier::Vec,
        &option_target,
        &vec_shape,
        &string_element,
        &facts,
    );
    assert_eq!(mismatch.matches(), false);
    assert_eq!(mismatch.adapter(), None);
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

    let SlotConversionPlan::BorrowedElement(policy) = &plan else {
        panic!("expected borrowed element policy");
    };
    assert_eq!(policy.target(), &target);
    assert_eq!(policy.carrier_shape(), &option_shape);
    assert_eq!(policy.element_shape(), &element_shape);
    assert_eq!(policy.carrier(), BorrowedElementCarrier::Option);
    assert!(policy.matches());
    let adapter = policy.adapter().expect("expected borrowed element adapter");
    assert_eq!(adapter.carrier(), BorrowedElementCarrier::Option);
    assert_eq!(adapter.element_shape(), &element_shape);
    assert_eq!(
        adapter.recovery(),
        &BorrowedElementRecoveryPolicy::CopyElements
    );
    assert!(adapter.unwrap_carrier());
    let projected =
        apply_slot_conversion_plan(RustExpr::Raw("values.get(&key)".to_string()), &plan);
    assert_eq!(
        emit_expr(&projected),
        "values.get(&key).copied().expect(\"Java null receiver\")"
    );
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "get".to_string(),
                        args: vec![RustExpr::Unary {
                            op: "&".to_string(),
                            expr: Box::new(RustExpr::Var("key".to_string())),
                        }],
                    }),
                    method: "copied".to_string(),
                    args: Vec::new(),
                },
            )),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        }
    );
    assert_eq!(
        projected.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
}

#[test]
fn optional_borrowed_element_unwrap_records_same_target_shape() {
    let external_value = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let mut element_shape = RustSlotShape::conservative(external_value.clone());
    element_shape.ownership = OwnershipShape::SharedBorrow;
    let option_shape = RustSlotShape::conservative(optional_java_type(external_value.clone()));
    let mut target = RustSlotShape::conservative(external_value.clone());
    target.ownership = OwnershipShape::SharedBorrow;

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        },
        &Slot::Argument(target.clone()),
        &FactSet::default(),
    );

    let projected =
        apply_slot_conversion_plan(RustExpr::Raw("values.get(&key)".to_string()), &plan);
    assert_eq!(
        projected.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "get".to_string(),
                args: vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("key".to_string())),
                }],
            }),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        }
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("optional borrowed unwrap should record its produced shape");
    assert_eq!(produced.java_ty, external_value);
    assert_eq!(produced.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(produced.view, target.view);
}

#[test]
fn optional_owned_element_move_unwrap_preserves_expr_structure() {
    let external_value = JavaType::Named {
        name: "ExternalValue".to_string(),
        args: Vec::new(),
    };
    let element_shape = RustSlotShape::conservative(external_value.clone());
    let option_shape = RustSlotShape::conservative(optional_java_type(external_value.clone()));
    let target = RustSlotShape::conservative(external_value);

    let plan = lowered_slot_conversion_plan_with_facts(
        &LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        },
        &Slot::Assignment(target),
        &FactSet::default(),
    );

    let projected = apply_slot_conversion_plan(RustExpr::Var("maybe".to_string()), &plan);
    assert_eq!(
        emit_expr(&projected),
        "maybe.expect(\"Java null receiver\")"
    );
    assert_eq!(
        projected.into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("maybe".to_string())),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        }
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

    let SlotConversionPlan::BorrowedElement(policy) = &plan else {
        panic!("expected borrowed element policy");
    };
    assert_eq!(policy.target(), &target);
    assert_eq!(policy.carrier_shape(), &option_shape);
    assert_eq!(policy.element_shape(), &element_shape);
    assert_eq!(policy.carrier(), BorrowedElementCarrier::Option);
    assert!(policy.matches());
    let adapter = policy.adapter().expect("expected borrowed element adapter");
    assert_eq!(adapter.carrier(), BorrowedElementCarrier::Option);
    assert_eq!(adapter.element_shape(), &expected_element_shape);
    assert_eq!(
        adapter.recovery(),
        &BorrowedElementRecoveryPolicy::BorrowElement
    );
    assert!(adapter.unwrap_carrier());
    let projected =
        apply_slot_conversion_plan(RustExpr::Raw("intervals.get(0)".to_string()), &plan);
    let projected_unshaped = projected.clone().into_unshaped();
    assert!(
        matches!(
            projected_unshaped,
            RustExpr::Cast { ref ty, ref expr }
                if ty == "&dyn Locatable"
                    && expr_unshaped_eq(expr, &intervals_get_zero_expect_expr())
        ),
        "dyn trait target adapter should compose the cast structurally: {projected_unshaped:?}"
    );
    assert_eq!(
        emit_expr(&projected),
        "intervals.get(0).expect(\"Java null receiver\") as &dyn Locatable"
    );
    let produced = projected
        .recorded_slot_shape()
        .expect("dyn trait target adapter should record the produced target slot");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
    assert_eq!(produced.dispatch, target.dispatch);
    let projected_again = apply_slot_conversion_plan(projected, &plan);
    assert_eq!(
        emit_expr(&projected_again),
        "intervals.get(0).expect(\"Java null receiver\") as &dyn Locatable"
    );
    assert!(matches!(
        projected_again.into_unshaped(),
        RustExpr::Cast { ty, .. } if ty == "&dyn Locatable"
    ));
}

#[test]
fn optional_boxed_dyn_trait_maps_borrow_over_payload() {
    let block_data = JavaType::Named {
        name: "BlockData".to_string(),
        args: Vec::new(),
    };
    let optional_block_data = optional_java_type(block_data.clone());
    let mut actual = RustSlotShape::conservative(optional_block_data.clone());
    actual.dispatch = DispatchShape::BoxedDynTrait {
        trait_name: "BlockData".to_string(),
    };
    actual.view = ViewShape::DynTraitView;
    let mut target = RustSlotShape::conservative(optional_block_data);
    target.ownership = OwnershipShape::SharedBorrow;
    target.dispatch = DispatchShape::DynTrait {
        trait_name: "BlockData".to_string(),
    };
    target.view = ViewShape::DynTraitView;
    let mut facts = FactSet::default();
    facts
        .type_capability_overrides
        .push(TypeCapabilityOverride {
            type_name: "BlockData".to_string(),
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

    let plan = shape_coercion_plan_with_facts(&actual, &target, &facts);
    let OptionalShapeCoercionPolicy::MapElement { element_plan, .. } = plan.optional() else {
        panic!("nullable boxed-to-borrowed trait conversion must map its payload");
    };
    assert_eq!(
        element_plan.dispatch(),
        &DispatchCoercionPolicy::BorrowBoxedDynTrait {
            trait_name: "BlockData".to_string(),
            mutable: false,
        }
    );
    assert_eq!(
        emit_expr(&apply_shape_coercion_plan(
            RustExpr::Var("block_data".to_string()),
            &plan,
        )),
        "block_data.as_ref().map(|value| value.as_ref() as &dyn BlockData)"
    );

    let borrowed_plan = shape_coercion_plan_with_facts(&target, &target, &facts);
    assert_eq!(
        emit_expr(&apply_shape_coercion_plan(
            RustExpr::Var("block_data".to_string()),
            &borrowed_plan,
        )),
        "block_data",
        "an Option<&dyn Trait> argument must not be reinterpreted as Option<Box<dyn Trait>>"
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
        &Slot::Argument(target.clone()),
        &facts,
    );

    let SlotConversionPlan::BorrowedElement(policy) = &plan else {
        panic!("expected borrowed element policy");
    };
    assert!(policy.matches());
    let adapter = policy.adapter().expect("expected borrowed element adapter");
    assert_eq!(
        adapter.recovery(),
        &BorrowedElementRecoveryPolicy::BorrowElement
    );
    assert!(adapter.unwrap_carrier());
    let projected =
        apply_slot_conversion_plan(RustExpr::Raw("intervals.get(0)".to_string()), &plan);
    assert_eq!(
        emit_expr(&projected),
        "{ let _java_value = intervals.get(0).expect(\"Java null receiver\"); &Locatable }"
    );
    assert!(matches!(
        projected.clone().into_unshaped(),
        RustExpr::Block { stmts, tail: Some(tail) }
            if matches!(
                stmts.as_slice(),
                [RustStmt::Let { name, value: Some(value), .. }]
                    if name == "_java_value"
                        && expr_unshaped_eq(value, &intervals_get_zero_expect_expr())
            ) && matches!(
                tail.as_ref(),
                RustExpr::Unary { op, expr }
                    if op == "&" && matches!(expr.as_ref(), RustExpr::Path(segments) if segments == &["Locatable".to_string()])
            )
    ));
    let produced = projected
        .recorded_slot_shape()
        .expect("concrete facade target adapter should record the produced target slot");
    assert_eq!(produced.java_ty, target.java_ty);
    assert_eq!(produced.ownership, target.ownership);
    assert_eq!(produced.view, target.view);
    assert_eq!(produced.dispatch, target.dispatch);
    let projected_again = apply_slot_conversion_plan(projected, &plan);
    assert_eq!(
        emit_expr(&projected_again),
        "{ let _java_value = intervals.get(0).expect(\"Java null receiver\"); &Locatable }"
    );
    assert!(matches!(
        projected_again.into_unshaped(),
        RustExpr::Block { tail: Some(tail), .. }
            if matches!(
                tail.as_ref(),
                RustExpr::Unary { op, expr }
                    if op == "&" && matches!(expr.as_ref(), RustExpr::Path(segments) if segments == &["Locatable".to_string()])
            )
    ));
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
        LoweredExpr::new(
            RustExpr::Raw("handles.iter().next()".to_string()),
            LoweredType::optional_element_borrow(external_handle.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(optional_java_type(
            external_handle,
        ))),
        &facts,
    );

    assert_compile_error_block_consuming(
        expr,
        "handles.iter().next()",
        "unsupported Java borrowed element ownership recovery for non-cloneable type",
    );
}

#[test]
fn coercion_uses_element_shape_for_borrowed_vec_string_view() {
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;
    element_shape.view = ViewShape::Str;

    let borrowed_string_view_vec = RustExpr::MethodCall {
        receiver: Box::new(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "iter".to_string(),
                args: Vec::new(),
            }),
            method: "map".to_string(),
            args: vec![RustExpr::Raw("|v| v.trim()".to_string())],
        }),
        method: "collect::<Vec<_>>".to_string(),
        args: Vec::new(),
    };
    let expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            borrowed_string_view_vec.clone(),
            LoweredType::vec_element_borrow(element_shape),
        ),
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
        &FactSet::default(),
    );

    assert_eq!(
        expr.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(borrowed_string_view_vec),
                    method: "into_iter".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "v".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("v".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    })),
                }],
            }),
            method: "collect::<Vec<_>>".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        emit_expr(&expr),
        "values.iter().map(|v| v.trim()).collect::<Vec<_>>().into_iter().map(|v| v.to_string()).collect::<Vec<_>>()"
    );
    assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));

    let mut copied_element_shape =
        RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    copied_element_shape.ownership = OwnershipShape::SharedBorrow;
    let borrowed_int_set = RustExpr::MethodCall {
        receiver: Box::new(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "iter".to_string(),
            args: Vec::new(),
        }),
        method: "collect::<std::collections::HashSet<_>>".to_string(),
        args: Vec::new(),
    };
    let copied_set = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            borrowed_int_set.clone(),
            LoweredType::set_element_borrow(copied_element_shape),
        ),
        &Slot::Return(RustSlotShape::conservative(set_java_type(
            JavaType::Primitive(PrimitiveType::Int),
        ))),
        &FactSet::default(),
    );
    assert_eq!(
        copied_set.clone().into_unshaped(),
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(borrowed_int_set),
                    method: "into_iter".to_string(),
                    args: Vec::new(),
                }),
                method: "copied".to_string(),
                args: Vec::new(),
            }),
            method: "collect::<std::collections::HashSet<_>>".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        emit_expr(&copied_set),
        "values.iter().collect::<std::collections::HashSet<_>>().into_iter().copied().collect::<std::collections::HashSet<_>>()"
    );
    assert_eq!(
        copied_set.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
}

#[test]
fn coercion_fuses_borrowed_collection_clone_recovery_into_terminal_collect() {
    let mut element_shape = RustSlotShape::conservative(JavaType::String);
    element_shape.ownership = OwnershipShape::SharedBorrow;

    let vec_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.iter().collect::<Vec<_>>()".to_string()),
            LoweredType::vec_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
        &FactSet::default(),
    );
    // `into_unshaped`: the clone-recovery producers record
    // `RustExprShape::OwnedRecovery`, which the emitter ignores, so the Rust
    // asserted here is unchanged.
    assert_eq!(
        emit_expr(&vec_expr),
        "values.iter().cloned().collect::<Vec<_>>()"
    );
    assert!(matches!(
        vec_expr.clone().into_unshaped(),
        RustExpr::MethodCall { method, receiver, .. }
            if method == "collect::<Vec<_>>"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { method, receiver, .. }
                        if method == "cloned"
                            && matches!(
                                receiver.as_ref(),
                                RustExpr::MethodCall { method, receiver, .. }
                                    if method == "iter"
                                        && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                            )
                )
    ));
    assert_eq!(
        vec_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );

    let structured_already_cloned_vec_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    }),
                    method: "cloned".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<Vec<_>>".to_string(),
                args: Vec::new(),
            },
            LoweredType::vec_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&structured_already_cloned_vec_expr),
        "values.iter().cloned().collect::<Vec<_>>()"
    );
    assert_eq!(
        structured_already_cloned_vec_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
    assert!(!matches!(
        structured_already_cloned_vec_expr.into_unshaped(),
        RustExpr::Raw(_)
    ));

    let tagged_vec_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::Raw("recover_values(values)".to_string()),
            ),
            LoweredType::vec_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(list_java_type(
            JavaType::String,
        ))),
        &FactSet::default(),
    );
    assert_eq!(
        tagged_vec_expr.clone().into_unshaped(),
        RustExpr::Raw("recover_values(values)".to_string())
    );
    assert_eq!(
        tagged_vec_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery),
        "a recorded owned recovery must not depend on a rendered clone suffix"
    );

    let set_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(RustExpr::Raw(
                "groups.iter().flat_map(|item| item.iter()).collect::<std::collections::HashSet<_>>()"
                    .to_string(),
            ), LoweredType::set_element_borrow(element_shape.clone())),
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&set_expr),
        "groups.iter().flat_map(|item| item.iter()).cloned().collect::<std::collections::HashSet<_>>()"
    );
    assert!(matches!(
        set_expr.clone().into_unshaped(),
        RustExpr::MethodCall { method, receiver, .. }
            if method == "collect::<std::collections::HashSet<_>>"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { method, .. } if method == "cloned"
                )
    ));
    assert_eq!(
        set_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );

    let simple_set_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::Raw("values.iter().collect::<std::collections::HashSet<_>>()".to_string()),
            LoweredType::set_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&simple_set_expr),
        "values.iter().cloned().collect::<std::collections::HashSet<_>>()"
    );
    assert!(matches!(
        simple_set_expr.clone().into_unshaped(),
        RustExpr::MethodCall { method, receiver, .. }
            if method == "collect::<std::collections::HashSet<_>>"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { method, receiver, .. }
                        if method == "cloned"
                            && matches!(
                                receiver.as_ref(),
                                RustExpr::MethodCall { method, receiver, .. }
                                    if method == "iter"
                                        && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                            )
                )
    ));

    let already_cloned_set_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("groups".to_string())),
                            method: "iter".to_string(),
                            args: Vec::new(),
                        }),
                        method: "flat_map".to_string(),
                        args: vec![RustExpr::Raw("|item| item.iter()".to_string())],
                    }),
                    method: "cloned".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<std::collections::HashSet<_>>".to_string(),
                args: Vec::new(),
            },
            LoweredType::set_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
        &FactSet::default(),
    );
    assert!(matches!(
        already_cloned_set_expr.clone().into_unshaped(),
        RustExpr::MethodCall { method, receiver, .. }
            if method == "collect::<std::collections::HashSet<_>>"
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { method, receiver, .. }
                        if method == "cloned"
                            && matches!(
                                receiver.as_ref(),
                                RustExpr::MethodCall { method, args, .. }
                                    if method == "flat_map"
                                        && matches!(args.as_slice(), [RustExpr::Raw(raw)] if raw == "|item| item.iter()")
                            )
                )
    ));
    assert_eq!(
        already_cloned_set_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );
    let structured_already_cloned_set_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("groups".to_string())),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    }),
                    method: "cloned".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<std::collections::HashSet<_>>".to_string(),
                args: Vec::new(),
            },
            LoweredType::set_element_borrow(element_shape.clone()),
        ),
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
        &FactSet::default(),
    );
    assert_eq!(
        emit_expr(&structured_already_cloned_set_expr),
        "groups.iter().cloned().collect::<std::collections::HashSet<_>>()"
    );
    assert_eq!(
        structured_already_cloned_set_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery)
    );

    let tagged_set_expr = apply_lowered_expr_to_slot_with_facts(
        LoweredExpr::new(
            RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::Raw("recover_groups(groups)".to_string()),
            ),
            LoweredType::set_element_borrow(element_shape),
        ),
        &Slot::Return(RustSlotShape::conservative(set_java_type(JavaType::String))),
        &FactSet::default(),
    );
    assert_eq!(
        tagged_set_expr.clone().into_unshaped(),
        RustExpr::Raw("recover_groups(groups)".to_string())
    );
    assert_eq!(
        tagged_set_expr.recorded_shape(),
        Some(RustExprShape::OwnedRecovery),
        "a recorded set recovery must not depend on a rendered clone suffix"
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
        LoweredExpr::new(
            RustExpr::Raw("handles.iter().collect::<Vec<_>>()".to_string()),
            LoweredType::vec_element_borrow(element_shape),
        ),
        &Slot::Return(RustSlotShape::conservative(list_java_type(external_handle))),
        &facts,
    );

    assert_compile_error_block_consuming(
        expr,
        "handles.iter().collect::<Vec<_>>()",
        "unsupported Java borrowed element ownership recovery for non-cloneable type",
    );
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
