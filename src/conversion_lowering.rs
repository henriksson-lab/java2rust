use crate::conversion_plan::{CastConversionPlan, InstanceOfPlan};
use crate::facts::FactSet;
use crate::rust_ir::RustExpr;

pub fn apply_cast_conversion(expr: RustExpr, plan: &CastConversionPlan) -> RustExpr {
    apply_cast_conversion_with_facts(expr, plan, &FactSet::default())
}

pub fn apply_cast_conversion_with_facts(
    expr: RustExpr,
    plan: &CastConversionPlan,
    facts: &FactSet,
) -> RustExpr {
    crate::coercion::apply_expr_to_shape_with_facts(expr, &plan.actual, &plan.target, facts)
}

pub fn apply_instanceof(expr: RustExpr, plan: &InstanceOfPlan) -> RustExpr {
    crate::coercion::apply_instanceof_check(expr, &plan.actual, &plan.target)
}
