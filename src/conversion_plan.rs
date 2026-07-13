use crate::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context};
use crate::java_ir::{Expr, JavaType};
use crate::shape_projection::project_java_value_shape;
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastConversionPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceOfPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
}

pub fn plan_cast_conversion(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> CastConversionPlan {
    CastConversionPlan {
        actual: infer_expr_shape_with_context(expr, env, ctx),
        target: project_java_value_shape(ctx.representations, target_ty.clone()),
    }
}

pub fn plan_instanceof(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> InstanceOfPlan {
    InstanceOfPlan {
        actual: infer_expr_shape_with_context(expr, env, ctx),
        target: project_java_value_shape(ctx.representations, target_ty.clone()),
    }
}
