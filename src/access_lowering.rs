use crate::expr_shape::ShapeEnv;
use crate::java_ir::Expr;
use crate::member_access_plan::{MemberAccessOp, MemberAccessPlan};
use crate::rust_ir::RustExpr;
use crate::types::TypeEnv;

pub fn lower_bare_name(name: &str, env: &ShapeEnv) -> RustExpr {
    if env.local_type(name).is_none() && env.field_type(name).is_some() {
        RustExpr::Field(
            Box::new(RustExpr::Var("self".to_string())),
            name.to_string(),
        )
    } else {
        RustExpr::Var(name.to_string())
    }
}

pub fn lower_field_access(
    receiver: &Expr,
    field: &str,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    if let Some(plan) = crate::member_access_plan::plan_member_access(receiver, field, type_env) {
        apply_member_access_plan(&plan, lowered_receiver)
    } else {
        RustExpr::Field(Box::new(lowered_receiver), field.to_string())
    }
}

fn apply_member_access_plan(plan: &MemberAccessPlan, receiver: RustExpr) -> RustExpr {
    match plan.op {
        MemberAccessOp::LenAsI32 => RustExpr::Raw(format!(
            "{}.len() as i32",
            crate::emit::emit_expr(&receiver)
        )),
    }
}
