use std::collections::BTreeSet;

use crate::conversion_plan::LoweredType;
use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    AccessorInlinePolicy, CallableKey, ExternalApiArgumentEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalLibraryMode, FacadeAbiFact, FacadeSlot, FactProvenance,
    FactSet, HierarchyStrategy, NullabilityState, ReceiverAbi, ReceiverMutabilityFact, ResultAbi,
    Staticness, TrivialAccessorKind, external_parameter_nullability_policy,
};
use crate::java_ir::{
    CallKind, Expr, JavaType, LambdaBody, PrimitiveType, ResolvedCall, ResolvedConstructor,
};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    apply_required_dispatch, borrowed_sequence_element_shape, value_shape_for_java_type,
};
use crate::signature::SignaturePlan;
use crate::slot_shape::{DispatchShape, MutabilityShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, PartialEq)]
pub enum CallTargetPlan<'a> {
    StaticSelf {
        rust_name: &'a str,
    },
    StaticOwner {
        owner: String,
        rust_name: String,
    },
    InstanceReceiver {
        receiver: &'a Expr,
        rust_name: &'a str,
    },
    InstanceSelf {
        rust_name: &'a str,
    },
    FallbackSelf {
        rust_name: String,
    },
    FallbackReceiver {
        receiver: &'a Expr,
        rust_name: String,
    },
    FallbackFree {
        rust_name: String,
    },
    JavaClassStatic {
        rust_name: String,
    },
    NativeCollectionMethod {
        receiver: &'a Expr,
        method: NativeCollectionMethod,
    },
    NativeCollectionStatic {
        method: NativeCollectionStaticMethod,
    },
    NativeJavaInstance {
        receiver: &'a Expr,
        method: NativeJavaInstanceMethod,
    },
    NativeJavaStatic {
        method: NativeJavaStaticMethod,
    },
    ExternalCrateReceiver {
        receiver: &'a Expr,
        crate_name: String,
        rust_name: String,
        mutates_receiver: bool,
    },
    ExternalFacadeReceiver {
        receiver: &'a Expr,
        rust_path: String,
        receiver_abi: ReceiverAbi,
        mutates_receiver: bool,
    },
    SuperBoundary {
        rust_name: String,
        return_type: JavaType,
        propagates_result: bool,
    },
}

impl<'a> CallTargetPlan<'a> {
    pub fn receiver_expr(&self) -> Option<&'a Expr> {
        match self {
            CallTargetPlan::InstanceReceiver { receiver, .. }
            | CallTargetPlan::FallbackReceiver { receiver, .. }
            | CallTargetPlan::ExternalCrateReceiver { receiver, .. }
            | CallTargetPlan::ExternalFacadeReceiver { receiver, .. }
            | CallTargetPlan::NativeCollectionMethod { receiver, .. }
            | CallTargetPlan::NativeJavaInstance { receiver, .. } => Some(receiver),
            CallTargetPlan::StaticSelf { .. }
            | CallTargetPlan::StaticOwner { .. }
            | CallTargetPlan::InstanceSelf { .. }
            | CallTargetPlan::FallbackSelf { .. }
            | CallTargetPlan::FallbackFree { .. }
            | CallTargetPlan::JavaClassStatic { .. }
            | CallTargetPlan::NativeCollectionStatic { .. }
            | CallTargetPlan::NativeJavaStatic { .. }
            | CallTargetPlan::SuperBoundary { .. } => None,
        }
    }

    pub fn adapts_optional_instance_receiver(&self) -> bool {
        matches!(
            self,
            CallTargetPlan::InstanceReceiver { .. }
                | CallTargetPlan::FallbackReceiver { .. }
                | CallTargetPlan::ExternalCrateReceiver { .. }
                | CallTargetPlan::ExternalFacadeReceiver { .. }
                | CallTargetPlan::NativeJavaInstance { .. }
        )
    }

    pub fn mutates_receiver(&self) -> bool {
        match self {
            CallTargetPlan::ExternalCrateReceiver {
                mutates_receiver, ..
            }
            | CallTargetPlan::ExternalFacadeReceiver {
                mutates_receiver, ..
            } => *mutates_receiver,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCollectionMethod {
    Len,
    IsEmpty,
    ListPush,
    ListGet,
    SetInsert,
    Extend,
    MapInsert,
    MapGet,
    MapContainsKey,
    MapKeys,
    MapIter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeCollectionExtendArgPlan {
    OwnedSource { slot_shape: RustSlotShape },
    BorrowedSequenceSource { element_shape: RustSlotShape },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCollectionStaticMethod {
    ArraysAsList,
    CollectionsEmptyList,
    CollectionsEmptySortedSet,
    CollectionsSingleton,
    CollectionsSingletonList,
    CollectionsSingletonMap,
    CollectionsNCopies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeJavaInstanceMethod {
    StringLen,
    StringIsEmpty,
    StringTrim,
    StringToLowercase,
    StringToString,
    StringContains,
    StringStartsWith,
    StringEndsWith,
    StringSplit,
    Equals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeJavaStaticMethod {
    MathMax,
    MathToIntExact,
    IntegerParseInt,
    DoubleParseDouble,
    StringFormat,
    StringJoin,
    ObjectsEquals,
}

pub fn native_collection_lowered_type(
    receiver: &Expr,
    method: NativeCollectionMethod,
    types: &TypeEnv,
) -> Option<LoweredType> {
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    match method {
        NativeCollectionMethod::ListGet => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(element_ty.clone()),
            ))
        }
        NativeCollectionMethod::MapGet => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(value_ty.clone()),
            ))
        }
        NativeCollectionMethod::MapInsert => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                value_ty.clone(),
            )))
        }
        _ => None,
    }
}

pub fn external_call_lowered_type(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<LoweredType> {
    let receiver_key =
        external_call_receiver_type_key(receiver, name, arg_count, facts, types, current_type)?;
    let effect = facts.external_api_effect_for_receiver_with_arity(
        receiver_key.as_deref(),
        name,
        Some(arg_count),
    )?;
    let return_ty = effect.return_type.clone()?;
    if let Some(facade_lowered) = facade_abi_external_call_lowered_type(
        receiver_key.as_deref(),
        name,
        arg_count,
        facts,
        &return_ty,
    ) {
        return Some(facade_lowered);
    }
    let rust_return_type = facts.external_rust_return_type_for_receiver(
        receiver_key.as_deref(),
        name,
        Some(arg_count),
    );
    let rust_return_is_shared_borrow = rust_return_type
        .is_some_and(|fact| external_return_slot_contains_shared_borrow(&fact.return_slot));

    if rust_return_is_shared_borrow
        && matches!(
            effect.return_nullability,
            NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
        )
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Some(LoweredType::optional_element_borrow_shape(
            borrowed_sequence_element_shape(return_ty),
        ));
    }

    let returns_optional = matches!(
        effect.return_nullability,
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    );
    if java_type_is_byte_array(&return_ty)
        && rust_return_type
            .is_none_or(|fact| rust_return_type_is_unsigned_byte_array(&fact.rust_type_name))
        && receiver_key.as_deref().is_some_and(|receiver_key| {
            facts
                .external_library_pack_for_type(receiver_key)
                .is_some_and(|pack| pack.mode == ExternalLibraryMode::RustCrateMapping)
                || crate::external_packs::external_library_pack_for_type(receiver_key)
                    .is_some_and(|pack| pack.mode == ExternalLibraryMode::RustCrateMapping)
        })
    {
        let semantic_ty = if returns_optional {
            crate::types::optional_java_type(return_ty)
        } else {
            return_ty
        };
        return Some(LoweredType::rust_unsigned_byte_array_return(
            semantic_ty,
            returns_optional,
        ));
    }

    let lowered = match effect.return_nullability {
        NullabilityState::MaybeNull
            if external_return_borrows_receiver(effect.role, &return_ty)
                && rust_return_type.is_none_or(|_| rust_return_is_shared_borrow) =>
        {
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(return_ty),
            ))
        }
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull => Some(
            LoweredType::from_java(crate::types::optional_java_type(return_ty)),
        ),
        NullabilityState::DefinitelyNonNull | NullabilityState::Unknown => {
            Some(LoweredType::from_java(return_ty))
        }
    }?;

    if let Some(rust_return_type) = rust_return_type
        && let Some(java_ty) = lowered.java_ty()
    {
        if external_return_slot_contains_shared_borrow(&rust_return_type.return_slot)
            && !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
        {
            return Some(LoweredType::rust_borrowed_return(
                java_ty.clone(),
                rust_return_type.rust_type_name.clone(),
            ));
        }
        if matches!(java_ty, JavaType::Primitive(_)) {
            return Some(LoweredType::rust_scalar_return(
                java_ty.clone(),
                rust_return_type.rust_type_name.clone(),
            ));
        }
    }

    Some(lowered)
}

fn facade_abi_external_call_lowered_type(
    receiver_key: Option<&str>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    return_ty: &JavaType,
) -> Option<LoweredType> {
    let receiver_key = receiver_key?;
    let key = CallableKey::method(
        receiver_key.to_string(),
        name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let abi = facts.facade_abi_for_key(&key)?;
    lowered_type_for_facade_abi(abi, return_ty)
}

fn lowered_type_for_facade_abi(abi: &FacadeAbiFact, return_ty: &JavaType) -> Option<LoweredType> {
    if matches!(return_ty, JavaType::Void) {
        return Some(LoweredType::from_java(return_ty.clone()));
    }

    let unwrapped_slot = match &abi.return_slot {
        FacadeSlot::Optional(slot) => slot.as_ref(),
        slot => slot,
    };
    let returns_optional = matches!(abi.result, ResultAbi::OptionWrapped)
        || matches!(abi.return_slot, FacadeSlot::Optional(_));

    if returns_optional && matches!(unwrapped_slot, FacadeSlot::Borrowed) {
        return Some(LoweredType::optional_element_borrow_shape(
            borrowed_sequence_element_shape(return_ty.clone()),
        ));
    }

    if java_type_is_byte_array(return_ty)
        && matches!(unwrapped_slot, FacadeSlot::Owned | FacadeSlot::Slice(_))
    {
        let semantic_ty = if returns_optional {
            crate::types::optional_java_type(return_ty.clone())
        } else {
            return_ty.clone()
        };
        return Some(LoweredType::rust_unsigned_byte_array_return(
            semantic_ty,
            returns_optional,
        ));
    }

    match unwrapped_slot {
        FacadeSlot::Borrowed if !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void) => {
            Some(LoweredType::rust_borrowed_return(
                return_ty.clone(),
                abi.rust_path.clone(),
            ))
        }
        FacadeSlot::Owned
        | FacadeSlot::Borrowed
        | FacadeSlot::MutBorrowed
        | FacadeSlot::Slice(_) => {
            let semantic_ty = if returns_optional {
                crate::types::optional_java_type(return_ty.clone())
            } else {
                return_ty.clone()
            };
            Some(LoweredType::from_java(semantic_ty))
        }
        FacadeSlot::DynTrait(_) | FacadeSlot::CrateHandle(_) => {
            Some(LoweredType::from_java(if returns_optional {
                crate::types::optional_java_type(return_ty.clone())
            } else {
                return_ty.clone()
            }))
        }
        FacadeSlot::Optional(_) => None,
    }
}

fn external_return_slot_contains_shared_borrow(slot: &FacadeSlot) -> bool {
    match slot {
        FacadeSlot::Borrowed => true,
        FacadeSlot::Optional(inner) | FacadeSlot::Slice(inner) => {
            external_return_slot_contains_shared_borrow(inner)
        }
        FacadeSlot::Owned
        | FacadeSlot::MutBorrowed
        | FacadeSlot::DynTrait(_)
        | FacadeSlot::CrateHandle(_) => false,
    }
}

fn rust_return_type_is_unsigned_byte_array(rust_type_name: &str) -> bool {
    let compact = rust_type_name.replace(' ', "");
    compact == "Vec<u8>" || compact == "Option<Vec<u8>>"
}

fn java_type_is_byte_array(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Array(element)
            if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
    )
}

pub fn external_call_returns_rust_result(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> bool {
    let Some(receiver_key) =
        external_call_receiver_type_key(receiver, name, arg_count, facts, types, current_type)
    else {
        return false;
    };
    if let Some(receiver_key) = receiver_key.as_deref() {
        let key = CallableKey::method(
            receiver_key.to_string(),
            name.to_string(),
            Staticness::Unknown,
            arg_count,
        );
        if let Some(abi) = facts.facade_abi_for_key(&key) {
            return matches!(abi.result, ResultAbi::ResultWrapped);
        }
    }
    if let Some(fallibility) =
        facts.external_rust_result_for_receiver(receiver_key.as_deref(), name, Some(arg_count))
    {
        return fallibility.returns_result;
    }
    facts
        .external_api_effect_for_receiver_with_arity(receiver_key.as_deref(), name, Some(arg_count))
        .is_some_and(|effect| !effect.thrown.is_empty())
}

pub fn external_call_has_thrown_effect_for_receiver(
    facts: &FactSet,
    receiver_name: &str,
    name: &str,
    arg_count: usize,
) -> bool {
    facts
        .external_api_effect_for_receiver_with_arity(Some(receiver_name), name, Some(arg_count))
        .is_some_and(|effect| !effect.thrown.is_empty())
}

pub fn external_call_throws(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> bool {
    external_call_returns_rust_result(receiver, name, arg_count, facts, types, current_type)
}

pub fn external_constructor_returns_rust_result(
    ty: &JavaType,
    arg_count: usize,
    facts: &FactSet,
) -> bool {
    let Some(type_name) = java_type_name(ty) else {
        return false;
    };
    facts
        .external_constructor_rust_result_for(type_name, arg_count)
        .is_some_and(|fact| fact.returns_result)
}

fn external_return_borrows_receiver(role: ExternalApiRole, return_ty: &JavaType) -> bool {
    matches!(role, ExternalApiRole::IndexedGetter)
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTargetPlan {
    pub owner_shape: RustSlotShape,
    pub rust_name: String,
    pub external_crate: Option<ExternalCrateNewTarget>,
    pub native_collection: Option<NativeCollectionNewTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateNewTarget {
    pub crate_name: String,
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCollectionNewTarget {
    Empty {
        kind: crate::stdlib::JavaCollectionKind,
    },
    WithCapacity {
        kind: crate::stdlib::JavaCollectionKind,
    },
    FromSource {
        kind: crate::stdlib::JavaCollectionKind,
    },
    FromSingleArg {
        kind: crate::stdlib::JavaCollectionKind,
    },
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
    IterableForEach {
        receiver: &'a Expr,
        param_names: Vec<String>,
        body: &'a LambdaBody,
    },
    NativeArraysSort {
        target: &'a Expr,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallExceptionPropagationPlan {
    PropagateResult,
    NoPropagation,
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
    if accessor.inline_policy != AccessorInlinePolicy::InlineCandidate {
        return None;
    }
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

pub fn plan_call_exception_propagation(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> CallExceptionPropagationPlan {
    if let Some(resolved) = resolved
        && facts.method_has_thrown_exceptions(&resolved.declaring_type, &resolved.rust_name)
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }
    if receiver.is_none()
        && let Some(current_type) = current_type
        && facts.method_has_thrown_exceptions(current_type, name)
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }
    if matches!(receiver, Some(Expr::This))
        && let Some(current_type) = current_type
        && facts.method_has_thrown_exceptions(current_type, name)
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }
    if let Some(receiver_type) =
        receiver.and_then(|receiver| receiver_source_type_name(receiver, types))
        && facts.method_has_thrown_exceptions(receiver_type, name)
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }

    if let Some(effect) = external_call_effect(receiver, name, args, facts, types, current_type)
        && !effect.thrown.is_empty()
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }

    CallExceptionPropagationPlan::NoPropagation
}

fn receiver_source_type_name<'a>(receiver: &Expr, types: &'a TypeEnv) -> Option<&'a str> {
    match receiver {
        Expr::Var(name) => java_type_name(types.local_type(name)?),
        Expr::FieldAccess(_, field) => java_type_name(types.field_type(field)?),
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

    CallExprPlan::Target(plan_call_target_with_facts_and_arity(
        receiver,
        name,
        args.len(),
        resolved,
        facts,
        types,
    ))
}

pub fn apply_fallback_receiver_rust_name<'a>(
    plan: CallExprPlan<'a>,
    fallback_rust_name: Option<String>,
) -> CallExprPlan<'a> {
    match (plan, fallback_rust_name) {
        (
            CallExprPlan::Target(CallTargetPlan::FallbackReceiver { receiver, .. }),
            Some(rust_name),
        ) => CallExprPlan::Target(CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name,
        }),
        (plan, _) => plan,
    }
}

pub fn apply_fallback_self_rust_name<'a>(
    plan: CallExprPlan<'a>,
    fallback_rust_name: Option<String>,
) -> CallExprPlan<'a> {
    match (plan, fallback_rust_name) {
        (CallExprPlan::Target(CallTargetPlan::FallbackFree { .. }), Some(rust_name)) => {
            CallExprPlan::Target(CallTargetPlan::FallbackSelf { rust_name })
        }
        (plan, _) => plan,
    }
}

fn external_call_effect<'a>(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    facts: &'a FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<&'a crate::facts::ExternalApiEffectFact> {
    let receiver_key =
        external_call_receiver_type_key(receiver, name, args.len(), facts, types, current_type)?;
    facts.external_api_effect_for_receiver_with_arity(
        receiver_key.as_deref(),
        name,
        Some(args.len()),
    )
}

pub fn external_call_receiver_type_key(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<Option<String>> {
    match receiver {
        None if crate::stdlib::is_known_receiverless_static_utility_method(name, arg_count) => {
            Some(None)
        }
        None => current_type.map(|type_name| Some(type_name.to_string())),
        Some(Expr::This | Expr::Super) => current_type.map(|type_name| Some(type_name.to_string())),
        Some(receiver) => external_receiver_expr_type_key(receiver, facts, types).map(Some),
    }
}

fn external_receiver_expr_type_key(
    receiver: &Expr,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<String> {
    let inferred = infer_expr_type(receiver, types);
    if weak_unknown_field_access_receiver(receiver, &inferred) {
        return infer_external_field_access_type(receiver, facts, types)
            .and_then(|external_field_ty| external_receiver_type_key(&external_field_ty, types));
    }
    external_receiver_type_key(&inferred, types)
}

fn infer_external_field_access_type(
    expr: &Expr,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<JavaType> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    let receiver_ty = infer_external_field_access_type(receiver, facts, types)
        .unwrap_or_else(|| infer_expr_type(receiver, types));
    external_pack_or_fact_field_type(&receiver_ty, field, facts)
}

fn external_pack_or_fact_field_type(
    receiver_ty: &JavaType,
    field: &str,
    facts: &FactSet,
) -> Option<JavaType> {
    let receiver_name = external_field_receiver_name(receiver_ty)?;
    if let Some(field_fact) = facts.external_field_for(&receiver_name, field)
        && let Some(field_type) = &field_fact.field_type
    {
        return Some(field_type.clone());
    }
    if let Some(field_fact) = crate::external_packs::external_field_for(&receiver_name, field)
        && let Some(field_type) = field_fact.field_type
    {
        return Some(field_type);
    }
    crate::stdlib::known_external_field_type_for_receiver(receiver_ty, field)
}

fn external_field_receiver_name(receiver_ty: &JavaType) -> Option<String> {
    match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        _ => None,
    }
}

fn weak_unknown_field_access_receiver(receiver: &Expr, inferred: &JavaType) -> bool {
    match (receiver, inferred) {
        (Expr::FieldAccess(_, field), JavaType::Unknown(name)) => {
            name == field
                || crate::parser::to_snake_case(name) == crate::parser::to_snake_case(field)
        }
        _ => false,
    }
}

fn external_receiver_type_key(java_ty: &JavaType, types: &TypeEnv) -> Option<String> {
    if let Some(element_ty) = crate::types::optional_element_type(java_ty) {
        return external_receiver_type_key(&element_ty, types);
    }
    if let JavaType::Named { name, args } = java_ty
        && args.is_empty()
        && let Some(bound) = types.type_param_bound(name)
    {
        return external_receiver_type_key(bound, types);
    }
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(
                rust_path_simple_type_name(name).unwrap_or(name),
            ))
        }
        JavaType::String => Some("String".to_string()),
        JavaType::Array(_) => Some("Array".to_string()),
        JavaType::Void | JavaType::Primitive(_) | JavaType::Wildcard(_) => None,
    }
}

fn rust_path_simple_type_name(name: &str) -> Option<&str> {
    name.rsplit_once("::").map(|(_, simple)| simple)
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
    if let Some(plan) = plan_iterable_for_each_statement(receiver.as_deref(), name, args) {
        return Some(plan);
    }
    if let Some(plan) = plan_native_arrays_sort_statement(receiver.as_deref(), name, args) {
        return Some(plan);
    }
    match plan_trivial_accessor_call(receiver.as_deref(), name, args, resolved.as_ref(), facts) {
        Some(TrivialAccessorCallPlan::Setter {
            receiver,
            field_name,
            value,
        }) => Some(CallStatementPlan::TrivialAccessorSetter {
            receiver,
            field_name,
            value,
        }),
        Some(TrivialAccessorCallPlan::Getter { .. }) | None => None,
    }
}

fn plan_native_arrays_sort_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<CallStatementPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) != "Arrays"
        || name != "sort"
        || args.len() != 1
    {
        return None;
    }
    Some(CallStatementPlan::NativeArraysSort { target: &args[0] })
}

fn plan_iterable_for_each_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<CallStatementPlan<'a>> {
    if name != "forEach" || args.len() != 1 {
        return None;
    }
    let receiver = receiver?;
    let Expr::Lambda { params, body } = &args[0] else {
        return None;
    };
    let param_names = params
        .iter()
        .map(|param| param.name.clone())
        .collect::<Vec<_>>();
    if param_names.is_empty() {
        return None;
    }
    Some(CallStatementPlan::IterableForEach {
        receiver,
        param_names,
        body,
    })
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
    plan_call_target_with_facts_and_arity(receiver, name, 0, resolved, facts, types)
}

pub fn plan_call_target_with_facts_and_arity<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    arg_count: usize,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    if matches!(receiver, Some(Expr::Super))
        && let Some(call) = resolved
        && matches!(call.kind, CallKind::Instance)
    {
        return CallTargetPlan::SuperBoundary {
            rust_name: call.rust_name.clone(),
            return_type: call.return_type.clone(),
            propagates_result: facts
                .method_has_thrown_exceptions(&call.declaring_type, &call.rust_name),
        };
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_java_logging_facade_call(receiver, name)
    {
        return target;
    }

    if let Some(target) = plan_native_collection_static_call(receiver, name) {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_external_crate_static_call(receiver, name, arg_count, facts, types)
    {
        return target;
    }

    if let Some(target) = plan_native_java_static_call(receiver, name) {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_external_facade_receiver_call(receiver, name, arg_count, facts, types)
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_external_crate_receiver_call(receiver, name, arg_count, facts, types)
    {
        return target;
    }

    if let Some(call) = resolved
        && let (CallKind::Instance, Some(receiver)) = (&call.kind, receiver)
    {
        return CallTargetPlan::InstanceReceiver {
            receiver,
            rust_name: &call.rust_name,
        };
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_native_collection_receiver_call(receiver, name, types)
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_native_java_receiver_call(receiver, name, types)
    {
        return target;
    }

    if let Some(target) = plan_java_class_static_call(receiver, name) {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(owner) = static_utility_owner_expr_type_name(receiver, name, facts, types)
    {
        return CallTargetPlan::StaticOwner {
            owner,
            rust_name: crate::parser::to_snake_case(name),
        };
    }

    if let Some(target) = plan_resolved_static_call(receiver, resolved, facts) {
        return target;
    }

    if let Some(call) = resolved {
        return match (&call.kind, receiver) {
            (CallKind::Instance, Some(receiver)) => CallTargetPlan::InstanceReceiver {
                receiver,
                rust_name: &call.rust_name,
            },
            (CallKind::Instance, None) => CallTargetPlan::InstanceSelf {
                rust_name: &call.rust_name,
            },
            (CallKind::Static, Some(_)) | (CallKind::Static, None) => unreachable!(),
        };
    }

    if let Some(receiver) = receiver {
        if let Some(owner) = qualified_static_owner_expr_type_name(receiver, types, facts) {
            return CallTargetPlan::StaticOwner {
                owner,
                rust_name: crate::parser::to_snake_case(name),
            };
        }
        CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name: external_method_rust_name_for_receiver(
                receiver, name, arg_count, facts, types,
            )
            .unwrap_or_else(|| crate::parser::to_snake_case(name)),
        }
    } else {
        if let Some(owner) = types.static_member_import_owner(name) {
            return CallTargetPlan::StaticOwner {
                owner: rust_static_owner_type_name_with_facts(owner, facts),
                rust_name: crate::parser::to_snake_case(name),
            };
        }
        let rust_name = crate::parser::to_snake_case(name);
        if let Some(owner) = types.static_member_import_owner(&rust_name) {
            return CallTargetPlan::StaticOwner {
                owner: rust_static_owner_type_name_with_facts(owner, facts),
                rust_name,
            };
        }
        CallTargetPlan::FallbackFree {
            rust_name: crate::parser::to_snake_case(name),
        }
    }
}

fn plan_resolved_static_call<'a>(
    receiver: Option<&'a Expr>,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
) -> Option<CallTargetPlan<'a>> {
    let call = resolved?;
    if call.kind != CallKind::Static {
        return None;
    }
    Some(match receiver {
        Some(_) => CallTargetPlan::StaticOwner {
            owner: rust_static_owner_type_name_with_facts(&call.declaring_type, facts),
            rust_name: call.rust_name.clone(),
        },
        None => CallTargetPlan::StaticSelf {
            rust_name: &call.rust_name,
        },
    })
}

fn plan_external_facade_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_name = external_receiver_expr_type_key(receiver, facts, types)?;
    let key = CallableKey::method(
        receiver_name,
        name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let abi = facts.facade_abi_for_key(&key)?;
    Some(CallTargetPlan::ExternalFacadeReceiver {
        receiver,
        rust_path: abi.rust_path.clone(),
        receiver_abi: abi.receiver,
        mutates_receiver: matches!(abi.receiver, ReceiverAbi::MutBorrowedSelf),
    })
}

fn plan_java_logging_facade_call<'a>(receiver: &'a Expr, name: &str) -> Option<CallTargetPlan<'a>> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if !matches!(
        crate::stdlib::canonical_external_receiver_type_key(receiver_name).as_str(),
        "logger" | "Logger"
    ) {
        return None;
    }
    if !matches!(name, "info" | "warn" | "debug" | "error" | "trace") {
        return None;
    }
    Some(CallTargetPlan::StaticOwner {
        owner: "logger".to_string(),
        rust_name: crate::parser::to_snake_case(name),
    })
}

fn static_utility_owner_expr_type_name(
    receiver: &Expr,
    name: &str,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<String> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    if let Some(owner) = types.static_type_alias(receiver_name) {
        return Some(rust_static_owner_type_name_with_facts(owner, facts));
    }
    let owner = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if facts
        .external_api_effect_for_receiver(Some(&owner), name)
        .is_some()
        || crate::external_packs::external_api_effect_for_receiver(Some(&owner), name).is_some()
        || crate::stdlib::external_api_method_role(Some(&owner), name, 0)
            != ExternalApiRole::Unknown
    {
        Some(rust_static_owner_type_name_with_facts(&owner, facts))
    } else {
        None
    }
}

pub fn rust_static_owner_type_name(owner: &str) -> String {
    rust_static_owner_type_name_with_facts(owner, &FactSet::default())
}

pub fn rust_static_owner_type_name_with_facts(owner: &str, facts: &FactSet) -> String {
    if let Some(mapped_owner) = external_crate_static_owner_type_name(owner, facts) {
        return mapped_owner;
    }
    crate::slot_render::rust_name_for_java_named_type(owner)
}

fn external_crate_static_owner_type_name(owner: &str, facts: &FactSet) -> Option<String> {
    if let Some(pack) = facts.external_library_pack_for_type(owner) {
        let owner = external_crate_static_owner_source_name(owner, pack);
        return external_crate_static_owner_type_name_from_pack(
            owner,
            pack.mode,
            pack.rust_target.as_deref(),
        );
    }
    let pack = crate::external_packs::external_library_pack_for_type(owner)?;
    let owner = external_crate_static_owner_source_name(owner, &pack);
    external_crate_static_owner_type_name_from_pack(owner, pack.mode, pack.rust_target.as_deref())
}

fn external_crate_static_owner_source_name<'a>(
    owner: &'a str,
    pack: &'a crate::facts::ExternalLibraryPackFact,
) -> &'a str {
    if crate::facts::external_type_name_matches(&pack.package_prefix, owner) {
        &pack.package_prefix
    } else {
        owner
    }
}

fn external_crate_static_owner_type_name_from_pack(
    owner: &str,
    mode: ExternalLibraryMode,
    rust_target: Option<&str>,
) -> Option<String> {
    if mode != ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let crate_name = rust_target?;
    let simple = owner
        .split('.')
        .filter(|segment| !segment.is_empty())
        .next_back()
        .unwrap_or(owner);
    let rust_owner = if simple
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        crate::parser::to_snake_case(simple)
    } else {
        upper_camel_from_snake_or_preserved(simple)
    };
    Some(format!("{crate_name}::{rust_owner}"))
}

fn qualified_static_owner_expr_type_name(
    receiver: &Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<String> {
    let Expr::FieldAccess(_, _) = receiver else {
        return None;
    };
    static_owner_expr_type_name(receiver, types, facts)
}

fn static_owner_expr_type_name(
    receiver: &Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<String> {
    match receiver {
        Expr::Var(name) => {
            if types.local_type(name).is_some() || types.field_type(name).is_some() {
                return None;
            }
            Some(rust_static_owner_type_name_with_facts(name, facts))
        }
        Expr::FieldAccess(inner, field) => {
            if is_static_constant_member(field) {
                return None;
            }
            let _ = static_owner_expr_type_name(inner, types, facts)?;
            Some(upper_camel_from_snake_or_preserved(field))
        }
        _ => None,
    }
}

fn upper_camel_from_snake_or_preserved(name: &str) -> String {
    if name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        return name.to_string();
    }
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in name.chars() {
        if ch == '_' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn is_static_constant_member(member: &str) -> bool {
    member
        .chars()
        .any(|ch| ch == '_' || ch.is_ascii_uppercase())
        && member
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn plan_native_collection_static_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
) -> Option<CallTargetPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Arrays"
        && matches!(name, "asList" | "as_list")
    {
        return Some(CallTargetPlan::NativeCollectionStatic {
            method: NativeCollectionStaticMethod::ArraysAsList,
        });
    }
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Collections" {
        let method = match name {
            "emptyList" | "empty_list" => NativeCollectionStaticMethod::CollectionsEmptyList,
            "emptySortedSet" | "empty_sorted_set" => {
                NativeCollectionStaticMethod::CollectionsEmptySortedSet
            }
            "singleton" => NativeCollectionStaticMethod::CollectionsSingleton,
            "singletonList" | "singleton_list" => {
                NativeCollectionStaticMethod::CollectionsSingletonList
            }
            "singletonMap" | "singleton_map" => {
                NativeCollectionStaticMethod::CollectionsSingletonMap
            }
            "nCopies" | "n_copies" => NativeCollectionStaticMethod::CollectionsNCopies,
            _ => return None,
        };
        return Some(CallTargetPlan::NativeCollectionStatic { method });
    }
    None
}

fn plan_native_collection_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    let kind = crate::stdlib::native_collection_receiver_kind_for_type(&receiver_ty)?;
    let method = match (kind, name) {
        (_, "size") => NativeCollectionMethod::Len,
        (_, "isEmpty") => NativeCollectionMethod::IsEmpty,
        (crate::stdlib::JavaCollectionKind::List, "add") => NativeCollectionMethod::ListPush,
        (crate::stdlib::JavaCollectionKind::List, "get") => NativeCollectionMethod::ListGet,
        (crate::stdlib::JavaCollectionKind::Set, "add") => NativeCollectionMethod::SetInsert,
        (
            crate::stdlib::JavaCollectionKind::List | crate::stdlib::JavaCollectionKind::Set,
            "addAll",
        ) => NativeCollectionMethod::Extend,
        (crate::stdlib::JavaCollectionKind::Map, "put") => NativeCollectionMethod::MapInsert,
        (crate::stdlib::JavaCollectionKind::Map, "get") => NativeCollectionMethod::MapGet,
        (crate::stdlib::JavaCollectionKind::Map, "containsKey" | "contains_key") => {
            NativeCollectionMethod::MapContainsKey
        }
        (crate::stdlib::JavaCollectionKind::Map, "keySet" | "key_set") => {
            NativeCollectionMethod::MapKeys
        }
        (crate::stdlib::JavaCollectionKind::Map, "entrySet" | "entry_set") => {
            NativeCollectionMethod::MapIter
        }
        _ => return None,
    };
    Some(CallTargetPlan::NativeCollectionMethod { receiver, method })
}

fn plan_native_java_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_ty = infer_expr_type(receiver, types);
    let effective_receiver_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
    let method = match (&effective_receiver_ty, name) {
        (JavaType::String, "length") => NativeJavaInstanceMethod::StringLen,
        (JavaType::String, "isEmpty") => NativeJavaInstanceMethod::StringIsEmpty,
        (JavaType::String, "trim") => NativeJavaInstanceMethod::StringTrim,
        (JavaType::String, "toLowerCase") => NativeJavaInstanceMethod::StringToLowercase,
        (JavaType::String, "toString") => NativeJavaInstanceMethod::StringToString,
        (JavaType::String, "contains") => NativeJavaInstanceMethod::StringContains,
        (JavaType::String, "startsWith") => NativeJavaInstanceMethod::StringStartsWith,
        (JavaType::String, "endsWith") => NativeJavaInstanceMethod::StringEndsWith,
        (JavaType::String, "split") => NativeJavaInstanceMethod::StringSplit,
        (_, "equals") => NativeJavaInstanceMethod::Equals,
        _ => return None,
    };
    Some(CallTargetPlan::NativeJavaInstance { receiver, method })
}

fn plan_native_java_static_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
) -> Option<CallTargetPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let method = match (receiver_key.as_str(), name) {
        ("Math", "max") => NativeJavaStaticMethod::MathMax,
        ("Math", "toIntExact") => NativeJavaStaticMethod::MathToIntExact,
        ("Integer", "parseInt") => NativeJavaStaticMethod::IntegerParseInt,
        ("Double", "parseDouble") => NativeJavaStaticMethod::DoubleParseDouble,
        ("String", "format") => NativeJavaStaticMethod::StringFormat,
        ("String", "join") => NativeJavaStaticMethod::StringJoin,
        ("Objects", "equals") => NativeJavaStaticMethod::ObjectsEquals,
        _ => return None,
    };
    Some(CallTargetPlan::NativeJavaStatic { method })
}

fn plan_java_class_static_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
) -> Option<CallTargetPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Class"
        && name == "forName"
    {
        return Some(CallTargetPlan::JavaClassStatic {
            rust_name: "for_name".to_string(),
        });
    }
    None
}

fn plan_external_crate_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_name = external_receiver_expr_type_key(receiver, facts, types)?;
    let effect = facts
        .external_api_effect_for_receiver_with_arity(Some(&receiver_name), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(&receiver_name),
                name,
                Some(arg_count),
            )
        })
        .or_else(|| {
            facts
                .external_api_effect_for_receiver(Some(&receiver_name), name)
                .cloned()
        })
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver(Some(&receiver_name), name)
        });
    if effect
        .as_ref()
        .is_some_and(|effect| effect.provenance != FactProvenance::ExternalLibraryPack)
    {
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
        .external_crate_api_mapping_for_receiver(
            &receiver_name,
            name,
            effect.as_ref().and_then(|effect| effect.arg_count),
        )
        .cloned()
        .or_else(|| {
            crate::external_packs::external_crate_api_mapping_for_receiver(
                &receiver_name,
                name,
                effect.as_ref().and_then(|effect| effect.arg_count),
            )
        });
    if effect.is_none() && mapping.is_none() {
        return None;
    }
    Some(CallTargetPlan::ExternalCrateReceiver {
        receiver,
        crate_name: pack.rust_target.clone()?,
        rust_name: mapping
            .map(|mapping| mapping.rust_name)
            .unwrap_or_else(|| external_method_rust_name(&receiver_name, name, arg_count, facts)),
        mutates_receiver: effect.as_ref().is_some_and(|effect| {
            effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
        }),
    })
}

fn plan_external_crate_static_call(
    receiver: &Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'static>> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    let receiver_key = types
        .static_type_alias(receiver_name)
        .map(crate::stdlib::canonical_external_receiver_type_key)
        .unwrap_or_else(|| crate::stdlib::canonical_external_receiver_type_key(receiver_name));
    let effect = facts
        .external_api_effect_for_receiver_with_arity(Some(&receiver_key), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(&receiver_key),
                name,
                Some(arg_count),
            )
        })
        .or_else(|| {
            facts
                .external_api_effect_for_receiver(Some(&receiver_key), name)
                .cloned()
        })
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver(Some(&receiver_key), name)
        })?;
    if effect.provenance != FactProvenance::ExternalLibraryPack {
        return None;
    }
    let pack = facts
        .external_library_pack_for_type(&receiver_key)
        .cloned()
        .or_else(|| crate::external_packs::external_library_pack_for_type(&receiver_key))?;
    if pack.mode != ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let mapping = facts
        .external_crate_api_mapping_for_receiver(&receiver_key, name, effect.arg_count)
        .cloned()
        .or_else(|| {
            crate::external_packs::external_crate_api_mapping_for_receiver(
                &receiver_key,
                name,
                effect.arg_count,
            )
        });
    Some(CallTargetPlan::StaticOwner {
        owner: rust_static_owner_type_name_with_facts(&receiver_key, facts),
        rust_name: mapping
            .map(|mapping| mapping.rust_name)
            .unwrap_or_else(|| external_method_rust_name(&receiver_key, name, arg_count, facts)),
    })
}

pub fn plan_new_target(
    ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    representations: &RepresentationPlan,
) -> NewTargetPlan {
    plan_new_target_with_facts(ty, resolved, representations, &FactSet::default(), 0)
}

pub fn plan_new_target_with_facts(
    ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    representations: &RepresentationPlan,
    facts: &FactSet,
    arg_count: usize,
) -> NewTargetPlan {
    let owner_ty = resolved
        .map(|resolved| resolved.constructed_type.clone())
        .unwrap_or_else(|| ty.clone());
    let rust_name = constructor_rust_name(&owner_ty, resolved, facts, arg_count);
    let external_crate = plan_external_crate_constructor(&owner_ty, &rust_name, facts, arg_count);
    let native_collection = plan_native_collection_constructor(&owner_ty, arg_count);
    NewTargetPlan {
        owner_shape: value_shape_for_java_type(representations, owner_ty),
        rust_name,
        external_crate,
        native_collection,
    }
}

pub fn plan_new_target_for_args_with_facts(
    ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    representations: &RepresentationPlan,
    facts: &FactSet,
    args: &[Expr],
    types: &TypeEnv,
) -> NewTargetPlan {
    let owner_ty = resolved
        .map(|resolved| resolved.constructed_type.clone())
        .unwrap_or_else(|| ty.clone());
    let rust_name = constructor_rust_name(&owner_ty, resolved, facts, args.len());
    let external_crate = plan_external_crate_constructor(&owner_ty, &rust_name, facts, args.len());
    let native_collection = plan_native_collection_constructor_for_args(&owner_ty, args, types)
        .or_else(|| plan_native_collection_constructor(&owner_ty, args.len()));
    NewTargetPlan {
        owner_shape: value_shape_for_java_type(representations, owner_ty),
        rust_name,
        external_crate,
        native_collection,
    }
}

fn constructor_rust_name(
    owner_ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    facts: &FactSet,
    arg_count: usize,
) -> String {
    if let Some(resolved) = resolved {
        return resolved.rust_name.clone();
    }
    let Some(type_name) = java_type_name(owner_ty) else {
        return "new".to_string();
    };
    if external_constructor_needs_arity_name(type_name, facts) {
        format!("new_{arg_count}")
    } else {
        "new".to_string()
    }
}

pub fn unresolved_constructor_rust_name_for_facts(
    owner_ty: &JavaType,
    facts: &FactSet,
    arg_count: usize,
) -> String {
    constructor_rust_name(owner_ty, None, facts, arg_count)
}

pub fn external_method_rust_name(
    receiver_type: &str,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    let base = crate::parser::to_snake_case(method_name);
    if external_method_needs_arity_name(receiver_type, method_name, facts) {
        format!("{base}_{arg_count}")
    } else {
        base
    }
}

fn external_method_rust_name_for_receiver(
    receiver: &Expr,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<String> {
    let receiver_type = external_receiver_expr_type_key(receiver, facts, types)?;
    Some(external_method_rust_name(
        &receiver_type,
        method_name,
        arg_count,
        facts,
    ))
}

fn external_method_needs_arity_name(
    receiver_type: &str,
    method_name: &str,
    facts: &FactSet,
) -> bool {
    let mut arities = external_method_arities_for(facts, receiver_type, method_name);
    arities.extend(external_method_arities_for(
        &crate::external_packs::builtin_external_library_packs(),
        receiver_type,
        method_name,
    ));
    arities.len() > 1
}

fn external_method_arities_for(
    facts: &FactSet,
    receiver_type: &str,
    method_name: &str,
) -> BTreeSet<usize> {
    facts
        .external_api_effect_facts
        .iter()
        .filter_map(|fact| {
            let fact_receiver = fact.receiver_type.as_deref()?;
            if !external_method_receiver_matches(fact_receiver, receiver_type)
                || !external_method_name_matches(&fact.method_name, method_name)
            {
                return None;
            }
            fact.arg_count
        })
        .collect()
}

fn external_method_receiver_matches(fact_receiver: &str, query_receiver: &str) -> bool {
    if fact_receiver == query_receiver {
        return true;
    }
    let fact_key = crate::stdlib::canonical_external_receiver_type_key(fact_receiver);
    let query_key = crate::stdlib::canonical_external_receiver_type_key(query_receiver);
    if fact_key == query_key {
        return true;
    }
    fact_key
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == query_key)
        || query_key
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == fact_key)
}

fn external_method_name_matches(fact_method_name: &str, query_method_name: &str) -> bool {
    fact_method_name == query_method_name
        || crate::parser::to_snake_case(fact_method_name)
            == crate::parser::to_snake_case(query_method_name)
}

fn external_constructor_needs_arity_name(type_name: &str, facts: &FactSet) -> bool {
    let arities = facts
        .external_constructor_facts
        .iter()
        .filter(|fact| external_constructor_type_matches(&fact.type_name, type_name))
        .map(|fact| fact.arg_types.len())
        .collect::<std::collections::BTreeSet<_>>();
    arities.len() > 1
}

fn external_constructor_type_matches(fact_type_name: &str, query_type_name: &str) -> bool {
    if fact_type_name == query_type_name {
        return true;
    }
    let fact_key = crate::stdlib::canonical_external_receiver_type_key(fact_type_name);
    let query_key = crate::stdlib::canonical_external_receiver_type_key(query_type_name);
    if fact_key == query_key {
        return true;
    }
    fact_key
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == query_key)
        || query_key
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == fact_key)
}

fn plan_native_collection_constructor(
    ty: &JavaType,
    arg_count: usize,
) -> Option<NativeCollectionNewTarget> {
    let kind = crate::stdlib::native_collection_constructor_kind_for_type(ty)?;
    match arg_count {
        0 => Some(NativeCollectionNewTarget::Empty { kind }),
        1 => Some(NativeCollectionNewTarget::FromSingleArg { kind }),
        _ => None,
    }
}

fn plan_native_collection_constructor_for_args(
    ty: &JavaType,
    args: &[Expr],
    types: &TypeEnv,
) -> Option<NativeCollectionNewTarget> {
    let kind = crate::stdlib::native_collection_constructor_kind_for_type(ty)?;
    match args {
        [] => Some(NativeCollectionNewTarget::Empty { kind }),
        [arg] if is_integral_capacity_arg(arg, types) => {
            Some(NativeCollectionNewTarget::WithCapacity { kind })
        }
        [arg] if is_collection_source_arg(arg, types) => {
            Some(NativeCollectionNewTarget::FromSource { kind })
        }
        [_] => Some(NativeCollectionNewTarget::FromSingleArg { kind }),
        _ => None,
    }
}

fn is_integral_capacity_arg(arg: &Expr, types: &TypeEnv) -> bool {
    if let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = arg
        && args.is_empty()
        && matches!(
            plan_native_collection_receiver_call(receiver, name, types),
            Some(CallTargetPlan::NativeCollectionMethod {
                method: NativeCollectionMethod::Len,
                ..
            })
        )
    {
        return true;
    }
    match infer_expr_type(arg, types) {
        JavaType::Primitive(
            crate::java_ir::PrimitiveType::Byte
            | crate::java_ir::PrimitiveType::Short
            | crate::java_ir::PrimitiveType::Int
            | crate::java_ir::PrimitiveType::Long,
        ) => true,
        JavaType::Named { name, .. } => {
            matches!(name.as_str(), "Byte" | "Short" | "Integer" | "Long")
        }
        _ => false,
    }
}

fn is_collection_source_arg(arg: &Expr, types: &TypeEnv) -> bool {
    let arg_ty = infer_expr_type(arg, types);
    crate::stdlib::java_collection_kind_for_type(&arg_ty).is_some()
        || crate::types::sequence_element_type(&arg_ty).is_some()
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
            signatures
                .method_arg_shape_by_rust_name(&call.declaring_type, &call.rust_name, index)
                .unwrap_or_else(|| resolved_call_boundary_arg_shape(call, index))
        })
        .collect()
}

fn resolved_call_boundary_arg_shape(call: &ResolvedCall, index: usize) -> RustSlotShape {
    let java_ty = call
        .arg_coercions
        .iter()
        .find(|coercion| coercion.arg_index == index)
        .map(|coercion| coercion.target.clone())
        .unwrap_or_else(|| JavaType::Unknown(format!("{}:arg{index}", call.rust_name)));
    RustSlotShape::for_storage(java_ty)
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

    native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
        .map(ArgumentLoweringPlan::Shaped)
        .or_else(|| {
            external_call_arg_shapes(receiver, name, arg_count, facts, types)
                .map(ArgumentLoweringPlan::Shaped)
        })
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
}

pub fn plan_call_arguments_with_facts_and_args(
    receiver: Option<&Expr>,
    name: &str,
    resolved: Option<&ResolvedCall>,
    args: &[Expr],
    signatures: &SignaturePlan,
    facts: &FactSet,
    types: &TypeEnv,
) -> ArgumentLoweringPlan {
    let arg_count = args.len();
    if let Some(call) = resolved {
        return ArgumentLoweringPlan::Shaped(resolved_call_arg_shapes(call, arg_count, signatures));
    }

    native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
        .map(ArgumentLoweringPlan::Shaped)
        .or_else(|| {
            external_call_arg_shapes(receiver, name, arg_count, facts, types)
                .map(ArgumentLoweringPlan::Shaped)
        })
        .or_else(|| {
            conservative_external_mutator_arg_shapes(receiver, name, args, signatures, facts, types)
                .map(ArgumentLoweringPlan::Shaped)
        })
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
}

pub fn native_collection_arg_shapes(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, &FactSet::default())
}

pub fn native_collection_arg_shapes_with_facts(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver = receiver?;
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    let method = match plan_native_collection_receiver_call(receiver, name, types)? {
        CallTargetPlan::NativeCollectionMethod { method, .. } => method,
        _ => return None,
    };
    let shapes = match method {
        NativeCollectionMethod::ListPush | NativeCollectionMethod::SetInsert if arg_count == 1 => {
            vec![collection_element_arg_shape_with_effective_facts(
                &receiver_ty,
                &effective_facts,
            )?]
        }
        NativeCollectionMethod::Extend if arg_count == 1 => {
            vec![storage_arg_shape_with_effective_facts(
                receiver_ty.clone(),
                &effective_facts,
            )]
        }
        NativeCollectionMethod::MapInsert if arg_count == 2 => {
            let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![
                storage_arg_shape_with_effective_facts(key_ty.clone(), &effective_facts),
                storage_arg_shape_with_effective_facts(value_ty.clone(), &effective_facts),
            ]
        }
        NativeCollectionMethod::ListGet if arg_count == 1 => {
            vec![RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            ))]
        }
        NativeCollectionMethod::MapGet | NativeCollectionMethod::MapContainsKey
            if arg_count == 1 =>
        {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(key_ty.clone())]
        }
        _ => return None,
    };
    Some(shapes)
}

fn collection_element_arg_shape_with_effective_facts(
    receiver_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Option<RustSlotShape> {
    let element_ty = crate::types::sequence_element_type(receiver_ty)?;
    Some(storage_arg_shape_with_effective_facts(element_ty, facts))
}

fn storage_arg_shape_with_effective_facts(
    java_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    if matches!(
        facts.object_representation_override_strategy_for(&java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && !facts.is_final_type(&java_ty)
    {
        apply_required_dispatch(&mut shape, DispatchShape::ErasedObject);
    }
    shape
}

pub fn plan_native_collection_extend_arg(
    receiver_ty: &JavaType,
    arg_shape: &RustSlotShape,
) -> Option<NativeCollectionExtendArgPlan> {
    let receiver_ty = effective_native_collection_receiver_type(receiver_ty);
    if arg_shape.ownership == OwnershipShape::SharedBorrow && arg_shape.view == ViewShape::Slice {
        let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
        return Some(NativeCollectionExtendArgPlan::BorrowedSequenceSource {
            element_shape: RustSlotShape::for_storage(element_ty),
        });
    }

    Some(NativeCollectionExtendArgPlan::OwnedSource {
        slot_shape: RustSlotShape::for_storage(receiver_ty),
    })
}

fn effective_native_collection_receiver_type(receiver_ty: &JavaType) -> JavaType {
    crate::types::optional_element_type(receiver_ty)
        .filter(|element_ty| crate::stdlib::java_collection_kind_for_type(element_ty).is_some())
        .unwrap_or_else(|| receiver_ty.clone())
}

pub fn external_call_arg_shapes(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_name = external_call_argument_receiver_key(receiver, name, types);
    let receiver_ty = receiver.map(|receiver| infer_expr_type(receiver, types));
    (0..arg_count)
        .map(|index| {
            facts
                .external_api_argument_effect_for_receiver(receiver_name.as_deref(), name, index)
                .map(|effect| {
                    argument_shape_from_external_effect(
                        effect,
                        receiver_ty.as_ref(),
                        &effective_facts,
                    )
                })
        })
        .collect()
}

pub fn external_constructor_arg_shapes(
    ty: &JavaType,
    rust_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_name = java_type_name(ty)?;
    let mut saw_fact = false;
    let shapes = (0..arg_count)
        .map(|index| {
            if let Some(effect) =
                facts.external_constructor_argument_effect_for(receiver_name, arg_count, index)
            {
                saw_fact = true;
                argument_shape_from_external_constructor_effect(effect, &effective_facts)
            } else if let Some(effect) = facts.external_api_argument_effect_for_receiver(
                Some(receiver_name),
                rust_name,
                index,
            ) {
                saw_fact = true;
                argument_shape_from_external_effect(effect, Some(ty), &effective_facts)
            } else {
                RustSlotShape::conservative(JavaType::Unknown(format!("{rust_name}:arg{index}")))
            }
        })
        .collect();
    saw_fact.then_some(shapes)
}

fn argument_shape_from_external_constructor_effect(
    effect: &crate::facts::ExternalConstructorArgumentEffectFact,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    argument_shape_from_external_metadata(
        effect.parameter_type.clone(),
        effect.parameter_nullability,
        effect.argument_mutability,
        effect.alias_policy,
        false,
        facts,
    )
}

fn external_call_argument_receiver_key(
    receiver: Option<&Expr>,
    name: &str,
    types: &TypeEnv,
) -> Option<String> {
    receiver
        .and_then(|receiver| static_call_argument_owner_key(receiver, types))
        .or_else(|| {
            receiver.and_then(|receiver| {
                java_type_name(&infer_expr_type(receiver, types)).map(str::to_string)
            })
        })
        .or_else(|| {
            static_member_import_owner_for_call(name, types)
                .map(crate::stdlib::canonical_external_receiver_type_key)
        })
}

fn static_call_argument_owner_key(receiver: &Expr, types: &TypeEnv) -> Option<String> {
    if let Some(owner) = imported_static_owner_for_call(receiver, types) {
        return Some(crate::stdlib::canonical_external_receiver_type_key(owner));
    }
    qualified_static_owner_java_key(receiver)
}

fn imported_static_owner_for_call<'a>(receiver: &Expr, types: &'a TypeEnv) -> Option<&'a str> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    types.static_type_alias(receiver_name)
}

fn static_member_import_owner_for_call<'a>(name: &str, types: &'a TypeEnv) -> Option<&'a str> {
    types
        .static_member_import_owner(name)
        .or_else(|| types.static_member_import_owner(&crate::parser::to_snake_case(name)))
}

fn qualified_static_owner_java_key(receiver: &Expr) -> Option<String> {
    let Expr::FieldAccess(_, _) = receiver else {
        return None;
    };
    let root = static_owner_root_segment(receiver)?;
    if !root
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        return None;
    }
    let owner = static_owner_java_name(receiver)?;
    let class_like_segments = owner
        .split('.')
        .filter(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
        })
        .count();
    (class_like_segments == 1).then_some(owner)
}

fn static_owner_root_segment(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name.as_str()),
        Expr::FieldAccess(receiver, _) => static_owner_root_segment(receiver),
        _ => None,
    }
}

fn static_owner_java_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(receiver, field) => {
            Some(format!("{}.{}", static_owner_java_name(receiver)?, field))
        }
        _ => None,
    }
}

fn conservative_external_mutator_arg_shapes(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    signatures: &SignaturePlan,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    if args.is_empty() {
        return None;
    }

    let receiver_name = receiver
        .and_then(|receiver| java_type_name(&infer_expr_type(receiver, types)).map(str::to_string));
    let role = facts
        .external_api_effect_for_receiver_with_arity(
            receiver_name.as_deref(),
            name,
            Some(args.len()),
        )
        .map(|effect| effect.role)
        .unwrap_or_else(|| {
            crate::stdlib::external_api_method_role(receiver_name.as_deref(), name, args.len())
        });

    if !matches!(
        role,
        ExternalApiRole::Setter | ExternalApiRole::FluentMutator | ExternalApiRole::IndexedSetter
    ) {
        return None;
    }

    let effective_facts = EffectiveFacts::new(facts);
    Some(
        args.iter()
            .map(|arg| {
                storage_arg_shape_with_effective_facts(
                    infer_arg_storage_java_type(arg, signatures, types),
                    &effective_facts,
                )
            })
            .collect(),
    )
}

fn infer_arg_storage_java_type(
    arg: &Expr,
    signatures: &SignaturePlan,
    types: &TypeEnv,
) -> JavaType {
    let inferred = infer_expr_type(arg, types);
    if !matches!(inferred, JavaType::Unknown(_)) {
        return inferred;
    }
    static_field_shape_for_expr(arg, signatures)
        .map(|shape| shape.java_ty.clone())
        .unwrap_or(inferred)
}

fn static_field_shape_for_expr<'a>(
    arg: &Expr,
    signatures: &'a SignaturePlan,
) -> Option<&'a RustSlotShape> {
    let Expr::FieldAccess(receiver, field) = arg else {
        return None;
    };
    let Expr::Var(owner) = receiver.as_ref() else {
        return None;
    };
    let owner_upper = upper_camel_from_snake_or_preserved(owner);
    let field_snake = crate::parser::to_snake_case(field);
    signatures
        .field_shape(owner, field)
        .or_else(|| signatures.field_shape(&owner_upper, field))
        .or_else(|| signatures.field_shape(owner, &field_snake))
        .or_else(|| signatures.field_shape(&owner_upper, &field_snake))
}

fn argument_shape_from_external_effect(
    effect: &ExternalApiArgumentEffectFact,
    receiver_ty: Option<&JavaType>,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    let keep_concrete_unknown = effect.parameter_type.is_none()
        && receiver_ty.is_some_and(|receiver_ty| {
            external_generic_facade_receiver_keeps_argument_concrete(receiver_ty, facts)
        });
    argument_shape_from_external_metadata(
        effect.parameter_type.clone(),
        effect.parameter_nullability,
        effect.argument_mutability,
        effect.alias_policy,
        keep_concrete_unknown,
        facts,
    )
}

fn argument_shape_from_external_metadata(
    parameter_type: Option<JavaType>,
    parameter_nullability: NullabilityState,
    argument_mutability: ReceiverMutabilityFact,
    alias_policy: ExternalArgumentAliasPolicy,
    keep_concrete_unknown: bool,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    let base_java_ty = parameter_type.unwrap_or_else(|| JavaType::Unknown("Object".to_string()));
    let mut java_ty = base_java_ty.clone();
    let parameter_nullability = external_parameter_nullability_policy(parameter_nullability);
    if matches!(
        parameter_nullability,
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    ) {
        java_ty = crate::types::optional_java_type(java_ty);
    }
    let mut shape = RustSlotShape::conservative(java_ty);
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        facts.object_representation_override_strategy_for(&base_java_ty)
    {
        apply_required_dispatch(
            &mut shape,
            DispatchShape::DynTrait {
                trait_name: trait_name.clone(),
            },
        );
    }

    if keep_concrete_unknown {
        if !matches!(shape.java_ty, JavaType::Primitive(_)) {
            shape.ownership = OwnershipShape::Owned;
        }
        return shape;
    }

    match alias_policy {
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

    if argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && !matches!(shape.java_ty, JavaType::Primitive(_))
    {
        shape.ownership = OwnershipShape::MutableBorrow;
        shape.mutability = MutabilityShape::Mutable;
    }

    shape
}

fn external_generic_facade_receiver_keeps_argument_concrete(
    receiver_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    facts.is_final_type(receiver_ty)
        && crate::stdlib::java_collection_kind_for_type(receiver_ty).is_some()
}

pub fn resolved_constructor_arg_shapes(
    constructor: &ResolvedConstructor,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Vec<RustSlotShape> {
    (0..arg_count)
        .map(|index| {
            signatures
                .constructor_arg_shape_by_rust_name(
                    &constructor.declaring_type,
                    &constructor.rust_name,
                    index,
                )
                .unwrap_or_else(|| resolved_constructor_boundary_arg_shape(constructor, index))
        })
        .collect()
}

fn resolved_constructor_boundary_arg_shape(
    constructor: &ResolvedConstructor,
    index: usize,
) -> RustSlotShape {
    constructor
        .params
        .get(index)
        .map(|param| RustSlotShape::for_storage(param.ty.clone()))
        .unwrap_or_else(|| {
            RustSlotShape::for_storage(JavaType::Unknown(format!(
                "{}:arg{index}",
                constructor.rust_name
            )))
        })
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
