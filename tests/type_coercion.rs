use new_java2rust::coercion::{
    Coercion, CoercionDiagnostic, LoweredExpr, RustPrimitiveTypeNamePolicy, apply_coercions,
    apply_expr_to_slot, apply_expr_to_slot_with_facts, apply_instanceof_check,
    apply_lowered_expr_to_slot, apply_lowered_expr_to_slot_with_facts, apply_shape_coercion_plan,
    apply_slot_conversion_plan, apply_syntax_coercion_plan,
    borrowed_element_recovery_fallback_message_policy, dispatch_coercion_fallback_message_policy,
    instanceof_fallback_message_policy, ownership_coercion_fallback_message_policy, plan_coercion,
    plan_coercion_with_facts, plan_shape_coercion, plan_shape_coercion_with_facts,
    render_instanceof_check, rust_primitive_type_name_policy,
};
use new_java2rust::conversion_lowering::{
    apply_lowered_slot_expr_conversion, apply_slot_expr_conversion,
};
use new_java2rust::conversion_plan::{
    AssignmentValuePlan, BorrowedDynToOwnedBoxedDynRecoveryPolicy, BorrowedElementAdapterPolicy,
    BorrowedElementCarrier, BorrowedElementRecoveryFallbackPolicy, BorrowedElementRecoveryPolicy,
    BorrowedElementSlotAdapterPolicy, CloneRecoveryPolicy, CoercionContentPolicy,
    CoercionOwnedRecoveryCapabilityPolicy, CoercionShapeContentPolicy,
    DispatchCoercionFallbackPolicy, DispatchCoercionPolicy, InstanceOfCheck,
    InstanceOfFallbackPolicy, InvalidCastPolicy, LoweredSlotExprConversionPlan, LoweredType,
    OptionalElementTargetMatchPolicy, OptionalShapeCoercionPolicy, OwnershipCoercionFallbackPolicy,
    OwnershipCoercionPolicy, PrimitiveRankPolicy, PrimitiveWideningPolicy, ReturnValuePlan,
    RuntimeTypeTargetPolicy, RustNominalTypeNamePolicy, ShapeCoercionPlan, Slot,
    SlotConversionPlan, SlotExprConversionPlan, SyntaxCoercionPlan, SyntaxSlotCoercionPolicy,
    VecElementTargetMatchPolicy, ViewCoercionPolicy, actual_shape_for_slot_expr, argument_slot,
    assignment_slot, assignment_slot_for_target, assignment_value_plan_for_shape,
    assignment_value_plan_for_target, borrowed_element_adapter_policy,
    borrowed_element_recovery_policy, borrowed_element_recovery_policy_with_effective_facts,
    borrowed_element_slot_adapter_policy, coercion_content_policy,
    coercion_content_policy_with_effective_facts, coercion_owned_recovery_capability_policy,
    coercion_owned_recovery_capability_policy_with_effective_facts, coercion_shape_content_policy,
    coercion_shape_content_policy_with_effective_facts, condition_slot, dispatch_coercion_policy,
    dispatch_coercion_policy_with_facts, index_slot, lowered_slot_conversion_plan,
    lowered_slot_conversion_plan_with_facts, optional_element_target_match_policy,
    optional_shape_coercion_policy, optional_shape_coercion_policy_with_facts,
    ownership_coercion_policy, ownership_coercion_policy_with_effective_facts,
    plan_instanceof_check, plan_lowered_slot_conversion, plan_slot_conversion,
    primitive_rank_policy, primitive_widening_policy, return_slot, return_value_plan,
    runtime_type_target_policy, rust_nominal_type_name_policy, shape_coercion_plan,
    shape_coercion_plan_with_facts, slot_conversion_plan, slot_conversion_plan_with_facts,
    syntax_coercion_plan, syntax_coercion_plan_with_facts, syntax_slot_coercion_policy,
    vec_element_target_match_policy, view_coercion_policy,
    view_coercion_policy_with_effective_facts,
};
use new_java2rust::effective_facts::EffectiveFacts;
use new_java2rust::expr_shape::{ExprShapeContext, ShapeEnv};
use new_java2rust::external_packs::hdf5_pack_facts;
use new_java2rust::facts::{
    FactProvenance, FactSet, OwnedRecoveryCapability, TypeCapabilityOverride,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
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
use new_java2rust::types::{TypeEnv, infer_expr_type, list_java_type, optional_java_type};

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
    assert_eq!(
        runtime_type_target_policy(&JavaType::String),
        RuntimeTypeTargetPolicy::Nominal {
            target_type: "String".to_string(),
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
    assert!(render_source.contains("emit::emit_expr"));
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
        SyntaxSlotCoercionPolicy::ToStringForConcat
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
        SyntaxSlotCoercionPolicy::ToStringForConcat,
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
        vec![Coercion::ToStringForConcat]
    );
    assert_eq!(
        plan_coercion_with_facts(&external_id, &Slot::StringConcatSegment, &facts),
        vec![Coercion::Identity]
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

    let dyn_check = plan_instanceof_check(&dyn_actual, &target);
    let boxed_check = plan_instanceof_check(&boxed_actual, &target);
    let erased_check = plan_instanceof_check(&erased_actual, &target);

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
            policy: InstanceOfFallbackPolicy::MissingRuntimeTarget {
                java_ty: JavaType::Wildcard(None),
            },
        }
    );
    assert_eq!(
        check.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_instanceof_missing_runtime_target",
            message:
                "unsupported Java instanceof check: target type Wildcard(None) has no concrete runtime type"
                    .to_string(),
        }]
    );
    assert_eq!(
        render_instanceof_check(RustExpr::Var("animal".to_string()), &check),
        RustExpr::Raw(
            "compile_error!(\"unsupported Java instanceof check: target type Wildcard(None) has no concrete runtime type\")"
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
        RustExpr::Raw(format!("compile_error!(\"{}\")", instance_message.message))
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
            "{{ let _ = object; compile_error!(\"{}\") }}",
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
        ownership_message.compile_error_expr_consuming(&RustExpr::Var("value".to_string())),
        RustExpr::Raw(format!(
            "{{ let _ = value; compile_error!(\"{}\") }}",
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
            "{{ let _ = item; compile_error!(\"{}\") }}",
            element_message.message
        ))
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
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
                trait_name: "Animal".to_string(),
                recovery: BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                    policy: CloneRecoveryPolicy::AuditRequired,
                },
            },
        }
    );
    assert_eq!(
        borrowed_dyn_to_owned_box.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter",
            message:
                "unsupported Java borrowed dyn trait to owned boxed dyn recovery for Animal; only conservative clone proof exists, but dyn clone-box adapter generation is not implemented"
                    .to_string(),
        }]
    );
    assert_eq!(
        plan_shape_coercion(&borrowed_dyn_actual, &owned_boxed_target),
        vec![Coercion::UnsupportedDispatch {
            actual_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
                trait_name: "Animal".to_string(),
                recovery: BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                    policy: CloneRecoveryPolicy::AuditRequired,
                },
            },
        }]
    );
    assert_eq!(
        apply_expr_to_slot(
            RustExpr::Var("animal".to_string()),
            borrowed_dyn_actual,
            &Slot::Argument(owned_boxed_target)
        ),
        RustExpr::Raw(
            "{ let _ = animal; compile_error!(\"unsupported Java borrowed dyn trait to owned boxed dyn recovery for Animal; only conservative clone proof exists, but dyn clone-box adapter generation is not implemented\") }"
                .to_string()
        )
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
        DispatchCoercionPolicy::Unsupported {
            actual_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
                trait_name: "Animal".to_string(),
                recovery: BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                    policy: CloneRecoveryPolicy::ProvenCloneCapability {
                        java_ty: animal.clone(),
                    },
                },
            },
        }
    );
    assert_eq!(
        policy.diagnostics(),
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter",
            message:
                "unsupported Java borrowed dyn trait to owned boxed dyn recovery for Animal; explicit clone proof exists, but dyn clone-box adapter generation is not implemented"
                    .to_string(),
        }]
    );
    assert_eq!(
        plan_shape_coercion_with_facts(&borrowed_dyn_actual, &owned_boxed_target, &facts),
        vec![Coercion::UnsupportedDispatch {
            actual_dispatch: DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            },
            target_dispatch: DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            },
            policy: DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
                trait_name: "Animal".to_string(),
                recovery: BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                    policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty: animal },
                },
            },
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
        vec![CoercionDiagnostic {
            code: "unsupported_borrowed_dyn_to_owned_boxed_dyn",
            message:
                "unsupported Java borrowed dyn trait to owned boxed dyn recovery for Animal; no ownership proof is available"
                    .to_string(),
        }]
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
        RustExpr::Raw(
            "{ let _ = animal; compile_error!(\"unsupported Java dispatch coercion: actual=ClosedEnum { enum_name: \"AnimalValue\" }, target=DynTrait { trait_name: \"Animal\" }\") }"
                .to_string()
        )
    );
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
        RustExpr::Raw(
            "values.iter().next().cloned()/* TODO(translation): validate added clone */"
                .to_string()
        )
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
        }
    );
    assert_eq!(
        borrowed_element_adapter_policy(BorrowedElementCarrier::Vec, &string_view, &facts),
        BorrowedElementAdapterPolicy {
            carrier: BorrowedElementCarrier::Vec,
            element_shape: string_view,
            recovery: BorrowedElementRecoveryPolicy::StringViewToOwned,
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
            adapter: Some(BorrowedElementAdapterPolicy {
                carrier: BorrowedElementCarrier::Option,
                element_shape: string_element.clone(),
                recovery: BorrowedElementRecoveryPolicy::CloneElements {
                    policy: CloneRecoveryPolicy::AuditRequired,
                },
            }),
        }
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
