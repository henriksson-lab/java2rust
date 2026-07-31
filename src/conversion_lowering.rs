use crate::coercion::{LoweredExpr, OptionalReceiverProjection, apply_slot_conversion_plan};
use crate::conversion_plan::{
    CastConversionPlan, InstanceOfPlan, JavaCastSemanticsPolicy, LoweredSlotExprConversionPlan,
    LoweredType, OptionalSlotConstructionPlan, ReusedOptionalUnwrapPlan,
    ReusedOptionalUnwrapProjection, Slot, SlotConversionPlan, SlotExprConversionPlan,
    plan_lowered_slot_conversion, receiver_method_autoref_satisfies_optional_slot,
};
use crate::facts::FactSet;
use crate::java_ir::PrimitiveType;
use crate::rust_ir::RustExpr;
use crate::slot_shape::{OwnershipShape, RustSlotShape};

pub fn apply_cast_conversion(expr: RustExpr, plan: &CastConversionPlan) -> RustExpr {
    if let JavaCastSemanticsPolicy::PrimitiveNumericCast { from, to, .. } = plan.semantics {
        if to == PrimitiveType::Char {
            return RustExpr::Raw(format!(
                "char::from_u32((({}) as u32) & 0xffff).unwrap_or('\\0')",
                crate::emit::emit_expr(&expr)
            ));
        }
        if from == PrimitiveType::Char {
            return RustExpr::Raw(format!(
                "(({}) as u32) as {}",
                crate::emit::emit_expr(&expr),
                crate::coercion::rust_primitive_type_name_policy(to).rust_name
            ));
        }
        return RustExpr::Cast {
            expr: Box::new(expr),
            ty: crate::coercion::rust_primitive_type_name_policy(to)
                .rust_name
                .to_string(),
        };
    }
    crate::coercion::apply_shape_coercion_plan(expr, &plan.coercion)
}

pub fn apply_instanceof(expr: RustExpr, plan: &InstanceOfPlan) -> RustExpr {
    crate::coercion::render_instanceof_check(expr, &plan.check)
}

pub fn apply_slot_expr_conversion(expr: RustExpr, plan: &SlotExprConversionPlan) -> RustExpr {
    apply_slot_conversion_plan(expr, &plan.conversion)
}

pub fn apply_lowered_slot_expr_conversion(
    lowered: LoweredExpr,
    plan: &LoweredSlotExprConversionPlan,
) -> RustExpr {
    apply_slot_conversion_plan(lowered.expr, &plan.conversion)
}

pub fn apply_optional_receiver_slot_projection(
    lowered: RustExpr,
    option_shape: RustSlotShape,
    receiver_slot: Slot,
    projection: OptionalReceiverProjection,
    element_ownership: OwnershipShape,
    facts: &FactSet,
) -> RustExpr {
    let projected = crate::coercion::project_optional_receiver(lowered, projection);
    let target_shape = match &receiver_slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => shape,
        _ => return projected,
    };
    let Some(actual) = LoweredType::optional_element_borrow_from_option_shape(
        option_shape,
        target_shape.java_ty.clone(),
        Some(element_ownership),
        facts,
    ) else {
        return projected;
    };
    let LoweredType::OptionalElementBorrow { element_shape, .. } = actual else {
        return projected;
    };
    let projected = LoweredExpr {
        expr: projected,
        actual: LoweredType::Slot(element_shape),
    };
    let conversion_plan = plan_lowered_slot_conversion(&projected.actual, receiver_slot, facts);
    if receiver_method_autoref_satisfies_optional_slot(&conversion_plan) {
        return projected.expr;
    }
    apply_lowered_slot_expr_conversion(projected, &conversion_plan)
}

pub fn apply_reused_optional_unwrap(
    option_expr: RustExpr,
    plan: &ReusedOptionalUnwrapPlan,
) -> LoweredExpr {
    let expr = match plan.projection {
        ReusedOptionalUnwrapProjection::KeepCarrier => option_expr,
        ReusedOptionalUnwrapProjection::ExpectBorrowedCarrier => {
            crate::coercion::unwrap_java_null_receiver_projection(option_expr)
        }
        ReusedOptionalUnwrapProjection::BorrowElement { ownership } => {
            crate::coercion::project_optional_element_borrow_carrier(option_expr, ownership)
                .unwrap_or_else(|| {
                    RustExpr::Raw(
                        "compile_error!(\"unsupported optional element projection\")".to_string(),
                    )
                })
        }
    };
    LoweredExpr {
        expr,
        actual: plan.actual.clone(),
    }
}

pub fn construct_optional_value_for_target(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
) -> RustExpr {
    let plan = LoweredSlotExprConversionPlan {
        actual: LoweredType::Slot(Box::new(construction.element_shape.clone())),
        slot: Slot::Assignment(construction.target.clone()),
        conversion: SlotConversionPlan::OptionalConstruction(construction),
    };
    apply_lowered_slot_expr_conversion(
        LoweredExpr {
            expr: value,
            actual: plan.actual.clone(),
        },
        &plan,
    )
}

pub fn construct_preconverted_optional_value_for_target(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
) -> RustExpr {
    let plan = LoweredSlotExprConversionPlan {
        actual: LoweredType::Slot(Box::new(construction.element_shape.clone())),
        slot: Slot::Assignment(construction.target.clone()),
        conversion: SlotConversionPlan::OptionalConstruction(construction),
    };
    apply_lowered_slot_expr_conversion(
        LoweredExpr {
            expr: value,
            actual: plan.actual.clone(),
        },
        &plan,
    )
}

pub fn adapt_optional_assignment_value_for_target(
    lowered_value: RustExpr,
    actual: RustSlotShape,
    conversion_plan: SlotExprConversionPlan,
    preconverted_some_plan: OptionalSlotConstructionPlan,
    already_optional_result: bool,
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
        return construct_preconverted_optional_value_for_target(inner, preconverted_some_plan);
    }
    apply_slot_expr_conversion(lowered_value, &conversion_plan)
}

fn rust_expr_is_none_value(expr: &RustExpr) -> bool {
    matches!(expr, RustExpr::Var(name) if name == "None")
}
