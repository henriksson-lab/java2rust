use crate::coercion::{LoweredExpr, apply_slot_conversion_plan};
use crate::conversion_plan::{
    CastConversionPlan, InstanceOfPlan, LoweredSlotExprConversionPlan, SlotExprConversionPlan,
};
use crate::rust_ir::RustExpr;

pub fn apply_cast_conversion(expr: RustExpr, plan: &CastConversionPlan) -> RustExpr {
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
