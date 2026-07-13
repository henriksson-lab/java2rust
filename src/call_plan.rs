use crate::facts::{FactSet, TrivialAccessorKind};
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

impl<'a> CallTargetPlan<'a> {
    pub fn receiver_expr(&self) -> Option<&'a Expr> {
        match self {
            CallTargetPlan::InstanceReceiver { receiver, .. }
            | CallTargetPlan::FallbackReceiver { receiver, .. } => Some(receiver),
            CallTargetPlan::StaticSelf { .. }
            | CallTargetPlan::InstanceSelf { .. }
            | CallTargetPlan::FallbackFree { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTargetPlan<'a> {
    pub owner_shape: RustSlotShape,
    pub rust_name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentLoweringPlan {
    Shaped(Vec<RustSlotShape>),
    Unshaped { arg_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrivialAccessorCallPlan<'a> {
    Getter {
        receiver: Option<&'a Expr>,
        field_name: String,
    },
    Setter {
        receiver: Option<&'a Expr>,
        field_name: String,
        value: &'a Expr,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallExprPlan<'a> {
    TrivialAccessorGetter {
        receiver: Option<&'a Expr>,
        field_name: String,
    },
    Target(CallTargetPlan<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallStatementPlan<'a> {
    TrivialAccessorSetter {
        receiver: Option<&'a Expr>,
        field_name: String,
        value: &'a Expr,
    },
}

pub fn plan_trivial_accessor_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
    resolved: Option<&ResolvedCall>,
    facts: &FactSet,
) -> Option<TrivialAccessorCallPlan<'a>> {
    let resolved = resolved?;
    if resolved.kind != CallKind::Instance {
        return None;
    }
    let accessor = facts.trivial_accessor_for(&resolved.declaring_type, name)?;
    match &accessor.kind {
        TrivialAccessorKind::Getter { field } if args.is_empty() => {
            Some(TrivialAccessorCallPlan::Getter {
                receiver,
                field_name: field.field_name.clone(),
            })
        }
        TrivialAccessorKind::Setter { field, .. } if args.len() == 1 => {
            Some(TrivialAccessorCallPlan::Setter {
                receiver,
                field_name: field.field_name.clone(),
                value: &args[0],
            })
        }
        _ => None,
    }
}

pub fn plan_call_expr<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    args: &'a [Expr],
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
) -> CallExprPlan<'a> {
    if let Some(TrivialAccessorCallPlan::Getter {
        receiver,
        field_name,
    }) = plan_trivial_accessor_call(receiver, name, args, resolved, facts)
    {
        return CallExprPlan::TrivialAccessorGetter {
            receiver,
            field_name,
        };
    }

    CallExprPlan::Target(plan_call_target(receiver, name, resolved))
}

pub fn plan_call_statement<'a>(expr: &'a Expr, facts: &FactSet) -> Option<CallStatementPlan<'a>> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    match plan_trivial_accessor_call(receiver.as_deref(), name, args, resolved.as_ref(), facts)? {
        TrivialAccessorCallPlan::Setter {
            receiver,
            field_name,
            value,
        } => Some(CallStatementPlan::TrivialAccessorSetter {
            receiver,
            field_name,
            value,
        }),
        TrivialAccessorCallPlan::Getter { .. } => None,
    }
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

pub fn plan_call_arguments(
    resolved: Option<&ResolvedCall>,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> ArgumentLoweringPlan {
    resolved
        .map(|call| {
            ArgumentLoweringPlan::Shaped(resolved_call_arg_shapes(call, arg_count, signatures))
        })
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
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

pub fn plan_constructor_arguments(
    resolved: Option<&ResolvedConstructor>,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> ArgumentLoweringPlan {
    resolved
        .map(|constructor| {
            ArgumentLoweringPlan::Shaped(resolved_constructor_arg_shapes(
                constructor,
                arg_count,
                signatures,
            ))
        })
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
}
