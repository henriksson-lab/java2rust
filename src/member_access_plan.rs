use crate::java_ir::{Expr, JavaType};
use crate::shape_projection::project_java_value_shape;
use crate::slot_shape::{OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{TypeEnv, infer_member_access_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberAccessOp {
    LenAsI32,
    StaticConstant { owner: String },
    RustField { rust_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaMemberAccessKind {
    ArrayLength,
    StaticConstant,
    InstanceField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaMemberAccessPolicy {
    pub member: String,
    pub java_ty: JavaType,
    pub kind: JavaMemberAccessKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessPlan {
    pub member: String,
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub op: MemberAccessOp,
}

pub fn java_member_access_policy(
    receiver: &Expr,
    member: &str,
    env: &TypeEnv,
) -> Option<JavaMemberAccessPolicy> {
    if member != "length" {
        if is_static_constant_member(member) && static_constant_owner(receiver, env).is_some() {
            return Some(JavaMemberAccessPolicy {
                member: member.to_string(),
                java_ty: infer_member_access_type(receiver, member, env)
                    .unwrap_or_else(|| JavaType::Unknown(member.to_string())),
                kind: JavaMemberAccessKind::StaticConstant,
            });
        }
        if let Some(java_ty) = infer_member_access_type(receiver, member, env) {
            return Some(JavaMemberAccessPolicy {
                member: member.to_string(),
                java_ty,
                kind: JavaMemberAccessKind::InstanceField,
            });
        }
        if matches!(receiver, Expr::This)
            && let Some(java_ty) = env
                .field_type(member)
                .or_else(|| env.field_type(&crate::parser::to_snake_case(member)))
        {
            return Some(JavaMemberAccessPolicy {
                member: member.to_string(),
                java_ty: java_ty.clone(),
                kind: JavaMemberAccessKind::InstanceField,
            });
        }
        return None;
    }
    let java_ty = infer_member_access_type(receiver, member, env)?;
    Some(JavaMemberAccessPolicy {
        member: member.to_string(),
        java_ty,
        kind: JavaMemberAccessKind::ArrayLength,
    })
}

pub fn plan_member_access(
    receiver: &Expr,
    member: &str,
    env: &TypeEnv,
) -> Option<MemberAccessPlan> {
    let policy = java_member_access_policy(receiver, member, env)?;
    let op = match policy.kind {
        JavaMemberAccessKind::ArrayLength => MemberAccessOp::LenAsI32,
        JavaMemberAccessKind::StaticConstant => MemberAccessOp::StaticConstant {
            owner: static_constant_owner(receiver, env)?,
        },
        JavaMemberAccessKind::InstanceField => MemberAccessOp::RustField {
            rust_name: crate::parser::to_snake_case(&policy.member),
        },
    };
    let shape = member_access_shape(policy.kind, policy.java_ty.clone());
    Some(MemberAccessPlan {
        member: policy.member.clone(),
        java_ty: policy.java_ty.clone(),
        shape,
        op,
    })
}

fn member_access_shape(kind: JavaMemberAccessKind, java_ty: JavaType) -> RustSlotShape {
    let mut shape = project_java_value_shape(None, java_ty);
    if kind == JavaMemberAccessKind::StaticConstant && shape.java_ty == JavaType::String {
        shape.ownership = OwnershipShape::SharedBorrow;
        shape.view = ViewShape::Str;
    }
    shape
}

fn static_constant_owner(receiver: &Expr, env: &TypeEnv) -> Option<String> {
    let Expr::Var(owner) = receiver else {
        return None;
    };
    if env.local_type(owner).is_some() || env.field_type(owner).is_some() {
        return None;
    }
    if owner
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(owner.to_string())
    } else if owner.contains('_') || owner.chars().all(|ch| ch.is_ascii_lowercase()) {
        Some(upper_camel_from_snake(owner))
    } else {
        None
    }
}

fn is_static_constant_member(member: &str) -> bool {
    member
        .chars()
        .any(|ch| ch == '_' || ch.is_ascii_uppercase())
        && member
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn upper_camel_from_snake(name: &str) -> String {
    let mut out = String::new();
    let mut capitalize = true;
    for ch in name.chars() {
        if ch == '_' {
            capitalize = true;
        } else if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    out
}
