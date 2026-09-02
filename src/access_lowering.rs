use crate::expr_shape::ShapeEnv;
use crate::java_ir::Expr;
use crate::member_access_plan::{MemberAccessOp, MemberAccessPlan};
use crate::rust_ir::RustExpr;
use crate::types::TypeEnv;
use crate::{
    coercion::{
        OptionalReceiverProjection, associated_const_projection, field_projection,
        owner_associated_path_projection, project_optional_receiver,
        self_associated_path_projection, self_field_projection, static_storage_accessor_projection,
    },
    emit,
};

pub fn lower_bare_name(name: &str, env: &ShapeEnv) -> RustExpr {
    if env.local_type(name).is_none()
        && env.is_instance_field(name)
        && !bare_name_looks_like_static_owner(name)
    {
        let field_name = env.instance_field_access_name(name).unwrap_or(name);
        self_field_projection(field_name)
    } else if bare_name_local_type(name, env).is_none()
        && let Some(static_name) = env.static_storage_field_access_name(name)
    {
        static_storage_accessor_for(static_name, env)
    } else if bare_name_local_type(name, env).is_none()
        && let Some(static_name) = env.static_field_access_name(name)
    {
        if let Some(owner) =
            static_member_import_owner(name, env).or_else(|| env.static_field_access_owner(name))
        {
            owner_associated_path_projection(
                &crate::slot_render::rust_name_for_java_named_type(owner),
                static_name,
            )
        } else {
            self_associated_path_projection(static_name)
        }
    } else if let Some(access_name) = env.binding_access_name(name) {
        RustExpr::Var(access_name.to_string())
    } else {
        RustExpr::Var(crate::parser::to_snake_case(name))
    }
}

fn static_member_import_owner<'a>(name: &str, env: &'a ShapeEnv) -> Option<&'a str> {
    env.type_env().static_member_import_owner(name).or_else(|| {
        env.type_env()
            .static_member_import_owner(&crate::parser::to_snake_case(name))
    })
}

fn bare_name_local_type<'a>(name: &str, env: &'a ShapeEnv) -> Option<&'a crate::java_ir::JavaType> {
    env.local_type(name).or_else(|| {
        let alias = crate::parser::to_snake_case(name);
        (alias != name).then(|| env.local_type(&alias)).flatten()
    })
}

pub fn render_bare_name(name: &str, env: &ShapeEnv) -> String {
    emit::emit_expr(&lower_bare_name(name, env))
}

/// A static-storage read, qualified with the declaring type when that is not
/// the type being lowered
/// ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
fn static_storage_accessor_for(static_name: &str, env: &ShapeEnv) -> RustExpr {
    match env.static_storage_accessor_qualifier(static_name) {
        Some(owner) => {
            crate::coercion::qualified_static_storage_accessor_projection(&owner, static_name)
        }
        None => static_storage_accessor_projection(static_name),
    }
}

fn bare_name_looks_like_static_owner(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
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
        let lowered_receiver = lower_nullable_array_element_member_receiver(
            receiver,
            type_env,
            &plan,
            lowered_receiver,
        );
        apply_member_access_plan(&plan, lowered_receiver)
    } else {
        let lowered_receiver =
            lower_nullable_array_element_field_receiver(receiver, type_env, lowered_receiver);
        field_projection(lowered_receiver, field)
    }
}

fn lower_nullable_array_element_field_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    lower_nullable_array_element_receiver(receiver, type_env, lowered_receiver)
}

fn lower_nullable_array_element_member_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    plan: &MemberAccessPlan,
    lowered_receiver: RustExpr,
) -> RustExpr {
    if !matches!(plan.op, MemberAccessOp::RustField { .. }) {
        return lowered_receiver;
    }
    lower_nullable_array_element_receiver(receiver, type_env, lowered_receiver)
}

fn lower_nullable_array_element_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    let Expr::ArrayAccess(array, _) = receiver else {
        return lowered_receiver;
    };
    let array_ty = crate::types::infer_expr_type(array, type_env);
    let Some(element_ty) = crate::types::sequence_element_type(&array_ty) else {
        return lowered_receiver;
    };
    if matches!(
        element_ty,
        crate::java_ir::JavaType::Primitive(_)
            | crate::java_ir::JavaType::Void
            | crate::java_ir::JavaType::Array(_)
    ) || crate::types::optional_element_type(&element_ty).is_some()
    {
        return lowered_receiver;
    }
    project_optional_receiver(lowered_receiver, OptionalReceiverProjection::Shared)
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
            let owner = if owner.contains('.') {
                crate::slot_render::rust_name_for_java_named_type(owner)
            } else {
                owner.clone()
            };
            associated_const_projection(&owner, &rust_associated_const_ident(&plan.member))
        }
        MemberAccessOp::RustField { rust_name } => field_projection(receiver, rust_name),
    }
}

fn rust_associated_const_ident(name: &str) -> String {
    if matches!(name, "in") {
        "r#in".to_string()
    } else {
        name.to_string()
    }
}
