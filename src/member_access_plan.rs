use crate::java_ir::{Expr, JavaType};
use crate::shape_projection::project_java_value_shape;
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_member_access_type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAccessOp {
    LenAsI32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaMemberAccessKind {
    ArrayLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaMemberAccessPolicy {
    pub member: String,
    pub java_ty: JavaType,
    pub kind: JavaMemberAccessKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessPlan {
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
    };
    Some(MemberAccessPlan {
        java_ty: policy.java_ty.clone(),
        shape: project_java_value_shape(None, policy.java_ty),
        op,
    })
}
