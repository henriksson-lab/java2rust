use crate::facts::{
    ExternalApiArgumentEffectFact, ExternalArgumentAliasPolicy, ExternalLibraryMode,
    FactProvenance, FactSet, ReceiverMutabilityFact, TrivialAccessorKind,
};
use crate::java_ir::{CallKind, Expr, JavaType, ResolvedCall, ResolvedConstructor};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    value_shape_for_java_type,
};
use crate::signature::SignaturePlan;
use crate::slot_shape::{MutabilityShape, OwnershipShape, RustSlotShape};
use crate::types::{TypeEnv, infer_expr_type};

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
    ExternalCrateReceiver {
        receiver: &'a Expr,
        crate_name: String,
        rust_name: String,
    },
}

impl<'a> CallTargetPlan<'a> {
    pub fn receiver_expr(&self) -> Option<&'a Expr> {
        match self {
            CallTargetPlan::InstanceReceiver { receiver, .. }
            | CallTargetPlan::FallbackReceiver { receiver, .. }
            | CallTargetPlan::ExternalCrateReceiver { receiver, .. } => Some(receiver),
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
    pub external_crate: Option<ExternalCrateNewTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateNewTarget {
    pub crate_name: String,
    pub type_name: String,
    pub rust_name: String,
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
    types: &TypeEnv,
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

    CallExprPlan::Target(plan_call_target_with_facts(
        receiver, name, resolved, facts, types,
    ))
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
    plan_call_target_with_facts(
        receiver,
        name,
        resolved,
        &FactSet::default(),
        &TypeEnv::default(),
    )
}

pub fn plan_call_target_with_facts<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    if let Some(receiver) = receiver
        && let Some(target) = plan_external_crate_receiver_call(receiver, name, facts, types)
    {
        return target;
    }

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

fn plan_external_crate_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let effect = facts
        .external_api_effect_for_receiver(Some(&receiver_name), name)
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver(Some(&receiver_name), name)
        })?;
    if effect.provenance != FactProvenance::ExternalLibraryPack {
        return None;
    }
    let pack = facts
        .external_library_pack_for_type(&receiver_name)
        .cloned()
        .or_else(|| crate::external_packs::external_library_pack_for_type(&receiver_name))?;
    if pack.mode != ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let mapping = facts
        .external_crate_api_mapping_for_receiver(&receiver_name, name, effect.arg_count)
        .cloned()
        .or_else(|| {
            crate::external_packs::external_crate_api_mapping_for_receiver(
                &receiver_name,
                name,
                effect.arg_count,
            )
        });
    Some(CallTargetPlan::ExternalCrateReceiver {
        receiver,
        crate_name: pack.rust_target.clone()?,
        rust_name: mapping
            .map(|mapping| mapping.rust_name)
            .unwrap_or_else(|| crate::parser::to_snake_case(name)),
    })
}

pub fn plan_new_target<'a>(
    ty: &JavaType,
    resolved: Option<&'a ResolvedConstructor>,
    representations: &RepresentationPlan,
) -> NewTargetPlan<'a> {
    plan_new_target_with_facts(ty, resolved, representations, &FactSet::default(), 0)
}

pub fn plan_new_target_with_facts<'a>(
    ty: &JavaType,
    resolved: Option<&'a ResolvedConstructor>,
    representations: &RepresentationPlan,
    facts: &FactSet,
    arg_count: usize,
) -> NewTargetPlan<'a> {
    let owner_ty = resolved
        .map(|resolved| JavaType::Named {
            name: resolved.declaring_type.clone(),
            args: Vec::new(),
        })
        .unwrap_or_else(|| ty.clone());
    let rust_name = resolved
        .map(|resolved| resolved.rust_name.as_str())
        .unwrap_or("new");
    let external_crate = plan_external_crate_constructor(&owner_ty, rust_name, facts, arg_count);
    NewTargetPlan {
        owner_shape: value_shape_for_java_type(representations, owner_ty),
        rust_name,
        external_crate,
    }
}

fn plan_external_crate_constructor(
    ty: &JavaType,
    rust_name: &str,
    facts: &FactSet,
    arg_count: usize,
) -> Option<ExternalCrateNewTarget> {
    let type_name = java_type_name(ty)?.to_string();
    let arg_types = vec![JavaType::Unknown("_".to_string()); arg_count];
    let constructor = facts
        .external_constructor_for(&type_name, &arg_types)
        .cloned()
        .or_else(|| crate::external_packs::external_constructor_for(&type_name, &arg_types))?;
    if constructor.provenance != FactProvenance::ExternalLibraryPack {
        return None;
    }
    let pack = facts
        .external_library_pack_for_type(&type_name)
        .cloned()
        .or_else(|| crate::external_packs::external_library_pack_for_type(&type_name))?;
    if pack.mode != ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let mapping = facts
        .external_crate_constructor_mapping_for(&type_name, arg_count)
        .cloned()
        .or_else(|| {
            crate::external_packs::external_crate_constructor_mapping_for(&type_name, arg_count)
        });
    Some(ExternalCrateNewTarget {
        crate_name: pack.rust_target.clone()?,
        type_name: mapping
            .as_ref()
            .and_then(|mapping| mapping.rust_type_name.clone())
            .unwrap_or_else(|| {
                type_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&type_name)
                    .to_string()
            }),
        rust_name: mapping
            .map(|mapping| mapping.rust_name)
            .unwrap_or_else(|| rust_name.to_string()),
    })
}

fn java_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
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

pub fn plan_call_arguments_with_facts(
    receiver: Option<&Expr>,
    name: &str,
    resolved: Option<&ResolvedCall>,
    arg_count: usize,
    signatures: &SignaturePlan,
    facts: &FactSet,
    types: &TypeEnv,
) -> ArgumentLoweringPlan {
    if let Some(call) = resolved {
        return ArgumentLoweringPlan::Shaped(resolved_call_arg_shapes(call, arg_count, signatures));
    }

    external_call_arg_shapes(receiver, name, arg_count, facts, types)
        .map(ArgumentLoweringPlan::Shaped)
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
}

fn external_call_arg_shapes(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    let receiver_name = receiver
        .and_then(|receiver| java_type_name(&infer_expr_type(receiver, types)).map(str::to_string));
    (0..arg_count)
        .map(|index| {
            facts
                .external_api_argument_effect_for_receiver(receiver_name.as_deref(), name, index)
                .map(argument_shape_from_external_effect)
        })
        .collect()
}

fn argument_shape_from_external_effect(effect: &ExternalApiArgumentEffectFact) -> RustSlotShape {
    let java_ty = effect
        .parameter_type
        .clone()
        .unwrap_or_else(|| JavaType::Unknown("Object".to_string()));
    let mut shape = RustSlotShape::conservative(java_ty);

    match effect.alias_policy {
        ExternalArgumentAliasPolicy::Borrowed if shape.java_ty == JavaType::String => {
            apply_borrowed_string_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed if matches!(shape.java_ty, JavaType::Array(_)) => {
            apply_borrowed_sequence_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed
            if !matches!(shape.java_ty, JavaType::Primitive(_)) =>
        {
            shape.ownership = OwnershipShape::SharedBorrow;
        }
        ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes => {
            if !matches!(shape.java_ty, JavaType::Primitive(_)) {
                shape.ownership = OwnershipShape::Owned;
            }
        }
        ExternalArgumentAliasPolicy::Unknown | ExternalArgumentAliasPolicy::Borrowed => {}
    }

    if effect.argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && !matches!(shape.java_ty, JavaType::Primitive(_))
    {
        shape.ownership = OwnershipShape::MutableBorrow;
        shape.mutability = MutabilityShape::Mutable;
    }

    shape
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
