use crate::java_ir::{Expr, JavaType};
use crate::shape_projection::project_java_value_shape;
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_member_access_type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAccessOp {
    LenAsI32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessPlan {
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub op: MemberAccessOp,
}

pub fn plan_member_access(
    receiver: &Expr,
    member: &str,
    env: &TypeEnv,
) -> Option<MemberAccessPlan> {
    if member == "length" {
        let java_ty = infer_member_access_type(receiver, member, env)?;
        return Some(MemberAccessPlan {
            java_ty: java_ty.clone(),
            shape: project_java_value_shape(None, java_ty),
            op: MemberAccessOp::LenAsI32,
        });
    }
    None
}
