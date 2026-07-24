use crate::facts::{ExternalLibraryMode, FactSet};
use crate::java_ir::{Expr, JavaType};
use crate::rust_ir::RustType;
use crate::shape_projection::project_java_value_shape;
use crate::slot_shape::{OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{
    ArrayLengthDimension, TypeEnv, array_length_dimension, infer_member_access_type,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberAccessOp {
    LenAsI32,
    Array2RowsAsI32,
    Array2ColsAsI32,
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
    java_member_access_policy_with_facts(receiver, member, env, &FactSet::default())
}

pub fn java_member_access_policy_with_facts(
    receiver: &Expr,
    member: &str,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<JavaMemberAccessPolicy> {
    if member != "length" {
        if let Some(java_ty) = external_static_constant_type(receiver, member, facts) {
            return Some(JavaMemberAccessPolicy {
                member: member.to_string(),
                java_ty,
                kind: JavaMemberAccessKind::StaticConstant,
            });
        }
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
    plan_member_access_with_facts(receiver, member, env, &FactSet::default())
}

pub fn plan_member_access_with_facts(
    receiver: &Expr,
    member: &str,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<MemberAccessPlan> {
    let policy = java_member_access_policy_with_facts(receiver, member, env, facts)?;
    let op = match policy.kind {
        JavaMemberAccessKind::ArrayLength => array_length_member_access_op(receiver, env),
        JavaMemberAccessKind::StaticConstant => MemberAccessOp::StaticConstant {
            owner: external_static_constant_rust_owner(
                receiver,
                &policy.member,
                &policy.java_ty,
                facts,
            )
            .or_else(|| static_constant_owner(receiver, env))?,
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

fn array_length_member_access_op(receiver: &Expr, env: &TypeEnv) -> MemberAccessOp {
    match array_length_dimension(receiver, env) {
        ArrayLengthDimension::Rows => MemberAccessOp::Array2RowsAsI32,
        ArrayLengthDimension::Columns => MemberAccessOp::Array2ColsAsI32,
        ArrayLengthDimension::Linear => MemberAccessOp::LenAsI32,
    }
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
    let upper_camel_owner = owner
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase());
    if env.local_type(owner).is_some() || (!upper_camel_owner && env.field_type(owner).is_some()) {
        return None;
    }
    if upper_camel_owner {
        Some(owner.to_string())
    } else if owner.contains('_') || owner.chars().all(|ch| ch.is_ascii_lowercase()) {
        Some(upper_camel_from_snake(owner))
    } else {
        None
    }
}

fn static_constant_rust_owner_from_name(owner: &str) -> Option<String> {
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

fn external_static_constant_type(
    receiver: &Expr,
    member: &str,
    facts: &FactSet,
) -> Option<JavaType> {
    let receiver_name = static_receiver_java_name(receiver)?;
    facts
        .external_field_for(&receiver_name, member)
        .and_then(|fact| fact.field_type.clone())
        .or_else(|| {
            crate::external_packs::external_field_for(&receiver_name, member)
                .and_then(|fact| fact.field_type)
        })
        .or_else(|| {
            let receiver_type = JavaType::Named {
                name: receiver_name.clone(),
                args: Vec::new(),
            };
            crate::stdlib::known_external_field_type_for_receiver(&receiver_type, member)
        })
}

fn external_static_constant_rust_owner(
    receiver: &Expr,
    member: &str,
    field_ty: &JavaType,
    facts: &FactSet,
) -> Option<String> {
    let receiver_name = static_receiver_java_name(receiver)?;
    if external_type_is_rust_crate_mapped(&receiver_name, facts)
        || java_type_is_rust_crate_mapped(field_ty, facts)
    {
        return match crate::slot_render::rust_type_for_java_type_with_facts(field_ty, facts) {
            RustType::Raw(owner) | RustType::Named(owner) => Some(owner),
            _ => None,
        };
    }
    if facts.external_field_for(&receiver_name, member).is_some()
        || crate::external_packs::external_field_for(&receiver_name, member).is_some()
        || {
            let receiver_type = JavaType::Named {
                name: receiver_name.clone(),
                args: Vec::new(),
            };
            crate::stdlib::known_external_field_type_for_receiver(&receiver_type, member).is_some()
        }
    {
        let owner = receiver_name.rsplit('.').next().unwrap_or(&receiver_name);
        return static_constant_rust_owner_from_name(owner);
    }
    None
}

fn external_type_is_rust_crate_mapped(type_name: &str, facts: &FactSet) -> bool {
    if facts
        .external_library_pack_for_type(type_name)
        .is_some_and(|pack| pack.mode == ExternalLibraryMode::RustCrateMapping)
    {
        return true;
    }
    crate::external_packs::external_library_pack_for_type(type_name)
        .is_some_and(|pack| pack.mode == ExternalLibraryMode::RustCrateMapping)
}

fn java_type_is_rust_crate_mapped(java_ty: &JavaType, facts: &FactSet) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            external_type_is_rust_crate_mapped(name, facts)
        }
        _ => false,
    }
}

fn static_receiver_java_name(receiver: &Expr) -> Option<String> {
    match receiver {
        Expr::Var(name) => Some(crate::stdlib::canonical_external_receiver_type_key(name)),
        Expr::FieldAccess(inner, field) => {
            Some(format!("{}.{}", static_receiver_java_name(inner)?, field))
        }
        _ => None,
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
