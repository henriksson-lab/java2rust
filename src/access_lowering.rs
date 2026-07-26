use crate::expr_shape::ShapeEnv;
use crate::java_ir::Expr;
use crate::member_access_plan::{MemberAccessOp, MemberAccessPlan};
use crate::rust_ir::RustExpr;
use crate::types::TypeEnv;
use crate::{
    coercion::{
        associated_const_projection, self_associated_path_projection,
        static_storage_accessor_projection,
    },
    emit,
};

pub fn lower_bare_name(name: &str, env: &ShapeEnv) -> RustExpr {
    if env.local_type(name).is_none()
        && env.is_instance_field(name)
        && !bare_name_looks_like_static_owner(name)
    {
        RustExpr::Field(
            Box::new(RustExpr::Var("self".to_string())),
            crate::parser::to_snake_case(name),
        )
    } else if env.local_type(name).is_none()
        && let Some(static_name) = env.static_storage_field_access_name(name)
    {
        static_storage_accessor_projection(static_name)
    } else if env.local_type(name).is_none()
        && let Some(static_name) = env.static_field_access_name(name)
    {
        self_associated_path_projection(static_name)
    } else {
        RustExpr::Var(crate::parser::to_snake_case(name))
    }
}

pub fn render_bare_name(name: &str, env: &ShapeEnv) -> String {
    if env.local_type(name).is_none()
        && env.is_instance_field(name)
        && !bare_name_looks_like_static_owner(name)
    {
        format!("self.{}", crate::parser::to_snake_case(name))
    } else if env.local_type(name).is_none()
        && let Some(static_name) = env.static_storage_field_access_name(name)
    {
        emit::emit_expr(&static_storage_accessor_projection(static_name))
    } else if env.local_type(name).is_none()
        && let Some(static_name) = env.static_field_access_name(name)
    {
        emit::emit_expr(&self_associated_path_projection(static_name))
    } else {
        crate::parser::to_snake_case(name)
    }
}

fn bare_name_looks_like_static_owner(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

pub fn lower_field_access(
    receiver: &Expr,
    field: &str,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    lower_field_access_with_facts(
        receiver,
        field,
        type_env,
        lowered_receiver,
        &crate::facts::FactSet::default(),
    )
}

pub fn lower_field_access_with_facts(
    receiver: &Expr,
    field: &str,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
    facts: &crate::facts::FactSet,
) -> RustExpr {
    if let Some(plan) =
        crate::member_access_plan::plan_member_access_with_facts(receiver, field, type_env, facts)
    {
        let lowered_receiver =
            lower_optional_sequence_member_receiver(receiver, type_env, &plan, lowered_receiver);
        apply_member_access_plan(&plan, lowered_receiver)
    } else {
        RustExpr::Field(Box::new(lowered_receiver), field.to_string())
    }
}

fn lower_optional_sequence_member_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    plan: &MemberAccessPlan,
    lowered_receiver: RustExpr,
) -> RustExpr {
    if !matches!(
        plan.op,
        MemberAccessOp::LenAsI32
            | MemberAccessOp::Array2RowsAsI32
            | MemberAccessOp::Array2ColsAsI32
    ) {
        return lowered_receiver;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, type_env);
    let Some(inner_ty) = crate::types::optional_element_type(&receiver_ty) else {
        return lowered_receiver;
    };
    if crate::types::sequence_element_type(&inner_ty).is_none()
        && crate::types::ndarray_array2_element_type(&inner_ty).is_none()
    {
        return lowered_receiver;
    }
    crate::coercion::project_optional_receiver(
        lowered_receiver,
        crate::coercion::OptionalReceiverProjection::Shared,
    )
}

fn apply_member_access_plan(plan: &MemberAccessPlan, receiver: RustExpr) -> RustExpr {
    match &plan.op {
        MemberAccessOp::LenAsI32 => crate::coercion::len_i32_projection(receiver),
        MemberAccessOp::Array2RowsAsI32 => crate::coercion::array2_rows_i32_projection(receiver),
        MemberAccessOp::Array2ColsAsI32 => crate::coercion::array2_cols_i32_projection(receiver),
        MemberAccessOp::StaticConstant { owner } => {
            associated_const_projection(owner, &rust_associated_const_ident(&plan.member))
        }
        MemberAccessOp::RustField { rust_name } => {
            RustExpr::Field(Box::new(receiver), rust_name.clone())
        }
    }
}

fn rust_associated_const_ident(name: &str) -> String {
    if matches!(name, "in") {
        "r#in".to_string()
    } else {
        name.to_string()
    }
}
