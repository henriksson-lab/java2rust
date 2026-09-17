use crate::coercion::{
    LoweredExpr, OptionalReceiverProjection, apply_slot_conversion_plan,
    java_char_from_u32_mask_projection, java_char_to_numeric_projection,
};
use crate::conversion_plan::{
    CastConversionPlan, InstanceOfPlan, JavaCastSemanticsPolicy, LoweredSlotExprConversionPlan,
    LoweredType, OptionalSlotConstructionPlan, ReusedOptionalUnwrapPlan,
    ReusedOptionalUnwrapProjection, Slot, SlotConversionPlan, SlotExprConversionPlan,
};
use crate::java_ir::PrimitiveType;
use crate::rust_ir::{RustExpr, RustExprShape};
use crate::slot_shape::RustSlotShape;

pub(crate) fn apply_cast_conversion(expr: RustExpr, plan: &CastConversionPlan) -> RustExpr {
    if let JavaCastSemanticsPolicy::PrimitiveNumericCast { from, to, .. } = plan.semantics() {
        let (from, to) = (*from, *to);
        if to == PrimitiveType::Char {
            return java_char_from_u32_mask_projection(expr);
        }
        if from == PrimitiveType::Char {
            return java_char_to_numeric_projection(expr, to);
        }
        return RustExpr::Cast {
            expr: Box::new(expr),
            ty: crate::coercion::rust_primitive_type_name_policy(to)
                .rust_name()
                .to_string(),
        };
    }
    crate::coercion::apply_shape_coercion_plan(expr, plan.coercion())
}

pub(crate) fn apply_instanceof(expr: RustExpr, plan: &InstanceOfPlan) -> RustExpr {
    crate::coercion::instanceof_check_projection(expr, plan.check())
}

pub(crate) fn apply_slot_expr_conversion(
    expr: RustExpr,
    plan: &SlotExprConversionPlan,
) -> RustExpr {
    apply_slot_conversion_plan(expr, plan.conversion())
}

pub(crate) fn apply_lowered_slot_expr_conversion(
    lowered: LoweredExpr,
    plan: &LoweredSlotExprConversionPlan,
) -> RustExpr {
    apply_slot_conversion_plan(
        normalize_lowered_expr_for_conversion(lowered, plan.conversion()),
        plan.conversion(),
    )
}

fn normalize_lowered_expr_for_conversion(
    lowered: LoweredExpr,
    conversion: &SlotConversionPlan,
) -> RustExpr {
    let (expr, actual) = lowered.into_parts();
    if expr.recorded_shape() == Some(RustExprShape::OptionalProjected) {
        return expr;
    }
    if let SlotConversionPlan::BorrowedElement(policy) = conversion
        && policy
            .adapter()
            .is_some_and(|adapter| adapter.unwrap_carrier())
    {
        return expr;
    }
    if matches!(
        conversion,
        SlotConversionPlan::OptionalRustBorrowedFacadeReturn(_)
    ) {
        return expr;
    }
    let LoweredType::OptionalElementBorrow { element_shape, .. } = actual else {
        return expr;
    };
    crate::coercion::project_optional_element_borrow_carrier_with_shape(
        expr.clone(),
        &element_shape,
    )
    .unwrap_or(expr)
}

pub(crate) fn apply_lowered_slot_conversion_to_expr(
    expr: RustExpr,
    plan: &LoweredSlotExprConversionPlan,
) -> RustExpr {
    apply_lowered_slot_expr_conversion(LoweredExpr::new(expr, plan.actual().clone()), plan)
}

pub(crate) fn apply_optional_receiver_slot_projection(
    lowered: RustExpr,
    projection: OptionalReceiverProjection,
    element_shape: Option<&RustSlotShape>,
    conversion_plan: Option<&LoweredSlotExprConversionPlan>,
) -> RustExpr {
    let projected = if let Some(element_shape) = element_shape {
        crate::coercion::project_optional_receiver_with_shape(lowered, projection, element_shape)
    } else {
        crate::coercion::project_optional_receiver(lowered, projection)
    };
    let Some(conversion_plan) = conversion_plan else {
        return projected;
    };
    let projected = LoweredExpr::new(projected, conversion_plan.actual().clone());
    // The projection above already discharges the optionality, and the
    // conversion can discharge it a second time -- giving
    // `cur.as_ref().ok_or_else(..)?.ok_or_else(..)?`, where the second unwrap
    // lands on the element the first produced (`E0599`). One of the four callers
    // of this function wrapped the result in this strip and the other three did
    // not; doing it here means no caller can forget. See claude_audits/0056.
    crate::coercion::strip_redundant_java_null_expect_after_unwrap(
        apply_lowered_slot_expr_conversion(projected, conversion_plan),
    )
}

pub(crate) fn apply_reused_optional_unwrap(
    option_expr: RustExpr,
    plan: &ReusedOptionalUnwrapPlan,
) -> LoweredExpr {
    let expr = match plan.projection() {
        ReusedOptionalUnwrapProjection::KeepCarrier => option_expr,
        ReusedOptionalUnwrapProjection::ExpectBorrowedCarrier => {
            crate::coercion::unwrap_java_null_receiver_projection(option_expr)
        }
        ReusedOptionalUnwrapProjection::BorrowElement { .. } => {
            let projected =
                if let LoweredType::OptionalElementBorrow { element_shape, .. } = plan.actual() {
                    crate::coercion::project_optional_element_borrow_carrier_with_shape(
                        option_expr,
                        element_shape,
                    )
                } else {
                    None
                };
            projected.unwrap_or_else(|| {
                crate::unsupported::compile_error_expr("unsupported optional element projection")
            })
        }
    };
    LoweredExpr::new(expr, plan.actual().clone())
}

pub(crate) fn construct_optional_value_for_target(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
) -> RustExpr {
    let plan = LoweredSlotExprConversionPlan::new(
        LoweredType::Slot(Box::new(construction.element_shape().clone())),
        Slot::Assignment(construction.target().clone()),
        SlotConversionPlan::OptionalConstruction(construction),
    );
    apply_lowered_slot_expr_conversion(LoweredExpr::new(value, plan.actual().clone()), &plan)
}

pub(crate) fn construct_preconverted_optional_value_for_target(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
) -> RustExpr {
    construct_preconverted_optional_value_for_target_from(value, construction, false)
}

/// `value_is_fresh_construction` says the element came from `new X(..)` -- the
/// one producer structurally guaranteed not to be behind a handle already.
pub(crate) fn construct_preconverted_optional_value_for_target_from(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
    value_is_fresh_construction: bool,
) -> RustExpr {
    let value = match (construction.element_handle(), value_is_fresh_construction) {
        (Some(placement), true) => {
            crate::slot_render::storage_placement_construction_expr(placement, value)
        }
        _ => value,
    };
    let plan = LoweredSlotExprConversionPlan::new(
        LoweredType::Slot(Box::new(construction.element_shape().clone())),
        Slot::Assignment(construction.target().clone()),
        SlotConversionPlan::OptionalConstruction(construction),
    );
    apply_lowered_slot_expr_conversion(LoweredExpr::new(value, plan.actual().clone()), &plan)
}

pub(crate) fn adapt_optional_assignment_value_for_target(
    lowered_value: RustExpr,
    actual: RustSlotShape,
    conversion_plan: SlotExprConversionPlan,
    preconverted_some_plan: OptionalSlotConstructionPlan,
    already_optional_result: bool,
    value_is_fresh_construction: bool,
) -> RustExpr {
    if already_optional_result {
        if let Some(inner) = crate::coercion::unwrap_some_expr(&lowered_value) {
            return inner;
        }
    }
    if actual.optional_element_type().is_some() {
        return lowered_value;
    }
    if rust_expr_is_none_value(&lowered_value) {
        return lowered_value;
    }
    if let Some(inner) = crate::coercion::unwrap_some_expr(&lowered_value) {
        return construct_preconverted_optional_value_for_target_from(
            inner,
            preconverted_some_plan,
            value_is_fresh_construction,
        );
    }
    // Still bare, so the handle goes on *before* the plan adds the `Some`.
    let lowered_value = match (
        preconverted_some_plan.element_handle(),
        value_is_fresh_construction,
    ) {
        (Some(placement), true) => {
            crate::slot_render::storage_placement_construction_expr(placement, lowered_value)
        }
        _ => lowered_value,
    };
    apply_slot_expr_conversion(lowered_value, &conversion_plan)
}

fn rust_expr_is_none_value(expr: &RustExpr) -> bool {
    matches!(expr, RustExpr::Var(name) if name == "None")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion_plan::{
        InstanceOfCheck, SlotExprConversionPlan, java_cast_semantics_policy_with_facts,
        optional_slot_construction_plan_for_actual, plan_lowered_slot_conversion,
        plan_slot_conversion, preconverted_optional_slot_construction_plan,
        shape_coercion_plan_with_facts,
    };
    use crate::java_ir::{JavaType, PrimitiveType};
    use crate::slot_shape::OwnershipShape;
    use crate::storage_strategy::{Escape, StoragePlan, TypeStorageFacts};
    use crate::types::optional_java_type;

    #[test]
    fn cast_and_instanceof_application_stays_structured() {
        let int_shape =
            RustSlotShape::conservative(crate::java_ir::JavaType::Primitive(PrimitiveType::Int));
        let char_shape =
            RustSlotShape::conservative(crate::java_ir::JavaType::Primitive(PrimitiveType::Char));
        let long_shape =
            RustSlotShape::conservative(crate::java_ir::JavaType::Primitive(PrimitiveType::Long));
        let facts = crate::facts::FactSet::default();

        let int_to_char_coercion = shape_coercion_plan_with_facts(&int_shape, &char_shape, &facts);
        let int_to_char = CastConversionPlan::new(
            int_shape.clone(),
            char_shape.clone(),
            java_cast_semantics_policy_with_facts(
                &int_shape,
                &char_shape,
                &int_to_char_coercion,
                &facts,
            ),
            int_to_char_coercion,
        );
        let lowered_char = apply_cast_conversion(RustExpr::Var("value".to_string()), &int_to_char);
        assert_eq!(
            lowered_char,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["char", "from_u32"])),
                    args: vec![RustExpr::Binary {
                        op: "&".to_string(),
                        left: Box::new(RustExpr::Cast {
                            expr: Box::new(RustExpr::Var("value".to_string())),
                            ty: "u32".to_string(),
                        }),
                        right: Box::new(RustExpr::Literal("0xffff".to_string())),
                    }],
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Literal("'\\0'".to_string())],
            }
        );
        assert_eq!(
            crate::emit::emit_expr(&lowered_char),
            "char::from_u32(value as u32 & 0xffff).unwrap_or('\\0')"
        );
        assert_eq!(
            crate::emit::emit_expr(&apply_cast_conversion(
                RustExpr::Raw("value + 1".to_string()),
                &int_to_char,
            )),
            "char::from_u32(((value + 1) as u32) & 0xffff).unwrap_or('\\0')"
        );

        let char_to_long_coercion =
            shape_coercion_plan_with_facts(&char_shape, &long_shape, &facts);
        let char_to_long = CastConversionPlan::new(
            char_shape.clone(),
            long_shape,
            java_cast_semantics_policy_with_facts(
                &char_shape,
                &RustSlotShape::conservative(crate::java_ir::JavaType::Primitive(
                    PrimitiveType::Long,
                )),
                &char_to_long_coercion,
                &facts,
            ),
            char_to_long_coercion,
        );
        let lowered_long = apply_cast_conversion(RustExpr::Var("ch".to_string()), &char_to_long);
        assert_eq!(
            lowered_long,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("ch".to_string())),
                    ty: "u32".to_string(),
                }),
                ty: "i64".to_string(),
            }
        );
        assert_eq!(crate::emit::emit_expr(&lowered_long), "(ch as u32) as i64");
        assert_eq!(
            crate::emit::emit_expr(&apply_cast_conversion(
                RustExpr::Raw("next_char()".to_string()),
                &char_to_long,
            )),
            "((next_char()) as u32) as i64"
        );

        let animal_shape = RustSlotShape::conservative(crate::java_ir::JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new(),
        });
        let dog_shape = RustSlotShape::conservative(crate::java_ir::JavaType::Named {
            name: "Dog".to_string(),
            args: Vec::new(),
        });
        let instanceof_plan = InstanceOfPlan::new(
            animal_shape,
            dog_shape,
            InstanceOfCheck::ClosedEnumVariant {
                enum_name: "AnimalValue".to_string(),
                variant_name: "Dog".to_string(),
            },
        );
        let instanceof_expr =
            apply_instanceof(RustExpr::Var("animal".to_string()), &instanceof_plan);
        assert_eq!(
            crate::emit::emit_expr(&instanceof_expr),
            "matches!(animal, AnimalValue::Dog(_))"
        );
    }

    #[test]
    fn slot_conversion_application_stays_structured_inside_lowering_module() {
        let byte_array = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)));
        let target = RustSlotShape::conservative(byte_array.clone());
        let lowered = LoweredType::rust_unsigned_byte_array_return(byte_array.clone(), false);
        let plan = plan_lowered_slot_conversion(
            &lowered,
            Slot::Return(target),
            &crate::facts::FactSet::default(),
        );
        let converted = apply_lowered_slot_expr_conversion(
            LoweredExpr::new(RustExpr::Raw("left + right".to_string()), lowered),
            &plan,
        );
        assert_eq!(
            crate::emit::emit_expr(&converted),
            "(left + right).into_iter().map(|byte| byte as i8).collect::<Vec<_>>()"
        );
        assert!(matches!(
            converted,
            RustExpr::MethodCall { method, .. } if method == "collect::<Vec<_>>"
        ));

        let optional_target = RustSlotShape::conservative(optional_java_type(byte_array.clone()));
        let optional_lowered = LoweredType::rust_unsigned_byte_array_return(byte_array, true);
        let optional_plan = plan_lowered_slot_conversion(
            &optional_lowered,
            Slot::Return(optional_target),
            &crate::facts::FactSet::default(),
        );
        let optional_converted = apply_lowered_slot_expr_conversion(
            LoweredExpr::new(RustExpr::Raw("maybe_bytes()".to_string()), optional_lowered),
            &optional_plan,
        );
        assert_eq!(
            crate::emit::emit_expr(&optional_converted),
            "maybe_bytes().map(|bytes| bytes.into_iter().map(|byte| byte as i8).collect::<Vec<_>>())"
        );
        assert!(matches!(
            optional_converted,
            RustExpr::MethodCall { method, .. } if method == "map"
        ));

        let object_target = crate::shape_projection::project_java_value_shape(
            None,
            JavaType::Unknown("Object".to_string()),
        );
        let owned_facade_lowered = LoweredType::rust_owned_return(
            JavaType::Named {
                name: "ExternalFactory.Product.Builder".to_string(),
                args: Vec::new(),
            },
            "ExternalFactory_Product_Builder".to_string(),
        );
        let owned_facade_plan = plan_lowered_slot_conversion(
            &owned_facade_lowered,
            Slot::Return(object_target),
            &crate::facts::FactSet::default(),
        );
        let owned_facade_converted = apply_lowered_slot_expr_conversion(
            LoweredExpr::new(
                RustExpr::Raw("value.to_builder()".to_string()),
                owned_facade_lowered,
            ),
            &owned_facade_plan,
        );
        assert_eq!(
            crate::emit::emit_expr(&owned_facade_converted),
            "Box::new(value.to_builder()) as Box<dyn std::any::Any>"
        );

        let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
        let slot_plan =
            plan_slot_conversion(int_shape, Slot::Index, &crate::facts::FactSet::default());
        assert_eq!(
            apply_slot_expr_conversion(RustExpr::Var("i".to_string()), &slot_plan),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("i".to_string())),
                ty: "usize".to_string(),
            }
        );

        let lowered_plan = plan_lowered_slot_conversion(
            &LoweredType::optional_element_borrow(JavaType::String),
            Slot::Index,
            &crate::facts::FactSet::default(),
        );
        let mut optional_element_shape = RustSlotShape::conservative(JavaType::String);
        optional_element_shape.ownership = OwnershipShape::SharedBorrow;
        assert_eq!(
            apply_lowered_slot_expr_conversion(
                LoweredExpr::new(
                    RustExpr::Var("value".to_string()),
                    LoweredType::optional_element_borrow(JavaType::String),
                ),
                &lowered_plan,
            ),
            RustExpr::shaped_as(
                crate::rust_ir::RustExprShape::OptionalProjected,
                Some(optional_element_shape),
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                },
            )
        );

        let uri_type = JavaType::Named {
            name: "URI".to_string(),
            args: Vec::new(),
        };
        let option_shape = RustSlotShape::conservative(optional_java_type(uri_type.clone()));
        let mut element_shape = RustSlotShape::conservative(uri_type);
        element_shape.ownership = OwnershipShape::SharedBorrow;
        let optional_display_actual = LoweredType::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        };
        let optional_display_plan = plan_lowered_slot_conversion(
            &optional_display_actual,
            Slot::StringConcatSegment,
            &crate::facts::FactSet::default(),
        );
        let converted_display = apply_lowered_slot_expr_conversion(
            LoweredExpr::new(
                RustExpr::Field(
                    Box::new(RustExpr::Var("box_value".to_string())),
                    "value".to_string(),
                ),
                optional_display_actual,
            ),
            &optional_display_plan,
        );
        assert_eq!(
            crate::emit::emit_expr(&converted_display),
            "box_value.value.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
    }

    #[test]
    fn optional_assignment_adapter_preserves_structured_none_without_rendering() {
        let facts = crate::facts::FactSet::default();
        let actual = RustSlotShape::conservative(crate::java_ir::JavaType::String);
        let target =
            RustSlotShape::conservative(optional_java_type(crate::java_ir::JavaType::String));
        let conversion_plan = SlotExprConversionPlan::new(
            actual.clone(),
            Slot::Assignment(target.clone()),
            SlotConversionPlan::OptionalConstruction(optional_slot_construction_plan_for_actual(
                &actual, &target, &facts,
            )),
        );
        let preconverted = preconverted_optional_slot_construction_plan(
            &target,
            crate::java_ir::JavaType::String,
            &facts,
        );

        assert_eq!(
            adapt_optional_assignment_value_for_target(
                RustExpr::Var("None".to_string()),
                actual,
                conversion_plan,
                preconverted,
                false,
                false,
            ),
            RustExpr::Var("None".to_string())
        );

        let shared_node = crate::java_ir::JavaType::Named {
            name: "SharedNode".to_string(),
            args: Vec::new(),
        };
        let mut storage_plan = StoragePlan::default();
        storage_plan.insert(
            "SharedNode".to_string(),
            TypeStorageFacts::default()
                .with_escape(Escape::Stored)
                .with_mutated(false)
                .with_mutation_provably_absent(true)
                .with_surface_forwardable(true),
        );
        let facts = crate::facts::FactSet {
            storage_strategies: storage_plan,
            ..crate::facts::FactSet::default()
        };
        let actual = RustSlotShape::conservative(shared_node.clone());
        let target = RustSlotShape::conservative(optional_java_type(shared_node));
        let conversion_plan = SlotExprConversionPlan::new(
            actual.clone(),
            Slot::Assignment(target.clone()),
            SlotConversionPlan::OptionalConstruction(optional_slot_construction_plan_for_actual(
                &actual, &target, &facts,
            )),
        );
        let preconverted =
            preconverted_optional_slot_construction_plan(&target, actual.java_ty.clone(), &facts);
        let adapted = adapt_optional_assignment_value_for_target(
            RustExpr::Call {
                function: Box::new(RustExpr::path(["SharedNode", "new"])),
                args: Vec::new(),
            },
            actual,
            conversion_plan,
            preconverted,
            false,
            true,
        );
        assert_eq!(
            crate::emit::emit_expr(&adapted),
            "Some(std::rc::Rc::new(SharedNode::new()))"
        );
        let RustExpr::Call { args, .. } = adapted.unshaped() else {
            panic!("optional construction should stay a Some call");
        };
        assert_eq!(
            args,
            &[RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Call {
                    function: Box::new(RustExpr::path(["SharedNode", "new"])),
                    args: Vec::new(),
                }],
            }]
        );
    }
}
