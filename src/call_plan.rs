use crate::java_ir::{CallKind, Expr, JavaType, ResolvedCall, ResolvedConstructor};
use crate::representation::RepresentationPlan;
use crate::shape_projection::value_shape_for_java_type;
use crate::signature::SignaturePlan;
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, PartialEq)]
pub enum CallTargetPlan<'a> {
    StaticSelf {
        rust_name: &'a str,
    },
    InstanceReceiver {
        receiver: &'a Expr,
        rust_name: &'a str,
    },
    InstanceSelf {
        rust_name: &'a str,
    },
    FallbackReceiver {
        receiver: &'a Expr,
        rust_name: String,
    },
    FallbackFree {
        rust_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTargetPlan<'a> {
    pub owner_shape: RustSlotShape,
    pub rust_name: &'a str,
}

pub fn plan_call_target<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    resolved: Option<&'a ResolvedCall>,
) -> CallTargetPlan<'a> {
    if let Some(call) = resolved {
        return match (&call.kind, receiver) {
            (CallKind::Static, _) => CallTargetPlan::StaticSelf {
                rust_name: &call.rust_name,
            },
            (CallKind::Instance, Some(receiver)) => CallTargetPlan::InstanceReceiver {
                receiver,
                rust_name: &call.rust_name,
            },
            (CallKind::Instance, None) => CallTargetPlan::InstanceSelf {
                rust_name: &call.rust_name,
            },
        };
    }

    if let Some(receiver) = receiver {
        CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name: crate::parser::to_snake_case(name),
        }
    } else {
        CallTargetPlan::FallbackFree {
            rust_name: crate::parser::to_snake_case(name),
        }
    }
}

pub fn plan_new_target<'a>(
    ty: &JavaType,
    resolved: Option<&'a ResolvedConstructor>,
    representations: &RepresentationPlan,
) -> NewTargetPlan<'a> {
    let owner_ty = resolved
        .map(|resolved| JavaType::Named {
            name: resolved.declaring_type.clone(),
            args: Vec::new(),
        })
        .unwrap_or_else(|| ty.clone());
    NewTargetPlan {
        owner_shape: value_shape_for_java_type(representations, owner_ty),
        rust_name: resolved
            .map(|resolved| resolved.rust_name.as_str())
            .unwrap_or("new"),
    }
}

pub fn resolved_call_arg_shapes(
    call: &ResolvedCall,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Vec<RustSlotShape> {
    (0..arg_count)
        .map(|index| {
            signatures.required_method_arg_shape_by_rust_name(
                &call.declaring_type,
                &call.rust_name,
                index,
            )
        })
        .collect()
}

pub fn resolved_constructor_arg_shapes(
    constructor: &ResolvedConstructor,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Vec<RustSlotShape> {
    (0..arg_count)
        .map(|index| {
            signatures.required_constructor_arg_shape_by_rust_name(
                &constructor.declaring_type,
                &constructor.rust_name,
                index,
            )
        })
        .collect()
}
