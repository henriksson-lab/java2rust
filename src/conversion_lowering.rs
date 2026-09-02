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

pub fn apply_cast_conversion(expr: RustExpr, plan: &CastConversionPlan) -> RustExpr {
    if let JavaCastSemanticsPolicy::PrimitiveNumericCast { from, to, .. } = plan.semantics {
        if to == PrimitiveType::Char {
            return java_char_from_u32_mask_projection(expr);
        }
        if from == PrimitiveType::Char {
            return java_char_to_numeric_projection(expr, to);
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
    crate::coercion::instanceof_check_projection(expr, &plan.check)
}

pub fn apply_slot_expr_conversion(expr: RustExpr, plan: &SlotExprConversionPlan) -> RustExpr {
    apply_slot_conversion_plan(expr, &plan.conversion)
}

pub fn apply_lowered_slot_expr_conversion(
    lowered: LoweredExpr,
    plan: &LoweredSlotExprConversionPlan,
) -> RustExpr {
    apply_slot_conversion_plan(
        normalize_lowered_expr_for_conversion(lowered, &plan.conversion),
        &plan.conversion,
    )
}

fn normalize_lowered_expr_for_conversion(
    lowered: LoweredExpr,
    conversion: &SlotConversionPlan,
) -> RustExpr {
    let LoweredExpr { expr, actual } = lowered;
    if expr.recorded_shape() == Some(RustExprShape::OptionalProjected) {
        return expr;
    }
    if let SlotConversionPlan::BorrowedElement(policy) = conversion
        && policy
            .adapter
            .as_ref()
            .is_some_and(|adapter| adapter.unwrap_carrier)
    {
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

pub fn apply_lowered_slot_conversion_to_expr(
    expr: RustExpr,
    plan: &LoweredSlotExprConversionPlan,
) -> RustExpr {
    apply_lowered_slot_expr_conversion(
        LoweredExpr {
            expr,
            actual: plan.actual.clone(),
        },
        plan,
    )
}

pub fn apply_optional_receiver_slot_projection(
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
    let projected = LoweredExpr {
        expr: projected,
        actual: conversion_plan.actual.clone(),
    };
    apply_lowered_slot_expr_conversion(projected, conversion_plan)
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
        ReusedOptionalUnwrapProjection::BorrowElement { .. } => {
            let projected =
                if let LoweredType::OptionalElementBorrow { element_shape, .. } = &plan.actual {
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
    construct_preconverted_optional_value_for_target_from(value, construction, false)
}

/// `value_is_fresh_construction` says the element came from `new X(..)` -- the
/// one producer structurally guaranteed not to be behind a handle already.
pub fn construct_preconverted_optional_value_for_target_from(
    value: RustExpr,
    construction: OptionalSlotConstructionPlan,
    value_is_fresh_construction: bool,
) -> RustExpr {
    let value = match (&construction.element_handle, value_is_fresh_construction) {
        (Some(placement), true) => {
            crate::slot_render::storage_placement_construction_expr(*placement, value)
        }
        _ => value,
    };
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
        &preconverted_some_plan.element_handle,
        value_is_fresh_construction,
    ) {
        (Some(placement), true) => {
            crate::slot_render::storage_placement_construction_expr(*placement, lowered_value)
        }
        _ => lowered_value,
    };
    apply_slot_expr_conversion(lowered_value, &conversion_plan)
}

fn rust_expr_is_none_value(expr: &RustExpr) -> bool {
    matches!(expr, RustExpr::Var(name) if name == "None")
}
