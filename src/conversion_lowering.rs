use crate::coercion::{LoweredExpr, apply_slot_conversion_plan};
use crate::conversion_plan::{
    CastConversionPlan, InstanceOfPlan, JavaCastSemanticsPolicy, LoweredSlotExprConversionPlan,
    SlotExprConversionPlan,
};
use crate::java_ir::PrimitiveType;
use crate::rust_ir::RustExpr;

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
