use std::collections::BTreeSet;

use crate::coercion::OptionalReceiverProjection;
use crate::conversion_plan::{
    LoweredSlotExprConversionPlan, LoweredType, Slot, argument_slot, index_slot,
    plan_lowered_slot_conversion, receiver_method_autoref_satisfies_optional_slot,
};
use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    AccessorInlinePolicy, CallableKey, ExternalApiArgumentEffectFact, ExternalApiRole,
    ExternalArgumentAliasPolicy, ExternalFactLookupIndex, ExternalLibraryMode, FacadeAbiFact,
    FacadeSlot, FactProvenance, FactSet, HierarchyStrategy, NullabilityState, ReceiverAbi,
    ReceiverMutabilityFact, ResultAbi, Staticness, TrivialAccessorKind, TypePattern,
    external_parameter_nullability_policy,
};
use crate::java_ir::{
    CallKind, Expr, JavaType, LambdaBody, PrimitiveType, ResolvedCall, ResolvedConstructor,
};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    apply_borrowed_sequence_content_projection, apply_borrowed_string_content_projection,
    apply_required_dispatch, borrowed_sequence_element_shape, java_type_is_erased_object_storage,
    rust_slot_shape_from_facade_arg_slot, storage_arg_shape_with_effective_facts,
    value_shape_for_java_type,
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
        receiver_ty: Option<JavaType>,
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
        noop_facade_method_call: bool,
        receiver_type: String,
        method_name: String,
        arg_count: usize,
    },
    SuperBoundary {
        declaring_type: String,
        rust_name: String,
        return_type: JavaType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CallTargetSignatureOwner<'receiver, 'target> {
    CurrentType,
    StaticOwner(&'target str),
    Receiver(&'receiver Expr),
    CallReceiver,
}

impl<'a> CallTargetPlan<'a> {
    pub fn receiver_expr(&self) -> Option<&'a Expr> {
        match self {
            CallTargetPlan::InstanceReceiver { receiver, .. }
            | CallTargetPlan::FallbackReceiver { receiver, .. }
            | CallTargetPlan::ExternalCrateReceiver { receiver, .. }
            | CallTargetPlan::NativeCollectionMethod { receiver, .. }
            | CallTargetPlan::NativeJavaInstance { receiver, .. } => Some(receiver),
            CallTargetPlan::ExternalFacadeReceiver {
                receiver,
                receiver_abi,
                ..
            } if *receiver_abi != ReceiverAbi::AssociatedFunction => Some(receiver),
            CallTargetPlan::StaticSelf { .. }
            | CallTargetPlan::StaticOwner { .. }
            | CallTargetPlan::InstanceSelf { .. }
            | CallTargetPlan::FallbackSelf { .. }
            | CallTargetPlan::FallbackFree { .. }
            | CallTargetPlan::JavaClassStatic { .. }
            | CallTargetPlan::ExternalFacadeReceiver { .. }
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
                | CallTargetPlan::NativeJavaInstance { .. }
                | CallTargetPlan::ExternalFacadeReceiver {
                    receiver_abi: ReceiverAbi::BorrowedSelf
                        | ReceiverAbi::MutBorrowedSelf
                        | ReceiverAbi::OwnedSelf
                        | ReceiverAbi::FreeFunction,
                    ..
                }
        ) && !matches!(
            self,
            CallTargetPlan::NativeJavaInstance {
                method: NativeJavaInstanceMethod::OptionalIsPresent
                    | NativeJavaInstanceMethod::OptionalGet
                    | NativeJavaInstanceMethod::OptionalOrElse,
                ..
            }
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

    pub(crate) fn rust_method_name(&self) -> Option<&str> {
        match self {
            CallTargetPlan::InstanceReceiver { rust_name, .. }
            | CallTargetPlan::InstanceSelf { rust_name }
            | CallTargetPlan::StaticSelf { rust_name } => Some(*rust_name),
            CallTargetPlan::FallbackReceiver { rust_name, .. }
            | CallTargetPlan::FallbackSelf { rust_name }
            | CallTargetPlan::FallbackFree { rust_name }
            | CallTargetPlan::StaticOwner { rust_name, .. }
            | CallTargetPlan::SuperBoundary { rust_name, .. } => Some(rust_name.as_str()),
            CallTargetPlan::ExternalFacadeReceiver { rust_path, .. } => {
                Some(rust_path.rsplit("::").next().unwrap_or(rust_path))
            }
            CallTargetPlan::ExternalCrateReceiver { rust_name, .. } => Some(rust_name.as_str()),
            CallTargetPlan::JavaClassStatic { rust_name } => Some(rust_name.as_str()),
            CallTargetPlan::NativeCollectionMethod { .. }
            | CallTargetPlan::NativeCollectionStatic { .. }
            | CallTargetPlan::NativeJavaInstance { .. }
            | CallTargetPlan::NativeJavaStatic { .. } => None,
        }
    }

    pub(crate) fn is_fallback_receiver_method(&self, method: &str) -> bool {
        matches!(
            self,
            CallTargetPlan::FallbackReceiver { rust_name, .. } if rust_name == method
        )
    }

    pub(crate) fn uses_receiver_argument_lowering(&self) -> bool {
        matches!(
            self,
            CallTargetPlan::FallbackReceiver { .. }
                | CallTargetPlan::ExternalCrateReceiver { .. }
                | CallTargetPlan::NativeCollectionMethod { .. }
                | CallTargetPlan::ExternalFacadeReceiver {
                    receiver_abi: ReceiverAbi::BorrowedSelf
                        | ReceiverAbi::MutBorrowedSelf
                        | ReceiverAbi::OwnedSelf
                        | ReceiverAbi::FreeFunction,
                    ..
                }
        )
    }

    pub(crate) fn signature_owner_source(&self) -> Option<CallTargetSignatureOwner<'a, '_>> {
        match self {
            CallTargetPlan::StaticOwner { owner, .. } if owner == "Self" => {
                Some(CallTargetSignatureOwner::CurrentType)
            }
            CallTargetPlan::StaticOwner { owner, .. } => {
                Some(CallTargetSignatureOwner::StaticOwner(owner.as_str()))
            }
            CallTargetPlan::StaticSelf { .. }
            | CallTargetPlan::FallbackSelf { .. }
            | CallTargetPlan::FallbackFree { .. }
            | CallTargetPlan::InstanceSelf { .. } => Some(CallTargetSignatureOwner::CurrentType),
            CallTargetPlan::InstanceReceiver { receiver, .. }
            | CallTargetPlan::FallbackReceiver { receiver, .. }
            | CallTargetPlan::ExternalCrateReceiver { receiver, .. } => {
                Some(CallTargetSignatureOwner::Receiver(receiver))
            }
            CallTargetPlan::ExternalFacadeReceiver { .. } => {
                Some(CallTargetSignatureOwner::CallReceiver)
            }
            _ => None,
        }
    }

    pub(crate) fn receiver_overload_current_name(&self) -> Option<&str> {
        match self {
            CallTargetPlan::FallbackReceiver { rust_name, .. } => Some(rust_name.as_str()),
            CallTargetPlan::InstanceReceiver { rust_name, .. } => Some(*rust_name),
            _ => None,
        }
    }

    pub(crate) fn into_fallback_receiver_with_rust_name(self, rust_name: String) -> Self {
        match self {
            CallTargetPlan::FallbackReceiver { receiver, .. }
            | CallTargetPlan::InstanceReceiver { receiver, .. } => {
                CallTargetPlan::FallbackReceiver {
                    receiver,
                    rust_name,
                }
            }
            other => other,
        }
    }
}

pub(crate) fn non_self_trivial_accessor_target<'a>(
    receiver: Option<&'a Expr>,
    rust_name: &'a str,
) -> Option<CallTargetPlan<'a>> {
    receiver.map(|receiver| CallTargetPlan::InstanceReceiver {
        receiver,
        rust_name,
    })
}

pub(crate) fn retarget_static_call_to_receiver_if<'a>(
    target: CallTargetPlan<'a>,
    receiver: Option<&'a Expr>,
    should_retarget: bool,
) -> CallTargetPlan<'a> {
    if !should_retarget {
        return target;
    }
    let Some(receiver) = receiver else {
        return target;
    };
    match target {
        CallTargetPlan::StaticOwner { rust_name, .. } => CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name,
        },
        CallTargetPlan::ExternalFacadeReceiver {
            rust_path,
            receiver_abi: ReceiverAbi::AssociatedFunction,
            ..
        } => CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name: rust_path
                .rsplit("::")
                .next()
                .unwrap_or(&rust_path)
                .to_string(),
        },
        other => other,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiverSlotPlan {
    receiver_abi: ReceiverAbi,
    slot: Option<Slot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptionalReceiverSlotPlan {
    receiver_abi: ReceiverAbi,
    slot: Slot,
    projection: OptionalReceiverProjection,
    element_ownership: OwnershipShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptionalReceiverSlotProjectionPlan {
    element_shape: Option<RustSlotShape>,
    conversion: Option<LoweredSlotExprConversionPlan>,
}

impl ReceiverSlotPlan {
    fn new(receiver_abi: ReceiverAbi, slot: Option<Slot>) -> Self {
        Self { receiver_abi, slot }
    }

    #[cfg(test)]
    pub(crate) fn receiver_abi(&self) -> ReceiverAbi {
        self.receiver_abi
    }

    #[cfg(test)]
    pub(crate) fn slot(&self) -> Option<&Slot> {
        self.slot.as_ref()
    }

    pub(crate) fn into_slot(self) -> Option<Slot> {
        self.slot
    }
}

impl OptionalReceiverSlotPlan {
    fn new(
        receiver_abi: ReceiverAbi,
        slot: Slot,
        projection: OptionalReceiverProjection,
        element_ownership: OwnershipShape,
    ) -> Self {
        Self {
            receiver_abi,
            slot,
            projection,
            element_ownership,
        }
    }

    #[cfg(test)]
    pub(crate) fn receiver_abi(&self) -> ReceiverAbi {
        self.receiver_abi
    }

    #[cfg(test)]
    pub(crate) fn slot(&self) -> &Slot {
        &self.slot
    }

    pub(crate) fn projection(&self) -> OptionalReceiverProjection {
        self.projection
    }

    #[cfg(test)]
    pub(crate) fn element_ownership(&self) -> OwnershipShape {
        self.element_ownership
    }

    pub(crate) fn projection_plan_for_option_shape(
        &self,
        option_shape: RustSlotShape,
        facts: &FactSet,
    ) -> OptionalReceiverSlotProjectionPlan {
        let target_shape = match &self.slot {
            Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => shape,
            _ => {
                return OptionalReceiverSlotProjectionPlan::none();
            }
        };
        let Some(actual) = LoweredType::optional_element_borrow_from_option_shape(
            option_shape,
            target_shape.java_ty.clone(),
            Some(self.element_ownership),
            facts,
        ) else {
            return OptionalReceiverSlotProjectionPlan::none();
        };
        let LoweredType::OptionalElementBorrow { element_shape, .. } = &actual else {
            return OptionalReceiverSlotProjectionPlan::none();
        };
        let element_shape = (**element_shape).clone();
        let conversion = plan_lowered_slot_conversion(&actual, self.slot.clone(), facts);
        let conversion = if receiver_method_autoref_satisfies_optional_slot(&conversion) {
            None
        } else {
            Some(conversion)
        };
        OptionalReceiverSlotProjectionPlan {
            element_shape: Some(element_shape),
            conversion,
        }
    }
}

impl OptionalReceiverSlotProjectionPlan {
    fn none() -> Self {
        Self {
            element_shape: None,
            conversion: None,
        }
    }

    pub(crate) fn element_shape(&self) -> Option<&RustSlotShape> {
        self.element_shape.as_ref()
    }

    pub(crate) fn conversion(&self) -> Option<&LoweredSlotExprConversionPlan> {
        self.conversion.as_ref()
    }
}

pub(crate) fn facade_receiver_slot_plan(
    receiver_abi: ReceiverAbi,
    java_ty: JavaType,
    facts: &FactSet,
) -> ReceiverSlotPlan {
    let effective_facts = EffectiveFacts::new(facts);
    let slot = match receiver_abi {
        ReceiverAbi::BorrowedSelf => Some(argument_slot(rust_slot_shape_from_facade_arg_slot(
            &FacadeSlot::Borrowed,
            java_ty,
            &effective_facts,
        ))),
        ReceiverAbi::MutBorrowedSelf => Some(argument_slot(rust_slot_shape_from_facade_arg_slot(
            &FacadeSlot::MutBorrowed,
            java_ty,
            &effective_facts,
        ))),
        ReceiverAbi::OwnedSelf | ReceiverAbi::FreeFunction => Some(argument_slot(
            rust_slot_shape_from_facade_arg_slot(&FacadeSlot::Owned, java_ty, &effective_facts),
        )),
        ReceiverAbi::AssociatedFunction => None,
    };
    ReceiverSlotPlan::new(receiver_abi, slot)
}

pub(crate) fn optional_receiver_slot_plan(
    java_ty: JavaType,
    mutable: bool,
    legacy_unchecked: bool,
    facts: &FactSet,
) -> OptionalReceiverSlotPlan {
    let receiver_abi = if mutable {
        ReceiverAbi::MutBorrowedSelf
    } else {
        ReceiverAbi::BorrowedSelf
    };
    let slot = facade_receiver_slot_plan(receiver_abi, java_ty, facts)
        .into_slot()
        .expect("borrowed receiver ABI has a receiver slot");
    let projection = match (mutable, legacy_unchecked) {
        (false, false) => OptionalReceiverProjection::Shared,
        (true, false) => OptionalReceiverProjection::Mutable,
        (false, true) => OptionalReceiverProjection::SharedUnchecked,
        (true, true) => OptionalReceiverProjection::MutableUnchecked,
    };
    let element_ownership = if mutable {
        OwnershipShape::MutableBorrow
    } else {
        OwnershipShape::SharedBorrow
    };
    OptionalReceiverSlotPlan::new(receiver_abi, slot, projection, element_ownership)
}

pub(crate) fn optional_borrowed_receiver_slot_plan(
    java_ty: JavaType,
    mutable: bool,
    facts: &FactSet,
) -> OptionalReceiverSlotPlan {
    let receiver_abi = if mutable {
        ReceiverAbi::MutBorrowedSelf
    } else {
        ReceiverAbi::BorrowedSelf
    };
    let slot = facade_receiver_slot_plan(receiver_abi, java_ty, facts)
        .into_slot()
        .expect("borrowed receiver ABI has a receiver slot");
    let element_ownership = if mutable {
        OwnershipShape::MutableBorrow
    } else {
        OwnershipShape::SharedBorrow
    };
    OptionalReceiverSlotPlan::new(
        receiver_abi,
        slot,
        OptionalReceiverProjection::Borrowed,
        element_ownership,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCollectionMethod {
    Len,
    IsEmpty,
    ListHashCode,
    ListAdd,
    PriorityQueueAdd,
    ArrayBlockingQueueAdd,
    ArrayBlockingQueueOffer,
    ArrayBlockingQueuePut,
    ListInsert,
    ListGet,
    ListSubList,
    ListIndexOf,
    ListSet,
    ListRemoveIndex,
    ListRemoveObject,
    QueueElement,
    QueueLastElement,
    QueuePeek,
    QueuePoll,
    QueueRemove,
    QueueLastRemove,
    Contains,
    SetHashCode,
    SetInsert,
    SetRemove,
    AddAll,
    RemoveAll,
    ContainsAll,
    RetainAll,
    Extend,
    MapHashCode,
    MapInsert,
    MapPutIfAbsent,
    MapGet,
    MapGetOrDefault,
    MapRemove,
    MapContainsKey,
    MapContainsValue,
    MapKeys,
    MapValues,
    MapIter,
    MapFirstKey,
    MapLastKey,
    MapLowerKey,
    MapFloorKey,
    MapCeilingKey,
    MapHigherKey,
    Iterator,
    ListIterator,
    ToArray,
}

impl NativeCollectionMethod {
    pub(crate) fn mutates_receiver(self) -> bool {
        match self {
            NativeCollectionMethod::ListAdd
            | NativeCollectionMethod::PriorityQueueAdd
            | NativeCollectionMethod::ArrayBlockingQueueAdd
            | NativeCollectionMethod::ArrayBlockingQueueOffer
            | NativeCollectionMethod::ArrayBlockingQueuePut
            | NativeCollectionMethod::ListInsert
            | NativeCollectionMethod::ListSet
            | NativeCollectionMethod::ListRemoveIndex
            | NativeCollectionMethod::ListRemoveObject
            | NativeCollectionMethod::QueuePoll
            | NativeCollectionMethod::QueueRemove
            | NativeCollectionMethod::QueueLastRemove
            | NativeCollectionMethod::SetInsert
            | NativeCollectionMethod::SetRemove
            | NativeCollectionMethod::AddAll
            | NativeCollectionMethod::RemoveAll
            | NativeCollectionMethod::RetainAll
            | NativeCollectionMethod::Extend
            | NativeCollectionMethod::MapInsert
            | NativeCollectionMethod::MapPutIfAbsent
            | NativeCollectionMethod::MapRemove => true,
            NativeCollectionMethod::Len
            | NativeCollectionMethod::IsEmpty
            | NativeCollectionMethod::ListHashCode
            | NativeCollectionMethod::ListGet
            | NativeCollectionMethod::ListSubList
            | NativeCollectionMethod::ListIndexOf
            | NativeCollectionMethod::QueueElement
            | NativeCollectionMethod::QueueLastElement
            | NativeCollectionMethod::QueuePeek
            | NativeCollectionMethod::Contains
            | NativeCollectionMethod::ContainsAll
            | NativeCollectionMethod::SetHashCode
            | NativeCollectionMethod::MapHashCode
            | NativeCollectionMethod::MapGet
            | NativeCollectionMethod::MapGetOrDefault
            | NativeCollectionMethod::MapContainsKey
            | NativeCollectionMethod::MapContainsValue
            | NativeCollectionMethod::MapKeys
            | NativeCollectionMethod::MapValues
            | NativeCollectionMethod::MapIter
            | NativeCollectionMethod::MapFirstKey
            | NativeCollectionMethod::MapLastKey
            | NativeCollectionMethod::MapLowerKey
            | NativeCollectionMethod::MapFloorKey
            | NativeCollectionMethod::MapCeilingKey
            | NativeCollectionMethod::MapHigherKey
            | NativeCollectionMethod::Iterator
            | NativeCollectionMethod::ListIterator
            | NativeCollectionMethod::ToArray => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NativeCollectionExtendArgPlan {
    OwnedSource { slot_shape: RustSlotShape },
    BorrowedSequenceSource { element_shape: RustSlotShape },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCollectionStaticMethod {
    ArraysAsList,
    ArraysEquals,
    ArraysDeepEquals,
    ArraysBinarySearch,
    ArraysCompare,
    ArraysMismatch,
    ArraysHashCode,
    ArraysDeepHashCode,
    ArraysCopyOf,
    ArraysCopyOfRange,
    ArraysToString,
    ArraysDeepToString,
    CollectionsEmptyList,
    CollectionsEmptySet,
    CollectionsEmptyMap,
    CollectionsEmptySortedSet,
    CollectionsUnmodifiableCollection,
    CollectionsUnmodifiableList,
    CollectionsUnmodifiableSet,
    CollectionsUnmodifiableMap,
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
    StringToUppercase,
    StringToString,
    ObjectToString,
    SourceObjectToString,
    SourceHashCode,
    StringContains,
    StringStartsWith,
    StringEndsWith,
    BitSetGet,
    FileExists,
    ReaderReady,
    BufferPut,
    StringReplace,
    StringReplaceFirst,
    StringReplaceAll,
    StringMatches,
    StringSplit,
    StringSubstring,
    StringToCharArray,
    StringHashCode,
    StringGetBytes,
    StringGetBytesWithCharset,
    StringIndexOf,
    StringLastIndexOf,
    StringCharAt,
    StringCompareTo,
    StringCompareToIgnoreCase,
    StringEqualsIgnoreCase,
    OptionalIsPresent,
    OptionalGet,
    OptionalOrElse,
    PrimitiveNumberValue {
        source: Option<PrimitiveType>,
        target: PrimitiveType,
    },
    PrimitiveHashCode {
        source: PrimitiveType,
    },
    PrimitiveCompareTo {
        source: PrimitiveType,
    },
    DecimalFormatFormat,
    MessageFormatFormat,
    Equals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeJavaStaticMethod {
    MathMax {
        primitive: PrimitiveType,
    },
    MathMin {
        primitive: PrimitiveType,
    },
    MathLog,
    MathLog10,
    MathExp,
    MathSqrt,
    MathPow,
    MathCeil,
    MathFloor,
    MathUnaryF64 {
        java_name: &'static str,
        rust_method: &'static str,
    },
    MathBinaryF64 {
        java_name: &'static str,
        rust_method: &'static str,
    },
    MathRound {
        source: PrimitiveType,
    },
    MathAbs {
        source: PrimitiveType,
    },
    MathToIntExact,
    MathFloorDiv {
        primitive: PrimitiveType,
    },
    MathFloorMod {
        primitive: PrimitiveType,
        return_primitive: PrimitiveType,
    },
    ByteParseByte,
    ShortParseShort,
    IntegerParseInt,
    IntegerDecode,
    IntegerValueOfString,
    LongParseLong,
    FloatParseFloat,
    DoubleParseDouble,
    FloatingIsNaN,
    CharacterPredicate {
        java_name: &'static str,
        rust_method: &'static str,
    },
    CharacterTransform {
        java_name: &'static str,
        rust_method: &'static str,
    },
    SystemGc,
    SystemExit,
    ThreadSleep,
    StringFormat,
    StringJoin,
    StringValueOf,
    OptionalEmpty,
    OptionalOf,
    OptionalOfNullable,
    ObjectsEquals,
    ObjectsToString,
    ObjectsNonNull,
    ObjectsIsNull,
    PatternCompile,
}

pub fn native_collection_lowered_type(
    receiver: &Expr,
    method: NativeCollectionMethod,
    types: &TypeEnv,
) -> Option<LoweredType> {
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    match method {
        NativeCollectionMethod::Len => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
        NativeCollectionMethod::IsEmpty => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Boolean,
        ))),
        NativeCollectionMethod::ListGet => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(element_ty.clone()),
            ))
        }
        NativeCollectionMethod::ListSubList => Some(LoweredType::from_java(receiver_ty.clone())),
        NativeCollectionMethod::ListIndexOf => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
        NativeCollectionMethod::ListAdd
        | NativeCollectionMethod::PriorityQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueOffer => Some(LoweredType::from_java(
            JavaType::Primitive(PrimitiveType::Boolean),
        )),
        NativeCollectionMethod::ArrayBlockingQueuePut => {
            Some(LoweredType::from_java(JavaType::Void))
        }
        NativeCollectionMethod::AddAll
        | NativeCollectionMethod::RemoveAll
        | NativeCollectionMethod::ContainsAll
        | NativeCollectionMethod::RetainAll => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Boolean,
        ))),
        NativeCollectionMethod::ListSet => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::ListRemoveIndex => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::QueueRemove => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::QueueLastRemove => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::QueueElement => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::QueueLastElement => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(element_ty.clone()))
        }
        NativeCollectionMethod::QueuePeek => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(element_ty.clone()),
            ))
        }
        NativeCollectionMethod::QueuePoll => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                element_ty.clone(),
            )))
        }
        NativeCollectionMethod::ListRemoveObject => Some(LoweredType::from_java(
            JavaType::Primitive(PrimitiveType::Boolean),
        )),
        NativeCollectionMethod::Contains => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Boolean,
        ))),
        NativeCollectionMethod::MapGet => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::optional_element_borrow_shape(
                borrowed_sequence_element_shape(value_ty.clone()),
            ))
        }
        NativeCollectionMethod::MapGetOrDefault => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(value_ty.clone()))
        }
        NativeCollectionMethod::MapFirstKey | NativeCollectionMethod::MapLastKey => {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(key_ty.clone()))
        }
        NativeCollectionMethod::MapLowerKey
        | NativeCollectionMethod::MapFloorKey
        | NativeCollectionMethod::MapCeilingKey
        | NativeCollectionMethod::MapHigherKey => {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                key_ty.clone(),
            )))
        }
        NativeCollectionMethod::MapValues => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(JavaType::Array(Box::new(
                value_ty.clone(),
            ))))
        }
        NativeCollectionMethod::MapInsert => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                value_ty.clone(),
            )))
        }
        NativeCollectionMethod::MapPutIfAbsent => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                value_ty.clone(),
            )))
        }
        NativeCollectionMethod::MapRemove => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(LoweredType::from_java(crate::types::optional_java_type(
                value_ty.clone(),
            )))
        }
        NativeCollectionMethod::MapContainsValue => Some(LoweredType::from_java(
            JavaType::Primitive(PrimitiveType::Boolean),
        )),
        NativeCollectionMethod::SetRemove => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Boolean,
        ))),
        NativeCollectionMethod::ListHashCode
        | NativeCollectionMethod::SetHashCode
        | NativeCollectionMethod::MapHashCode => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
        NativeCollectionMethod::ToArray => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(JavaType::Array(Box::new(
                element_ty.clone(),
            ))))
        }
        NativeCollectionMethod::Iterator => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(JavaType::Named {
                name: "Iterator".to_string(),
                args: vec![element_ty.clone()],
            }))
        }
        NativeCollectionMethod::ListIterator => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            Some(LoweredType::from_java(JavaType::Named {
                name: "ListIterator".to_string(),
                args: vec![element_ty.clone()],
            }))
        }
        _ => None,
    }
}

pub fn native_java_call_lowered_type(target: &CallTargetPlan<'_>) -> Option<LoweredType> {
    match target {
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::IntegerParseInt
                | NativeJavaStaticMethod::MathRound {
                    source: PrimitiveType::Float,
                }
                | NativeJavaStaticMethod::MathToIntExact
                | NativeJavaStaticMethod::MathFloorDiv {
                    primitive: PrimitiveType::Int,
                }
                | NativeJavaStaticMethod::MathFloorMod {
                    return_primitive: PrimitiveType::Int,
                    ..
                },
        }
        | CallTargetPlan::NativeJavaInstance {
            method:
                NativeJavaInstanceMethod::StringLen
                | NativeJavaInstanceMethod::SourceHashCode
                | NativeJavaInstanceMethod::StringIndexOf
                | NativeJavaInstanceMethod::StringLastIndexOf
                | NativeJavaInstanceMethod::StringHashCode
                | NativeJavaInstanceMethod::PrimitiveHashCode { .. },
            ..
        }
        | CallTargetPlan::NativeCollectionStatic {
            method: NativeCollectionStaticMethod::ArraysHashCode,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::MathRound { .. },
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Long,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::MathFloorDiv {
                    primitive: PrimitiveType::Long,
                },
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Long,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::MathFloorMod {
                    return_primitive: PrimitiveType::Long,
                    ..
                },
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Long,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::MathMax { primitive }
                | NativeJavaStaticMethod::MathMin { primitive }
                | NativeJavaStaticMethod::MathAbs { source: primitive },
        } => Some(LoweredType::from_java(JavaType::Primitive(*primitive))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::FloatParseFloat,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Float,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::ByteParseByte,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Byte,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::ShortParseShort,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Short,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::LongParseLong,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Long,
        ))),
        // `Integer.parseInt` and `Float.parseFloat` were both here and
        // `Double.parseDouble` was not, behind a `_ => None` that made the
        // omission invisible — `parse<T>` returns `T` is the rule, and only
        // some instances were listed.
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::DoubleParseDouble,
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Double,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::MathLog
                | NativeJavaStaticMethod::MathLog10
                | NativeJavaStaticMethod::MathExp
                | NativeJavaStaticMethod::MathSqrt
                | NativeJavaStaticMethod::MathPow
                | NativeJavaStaticMethod::MathCeil
                | NativeJavaStaticMethod::MathFloor
                | NativeJavaStaticMethod::MathUnaryF64 { .. }
                | NativeJavaStaticMethod::MathBinaryF64 { .. },
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Double,
        ))),
        CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::FloatingIsNaN
                | NativeJavaStaticMethod::CharacterPredicate { .. }
                | NativeJavaStaticMethod::ObjectsEquals
                | NativeJavaStaticMethod::ObjectsNonNull
                | NativeJavaStaticMethod::ObjectsIsNull,
        }
        | CallTargetPlan::NativeJavaInstance {
            method:
                NativeJavaInstanceMethod::StringIsEmpty
                | NativeJavaInstanceMethod::StringContains
                | NativeJavaInstanceMethod::StringStartsWith
                | NativeJavaInstanceMethod::StringEndsWith
                | NativeJavaInstanceMethod::StringMatches
                | NativeJavaInstanceMethod::StringEqualsIgnoreCase
                | NativeJavaInstanceMethod::Equals
                | NativeJavaInstanceMethod::BitSetGet
                | NativeJavaInstanceMethod::FileExists
                | NativeJavaInstanceMethod::ReaderReady
                | NativeJavaInstanceMethod::OptionalIsPresent,
            ..
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Boolean,
        ))),
        CallTargetPlan::NativeJavaInstance {
            method:
                NativeJavaInstanceMethod::StringTrim
                | NativeJavaInstanceMethod::StringToLowercase
                | NativeJavaInstanceMethod::StringToUppercase
                | NativeJavaInstanceMethod::StringToString
                | NativeJavaInstanceMethod::ObjectToString
                | NativeJavaInstanceMethod::SourceObjectToString
                | NativeJavaInstanceMethod::StringReplace
                | NativeJavaInstanceMethod::StringReplaceFirst
                | NativeJavaInstanceMethod::StringReplaceAll
                | NativeJavaInstanceMethod::DecimalFormatFormat
                | NativeJavaInstanceMethod::MessageFormatFormat,
            ..
        }
        | CallTargetPlan::NativeJavaStatic {
            method:
                NativeJavaStaticMethod::StringFormat
                | NativeJavaStaticMethod::StringJoin
                | NativeJavaStaticMethod::StringValueOf
                | NativeJavaStaticMethod::ObjectsToString,
        } => Some(LoweredType::Slot(Box::new(
            crate::shape_projection::owned_string_value_shape(),
        ))),
        CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::PatternCompile,
        } => Some(LoweredType::from_java(JavaType::Named {
            name: "Pattern".to_string(),
            args: Vec::new(),
        })),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringSplit,
            ..
        } => Some(LoweredType::Slot(Box::new(
            crate::shape_projection::string_list_value_shape(),
        ))),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringToCharArray,
            ..
        } => Some(LoweredType::from_java(JavaType::Array(Box::new(
            JavaType::Primitive(PrimitiveType::Char),
        )))),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::StringCharAt,
            ..
        }
        | CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::CharacterTransform { .. },
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Char,
        ))),
        CallTargetPlan::NativeJavaInstance {
            method:
                NativeJavaInstanceMethod::StringCompareTo
                | NativeJavaInstanceMethod::StringCompareToIgnoreCase,
            ..
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
        CallTargetPlan::NativeJavaInstance {
            method: NativeJavaInstanceMethod::PrimitiveCompareTo { .. },
            ..
        } => Some(LoweredType::from_java(JavaType::Primitive(
            PrimitiveType::Int,
        ))),
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
    let external_index = facts.external_lookup_index();
    external_call_lowered_type_with_external_index(
        receiver,
        name,
        arg_count,
        facts,
        &external_index,
        types,
        current_type,
    )
}

pub(crate) fn external_receiver_overload_is_modeled(
    external_index: &ExternalFactLookupIndex<'_>,
    receiver_key: &str,
    name: &str,
    arg_count: usize,
) -> bool {
    external_index
        .external_api_effect_for_receiver_with_arity(Some(receiver_key), name, Some(arg_count))
        .is_some()
        || (!external_index.facts().disable_builtin_external_packs
            && crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(receiver_key),
                name,
                Some(arg_count),
            )
            .is_some())
}

pub(crate) fn external_constructor_is_modeled(
    facts: &FactSet,
    type_name: &str,
    arg_count: usize,
) -> bool {
    let arg_types = vec![JavaType::Unknown("_".to_string()); arg_count];
    facts
        .external_constructor_for(type_name, &arg_types)
        .is_some()
        || (!facts.disable_builtin_external_packs
            && crate::external_packs::external_constructor_for(type_name, &arg_types).is_some())
}

pub(crate) fn external_call_lowered_type_with_external_index(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<LoweredType> {
    let receiver_ty = receiver.map(|receiver| infer_expr_type(receiver, types));
    let receiver_key = external_call_receiver_type_key_with_index(
        receiver,
        name,
        arg_count,
        facts,
        external_index,
        types,
        current_type,
    )?;
    let Some(effect) = external_index
        .external_api_effect_for_receiver_with_arity(receiver_key.as_deref(), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                receiver_key.as_deref(),
                name,
                Some(arg_count),
            )
        })
    else {
        if receiver_key
            .as_deref()
            .is_some_and(external_receiver_key_is_object_erasure)
            && let Some(lowered) =
                erased_object_facade_method_return_lowered_type(name, arg_count, facts)
        {
            return Some(lowered);
        }
        let snake_name = crate::parser::to_snake_case(name);
        let (method_name, return_ty) =
            [name, snake_name.as_str()]
                .into_iter()
                .find_map(|method_name| {
                    crate::stdlib::known_external_method_return_type(
                        receiver_key.as_deref(),
                        method_name,
                        arg_count,
                    )
                    .map(|return_ty| (method_name, return_ty))
                })?;
        let return_ty = crate::types::receiver_generic_return_type(receiver_ty.as_ref(), return_ty);
        return Some(known_external_call_lowered_type(
            receiver_key.as_deref(),
            method_name,
            arg_count,
            return_ty,
        ));
    };
    let rust_method = receiver_key
        .as_deref()
        .map(|receiver_key| external_method_rust_name(receiver_key, name, arg_count, facts))
        .unwrap_or_else(|| crate::parser::to_snake_case(name));
    let return_ty = crate::types::receiver_generic_return_type(
        receiver_ty.as_ref(),
        effect.return_type.clone().or_else(|| {
            crate::stdlib::inferred_external_method_return_type(
                receiver_key.as_deref(),
                name,
                &rust_method,
                arg_count,
            )
            .filter(|return_type| {
                crate::stdlib::inferred_external_effect_return_type_can_replace_opaque(
                    name,
                    return_type,
                )
            })
        })?,
    );
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
    let rust_return_is_shared_borrow =
        rust_return_type.is_some_and(|fact| fact.return_slot.contains_shared_borrow());

    if rust_return_is_shared_borrow
        && matches!(
            effect.return_nullability,
            NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
        )
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Some(LoweredType::optional_rust_borrowed_return(
            return_ty,
            rust_return_type.and_then(|fact| fact.borrowed_return_recovery.map(Into::into)),
        ));
    }
    if rust_return_type.is_some_and(|fact| optional_owned_facade_slot(&fact.return_slot))
        && matches!(
            effect.return_nullability,
            NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
        )
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Some(LoweredType::optional_rust_owned_return(
            return_ty,
            rust_return_type
                .map(|fact| fact.rust_type_name.clone())
                .unwrap_or_default(),
        ));
    }

    let returns_optional = matches!(
        effect.return_nullability,
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
    );
    if !returns_optional
        && external_return_type_is_typed_erased_facade(&return_ty, facts)
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Some(LoweredType::rust_owned_return(
            return_ty,
            rust_return_type
                .map(|fact| fact.rust_type_name.clone())
                .unwrap_or_default(),
        ));
    }
    if java_type_is_erased_object_storage(&return_ty) {
        let semantic_ty = if returns_optional {
            crate::types::optional_java_type(return_ty)
        } else {
            return_ty
        };
        return Some(LoweredType::Slot(Box::new(
            crate::shape_projection::project_java_value_shape(None, semantic_ty),
        )));
    }
    if java_type_is_byte_array(&return_ty)
        && rust_return_type
            .is_none_or(|fact| rust_return_type_is_unsigned_byte_array(&fact.rust_type_name))
        && receiver_key.as_deref().is_some_and(|receiver_key| {
            external_index
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
        if rust_return_type.return_slot.contains_shared_borrow()
            && !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
        {
            return Some(LoweredType::rust_borrowed_return(
                java_ty.clone(),
                rust_return_type.rust_type_name.clone(),
                rust_return_type.borrowed_return_recovery.map(Into::into),
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

fn optional_owned_facade_slot(slot: &FacadeSlot) -> bool {
    matches!(slot, FacadeSlot::Optional(inner) if matches!(inner.as_ref(), FacadeSlot::Owned))
}

pub(crate) fn external_call_lowered_type_from_static_owner_plan_with_external_index(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<LoweredType> {
    let plan = plan_call_expr_with_external_index(
        receiver,
        name,
        args,
        resolved,
        facts,
        external_index,
        types,
    );
    let CallExprPlan::Target(CallTargetPlan::StaticOwner { owner, .. }) = plan else {
        return None;
    };
    static_owner_result_type_candidates(receiver, &owner, types)
        .into_iter()
        .find_map(|owner| {
            let arg_types = args
                .iter()
                .map(|arg| infer_expr_type(arg, types))
                .collect::<Vec<_>>();
            external_index
                .external_api_effect_for_call(Some(&owner), name, &arg_types)
                .cloned()
                .or_else(|| {
                    crate::external_packs::external_api_effect_for_call(
                        Some(&owner),
                        name,
                        &arg_types,
                    )
                })
                .and_then(external_effect_lowered_type)
                .or_else(|| {
                    let receiver_ty = JavaType::Named {
                        name: owner.clone(),
                        args: Vec::new(),
                    };
                    let arg_types = args
                        .iter()
                        .map(|arg| infer_expr_type(arg, types))
                        .collect::<Vec<_>>();
                    crate::stdlib::known_external_method_return_type_for_call(
                        Some(&receiver_ty),
                        name,
                        &arg_types,
                    )
                    .or_else(|| {
                        crate::stdlib::known_external_method_return_type(
                            Some(&owner),
                            name,
                            args.len(),
                        )
                    })
                    .map(|return_ty| {
                        known_external_call_lowered_type(Some(&owner), name, args.len(), return_ty)
                    })
                })
        })
}

pub(crate) fn external_receiver_call_return_type_with_external_index(
    receiver: &Expr,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<JavaType> {
    let Expr::Call {
        receiver: call_receiver,
        name,
        args,
        resolved,
        ..
    } = receiver
    else {
        return None;
    };
    let resolved = resolved.as_ref()?;
    let arity = Some(args.len());
    for method_name in [name.as_str(), resolved.rust_name.as_str()] {
        if let Some(return_type) = external_index
            .external_api_effect_for_receiver_with_arity(
                Some(&resolved.declaring_type),
                method_name,
                arity,
            )
            .cloned()
            .or_else(|| {
                crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(&resolved.declaring_type),
                    method_name,
                    arity,
                )
            })
            .and_then(|effect| effect.return_type)
        {
            let receiver_type = call_receiver
                .as_deref()
                .map(|receiver| infer_expr_type(receiver, types));
            return Some(crate::types::receiver_generic_return_type(
                receiver_type.as_ref(),
                return_type,
            ));
        }
    }
    None
}

pub(crate) fn resource_close_declares_result_with_external_index(
    resource_ty: &JavaType,
    external_index: &ExternalFactLookupIndex<'_>,
) -> bool {
    resource_type_names_for_close_lookup(resource_ty)
        .into_iter()
        .any(|receiver_type| {
            external_index
                .external_api_effect_for_receiver_with_arity(Some(receiver_type), "close", Some(0))
                .is_some_and(|effect| !effect.thrown.is_empty())
                || crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(receiver_type),
                    "close",
                    Some(0),
                )
                .is_some_and(|effect| !effect.thrown.is_empty())
        })
}

fn external_effect_lowered_type(
    effect: crate::facts::ExternalApiEffectFact,
) -> Option<LoweredType> {
    let return_ty = effect.return_type?;
    match effect.return_nullability {
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
            if !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            Some(LoweredType::from_java(crate::types::optional_java_type(
                return_ty,
            )))
        }
        _ => Some(LoweredType::from_java(return_ty)),
    }
}

fn static_owner_result_type_candidates(
    receiver: Option<&Expr>,
    planned_owner: &str,
    types: &TypeEnv,
) -> Vec<String> {
    let mut owners = Vec::new();
    if let Some(Expr::Var(receiver_name)) = receiver {
        if let Some(alias) = types.static_type_alias(receiver_name) {
            owners.push(alias.to_string());
            owners.push(crate::stdlib::canonical_external_receiver_type_key(alias));
        }
        owners.push(receiver_name.clone());
        owners.push(crate::stdlib::canonical_external_receiver_type_key(
            receiver_name,
        ));
    }
    owners.push(planned_owner.to_string());
    owners.push(crate::stdlib::canonical_external_receiver_type_key(
        planned_owner,
    ));
    owners.dedup();
    owners
}

fn resource_type_names_for_close_lookup(ty: &JavaType) -> Vec<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            vec![name.as_str(), name.rsplit('.').next().unwrap_or(name)]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn known_external_call_lowered_type(
    receiver_key: Option<&str>,
    name: &str,
    arg_count: usize,
    return_ty: JavaType,
) -> LoweredType {
    match crate::stdlib::external_api_return_nullability(
        receiver_key,
        name,
        arg_count,
        ExternalApiRole::Getter,
    ) {
        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
            if !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            LoweredType::from_java(crate::types::optional_java_type(return_ty))
        }
        _ => LoweredType::from_java(return_ty),
    }
}

fn facade_abi_external_call_lowered_type(
    receiver_key: Option<&str>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    return_ty: &JavaType,
) -> Option<LoweredType> {
    let receiver_key = receiver_key?;
    let abi = facts.facade_abi_for_external_method(receiver_key, name, arg_count)?;
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
        return Some(LoweredType::optional_rust_borrowed_return(
            return_ty.clone(),
            abi.borrowed_return_recovery.map(Into::into),
        ));
    }
    if returns_optional
        && matches!(unwrapped_slot, FacadeSlot::Owned)
        && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Some(LoweredType::optional_rust_owned_return(
            return_ty.clone(),
            abi.rust_path.clone(),
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
                abi.borrowed_return_recovery.map(Into::into),
            ))
        }
        FacadeSlot::Owned
        | FacadeSlot::Index
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

fn rust_return_type_is_unsigned_byte_array(rust_type_name: &str) -> bool {
    let compact = rust_type_name.replace(' ', "");
    compact == "Vec<u8>" || compact == "Option<Vec<u8>>"
}

fn external_return_type_is_typed_erased_facade(java_type: &JavaType, facts: &FactSet) -> bool {
    let name = match java_type {
        JavaType::Named { name, args } if args.is_empty() => name.as_str(),
        JavaType::Unknown(name) => name.as_str(),
        _ => return false,
    };
    if matches!(name, "Object" | "java.lang.Object") {
        return false;
    }
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let exact = JavaType::Named {
        name: canonical.clone(),
        args: Vec::new(),
    };
    let effective_facts = EffectiveFacts::new(facts);
    if matches!(
        effective_facts.object_representation_override_strategy_for(&exact),
        Some(HierarchyStrategy::ErasedObject)
    ) {
        return true;
    }
    let suffix = format!(".{canonical}");
    facts.hierarchy_overrides.iter().any(|override_| {
        if override_.strategy != HierarchyStrategy::ErasedObject {
            return false;
        }
        let candidate = crate::stdlib::canonical_external_receiver_type_key(&override_.type_name);
        if candidate == canonical || !candidate.ends_with(&suffix) {
            return false;
        }
        let candidate_ty = JavaType::Named {
            name: candidate,
            args: Vec::new(),
        };
        matches!(
            effective_facts.object_representation_override_strategy_for(&candidate_ty),
            Some(HierarchyStrategy::ErasedObject)
        )
    })
}

pub(crate) fn erased_object_facade_method_return_lowered_type(
    name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> Option<LoweredType> {
    let rust_name = crate::parser::to_snake_case(name);
    let mut unique: Option<(JavaType, bool)> = None;
    for effect in facts
        .external_api_effect_facts
        .iter()
        .filter(|effect| effect.arg_count.is_none_or(|count| count == arg_count))
        .filter(|effect| crate::parser::to_snake_case(&effect.method_name) == rust_name)
    {
        let Some(return_ty) = effect.return_type.clone() else {
            continue;
        };
        if !external_return_type_is_typed_erased_facade(&return_ty, facts) {
            continue;
        }
        let returns_optional = matches!(
            effect.return_nullability,
            NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
        );
        let candidate = (return_ty, returns_optional);
        match &unique {
            None => unique = Some(candidate),
            Some(existing) if existing == &candidate => {}
            Some(_) => return None,
        }
    }
    match unique {
        Some((return_ty, true)) => Some(LoweredType::optional_rust_owned_return(
            return_ty.clone(),
            String::new(),
        )),
        Some((return_ty, false)) => Some(LoweredType::rust_owned_return(
            return_ty.clone(),
            String::new(),
        )),
        None => None,
    }
}

fn external_receiver_key_is_object_erasure(receiver_key: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(receiver_key).as_str(),
        "Object" | "java.lang.Object"
    )
}

fn java_type_is_byte_array(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Array(element)
            if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
    )
}

#[allow(dead_code)]
pub(crate) fn external_call_returns_rust_result(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> bool {
    let external_index = facts.external_lookup_index();
    external_call_returns_rust_result_with_external_index(
        receiver,
        name,
        arg_count,
        facts,
        &external_index,
        types,
        current_type,
    )
}

pub(crate) fn external_call_returns_rust_result_with_external_index(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> bool {
    let Some(receiver_key) = external_call_receiver_type_key_with_index(
        receiver,
        name,
        arg_count,
        facts,
        external_index,
        types,
        current_type,
    ) else {
        return false;
    };
    if let Some(receiver_key) = receiver_key.as_deref() {
        if let Some(abi) = facts.facade_abi_for_external_method(receiver_key, name, arg_count) {
            return matches!(abi.result, ResultAbi::ResultWrapped);
        }
    }
    if let Some(fallibility) =
        facts.external_rust_result_for_receiver(receiver_key.as_deref(), name, Some(arg_count))
    {
        return fallibility.returns_result;
    }
    external_index
        .external_api_effect_for_receiver_with_arity(receiver_key.as_deref(), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            (!facts.disable_builtin_external_packs).then(|| {
                crate::external_packs::external_api_effect_for_receiver_with_arity(
                    receiver_key.as_deref(),
                    name,
                    Some(arg_count),
                )
            })?
        })
        .is_some_and(|effect| facts.thrown_types_include_checked_exception(&effect.thrown))
}

pub(crate) fn external_call_returns_rust_result_for_receiver(
    facts: &FactSet,
    receiver_key: &str,
    name: &str,
    arg_count: usize,
) -> bool {
    if let Some(abi) = facts.facade_abi_for_external_method(receiver_key, name, arg_count) {
        return matches!(abi.result, ResultAbi::ResultWrapped);
    }
    if let Some(fallibility) =
        facts.external_rust_result_for_receiver(Some(receiver_key), name, Some(arg_count))
    {
        return fallibility.returns_result;
    }
    external_call_has_thrown_effect_for_receiver(facts, receiver_key, name, arg_count)
}

pub(crate) fn external_call_has_thrown_effect_for_receiver(
    facts: &FactSet,
    receiver_name: &str,
    name: &str,
    arg_count: usize,
) -> bool {
    let external_index = facts.external_lookup_index();
    external_index
        .external_api_effect_for_receiver_with_arity(Some(receiver_name), name, Some(arg_count))
        .cloned()
        .or_else(|| {
            (!facts.disable_builtin_external_packs).then(|| {
                crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(receiver_name),
                    name,
                    Some(arg_count),
                )
            })?
        })
        .is_some_and(|effect| facts.thrown_types_include_checked_exception(&effect.thrown))
}

pub(crate) fn external_constructor_returns_rust_result(
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
    owner_shape: RustSlotShape,
    rust_name: String,
    external_crate: Option<ExternalCrateNewTarget>,
    native_collection: Option<NativeCollectionNewTarget>,
    native_string: Option<NativeStringNewTarget>,
}

impl NewTargetPlan {
    pub fn new(
        owner_shape: RustSlotShape,
        rust_name: String,
        external_crate: Option<ExternalCrateNewTarget>,
        native_collection: Option<NativeCollectionNewTarget>,
        native_string: Option<NativeStringNewTarget>,
    ) -> Self {
        Self {
            owner_shape,
            rust_name,
            external_crate,
            native_collection,
            native_string,
        }
    }

    pub fn owner_shape(&self) -> &RustSlotShape {
        &self.owner_shape
    }

    pub fn rust_name(&self) -> &str {
        &self.rust_name
    }

    pub fn external_crate(&self) -> Option<&ExternalCrateNewTarget> {
        self.external_crate.as_ref()
    }

    pub fn native_collection(&self) -> Option<NativeCollectionNewTarget> {
        self.native_collection
    }

    pub fn native_string(&self) -> Option<NativeStringNewTarget> {
        self.native_string
    }

    pub(crate) fn rename_constructor(&mut self, rust_name: String) {
        self.rust_name = rust_name;
    }

    pub(crate) fn set_native_collection(
        &mut self,
        native_collection: Option<NativeCollectionNewTarget>,
    ) {
        self.native_collection = native_collection;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCrateNewTarget {
    crate_name: String,
    type_name: String,
    rust_name: String,
}

impl ExternalCrateNewTarget {
    pub fn new(crate_name: String, type_name: String, rust_name: String) -> Self {
        Self {
            crate_name,
            type_name,
            rust_name,
        }
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn rust_name(&self) -> &str {
        &self.rust_name
    }
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
    PriorityQueueComparatorUnsupported {
        kind: crate::stdlib::JavaCollectionKind,
    },
    PriorityQueueComparatorCallback {
        kind: crate::stdlib::JavaCollectionKind,
    },
    PriorityQueueComparator {
        kind: crate::stdlib::JavaCollectionKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeStringNewTarget {
    FromString,
    FromBytes,
    FromByteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentLoweringPlan {
    Shaped(Vec<RustSlotShape>),
    Slotted(Vec<Slot>),
    Unshaped { arg_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TrivialAccessorCallPlan<'a> {
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
        param_source_names: Vec<String>,
        body: &'a LambdaBody,
    },
    NativeArraysSort {
        target: &'a Expr,
        from: Option<&'a Expr>,
        to: Option<&'a Expr>,
    },
    NativeCollectionsSort {
        target: &'a Expr,
        comparator: Option<&'a Expr>,
    },
    NativeArraysFill {
        target: &'a Expr,
        from: Option<&'a Expr>,
        to: Option<&'a Expr>,
        value: &'a Expr,
    },
    NativeSystemPrintStream {
        stream: NativeSystemStream,
        method: NativePrintStreamMethod,
        arg: Option<&'a Expr>,
    },
    NativeJavaExceptionPrintStackTrace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallExceptionPropagationPlan {
    PropagateResult,
    NoPropagation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSystemStream {
    Out,
    Err,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePrintStreamMethod {
    Print,
    Println,
}

pub(crate) fn plan_trivial_accessor_call<'a>(
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
                field_name: field.field_name().to_string(),
            })
        }
        TrivialAccessorKind::Setter { field, .. } if args.len() == 1 => {
            Some(TrivialAccessorCallPlan::Setter {
                receiver,
                field_name: field.field_name().to_string(),
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
    let external_index = facts.external_lookup_index();
    plan_call_exception_propagation_with_external_index(
        receiver,
        name,
        args,
        resolved,
        facts,
        &external_index,
        types,
        current_type,
    )
}

/// Whether a call propagates an exception, for a caller that has **no solved
/// fallibility plan**.
///
/// The source-method probes below re-derive fallibility from the fact set. That
/// is the third decision point
/// [0010](../claude_audits/0010-fallibility-is-decided-three-times-and-disagrees.md)
/// is named for, and it is why lowering must not call this: lowering has a
/// solved plan and reaches
/// [`plan_external_call_exception_propagation`] instead, so a probe cannot
/// contradict the plan. Measured over 100 corpus files, the probes fired **0**
/// times once the plan was consulted first; splitting the function makes that
/// unreachability structural rather than incidental.
pub(crate) fn plan_call_exception_propagation_with_external_index(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
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

    plan_external_call_exception_propagation(
        receiver,
        name,
        args,
        facts,
        external_index,
        types,
        current_type,
    )
}

/// Whether an **external** call propagates a checked exception.
///
/// Contains no source-method probe, so a caller that has already asked the
/// solved fallibility plan about source callables cannot have that answer
/// second-guessed here. This is the entry point lowering uses.
pub(crate) fn plan_external_call_exception_propagation(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> CallExceptionPropagationPlan {
    if external_call_returns_rust_result_with_external_index(
        receiver,
        name,
        args.len(),
        facts,
        external_index,
        types,
        current_type,
    ) {
        return CallExceptionPropagationPlan::PropagateResult;
    }
    if let Some(effect) = external_call_effect_with_external_index(
        receiver,
        name,
        args,
        facts,
        external_index,
        types,
        current_type,
    ) && facts.thrown_types_include_checked_exception(&effect.thrown)
    {
        return CallExceptionPropagationPlan::PropagateResult;
    }

    CallExceptionPropagationPlan::NoPropagation
}

fn receiver_source_type_name<'a>(receiver: &'a Expr, types: &'a TypeEnv) -> Option<&'a str> {
    match receiver {
        Expr::Var(name) => {
            if let Some(local_type) = types.local_type(name) {
                return java_type_name(local_type);
            }
            if types.field_type(name).is_some() {
                return None;
            }
            types.static_type_alias(name).or(Some(name.as_str()))
        }
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
    let external_index = facts.external_lookup_index();
    plan_call_expr_with_external_index(
        receiver,
        name,
        args,
        resolved,
        facts,
        &external_index,
        types,
    )
}

pub fn plan_call_expr_with_external_index<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    args: &'a [Expr],
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
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

    let target = plan_call_target_with_external_index_and_args(
        receiver,
        name,
        args,
        resolved,
        facts,
        external_index,
        types,
    );
    CallExprPlan::Target(retarget_call_target_with_facade_abi_args(
        target,
        receiver,
        name,
        args,
        facts,
        external_index,
        types,
    ))
}

fn retarget_call_target_with_facade_abi_args<'a>(
    target: CallTargetPlan<'a>,
    receiver: Option<&'a Expr>,
    name: &str,
    args: &[Expr],
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    if !matches!(
        target,
        CallTargetPlan::FallbackReceiver { .. } | CallTargetPlan::ExternalFacadeReceiver { .. }
    ) {
        return target;
    }
    let Some(receiver) = receiver else {
        return target;
    };
    let Some(abi) =
        facade_abi_for_receiver_call(receiver, name, args, facts, external_index, types)
    else {
        return target;
    };
    CallTargetPlan::ExternalFacadeReceiver {
        receiver,
        rust_path: abi.rust_path.clone(),
        receiver_abi: abi.receiver,
        mutates_receiver: matches!(abi.receiver, ReceiverAbi::MutBorrowedSelf),
        noop_facade_method_call: crate::stdlib::external_facade_receiver_call_is_noop_method(
            facts,
            abi.key.owner_type(),
            abi.key.member_name(),
            abi.key.arity(),
        ),
        receiver_type: abi.key.owner_type().to_string(),
        method_name: abi.key.member_name().to_string(),
        arg_count: abi.key.arity(),
    }
}

pub(crate) fn apply_fallback_receiver_rust_name<'a>(
    plan: CallExprPlan<'a>,
    fallback_rust_name: Option<String>,
) -> CallExprPlan<'a> {
    match (plan, fallback_rust_name) {
        (
            CallExprPlan::Target(CallTargetPlan::FallbackReceiver {
                receiver,
                rust_name: planned_name,
            }),
            Some(rust_name),
        ) if !method_name_has_arity_suffix(&planned_name) => {
            CallExprPlan::Target(CallTargetPlan::FallbackReceiver {
                receiver,
                rust_name,
            })
        }
        (plan, _) => plan,
    }
}

fn method_name_has_arity_suffix(name: &str) -> bool {
    name.rsplit_once('_').is_some_and(|(_, suffix)| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

pub(crate) fn apply_fallback_self_rust_name<'a>(
    plan: CallExprPlan<'a>,
    fallback_rust_name: Option<String>,
) -> CallExprPlan<'a> {
    match (plan, fallback_rust_name) {
        (CallExprPlan::Target(CallTargetPlan::InstanceSelf { .. }), Some(rust_name)) => {
            CallExprPlan::Target(CallTargetPlan::FallbackSelf { rust_name })
        }
        (CallExprPlan::Target(CallTargetPlan::StaticSelf { .. }), Some(rust_name)) => {
            CallExprPlan::Target(CallTargetPlan::StaticOwner {
                owner: "Self".to_string(),
                rust_name,
            })
        }
        (CallExprPlan::Target(CallTargetPlan::FallbackFree { .. }), Some(rust_name)) => {
            CallExprPlan::Target(CallTargetPlan::FallbackSelf { rust_name })
        }
        (plan, _) => plan,
    }
}

pub(crate) fn apply_fallback_static_owner_rust_name<'a>(
    plan: CallExprPlan<'a>,
    fallback_rust_name: Option<String>,
) -> CallExprPlan<'a> {
    match (plan, fallback_rust_name) {
        (
            CallExprPlan::Target(CallTargetPlan::StaticOwner {
                owner,
                rust_name: current,
            }),
            Some(rust_name),
        ) if rust_name == current
            || rust_name
                .strip_prefix(&current)
                .is_some_and(|suffix| suffix.starts_with('_')) =>
        {
            CallExprPlan::Target(CallTargetPlan::StaticOwner { owner, rust_name })
        }
        (
            CallExprPlan::Target(CallTargetPlan::StaticOwner {
                owner,
                rust_name: current,
            }),
            Some(rust_name),
        ) if current.rsplit_once('_').is_some_and(|(base, suffix)| {
            !base.is_empty()
                && !suffix.is_empty()
                && suffix.bytes().all(|byte| byte.is_ascii_digit())
                && rust_name
                    .strip_prefix(base)
                    .is_some_and(|suffix| suffix.starts_with('_'))
        }) =>
        {
            CallExprPlan::Target(CallTargetPlan::StaticOwner { owner, rust_name })
        }
        (plan, _) => plan,
    }
}

fn external_call_effect_with_external_index<'a>(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    facts: &'a FactSet,
    external_index: &ExternalFactLookupIndex<'a>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<&'a crate::facts::ExternalApiEffectFact> {
    let receiver_key = external_call_receiver_type_key_with_index(
        receiver,
        name,
        args.len(),
        facts,
        external_index,
        types,
        current_type,
    )?;
    external_index.external_api_effect_for_receiver_with_arity(
        receiver_key.as_deref(),
        name,
        Some(args.len()),
    )
}

pub(crate) fn external_call_receiver_type_key(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<Option<String>> {
    let external_index = facts.external_lookup_index();
    external_call_receiver_type_key_with_index(
        receiver,
        name,
        arg_count,
        facts,
        &external_index,
        types,
        current_type,
    )
}

fn external_call_receiver_type_key_with_index(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<Option<String>> {
    match receiver {
        None if crate::stdlib::is_known_receiverless_static_utility_method(name, arg_count) => {
            Some(None)
        }
        None => current_type.map(|type_name| Some(type_name.to_string())),
        Some(Expr::This | Expr::Super) => current_type.map(|type_name| Some(type_name.to_string())),
        Some(receiver) => imported_static_owner_for_call(receiver, types)
            .map(crate::stdlib::canonical_external_receiver_type_key)
            .or_else(|| {
                external_receiver_expr_type_key_with_index(
                    receiver,
                    name,
                    facts,
                    external_index,
                    types,
                    current_type,
                )
            })
            .map(Some),
    }
}

fn external_receiver_expr_type_key_with_index(
    receiver: &Expr,
    method_name: &str,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
    current_type: Option<&str>,
) -> Option<String> {
    if let Expr::Call {
        receiver: inner_receiver,
        name,
        args,
        ..
    } = receiver
        && let Some(lowered) = external_call_lowered_type_with_external_index(
            inner_receiver.as_deref(),
            name,
            args.len(),
            facts,
            external_index,
            types,
            current_type,
        )
        && let Some(java_ty) = lowered.java_ty()
        && let Some(key) =
            crate::stdlib::external_api_method_receiver_owner_key(java_ty, types, method_name)
    {
        return Some(key);
    }
    let inferred = infer_expr_type(receiver, types);
    if weak_unknown_field_access_receiver(receiver, &inferred) {
        return infer_external_field_access_type_with_index(receiver, facts, external_index, types)
            .and_then(|external_field_ty| {
                crate::stdlib::external_api_method_receiver_owner_key(
                    &external_field_ty,
                    types,
                    method_name,
                )
            });
    }
    crate::stdlib::external_api_method_receiver_owner_key(&inferred, types, method_name)
}

fn infer_external_field_access_type_with_index(
    expr: &Expr,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<JavaType> {
    let Expr::FieldAccess(receiver, field) = expr else {
        return None;
    };
    let receiver_ty =
        infer_external_field_access_type_with_index(receiver, facts, external_index, types)
            .unwrap_or_else(|| infer_expr_type(receiver, types));
    external_pack_or_fact_field_type_with_index(&receiver_ty, field, external_index)
}

fn external_pack_or_fact_field_type_with_index(
    receiver_ty: &JavaType,
    field: &str,
    external_index: &ExternalFactLookupIndex<'_>,
) -> Option<JavaType> {
    let receiver_name = crate::stdlib::external_named_receiver_owner_key(receiver_ty)?;
    if let Some(field_fact) = external_index.external_field_for(&receiver_name, field)
        && let Some(field_type) = &field_fact.field_type
    {
        return Some(field_type.clone());
    }
    if !external_index.facts().disable_builtin_external_packs
        && let Some(field_fact) = crate::external_packs::external_field_for(&receiver_name, field)
        && let Some(field_type) = field_fact.field_type
    {
        return Some(field_type);
    }
    crate::stdlib::known_external_field_type_for_receiver(receiver_ty, field)
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

pub fn plan_call_statement<'a>(
    expr: &'a Expr,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallStatementPlan<'a>> {
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
    if let Some(plan) = plan_native_arrays_statement(receiver.as_deref(), name, args) {
        return Some(plan);
    }
    if let Some(plan) = plan_native_collections_statement(receiver.as_deref(), name, args) {
        return Some(plan);
    }
    if let Some(plan) = plan_native_system_print_stream_statement(
        receiver.as_deref(),
        name,
        args,
        resolved.as_ref(),
        types,
    ) {
        return Some(plan);
    }
    if plan_java_exception_print_stack_trace_statement(receiver.as_deref(), name, args).is_some() {
        return Some(CallStatementPlan::NativeJavaExceptionPrintStackTrace);
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

fn plan_java_exception_print_stack_trace_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<()> {
    receiver?;
    if !matches!(name, "printStackTrace" | "print_stack_trace") || !matches!(args.len(), 0 | 1) {
        return None;
    }
    Some(())
}

fn plan_native_system_print_stream_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
    resolved: Option<&ResolvedCall>,
    types: &TypeEnv,
) -> Option<CallStatementPlan<'a>> {
    if !resolved_call_can_use_native_system_print_stream(resolved) {
        return None;
    }
    if source_system_print_stream_field_shadows_native_receiver(receiver?, types) {
        return None;
    }
    let stream = system_print_stream_receiver(receiver?)?;
    let method = match (name, args.len()) {
        ("print", 1) => NativePrintStreamMethod::Print,
        ("println", 0 | 1) => NativePrintStreamMethod::Println,
        _ => return None,
    };
    Some(CallStatementPlan::NativeSystemPrintStream {
        stream,
        method,
        arg: args.first(),
    })
}

fn resolved_call_can_use_native_system_print_stream(resolved: Option<&ResolvedCall>) -> bool {
    let Some(resolved) = resolved else {
        return true;
    };
    resolved.kind == CallKind::Instance
        && crate::stdlib::canonical_external_receiver_type_key(&resolved.declaring_type)
            == "PrintStream"
}

fn source_system_print_stream_field_shadows_native_receiver(
    receiver: &Expr,
    types: &TypeEnv,
) -> bool {
    let Expr::FieldAccess(owner, field) = receiver else {
        return false;
    };
    let Expr::Var(owner_name) = owner.as_ref() else {
        return false;
    };
    if crate::stdlib::canonical_external_receiver_type_key(owner_name) != "System" {
        return false;
    }
    matches!(field.as_str(), "out" | "err" | "OUT" | "ERR")
        && (types.type_field_type(owner_name, field).is_some()
            || types
                .static_type_alias(owner_name)
                .and_then(|alias| types.type_field_type(alias, field))
                .is_some())
}

fn system_print_stream_receiver(receiver: &Expr) -> Option<NativeSystemStream> {
    let Expr::FieldAccess(owner, field) = receiver else {
        return None;
    };
    let Expr::Var(owner_name) = owner.as_ref() else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(owner_name) != "System" {
        return None;
    }
    match field.as_str() {
        "out" | "OUT" => Some(NativeSystemStream::Out),
        "err" | "ERR" => Some(NativeSystemStream::Err),
        _ => None,
    }
}

fn plan_native_arrays_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<CallStatementPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) != "Arrays" {
        return None;
    }
    match (name, args) {
        ("sort" | "parallelSort" | "parallel_sort", [target]) => {
            Some(CallStatementPlan::NativeArraysSort {
                target,
                from: None,
                to: None,
            })
        }
        ("sort" | "parallelSort" | "parallel_sort", [target, from, to]) => {
            Some(CallStatementPlan::NativeArraysSort {
                target,
                from: Some(from),
                to: Some(to),
            })
        }
        ("fill", [target, value]) => Some(CallStatementPlan::NativeArraysFill {
            target,
            from: None,
            to: None,
            value,
        }),
        ("fill", [target, from, to, value]) => Some(CallStatementPlan::NativeArraysFill {
            target,
            from: Some(from),
            to: Some(to),
            value,
        }),
        _ => None,
    }
}

fn plan_native_collections_statement<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    args: &'a [Expr],
) -> Option<CallStatementPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(receiver_name) != "Collections" {
        return None;
    }
    match (name, args) {
        ("sort", [target]) => Some(CallStatementPlan::NativeCollectionsSort {
            target,
            comparator: None,
        }),
        ("sort", [target, comparator]) => Some(CallStatementPlan::NativeCollectionsSort {
            target,
            comparator: Some(comparator),
        }),
        _ => None,
    }
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
    let param_source_names = params
        .iter()
        .map(|param| param.source_name.clone())
        .collect::<Vec<_>>();
    if param_names.is_empty() {
        return None;
    }
    Some(CallStatementPlan::IterableForEach {
        receiver,
        param_names,
        param_source_names,
        body,
    })
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
    let external_index = facts.external_lookup_index();
    plan_call_target_with_external_index_and_arity(
        receiver,
        name,
        arg_count,
        resolved,
        facts,
        &external_index,
        types,
    )
}

pub fn plan_call_target_with_external_index_and_arity<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    arg_count: usize,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    plan_call_target_with_external_index_impl(
        receiver,
        name,
        arg_count,
        None,
        resolved,
        facts,
        external_index,
        types,
    )
}

pub fn plan_call_target_with_external_index_and_args<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    args: &'a [Expr],
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    plan_call_target_with_external_index_impl(
        receiver,
        name,
        args.len(),
        Some(args),
        resolved,
        facts,
        external_index,
        types,
    )
}

fn plan_call_target_with_external_index_impl<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    arg_count: usize,
    args: Option<&'a [Expr]>,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> CallTargetPlan<'a> {
    if matches!(receiver, Some(Expr::Super))
        && let Some(call) = resolved
        && matches!(call.kind, CallKind::Instance)
    {
        return CallTargetPlan::SuperBoundary {
            declaring_type: call.declaring_type.clone(),
            rust_name: call.rust_name.clone(),
            return_type: call.return_type.clone(),
        };
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_java_logging_facade_call(receiver, name)
    {
        return target;
    }

    if let Some(target) = plan_native_collection_static_call(receiver, name, resolved, types) {
        return target;
    }

    if let Some(target) =
        plan_native_java_static_call(receiver, name, arg_count, args, facts, types, resolved)
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_external_crate_static_call(receiver, name, arg_count, facts, external_index, types)
    {
        return target;
    }

    if receiver.is_none()
        && let Some(owner) = static_member_import_owner_for_call(name, types)
        && let Some(target) = plan_native_java_static_owner_call(owner, name, args, types)
    {
        return target;
    }

    if let Some(call) = resolved
        && let (CallKind::Instance, Some(receiver)) = (&call.kind, receiver)
        && let Some(external_rust_name) =
            resolved_external_instance_call_rust_name(call, name, arg_count, facts)
    {
        return CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name: external_rust_name,
        };
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_native_collection_receiver_call(receiver, name, args, types)
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) = static_utility_owner_call_target(
            receiver,
            name,
            arg_count,
            args,
            resolved,
            facts,
            external_index,
            types,
        )
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_external_facade_receiver_call(
            receiver,
            name,
            arg_count,
            facts,
            external_index,
            types,
        )
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_external_crate_receiver_call(
            receiver,
            name,
            arg_count,
            facts,
            external_index,
            types,
        )
    {
        return target;
    }

    if let Some(call) = resolved
        && let (CallKind::Instance, None) = (&call.kind, receiver)
        && let Some(external_rust_name) =
            resolved_external_instance_call_rust_name(call, name, arg_count, facts)
    {
        return CallTargetPlan::FallbackSelf {
            rust_name: external_rust_name,
        };
    }

    if name != "equals"
        && let Some(receiver) = receiver
        && let Some(target) = plan_native_java_receiver_call(receiver, name, arg_count, types)
    {
        return target;
    }

    if name != "equals"
        && let Some(receiver) = receiver
        && let Some(target) =
            plan_resolved_native_java_receiver_call(receiver, name, arg_count, resolved)
    {
        return target;
    }

    if let Some(call) = resolved
        && let (CallKind::Instance, Some(receiver)) = (&call.kind, receiver)
        && let Some(target) = plan_resolved_java_object_to_string_fallback(
            receiver, name, arg_count, call, facts, types,
        )
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) = plan_native_java_receiver_call(receiver, name, arg_count, types)
    {
        return target;
    }

    if let Some(target) = plan_java_class_static_call(receiver, name) {
        return target;
    }

    if let Some(target) = plan_resolved_static_call(receiver, name, arg_count, resolved, facts) {
        return target;
    }

    if let Some(call) = resolved
        && let (CallKind::Instance, Some(receiver)) = (&call.kind, receiver)
        && let Some(target) = plan_resolved_java_source_hash_code_fallback(
            receiver, name, arg_count, call, facts, types,
        )
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

    if let Some(call) = resolved {
        if let Some(external_rust_name) =
            resolved_external_instance_call_rust_name(call, name, arg_count, facts)
        {
            return match (&call.kind, receiver) {
                (CallKind::Instance, Some(receiver)) => CallTargetPlan::FallbackReceiver {
                    receiver,
                    rust_name: external_rust_name,
                },
                (CallKind::Instance, None) => CallTargetPlan::FallbackSelf {
                    rust_name: external_rust_name,
                },
                (CallKind::Static, Some(_)) | (CallKind::Static, None) => unreachable!(),
            };
        }
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

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_java_object_to_string_fallback(receiver, name, arg_count, facts, types)
    {
        return target;
    }

    if let Some(receiver) = receiver
        && let Some(target) =
            plan_java_source_hash_code_fallback(receiver, name, arg_count, facts, types)
    {
        return target;
    }

    if let Some(receiver) = receiver {
        if let Some(owner) =
            qualified_static_owner_expr_type_name(receiver, name, arg_count, types, facts)
        {
            return CallTargetPlan::StaticOwner {
                owner,
                rust_name: crate::parser::to_snake_case(name),
            };
        }
        CallTargetPlan::FallbackReceiver {
            receiver,
            rust_name: external_method_rust_name_for_receiver(
                receiver,
                name,
                arg_count,
                facts,
                external_index,
                types,
            )
            .unwrap_or_else(|| crate::parser::to_snake_case(name)),
        }
    } else {
        if let Some(owner) = types.static_member_import_owner(name) {
            return CallTargetPlan::StaticOwner {
                owner: rust_static_owner_type_name_with_facts(owner, facts),
                rust_name: external_method_rust_name(owner, name, arg_count, facts),
            };
        }
        let rust_name = crate::parser::to_snake_case(name);
        if let Some(owner) = types.static_member_import_owner(&rust_name) {
            return CallTargetPlan::StaticOwner {
                owner: rust_static_owner_type_name_with_facts(owner, facts),
                rust_name: external_method_rust_name(owner, name, arg_count, facts),
            };
        }
        CallTargetPlan::FallbackFree {
            rust_name: crate::parser::to_snake_case(name),
        }
    }
}

fn resolved_external_instance_call_rust_name(
    call: &ResolvedCall,
    java_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> Option<String> {
    if call.kind != CallKind::Instance {
        return None;
    }
    let external_name =
        external_method_rust_name(&call.declaring_type, java_name, arg_count, facts);
    if external_name != call.rust_name
        && crate::stdlib::canonical_external_receiver_type_key(&call.declaring_type)
            == "PrintStream"
        && matches!(
            external_method_base_rust_name(java_name).as_str(),
            "print" | "println"
        )
    {
        return Some(external_name);
    }
    let effect = facts
        .external_lookup_index()
        .external_api_effect_for_receiver_with_arity(
            Some(&call.declaring_type),
            java_name,
            Some(arg_count),
        )
        .cloned()
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver_with_arity(
                Some(&call.declaring_type),
                java_name,
                Some(arg_count),
            )
        })
        .or_else(|| {
            crate::external_packs::java_platform_api_effect_for_receiver_with_arity(
                &call.declaring_type,
                java_name,
                Some(arg_count),
            )
        })?;
    if !matches!(
        effect.provenance,
        FactProvenance::BuiltinExternalModel
            | FactProvenance::ExternalLibraryPack
            | FactProvenance::UserOverride
    ) {
        return None;
    }
    (external_name != call.rust_name).then_some(external_name)
}

fn plan_java_object_to_string_fallback<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    if matches!(name, "toString" | "to_string")
        && arg_count == 0
        && receiver_declared_source_object_type(receiver, facts, types).is_some()
    {
        Some(CallTargetPlan::NativeJavaInstance {
            receiver,
            method: NativeJavaInstanceMethod::SourceObjectToString,
        })
    } else {
        None
    }
}

fn plan_java_source_hash_code_fallback<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    if matches!(name, "hashCode" | "hash_code")
        && arg_count == 0
        && receiver_declared_source_hash_code_type(receiver, facts, types).is_some()
    {
        Some(CallTargetPlan::NativeJavaInstance {
            receiver,
            method: NativeJavaInstanceMethod::SourceHashCode,
        })
    } else {
        None
    }
}

fn plan_resolved_java_object_to_string_fallback<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    call: &ResolvedCall,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    if !matches!(name, "toString" | "to_string")
        || arg_count != 0
        || call.rust_name != "to_string"
        || call.return_type != JavaType::String
    {
        return None;
    }
    let receiver_name = receiver_declared_source_object_type(receiver, facts, types)?;
    if facts.method_effect_for(receiver_name, "toString").is_some()
        || facts
            .method_effect_for(receiver_name, "to_string")
            .is_some()
    {
        return None;
    }
    Some(CallTargetPlan::NativeJavaInstance {
        receiver,
        method: NativeJavaInstanceMethod::SourceObjectToString,
    })
}

fn plan_resolved_java_source_hash_code_fallback<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    call: &ResolvedCall,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    if !matches!(name, "hashCode" | "hash_code")
        || arg_count != 0
        || call.rust_name != "hash_code"
        || call.return_type != JavaType::Primitive(PrimitiveType::Int)
    {
        return None;
    }
    receiver_declared_source_hash_code_type(receiver, facts, types)?;
    Some(CallTargetPlan::NativeJavaInstance {
        receiver,
        method: NativeJavaInstanceMethod::SourceHashCode,
    })
}

fn receiver_declared_source_object_type<'a>(
    receiver: &'a Expr,
    facts: &FactSet,
    types: &'a TypeEnv,
) -> Option<&'a str> {
    let receiver_ty = match receiver {
        Expr::Var(name) => types.local_type(name).or_else(|| types.field_type(name))?,
        Expr::FieldAccess(_, field) => types.field_type(field)?,
        Expr::This => return None,
        _ => return None,
    };
    let receiver_name = java_type_name(receiver_ty)?;
    if !java_object_to_string_source_fallback_receiver(receiver_name, facts) {
        return None;
    }
    Some(receiver_name)
}

fn receiver_declared_source_hash_code_type<'a>(
    receiver: &'a Expr,
    facts: &FactSet,
    types: &'a TypeEnv,
) -> Option<&'a str> {
    let receiver_ty = match receiver {
        Expr::Var(name) => types.local_type(name).or_else(|| types.field_type(name))?,
        Expr::FieldAccess(_, field) => types.field_type(field)?,
        Expr::This => return None,
        _ => return None,
    };
    let receiver_name = java_type_name(receiver_ty)?;
    if !java_source_hash_code_fallback_receiver(receiver_name, facts) {
        return None;
    }
    Some(receiver_name)
}

fn java_object_to_string_source_fallback_receiver(receiver_name: &str, facts: &FactSet) -> bool {
    java_source_object_fallback_receiver(receiver_name, facts, "toString")
}

fn java_source_hash_code_fallback_receiver(receiver_name: &str, facts: &FactSet) -> bool {
    java_source_object_fallback_receiver(receiver_name, facts, "hashCode")
}

fn java_source_object_fallback_receiver(
    receiver_name: &str,
    facts: &FactSet,
    method_name: &str,
) -> bool {
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(&receiver_key);
    if matches!(
        receiver_simple,
        "String"
            | "Object"
            | "StringBuilder"
            | "StringBuffer"
            | "CharSequence"
            | "Appendable"
            | "Class"
            | "Path"
            | "File"
            | "URI"
    ) {
        return false;
    }
    if crate::stdlib::java_collection_kind_for_type(&JavaType::Named {
        name: receiver_key.clone(),
        args: Vec::new(),
    })
    .is_some()
    {
        return false;
    }
    if facts
        .external_library_pack_for_type(&receiver_key)
        .is_some()
        || crate::external_packs::external_library_pack_for_type(&receiver_key).is_some()
        || facts
            .external_crate_rust_type_name_for(&receiver_key)
            .is_some()
    {
        return false;
    }
    facts
        .facade_abi_for_external_method(&receiver_key, method_name, 0)
        .is_none()
}

fn plan_resolved_static_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    arg_count: usize,
    resolved: Option<&'a ResolvedCall>,
    facts: &FactSet,
) -> Option<CallTargetPlan<'a>> {
    let call = resolved?;
    if call.kind != CallKind::Static {
        return None;
    }
    Some(match receiver {
        Some(_) => CallTargetPlan::StaticOwner {
            owner: resolved_static_owner_type_name(&call.declaring_type, name, arg_count, facts),
            rust_name: call.rust_name.clone(),
        },
        None => CallTargetPlan::StaticSelf {
            rust_name: &call.rust_name,
        },
    })
}

fn resolved_static_owner_type_name(
    owner: &str,
    method: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    if let Some(alias) = crate::javalibs::preferred_static_owner_alias(owner, method, arg_count) {
        return rust_static_owner_type_name_with_facts(alias, facts);
    }
    let rendered = rust_static_owner_type_name_with_facts(owner, facts);
    if owner.contains('.') || rendered != owner {
        return rendered;
    }
    upper_camel_from_snake_or_preserved(owner)
}

fn plan_external_facade_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    if plan_native_java_receiver_call(receiver, name, arg_count, types).is_some() {
        return None;
    }
    if let Expr::Var(receiver_name) = receiver
        && types.local_type(receiver_name).is_none()
        && types.field_type(receiver_name).is_none()
    {
        if types.static_type_alias(receiver_name).is_some() {
            return None;
        }
        let owner = types
            .static_type_alias(receiver_name)
            .map(crate::stdlib::canonical_external_receiver_type_key)
            .unwrap_or_else(|| crate::stdlib::canonical_external_receiver_type_key(receiver_name));
        if crate::stdlib::external_api_method_role(Some(&owner), name, arg_count)
            != ExternalApiRole::Unknown
        {
            return None;
        }
    }
    let receiver_name = external_receiver_expr_type_key_with_index(
        receiver,
        name,
        facts,
        external_index,
        types,
        None,
    )?;
    if matches!(name, "toString" | "to_string")
        && arg_count == 0
        && platform_type_has_direct_object_to_string(&receiver_name)
    {
        return Some(CallTargetPlan::NativeJavaInstance {
            receiver,
            method: NativeJavaInstanceMethod::ObjectToString,
        });
    }
    let abi = facts.facade_abi_for_external_method(&receiver_name, name, arg_count)?;
    Some(CallTargetPlan::ExternalFacadeReceiver {
        receiver,
        rust_path: abi.rust_path.clone(),
        receiver_abi: abi.receiver,
        mutates_receiver: matches!(abi.receiver, ReceiverAbi::MutBorrowedSelf),
        noop_facade_method_call: crate::stdlib::external_facade_receiver_call_is_noop_method(
            facts,
            &receiver_name,
            name,
            arg_count,
        ),
        receiver_type: receiver_name,
        method_name: name.to_string(),
        arg_count,
    })
}

fn plan_java_logging_facade_call<'a>(receiver: &'a Expr, name: &str) -> Option<CallTargetPlan<'a>> {
    let receiver_name = java_logging_receiver_name(receiver)?;
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

fn java_logging_receiver_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(receiver_name) => Some(receiver_name),
        Expr::FieldAccess(_, field) => Some(field),
        _ => None,
    }
}

fn static_utility_owner_call_target<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    args: Option<&'a [Expr]>,
    resolved: Option<&ResolvedCall>,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    if crate::stdlib::is_known_object_inherited_method(name, arg_count) {
        return None;
    }
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let native_receiver_key =
        crate::javalibs::native_java_static_owner_alias(&receiver_key).unwrap_or(&receiver_key);
    if crate::javalibs::native_java_static_owner_alias(&receiver_key).is_some()
        && let Some(target) = plan_native_java_static_owner_call(receiver_name, name, args, types)
    {
        return Some(target);
    }
    if matches!(name, "valueOf" | "value_of")
        && receiver_key.rsplit('.').next().unwrap_or(&receiver_key) == "Integer"
        && args
            .and_then(|args| args.first())
            .is_some_and(|arg| matches!(infer_expr_type(arg, types), JavaType::String))
    {
        return Some(CallTargetPlan::NativeJavaStatic {
            method: NativeJavaStaticMethod::IntegerValueOfString,
        });
    }
    if !unqualified_lowercase_native_static_owner_alias(receiver_name, native_receiver_key)
        && !resolved_static_owner_shadows_native_owner(
            resolved,
            receiver_name,
            native_receiver_key
                .rsplit('.')
                .next()
                .unwrap_or(native_receiver_key),
            types,
        )
        && let Some(target) = plan_native_java_static_owner_call(receiver_name, name, args, types)
    {
        return Some(target);
    }
    if let Some(owner) = types.static_type_alias(receiver_name) {
        let owner =
            crate::javalibs::preferred_static_owner_alias(owner, name, arg_count).unwrap_or(owner);
        return Some(CallTargetPlan::StaticOwner {
            owner: rust_static_owner_type_name_with_facts(owner, facts),
            rust_name: external_method_rust_name(owner, name, arg_count, facts),
        });
    }
    if let Some((owner_key, owner)) =
        external_fact_proven_static_owner(receiver_name, name, arg_count, facts, external_index)
    {
        return Some(CallTargetPlan::StaticOwner {
            owner,
            rust_name: external_method_rust_name(&owner_key, name, arg_count, facts),
        });
    }
    let query_owner = receiver_key;
    if crate::stdlib::external_api_method_role(Some(&query_owner), name, arg_count)
        != ExternalApiRole::Unknown
        && (unproven_simple_receiver_can_be_static_owner(receiver_name)
            || query_owner != receiver_name.as_str())
    {
        Some(CallTargetPlan::StaticOwner {
            owner: rust_static_owner_type_name_with_facts(&query_owner, facts),
            rust_name: external_method_rust_name(&query_owner, name, arg_count, facts),
        })
    } else if unproven_simple_receiver_can_be_static_owner(receiver_name) {
        let owner_key = upper_camel_from_snake_or_preserved(receiver_name);
        Some(CallTargetPlan::StaticOwner {
            owner: rust_static_owner_type_name_with_facts(&owner_key, facts),
            rust_name: external_method_rust_name(&owner_key, name, arg_count, facts),
        })
    } else {
        let owner_key = upper_camel_from_snake_or_preserved(receiver_name);
        if owner_key != receiver_name.as_str()
            && (crate::stdlib::external_api_method_role(Some(&owner_key), name, arg_count)
                != ExternalApiRole::Unknown
                || matches!((name, arg_count), ("valueOf" | "value_of", 1)))
        {
            let owner = if matches!((name, arg_count), ("valueOf" | "value_of", 1)) {
                owner_key.clone()
            } else {
                rust_static_owner_type_name_with_facts(&owner_key, facts)
            };
            Some(CallTargetPlan::StaticOwner {
                owner,
                rust_name: external_method_rust_name(&owner_key, name, arg_count, facts),
            })
        } else {
            None
        }
    }
}

fn external_fact_proven_static_owner(
    receiver_name: &str,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
) -> Option<(String, String)> {
    let mut query_owners = vec![crate::stdlib::canonical_external_receiver_type_key(
        receiver_name,
    )];
    let upper_camel = upper_camel_from_snake_or_preserved(receiver_name);
    let upper_camel = crate::stdlib::canonical_external_receiver_type_key(&upper_camel);
    if !query_owners.iter().any(|owner| owner == &upper_camel) {
        query_owners.push(upper_camel);
    }

    for query_owner in query_owners {
        if !crate::javalibs::is_static_facade_method(&query_owner, name, arg_count) {
            continue;
        }
        let effect = external_index
            .external_api_effect_for_receiver_with_arity(Some(&query_owner), name, Some(arg_count))
            .cloned()
            .or_else(|| {
                crate::external_packs::external_api_effect_for_receiver_with_arity(
                    Some(&query_owner),
                    name,
                    Some(arg_count),
                )
            })
            .or_else(|| {
                external_index
                    .external_api_effect_for_receiver(Some(&query_owner), name)
                    .cloned()
            })
            .or_else(|| {
                crate::external_packs::external_api_effect_for_receiver(Some(&query_owner), name)
            });
        if let Some(effect) = effect {
            let owner = effect.receiver_type.as_deref().unwrap_or(&query_owner);
            if let Some(alias) =
                crate::javalibs::preferred_static_owner_alias(owner, name, arg_count)
            {
                return Some((
                    alias.to_string(),
                    rust_static_owner_type_name_with_facts(alias, facts),
                ));
            }
            return Some((
                owner.to_string(),
                rust_static_owner_type_name_with_facts(owner, facts),
            ));
        }
    }

    None
}

fn unproven_simple_receiver_can_be_static_owner(receiver_name: &str) -> bool {
    receiver_name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        || receiver_name.ends_with("_util")
        || receiver_name.ends_with("_utils")
}

pub(crate) fn rust_static_owner_type_name(owner: &str) -> String {
    rust_static_owner_type_name_with_facts(owner, &FactSet::default())
}

pub(crate) fn rust_static_owner_type_name_with_facts(owner: &str, facts: &FactSet) -> String {
    let canonical_owner = crate::stdlib::canonical_external_receiver_type_key(owner);
    match canonical_owner.as_str() {
        "arg_utils" => {
            return "ArgUtils".to_string();
        }
        "OrgDisqBioDisqTabixIndexWriteOption" => {
            return "TabixIndexWriteOption".to_string();
        }
        "OrgBroadinstituteHellbenderUtilsReadReadUtils" => {
            return "ReadUtils".to_string();
        }
        "OrgBroadinstituteHellbenderUtilsReadSAMRecordToGATKReadAdapter" => {
            return "SAMRecordToGATKReadAdapter".to_string();
        }
        "OrgBroadinstituteHellbenderUtilsVariantVariantContextVariantAdapter" => {
            return "VariantContextVariantAdapter".to_string();
        }
        "OrgBroadinstituteHellbenderUtilsSparkSparkUtils" => {
            return "SparkUtils".to_string();
        }
        _ => {}
    }
    if let Some(mapped_owner) = external_crate_static_owner_type_name(owner, facts) {
        return mapped_owner;
    }
    crate::java_naming::rust_name_for_java_named_type(&canonical_owner)
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
    method: &str,
    arg_count: usize,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<String> {
    if crate::stdlib::is_known_object_inherited_method(method, arg_count) {
        return None;
    }
    let owner = static_owner_expr_type_name(receiver, types, facts)?;
    if let Some(alias) = crate::javalibs::preferred_static_owner_alias(&owner, method, arg_count) {
        return Some(rust_static_owner_type_name_with_facts(alias, facts));
    }
    Some(owner)
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
            let owner = types
                .static_type_alias(name)
                .map(str::to_string)
                .unwrap_or_else(|| upper_camel_from_snake_or_preserved(name));
            Some(rust_static_owner_type_name_with_facts(&owner, facts))
        }
        Expr::FieldAccess(inner, field) => {
            if is_static_constant_member(field) {
                return None;
            }
            if static_owner_field_access_resolves_to_source_field(inner, field, types) {
                return None;
            }
            let _ = static_owner_expr_type_name(inner, types, facts)?;
            Some(upper_camel_from_snake_or_preserved(field))
        }
        _ => None,
    }
}

fn static_owner_field_access_resolves_to_source_field(
    owner: &Expr,
    field: &str,
    types: &TypeEnv,
) -> bool {
    let Some(owner_name) = static_owner_expr_java_type_name(owner, types) else {
        return false;
    };
    let field_alias = crate::parser::to_snake_case(field);
    types.type_field_type(&owner_name, field).is_some()
        || types.type_field_type(&owner_name, &field_alias).is_some()
}

fn static_owner_expr_java_type_name(receiver: &Expr, types: &TypeEnv) -> Option<String> {
    match receiver {
        Expr::Var(name) => {
            if types.local_type(name).is_some() || types.field_type(name).is_some() {
                return None;
            }
            Some(
                types
                    .static_type_alias(name)
                    .map(str::to_string)
                    .unwrap_or_else(|| upper_camel_from_snake_or_preserved(name)),
            )
        }
        Expr::FieldAccess(inner, field) => {
            if is_static_constant_member(field)
                || static_owner_field_access_resolves_to_source_field(inner, field, types)
            {
                return None;
            }
            let owner = static_owner_expr_java_type_name(inner, types)?;
            Some(format!("{owner}.{field}"))
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
    resolved: Option<&ResolvedCall>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if receiver_key == "Arrays" {
        if resolved_static_owner_shadows_native_owner(resolved, receiver_name, "Arrays", types)
            || static_type_alias_shadows_native_owner(receiver_name, "Arrays", types)
        {
            return None;
        }
        let method = match name {
            "asList" | "as_list" => NativeCollectionStaticMethod::ArraysAsList,
            "equals" => NativeCollectionStaticMethod::ArraysEquals,
            "deepEquals" | "deep_equals" => NativeCollectionStaticMethod::ArraysDeepEquals,
            "binarySearch" | "binary_search" => NativeCollectionStaticMethod::ArraysBinarySearch,
            "compare" => NativeCollectionStaticMethod::ArraysCompare,
            "mismatch" => NativeCollectionStaticMethod::ArraysMismatch,
            "hashCode" | "hash_code" => NativeCollectionStaticMethod::ArraysHashCode,
            "deepHashCode" | "deep_hash_code" => NativeCollectionStaticMethod::ArraysDeepHashCode,
            "copyOf" | "copy_of" => NativeCollectionStaticMethod::ArraysCopyOf,
            "copyOfRange" | "copy_of_range" => NativeCollectionStaticMethod::ArraysCopyOfRange,
            "toString" | "to_string" => NativeCollectionStaticMethod::ArraysToString,
            "deepToString" | "deep_to_string" => NativeCollectionStaticMethod::ArraysDeepToString,
            _ => return None,
        };
        return Some(CallTargetPlan::NativeCollectionStatic { method });
    }
    if receiver_key == "Collections" {
        if resolved_static_owner_shadows_native_owner(resolved, receiver_name, "Collections", types)
            || static_type_alias_shadows_native_owner(receiver_name, "Collections", types)
        {
            return None;
        }
        let method = match name {
            "emptyList" | "empty_list" => NativeCollectionStaticMethod::CollectionsEmptyList,
            "emptySet" | "empty_set" => NativeCollectionStaticMethod::CollectionsEmptySet,
            "emptyMap" | "empty_map" => NativeCollectionStaticMethod::CollectionsEmptyMap,
            "emptySortedSet" | "empty_sorted_set" => {
                NativeCollectionStaticMethod::CollectionsEmptySortedSet
            }
            "unmodifiableCollection" | "unmodifiable_collection" => {
                NativeCollectionStaticMethod::CollectionsUnmodifiableCollection
            }
            "unmodifiableList" | "unmodifiable_list" => {
                NativeCollectionStaticMethod::CollectionsUnmodifiableList
            }
            "unmodifiableSet" | "unmodifiable_set" => {
                NativeCollectionStaticMethod::CollectionsUnmodifiableSet
            }
            "unmodifiableMap" | "unmodifiable_map" => {
                NativeCollectionStaticMethod::CollectionsUnmodifiableMap
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
    args: Option<&'a [Expr]>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    let kind = crate::stdlib::native_collection_receiver_kind_for_type(&receiver_ty)?;
    let method = match (kind, name) {
        (_, "size") => NativeCollectionMethod::Len,
        (_, "isEmpty") => NativeCollectionMethod::IsEmpty,
        (_, "toArray" | "to_array") => NativeCollectionMethod::ToArray,
        (crate::stdlib::JavaCollectionKind::List, "hashCode" | "hash_code") => {
            NativeCollectionMethod::ListHashCode
        }
        (crate::stdlib::JavaCollectionKind::List, "add")
            if args.is_some_and(|args| args.len() == 2) =>
        {
            NativeCollectionMethod::ListInsert
        }
        (crate::stdlib::JavaCollectionKind::List, "add")
            if matches!(
                native_queue_receiver_kind(&receiver_ty),
                Some(NativeQueueReceiverKind::ArrayBlockingQueue)
            ) && args.is_none_or(|args| args.len() == 1) =>
        {
            NativeCollectionMethod::ArrayBlockingQueueAdd
        }
        (crate::stdlib::JavaCollectionKind::List, "add")
            if matches!(
                native_queue_receiver_kind(&receiver_ty),
                Some(NativeQueueReceiverKind::PriorityQueue)
            ) =>
        {
            NativeCollectionMethod::PriorityQueueAdd
        }
        (crate::stdlib::JavaCollectionKind::List, "add") => NativeCollectionMethod::ListAdd,
        (crate::stdlib::JavaCollectionKind::List, "put")
            if matches!(
                native_queue_receiver_kind(&receiver_ty),
                Some(NativeQueueReceiverKind::ArrayBlockingQueue)
            ) && args.is_none_or(|args| args.len() == 1) =>
        {
            NativeCollectionMethod::ArrayBlockingQueuePut
        }
        (crate::stdlib::JavaCollectionKind::List, "offer")
            if matches!(
                native_queue_receiver_kind(&receiver_ty),
                Some(NativeQueueReceiverKind::ArrayBlockingQueue)
            ) && args.is_none_or(|args| args.len() == 1) =>
        {
            NativeCollectionMethod::ArrayBlockingQueueOffer
        }
        (crate::stdlib::JavaCollectionKind::List, "offer")
            if matches!(
                native_queue_receiver_kind(&receiver_ty),
                Some(NativeQueueReceiverKind::PriorityQueue)
            ) =>
        {
            NativeCollectionMethod::PriorityQueueAdd
        }
        (crate::stdlib::JavaCollectionKind::List, "offer")
            if native_queue_receiver_kind(&receiver_ty)
                .is_some_and(NativeQueueReceiverKind::offer_always_inserts) =>
        {
            NativeCollectionMethod::ListAdd
        }
        (crate::stdlib::JavaCollectionKind::List, "get") => NativeCollectionMethod::ListGet,
        (crate::stdlib::JavaCollectionKind::List, "subList" | "sub_list") => {
            NativeCollectionMethod::ListSubList
        }
        (crate::stdlib::JavaCollectionKind::List, "indexOf" | "index_of") => {
            NativeCollectionMethod::ListIndexOf
        }
        (crate::stdlib::JavaCollectionKind::List, "set") => NativeCollectionMethod::ListSet,
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_head_peek_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueuePeek
        }
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_head_poll_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueuePoll
        }
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_head_read_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueueElement
        }
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_tail_read_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueueLastElement
        }
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_head_remove_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueueRemove
        }
        (crate::stdlib::JavaCollectionKind::List, name)
            if native_queue_receiver_kind(&receiver_ty).is_some()
                && is_queue_tail_remove_method(name)
                && args.is_some_and(|args| args.is_empty()) =>
        {
            NativeCollectionMethod::QueueLastRemove
        }
        (crate::stdlib::JavaCollectionKind::List, "remove")
            if list_remove_argument_is_index(args, types) =>
        {
            NativeCollectionMethod::ListRemoveIndex
        }
        (crate::stdlib::JavaCollectionKind::List, "remove")
            if list_remove_argument_is_object(args, types) =>
        {
            NativeCollectionMethod::ListRemoveObject
        }
        (crate::stdlib::JavaCollectionKind::List, "contains") => NativeCollectionMethod::Contains,
        (crate::stdlib::JavaCollectionKind::List, "containsAll" | "contains_all") => {
            NativeCollectionMethod::ContainsAll
        }
        (kind, "hashCode" | "hash_code") if kind.is_set_like() => {
            NativeCollectionMethod::SetHashCode
        }
        (kind, "contains") if kind.is_set_like() => NativeCollectionMethod::Contains,
        (kind, "containsAll" | "contains_all") if kind.is_set_like() => {
            NativeCollectionMethod::ContainsAll
        }
        (kind, "add") if kind.is_set_like() => NativeCollectionMethod::SetInsert,
        (kind, "remove") if kind.is_set_like() => NativeCollectionMethod::SetRemove,
        (crate::stdlib::JavaCollectionKind::List, "addAll") => NativeCollectionMethod::AddAll,
        (crate::stdlib::JavaCollectionKind::List, "removeAll" | "remove_all") => {
            NativeCollectionMethod::RemoveAll
        }
        (crate::stdlib::JavaCollectionKind::List, "retainAll" | "retain_all") => {
            NativeCollectionMethod::RetainAll
        }
        (kind, "addAll") if kind.is_set_like() => NativeCollectionMethod::AddAll,
        (kind, "removeAll" | "remove_all") if kind.is_set_like() => {
            NativeCollectionMethod::RemoveAll
        }
        (kind, "retainAll" | "retain_all") if kind.is_set_like() => {
            NativeCollectionMethod::RetainAll
        }
        (kind, "hashCode" | "hash_code") if kind.is_map_like() => {
            NativeCollectionMethod::MapHashCode
        }
        (kind, "putAll" | "put_all") if kind.is_map_like() => NativeCollectionMethod::Extend,
        (kind, "put") if kind.is_map_like() => NativeCollectionMethod::MapInsert,
        (kind, "putIfAbsent" | "put_if_absent") if kind.is_map_like() => {
            NativeCollectionMethod::MapPutIfAbsent
        }
        (kind, "get") if kind.is_map_like() => NativeCollectionMethod::MapGet,
        (kind, "getOrDefault" | "get_or_default") if kind.is_map_like() => {
            NativeCollectionMethod::MapGetOrDefault
        }
        (kind, "remove") if kind.is_map_like() => NativeCollectionMethod::MapRemove,
        (kind, "containsKey" | "contains_key") if kind.is_map_like() => {
            NativeCollectionMethod::MapContainsKey
        }
        (kind, "containsValue" | "contains_value") if kind.is_map_like() => {
            NativeCollectionMethod::MapContainsValue
        }
        (kind, "keySet" | "key_set") if kind.is_map_like() => NativeCollectionMethod::MapKeys,
        (kind, "values") if kind.is_map_like() => NativeCollectionMethod::MapValues,
        (kind, "entrySet" | "entry_set") if kind.is_map_like() => NativeCollectionMethod::MapIter,
        (crate::stdlib::JavaCollectionKind::SortedMap, "firstKey" | "first_key") => {
            NativeCollectionMethod::MapFirstKey
        }
        (crate::stdlib::JavaCollectionKind::SortedMap, "lastKey" | "last_key") => {
            NativeCollectionMethod::MapLastKey
        }
        (crate::stdlib::JavaCollectionKind::SortedMap, "lowerKey" | "lower_key") => {
            NativeCollectionMethod::MapLowerKey
        }
        (crate::stdlib::JavaCollectionKind::SortedMap, "floorKey" | "floor_key") => {
            NativeCollectionMethod::MapFloorKey
        }
        (crate::stdlib::JavaCollectionKind::SortedMap, "ceilingKey" | "ceiling_key") => {
            NativeCollectionMethod::MapCeilingKey
        }
        (crate::stdlib::JavaCollectionKind::SortedMap, "higherKey" | "higher_key") => {
            NativeCollectionMethod::MapHigherKey
        }
        (crate::stdlib::JavaCollectionKind::List, "iterator") => NativeCollectionMethod::Iterator,
        (crate::stdlib::JavaCollectionKind::List, "listIterator" | "list_iterator") => {
            NativeCollectionMethod::ListIterator
        }
        (kind, "iterator") if kind.is_set_like() => NativeCollectionMethod::Iterator,
        _ => return None,
    };
    Some(CallTargetPlan::NativeCollectionMethod {
        receiver,
        receiver_ty: Some(receiver_ty),
        method,
    })
}

fn list_remove_argument_is_object(args: Option<&[Expr]>, types: &TypeEnv) -> bool {
    let Some([arg]) = args else {
        return false;
    };
    !list_remove_argument_type_is_index(&infer_expr_type(arg, types))
}

fn list_remove_argument_is_index(args: Option<&[Expr]>, types: &TypeEnv) -> bool {
    let Some([arg]) = args else {
        return false;
    };
    list_remove_argument_type_is_index(&infer_expr_type(arg, types))
}

fn list_remove_argument_type_is_index(ty: &JavaType) -> bool {
    matches!(
        ty,
        JavaType::Primitive(
            PrimitiveType::Byte
                | PrimitiveType::Short
                | PrimitiveType::Char
                | PrimitiveType::Int
                | PrimitiveType::Long
        )
    )
}

fn plan_native_java_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_ty = infer_expr_type(receiver, types);
    if let Some(method) = native_optional_receiver_method(receiver, name, arg_count, types) {
        return Some(CallTargetPlan::NativeJavaInstance { receiver, method });
    }
    let effective_receiver_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
    if matches!(effective_receiver_ty, JavaType::String)
        && let Some(method) = native_java_string_instance_method(name, arg_count)
    {
        return Some(CallTargetPlan::NativeJavaInstance { receiver, method });
    }
    let method = match (&effective_receiver_ty, name) {
        (JavaType::Named { name, .. } | JavaType::Unknown(name), "toString" | "to_string")
            if arg_count == 0 && platform_type_has_direct_object_to_string(name) =>
        {
            NativeJavaInstanceMethod::ObjectToString
        }
        (JavaType::Named { name, .. }, "get") if arg_count == 1 && is_bitset_receiver(name) => {
            NativeJavaInstanceMethod::BitSetGet
        }
        (JavaType::Named { name, .. }, "exists") if arg_count == 0 && is_file_receiver(name) => {
            NativeJavaInstanceMethod::FileExists
        }
        (JavaType::Named { name, .. }, "ready") if arg_count == 0 && is_reader_receiver(name) => {
            NativeJavaInstanceMethod::ReaderReady
        }
        (JavaType::Named { name, .. }, "put") if arg_count == 1 && is_buffer_receiver(name) => {
            NativeJavaInstanceMethod::BufferPut
        }
        (JavaType::Primitive(source), "byteValue" | "byte_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Byte,
            }
        }
        (JavaType::Primitive(source), "shortValue" | "short_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Short,
            }
        }
        (JavaType::Primitive(source), "intValue" | "int_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Int,
            }
        }
        (JavaType::Primitive(source), "longValue" | "long_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Long,
            }
        }
        (JavaType::Primitive(source), "floatValue" | "float_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Float,
            }
        }
        (JavaType::Primitive(source), "doubleValue" | "double_value") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source: Some(*source),
                target: PrimitiveType::Double,
            }
        }
        (JavaType::Primitive(source), "hashCode" | "hash_code") if arg_count == 0 => {
            NativeJavaInstanceMethod::PrimitiveHashCode { source: *source }
        }
        (JavaType::Primitive(source), "compareTo" | "compare_to") if arg_count == 1 => {
            NativeJavaInstanceMethod::PrimitiveCompareTo { source: *source }
        }
        (JavaType::Named { name, .. } | JavaType::Unknown(name), method)
            if arg_count == 0
                && crate::stdlib::boxed_primitive_type(name.rsplit('.').next().unwrap_or(name))
                    .is_some()
                && primitive_value_method_target(method).is_some() =>
        {
            let source =
                crate::stdlib::boxed_primitive_type(name.rsplit('.').next().unwrap_or(name));
            NativeJavaInstanceMethod::PrimitiveNumberValue {
                source,
                target: primitive_value_method_target(method)
                    .expect("checked primitive value method target"),
            }
        }
        (JavaType::Named { name, .. } | JavaType::Unknown(name), "hashCode" | "hash_code")
            if arg_count == 0
                && crate::stdlib::boxed_primitive_type(name.rsplit('.').next().unwrap_or(name))
                    .is_some() =>
        {
            NativeJavaInstanceMethod::PrimitiveHashCode {
                source: crate::stdlib::boxed_primitive_type(
                    name.rsplit('.').next().unwrap_or(name),
                )
                .expect("checked boxed primitive hashCode owner"),
            }
        }
        (JavaType::Named { name, .. } | JavaType::Unknown(name), "compareTo" | "compare_to")
            if arg_count == 1
                && crate::stdlib::boxed_primitive_type(name.rsplit('.').next().unwrap_or(name))
                    .is_some() =>
        {
            NativeJavaInstanceMethod::PrimitiveCompareTo {
                source: crate::stdlib::boxed_primitive_type(
                    name.rsplit('.').next().unwrap_or(name),
                )
                .expect("checked boxed primitive compareTo owner"),
            }
        }
        (JavaType::Named { name, .. }, "format")
            if arg_count == 1 && is_message_format_receiver(name) =>
        {
            NativeJavaInstanceMethod::MessageFormatFormat
        }
        (JavaType::Named { name, .. }, "format") if arg_count == 1 && is_format_receiver(name) => {
            NativeJavaInstanceMethod::DecimalFormatFormat
        }
        (_, "equals") if arg_count == 1 => NativeJavaInstanceMethod::Equals,
        _ => return None,
    };
    Some(CallTargetPlan::NativeJavaInstance { receiver, method })
}

fn platform_type_has_direct_object_to_string(type_name: &str) -> bool {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(type_name);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    crate::stdlib::platform_type_has_erased_display_support(simple)
}

fn native_optional_receiver_method(
    receiver: &Expr,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
) -> Option<NativeJavaInstanceMethod> {
    let receiver_name = receiver_source_type_name(receiver, types)?;
    if !matches!(
        receiver_name.rsplit('.').next().unwrap_or(receiver_name),
        "Optional" | "OptionalInt" | "OptionalLong" | "OptionalDouble"
    ) {
        return None;
    }
    match (name, arg_count) {
        ("isPresent" | "is_present", 0) => Some(NativeJavaInstanceMethod::OptionalIsPresent),
        ("get", 0) => Some(NativeJavaInstanceMethod::OptionalGet),
        ("orElse" | "or_else", 1) => Some(NativeJavaInstanceMethod::OptionalOrElse),
        _ => None,
    }
}

fn plan_resolved_native_java_receiver_call<'a>(
    receiver: &'a Expr,
    name: &str,
    arg_count: usize,
    resolved: Option<&ResolvedCall>,
) -> Option<CallTargetPlan<'a>> {
    let call = resolved?;
    if call.kind != CallKind::Instance || !is_string_receiver(&call.declaring_type) {
        return None;
    }
    let method = native_java_string_instance_method(name, arg_count)?;
    Some(CallTargetPlan::NativeJavaInstance { receiver, method })
}

pub(crate) fn native_java_string_instance_method(
    name: &str,
    arg_count: usize,
) -> Option<NativeJavaInstanceMethod> {
    Some(match (name, arg_count) {
        ("length", 0) => NativeJavaInstanceMethod::StringLen,
        ("isEmpty", 0) => NativeJavaInstanceMethod::StringIsEmpty,
        ("trim", 0) => NativeJavaInstanceMethod::StringTrim,
        ("toLowerCase", 0) => NativeJavaInstanceMethod::StringToLowercase,
        ("toUpperCase", 0) => NativeJavaInstanceMethod::StringToUppercase,
        ("toString", 0) => NativeJavaInstanceMethod::StringToString,
        ("contains", 1) => NativeJavaInstanceMethod::StringContains,
        ("startsWith", 1) => NativeJavaInstanceMethod::StringStartsWith,
        ("endsWith", 1) => NativeJavaInstanceMethod::StringEndsWith,
        ("replace", 2) => NativeJavaInstanceMethod::StringReplace,
        ("replaceFirst" | "replace_first", 2) => NativeJavaInstanceMethod::StringReplaceFirst,
        ("replaceAll" | "replace_all", 2) => NativeJavaInstanceMethod::StringReplaceAll,
        ("matches", 1) => NativeJavaInstanceMethod::StringMatches,
        ("split", 1 | 2) => NativeJavaInstanceMethod::StringSplit,
        ("substring", 1 | 2) => NativeJavaInstanceMethod::StringSubstring,
        ("toCharArray" | "to_char_array", 0) => NativeJavaInstanceMethod::StringToCharArray,
        ("hashCode" | "hash_code", 0) => NativeJavaInstanceMethod::StringHashCode,
        ("getBytes" | "get_bytes", 0) => NativeJavaInstanceMethod::StringGetBytes,
        ("getBytes" | "get_bytes", 1) => NativeJavaInstanceMethod::StringGetBytesWithCharset,
        ("indexOf" | "index_of", 1 | 2) => NativeJavaInstanceMethod::StringIndexOf,
        ("lastIndexOf" | "last_index_of", 1 | 2) => NativeJavaInstanceMethod::StringLastIndexOf,
        ("charAt" | "char_at", 1) => NativeJavaInstanceMethod::StringCharAt,
        ("compareTo" | "compare_to", 1) => NativeJavaInstanceMethod::StringCompareTo,
        ("compareToIgnoreCase" | "compare_to_ignore_case", 1) => {
            NativeJavaInstanceMethod::StringCompareToIgnoreCase
        }
        ("equalsIgnoreCase" | "equals_ignore_case", 1) => {
            NativeJavaInstanceMethod::StringEqualsIgnoreCase
        }
        _ => return None,
    })
}

pub(crate) fn native_java_instance_receiver_type_hint(
    name: &str,
    arg_count: usize,
) -> Option<JavaType> {
    native_java_string_instance_method(name, arg_count).map(|_| JavaType::String)
}

pub(crate) fn native_java_unbound_method_reference_receiver_type(target: &str) -> Option<JavaType> {
    match crate::stdlib::canonical_external_receiver_type_key(target).as_str() {
        "String" => Some(JavaType::String),
        "Integer" => Some(JavaType::Named {
            name: "Integer".to_string(),
            args: Vec::new(),
        }),
        "Double" => Some(JavaType::Named {
            name: "Double".to_string(),
            args: Vec::new(),
        }),
        "Float" => Some(JavaType::Named {
            name: "Float".to_string(),
            args: Vec::new(),
        }),
        _ => None,
    }
}

fn is_string_receiver(name: &str) -> bool {
    matches!(name, "String" | "java.lang.String")
}

fn primitive_value_method_target(method: &str) -> Option<PrimitiveType> {
    match method {
        "byteValue" | "byte_value" => Some(PrimitiveType::Byte),
        "shortValue" | "short_value" => Some(PrimitiveType::Short),
        "intValue" | "int_value" => Some(PrimitiveType::Int),
        "longValue" | "long_value" => Some(PrimitiveType::Long),
        "floatValue" | "float_value" => Some(PrimitiveType::Float),
        "doubleValue" | "double_value" => Some(PrimitiveType::Double),
        _ => None,
    }
}

fn is_format_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "DecimalFormat" | "NumberFormat" | "DateFormat" | "MessageFormat" | "Format"
    )
}

fn is_message_format_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "MessageFormat"
    )
}

fn is_file_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "File"
    )
}

fn is_bitset_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "BitSet"
    )
}

fn is_reader_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "Reader" | "BufferedReader" | "InputStreamReader"
    )
}

fn is_buffer_receiver(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name)
            .rsplit('.')
            .next()
            .unwrap_or(name),
        "ByteBuffer" | "CharBuffer"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeQueueReceiverKind {
    Queue,
    Deque,
    PriorityQueue,
    ArrayDeque,
    ArrayBlockingQueue,
    LinkedList,
}

impl NativeQueueReceiverKind {
    fn from_simple_name(name: &str) -> Option<Self> {
        match name {
            "Queue" => Some(Self::Queue),
            "Deque" => Some(Self::Deque),
            "PriorityQueue" => Some(Self::PriorityQueue),
            "ArrayDeque" => Some(Self::ArrayDeque),
            "ArrayBlockingQueue" => Some(Self::ArrayBlockingQueue),
            "LinkedList" => Some(Self::LinkedList),
            _ => None,
        }
    }

    fn offer_always_inserts(self) -> bool {
        matches!(self, Self::ArrayDeque | Self::LinkedList)
    }
}

fn native_queue_receiver_kind(ty: &JavaType) -> Option<NativeQueueReceiverKind> {
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = ty else {
        return None;
    };
    crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .and_then(NativeQueueReceiverKind::from_simple_name)
}

fn is_queue_head_read_method(name: &str) -> bool {
    matches!(name, "element" | "getFirst" | "get_first")
}

fn is_queue_tail_read_method(name: &str) -> bool {
    matches!(name, "getLast" | "get_last")
}

fn is_queue_head_peek_method(name: &str) -> bool {
    matches!(name, "peek" | "peekFirst" | "peek_first")
}

fn is_queue_head_poll_method(name: &str) -> bool {
    matches!(name, "poll" | "pollFirst" | "poll_first")
}

fn is_queue_head_remove_method(name: &str) -> bool {
    matches!(name, "remove" | "removeFirst" | "remove_first" | "pop")
}

fn is_queue_tail_remove_method(name: &str) -> bool {
    matches!(name, "removeLast" | "remove_last")
}

fn plan_native_java_static_call<'a>(
    receiver: Option<&'a Expr>,
    name: &str,
    _arg_count: usize,
    args: Option<&'a [Expr]>,
    _facts: &FactSet,
    types: &TypeEnv,
    resolved: Option<&ResolvedCall>,
) -> Option<CallTargetPlan<'a>> {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return None;
    };
    if !receiver_is_native_static_owner(receiver_name, types) {
        return None;
    }
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let receiver_key =
        crate::javalibs::native_java_static_owner_alias(&receiver_key).unwrap_or(&receiver_key);
    if unqualified_lowercase_native_static_owner_alias(receiver_name, receiver_key) {
        return None;
    }
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(receiver_key);
    if resolved_static_owner_shadows_native_owner(resolved, receiver_name, receiver_simple, types)
        || static_type_alias_shadows_native_owner(receiver_name, receiver_simple, types)
    {
        return None;
    }
    plan_native_java_static_owner_call(receiver_name, name, args, types)
}

fn unqualified_lowercase_native_static_owner_alias(
    receiver_name: &str,
    receiver_key: &str,
) -> bool {
    receiver_name != receiver_key
        && !receiver_name.contains('.')
        && receiver_name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
}

fn resolved_static_owner_shadows_native_owner(
    resolved: Option<&ResolvedCall>,
    receiver_name: &str,
    native_owner: &str,
    types: &TypeEnv,
) -> bool {
    let Some(call) = resolved else {
        return false;
    };
    if call.kind != CallKind::Static {
        return false;
    }
    if native_static_owner_qualified_names(native_owner).contains(&call.declaring_type.as_str()) {
        return false;
    }
    if types
        .static_type_alias(receiver_name)
        .is_some_and(|alias| native_static_owner_qualified_names(native_owner).contains(&alias))
    {
        return false;
    }
    let declaring_key = crate::stdlib::canonical_external_receiver_type_key(&call.declaring_type);
    if let Some(alias) = crate::javalibs::native_java_static_owner_alias(&declaring_key)
        && native_static_owner_matches(alias, native_owner)
    {
        return false;
    }
    !native_static_owner_matches(&call.declaring_type, native_owner)
        || call.declaring_type == receiver_name
}

fn static_type_alias_shadows_native_owner(
    receiver_name: &str,
    native_owner: &str,
    types: &TypeEnv,
) -> bool {
    types
        .static_type_alias(receiver_name)
        .is_some_and(|alias| !native_static_owner_matches(alias, native_owner))
}

fn native_static_owner_matches(owner: &str, native_owner: &str) -> bool {
    let owner_key = crate::stdlib::canonical_external_receiver_type_key(owner);
    let owner_key =
        crate::javalibs::native_java_static_owner_alias(&owner_key).unwrap_or(owner_key.as_str());
    let native_owner_key = crate::stdlib::canonical_external_receiver_type_key(native_owner);
    let native_owner_key = crate::javalibs::native_java_static_owner_alias(&native_owner_key)
        .unwrap_or(native_owner_key.as_str());
    if owner_key == native_owner_key {
        return true;
    }
    if owner == native_owner {
        return true;
    }
    native_static_owner_qualified_names(native_owner)
        .iter()
        .any(|known| owner == *known)
}

fn native_static_owner_qualified_names(native_owner: &str) -> &'static [&'static str] {
    match native_owner {
        "Arrays" => &["java.util.Arrays"],
        "Collections" => &["java.util.Collections"],
        "Objects" => &["java.util.Objects"],
        "Math" => &[
            "java.lang.Math",
            "java.lang.StrictMath",
            "org.apache.commons.math3.util.FastMath",
        ],
        "Byte" => &["java.lang.Byte"],
        "Short" => &["java.lang.Short"],
        "Integer" => &["java.lang.Integer"],
        "Long" => &["java.lang.Long"],
        "Float" => &["java.lang.Float"],
        "Double" => &["java.lang.Double"],
        "Character" => &["java.lang.Character"],
        "System" => &["java.lang.System"],
        "Thread" => &["java.lang.Thread"],
        "String" => &["java.lang.String"],
        "Optional" => &["java.util.Optional"],
        "JavaUtilOptional" => &["java.util.Optional"],
        _ => &[],
    }
}

fn receiver_is_native_static_owner(receiver_name: &str, types: &TypeEnv) -> bool {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let receiver_key =
        crate::javalibs::native_java_static_owner_alias(&canonical).unwrap_or(canonical.as_str());
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(receiver_key);
    types.static_type_alias(receiver_name).is_some()
        || receiver_name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        || receiver_name.contains('.')
        || (types.local_type(receiver_name).is_none()
            && !native_static_owner_qualified_names(receiver_simple).is_empty())
}

pub(crate) fn plan_native_java_static_owner_call<'a>(
    receiver_name: &str,
    name: &str,
    args: Option<&'a [Expr]>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    let receiver_key =
        crate::javalibs::native_java_static_owner_alias(&receiver_key).unwrap_or(&receiver_key);
    let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(receiver_key);
    let method = match (receiver_simple, name) {
        ("Math", "max") => NativeJavaStaticMethod::MathMax {
            primitive: math_binary_overload_primitive(args, types),
        },
        ("Math", "min") => NativeJavaStaticMethod::MathMin {
            primitive: math_binary_overload_primitive(args, types),
        },
        ("Math", "log") => NativeJavaStaticMethod::MathLog,
        ("Math", "log10") => NativeJavaStaticMethod::MathLog10,
        ("Math", "exp") => NativeJavaStaticMethod::MathExp,
        ("Math", "sqrt") => NativeJavaStaticMethod::MathSqrt,
        ("Math", "pow") => NativeJavaStaticMethod::MathPow,
        ("Math", "ceil") => NativeJavaStaticMethod::MathCeil,
        ("Math", "floor") => NativeJavaStaticMethod::MathFloor,
        ("Math", "sin") => math_unary_f64_method("sin", "sin"),
        ("Math", "cos") => math_unary_f64_method("cos", "cos"),
        ("Math", "tan") => math_unary_f64_method("tan", "tan"),
        ("Math", "asin") => math_unary_f64_method("asin", "asin"),
        ("Math", "acos") => math_unary_f64_method("acos", "acos"),
        ("Math", "atan") => math_unary_f64_method("atan", "atan"),
        ("Math", "sinh") => math_unary_f64_method("sinh", "sinh"),
        ("Math", "cosh") => math_unary_f64_method("cosh", "cosh"),
        ("Math", "tanh") => math_unary_f64_method("tanh", "tanh"),
        ("Math", "cbrt") => math_unary_f64_method("cbrt", "cbrt"),
        ("Math", "rint") => math_unary_f64_method("rint", "round_ties_even"),
        ("Math", "signum") => math_unary_f64_method("signum", "signum"),
        ("Math", "toRadians" | "to_radians") => math_unary_f64_method("toRadians", "to_radians"),
        ("Math", "toDegrees" | "to_degrees") => math_unary_f64_method("toDegrees", "to_degrees"),
        ("Math", "atan2") => math_binary_f64_method("atan2", "atan2"),
        ("Math", "hypot") => math_binary_f64_method("hypot", "hypot"),
        ("Math", "round") => NativeJavaStaticMethod::MathRound {
            source: math_unary_overload_primitive(args, types, PrimitiveType::Double),
        },
        ("Math", "abs") => NativeJavaStaticMethod::MathAbs {
            source: math_unary_overload_primitive(args, types, PrimitiveType::Int),
        },
        ("Math", "toIntExact" | "to_int_exact") => NativeJavaStaticMethod::MathToIntExact,
        ("Math", "floorDiv" | "floor_div") => NativeJavaStaticMethod::MathFloorDiv {
            primitive: math_binary_overload_primitive(args, types),
        },
        ("Math", "floorMod" | "floor_mod") => math_floor_mod_method(args, types),
        ("Byte", "parseByte" | "parse_byte") => NativeJavaStaticMethod::ByteParseByte,
        ("Short", "parseShort" | "parse_short") => NativeJavaStaticMethod::ShortParseShort,
        ("Integer", "parseInt" | "parse_int") => NativeJavaStaticMethod::IntegerParseInt,
        ("Integer", "decode") => NativeJavaStaticMethod::IntegerDecode,
        ("Integer", "valueOf" | "value_of")
            if args
                .and_then(|args| args.first())
                .is_some_and(|arg| matches!(infer_expr_type(arg, types), JavaType::String)) =>
        {
            NativeJavaStaticMethod::IntegerValueOfString
        }
        ("Long", "parseLong" | "parse_long") => NativeJavaStaticMethod::LongParseLong,
        ("Float", "parseFloat" | "parse_float") => NativeJavaStaticMethod::FloatParseFloat,
        ("Double", "parseDouble" | "parse_double") => NativeJavaStaticMethod::DoubleParseDouble,
        ("Double" | "Float", "isNaN" | "is_na_n") => NativeJavaStaticMethod::FloatingIsNaN,
        ("Character", "isDigit" | "is_digit") => character_predicate("isDigit", "is_ascii_digit"),
        ("Character", "isLetter" | "is_letter") => character_predicate("isLetter", "is_alphabetic"),
        ("Character", "isWhitespace" | "is_whitespace") => {
            character_predicate("isWhitespace", "is_whitespace")
        }
        ("Character", "toUpperCase" | "to_upper_case") => {
            character_transform("toUpperCase", "to_ascii_uppercase")
        }
        ("Character", "toLowerCase" | "to_lower_case") => {
            character_transform("toLowerCase", "to_ascii_lowercase")
        }
        ("System", "gc") => NativeJavaStaticMethod::SystemGc,
        ("System", "exit") => NativeJavaStaticMethod::SystemExit,
        ("Thread", "sleep") => NativeJavaStaticMethod::ThreadSleep,
        ("String", "format") => NativeJavaStaticMethod::StringFormat,
        ("String", "join") => NativeJavaStaticMethod::StringJoin,
        ("String", "valueOf" | "value_of") => NativeJavaStaticMethod::StringValueOf,
        ("Optional" | "JavaUtilOptional", "empty") => NativeJavaStaticMethod::OptionalEmpty,
        ("Optional" | "JavaUtilOptional", "of") => NativeJavaStaticMethod::OptionalOf,
        ("Optional" | "JavaUtilOptional", "ofNullable" | "of_nullable") => {
            NativeJavaStaticMethod::OptionalOfNullable
        }
        ("Objects", "equals") => NativeJavaStaticMethod::ObjectsEquals,
        ("Objects", "toString" | "to_string") => NativeJavaStaticMethod::ObjectsToString,
        ("Objects", "nonNull" | "non_null") => NativeJavaStaticMethod::ObjectsNonNull,
        ("Objects", "isNull" | "is_null") => NativeJavaStaticMethod::ObjectsIsNull,
        ("Pattern" | "JavaUtilRegexPattern", "compile") => NativeJavaStaticMethod::PatternCompile,
        _ => return None,
    };
    Some(CallTargetPlan::NativeJavaStatic { method })
}

fn character_predicate(
    java_name: &'static str,
    rust_method: &'static str,
) -> NativeJavaStaticMethod {
    NativeJavaStaticMethod::CharacterPredicate {
        java_name,
        rust_method,
    }
}

fn character_transform(
    java_name: &'static str,
    rust_method: &'static str,
) -> NativeJavaStaticMethod {
    NativeJavaStaticMethod::CharacterTransform {
        java_name,
        rust_method,
    }
}

fn math_unary_f64_method(
    java_name: &'static str,
    rust_method: &'static str,
) -> NativeJavaStaticMethod {
    NativeJavaStaticMethod::MathUnaryF64 {
        java_name,
        rust_method,
    }
}

fn math_binary_f64_method(
    java_name: &'static str,
    rust_method: &'static str,
) -> NativeJavaStaticMethod {
    NativeJavaStaticMethod::MathBinaryF64 {
        java_name,
        rust_method,
    }
}

fn math_floor_mod_method(args: Option<&[Expr]>, types: &TypeEnv) -> NativeJavaStaticMethod {
    let left = args
        .and_then(|args| args.first())
        .and_then(|expr| expr_numeric_primitive(expr, types));
    let right = args
        .and_then(|args| args.get(1))
        .and_then(|expr| expr_numeric_primitive(expr, types));
    let primitive = numeric_binary_overload_primitive(left, right, PrimitiveType::Int);
    let return_primitive = if matches!(left, Some(PrimitiveType::Long))
        && !matches!(right, Some(PrimitiveType::Long))
    {
        PrimitiveType::Int
    } else {
        primitive
    };
    NativeJavaStaticMethod::MathFloorMod {
        primitive,
        return_primitive,
    }
}

fn math_binary_overload_primitive(args: Option<&[Expr]>, types: &TypeEnv) -> PrimitiveType {
    let Some([left, right]) = args else {
        return PrimitiveType::Int;
    };
    numeric_binary_overload_primitive(
        expr_numeric_primitive(left, types),
        expr_numeric_primitive(right, types),
        PrimitiveType::Int,
    )
}

fn math_unary_overload_primitive(
    args: Option<&[Expr]>,
    types: &TypeEnv,
    default: PrimitiveType,
) -> PrimitiveType {
    let Some([arg]) = args else {
        return default;
    };
    expr_numeric_primitive(arg, types).unwrap_or(default)
}

fn expr_numeric_primitive(expr: &Expr, types: &TypeEnv) -> Option<PrimitiveType> {
    let JavaType::Primitive(primitive) = infer_expr_type(expr, types) else {
        return None;
    };
    Some(primitive)
}

fn numeric_binary_overload_primitive(
    left: Option<PrimitiveType>,
    right: Option<PrimitiveType>,
    default: PrimitiveType,
) -> PrimitiveType {
    use PrimitiveType::*;
    match std::cmp::max(primitive_numeric_rank(left), primitive_numeric_rank(right)) {
        5 => Double,
        4 => Float,
        3 => Long,
        2 => Int,
        _ => default,
    }
}

fn primitive_numeric_rank(primitive: Option<PrimitiveType>) -> u8 {
    use PrimitiveType::*;
    match primitive {
        Some(Double) => 5,
        Some(Float) => 4,
        Some(Long) => 3,
        Some(Int | Short | Byte | Char) => 2,
        _ => 0,
    }
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
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'a>> {
    let receiver_name = external_receiver_expr_type_key_with_index(
        receiver,
        name,
        facts,
        external_index,
        types,
        None,
    )?;
    let effect = external_index
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
            external_index
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
    let pack = external_index
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
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<CallTargetPlan<'static>> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    if crate::stdlib::is_known_object_inherited_method(name, arg_count) {
        return None;
    }
    let receiver_key = types
        .static_type_alias(receiver_name)
        .map(crate::stdlib::canonical_external_receiver_type_key)
        .unwrap_or_else(|| crate::stdlib::canonical_external_receiver_type_key(receiver_name));
    if plan_native_java_static_owner_call(&receiver_key, name, None, types).is_some() {
        return None;
    }
    let effect = external_index
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
            external_index
                .external_api_effect_for_receiver(Some(&receiver_key), name)
                .cloned()
        })
        .or_else(|| {
            crate::external_packs::external_api_effect_for_receiver(Some(&receiver_key), name)
        })?;
    if effect.provenance != FactProvenance::ExternalLibraryPack {
        return None;
    }
    let pack = external_index
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
    let native_collection = plan_native_collection_constructor(&owner_ty, arg_count, facts);
    let native_string = plan_native_string_constructor(&owner_ty, arg_count, None);
    NewTargetPlan::new(
        value_shape_for_java_type(representations, owner_ty),
        rust_name,
        external_crate,
        native_collection,
        native_string,
    )
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
    let native_collection =
        plan_native_collection_constructor_for_args(&owner_ty, args, types, facts)
            .or_else(|| plan_native_collection_constructor(&owner_ty, args.len(), facts));
    let native_string = plan_native_string_constructor(&owner_ty, args.len(), Some((args, types)));
    NewTargetPlan::new(
        value_shape_for_java_type(representations, owner_ty),
        rust_name,
        external_crate,
        native_collection,
        native_string,
    )
}

fn constructor_rust_name(
    owner_ty: &JavaType,
    resolved: Option<&ResolvedConstructor>,
    facts: &FactSet,
    arg_count: usize,
) -> String {
    if let Some(resolved) = resolved {
        return constructor_member_rust_name(&resolved.rust_name).to_string();
    }
    let Some(type_name) = java_type_name(owner_ty) else {
        return "new".to_string();
    };
    if java_mutable_string_buffer_constructor_uses_arity_name(type_name) {
        return format!("new_{arg_count}");
    }
    if external_constructor_needs_arity_name(type_name, facts) {
        format!("new_{arg_count}")
    } else {
        "new".to_string()
    }
}

fn constructor_member_rust_name(rust_name: &str) -> &str {
    rust_name.rsplit("::").next().unwrap_or(rust_name)
}

fn java_mutable_string_buffer_constructor_uses_arity_name(type_name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(type_name).as_str(),
        "StringBuilder" | "StringBuffer" | "StringTokenizer" | "BitSet" | "ZipEntry"
    )
}

fn plan_native_string_constructor(
    owner_ty: &JavaType,
    arg_count: usize,
    args_and_types: Option<(&[Expr], &TypeEnv)>,
) -> Option<NativeStringNewTarget> {
    let owner = java_type_name(owner_ty)?;
    if crate::stdlib::canonical_external_receiver_type_key(owner) != "String" {
        return None;
    }
    if let Some((args, types)) = args_and_types
        && let [arg] = args
        && infer_expr_type(arg, types) == JavaType::String
    {
        return Some(NativeStringNewTarget::FromString);
    }
    match arg_count {
        1 | 2 => Some(NativeStringNewTarget::FromBytes),
        3 | 4 => Some(NativeStringNewTarget::FromByteRange),
        _ => None,
    }
}

pub fn external_facade_method_rust_name_for_abi(
    receiver_type: &str,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    external_method_rust_name(receiver_type, method_name, arg_count, facts)
}

pub fn external_facade_constructor_rust_name_for_abi(
    type_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    let owner_ty = JavaType::Named {
        name: type_name.to_string(),
        args: Vec::new(),
    };
    constructor_rust_name(&owner_ty, None, facts, arg_count)
}

pub(crate) fn unresolved_constructor_rust_name_for_facts(
    owner_ty: &JavaType,
    facts: &FactSet,
    arg_count: usize,
) -> String {
    constructor_rust_name(owner_ty, None, facts, arg_count)
}

pub(crate) fn external_method_rust_name(
    receiver_type: &str,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    let base = external_method_base_rust_name(method_name);
    let receiver_key = crate::stdlib::canonical_external_receiver_type_key(receiver_type);
    if boxed_primitive_static_to_string_uses_unsuffixed_name(&receiver_key, &base, arg_count) {
        return base;
    }
    if receiver_key == "PrintStream"
        && matches!(base.as_str(), "print" | "println")
        && arg_count > 0
    {
        return format!("{base}_{arg_count}");
    }
    if java_platform_method_uses_suffix_only_for_multi_arg_overloads(&receiver_key, &base)
        && arg_count == 1
    {
        return base;
    }
    if external_method_needs_arity_name(receiver_type, method_name, facts) {
        format!("{base}_{arg_count}")
    } else {
        base
    }
}

fn external_method_base_rust_name(method_name: &str) -> String {
    if method_name
        .strip_prefix('_')
        .is_some_and(|tail| tail.starts_with(|ch: char| ch.is_ascii_digit()))
    {
        method_name.to_string()
    } else {
        crate::parser::to_snake_case(method_name)
    }
}

fn boxed_primitive_static_to_string_uses_unsuffixed_name(
    receiver_key: &str,
    method_name: &str,
    arg_count: usize,
) -> bool {
    method_name == "to_string"
        && arg_count == 1
        && receiver_key
            .rsplit('.')
            .next()
            .and_then(crate::stdlib::boxed_primitive_type)
            .is_some()
}

fn java_platform_method_uses_suffix_only_for_multi_arg_overloads(
    receiver_key: &str,
    method_name: &str,
) -> bool {
    matches!(method_name, "read" | "write" | "sleep")
        && java_platform_method_arities_for(receiver_key, method_name).contains(&1)
}

fn external_method_rust_name_for_receiver(
    receiver: &Expr,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<String> {
    let receiver_type = external_receiver_expr_type_key_with_index(
        receiver,
        method_name,
        facts,
        external_index,
        types,
        None,
    )?;
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
    if !facts.disable_builtin_external_packs {
        arities.extend(crate::external_packs::external_api_arities_for_receiver(
            receiver_type,
            method_name,
        ));
        arities.extend(java_platform_method_arities_for(receiver_type, method_name));
    }
    arities.len() > 1
}

fn java_platform_method_arities_for(receiver_type: &str, method_name: &str) -> BTreeSet<usize> {
    (0..=4)
        .filter(|arity| {
            crate::external_packs::java_platform_api_effect_for_receiver_with_arity(
                receiver_type,
                method_name,
                Some(*arity),
            )
            .is_some()
        })
        .collect()
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
    let fact_aliases = external_method_receiver_aliases(fact_receiver);
    let query_aliases = external_method_receiver_aliases(query_receiver);
    if fact_aliases
        .iter()
        .any(|fact_alias| query_aliases.contains(fact_alias))
    {
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

fn external_method_receiver_aliases(receiver: &str) -> BTreeSet<String> {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(receiver);
    let mut aliases = BTreeSet::from([
        receiver.to_string(),
        canonical.clone(),
        crate::java_naming::rust_name_for_java_named_type(receiver),
        crate::java_naming::rust_name_for_java_named_type(&canonical),
    ]);
    if let Some(simple) = receiver.rsplit('.').next() {
        aliases.insert(simple.to_string());
        aliases.insert(crate::java_naming::rust_name_for_java_named_type(simple));
    }
    if let Some(simple) = canonical.rsplit('.').next() {
        aliases.insert(simple.to_string());
        aliases.insert(crate::java_naming::rust_name_for_java_named_type(simple));
    }
    aliases
}

fn external_method_name_matches(fact_method_name: &str, query_method_name: &str) -> bool {
    fact_method_name == query_method_name
        || crate::parser::to_snake_case(fact_method_name)
            == crate::parser::to_snake_case(query_method_name)
}

fn external_constructor_needs_arity_name(type_name: &str, facts: &FactSet) -> bool {
    let mut arities = external_constructor_arities_for(facts, type_name);
    if !facts.disable_builtin_external_packs {
        arities.extend(crate::external_packs::external_constructor_arities_for(
            type_name,
        ));
    }
    arities.len() > 1
}

fn external_constructor_arities_for(facts: &FactSet, type_name: &str) -> BTreeSet<usize> {
    facts
        .external_constructor_facts
        .iter()
        .filter(|fact| external_constructor_type_matches(&fact.type_name, type_name))
        .map(|fact| fact.arg_types.len())
        .collect()
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
    facts: &FactSet,
) -> Option<NativeCollectionNewTarget> {
    let kind = native_collection_constructor_kind_with_facts(ty, facts)?;
    match arg_count {
        0 => Some(NativeCollectionNewTarget::Empty { kind }),
        1 => Some(NativeCollectionNewTarget::FromSingleArg { kind }),
        _ => None,
    }
}

pub(crate) fn plan_native_collection_constructor_for_args(
    ty: &JavaType,
    args: &[Expr],
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<NativeCollectionNewTarget> {
    let kind = native_collection_constructor_kind_with_facts(ty, facts)?;
    match args {
        [] => Some(NativeCollectionNewTarget::Empty { kind }),
        [arg] if kind.supports_capacity_constructor() && is_integral_capacity_arg(arg, types) => {
            Some(NativeCollectionNewTarget::WithCapacity { kind })
        }
        [arg] if is_collection_source_arg(arg, types) => {
            Some(NativeCollectionNewTarget::FromSource { kind })
        }
        [arg]
            if matches!(
                native_queue_receiver_kind(ty),
                Some(NativeQueueReceiverKind::PriorityQueue)
            ) && is_priority_queue_comparator_constructor_arg(arg, types, facts) =>
        {
            if priority_queue_comparator_constructor_arg_is_callback_param(arg, types) {
                Some(NativeCollectionNewTarget::PriorityQueueComparatorCallback { kind })
            } else if priority_queue_comparator_constructor_arg_is_comparator_object_binding(
                arg, types,
            ) {
                Some(NativeCollectionNewTarget::PriorityQueueComparator { kind })
            } else if priority_queue_comparator_constructor_arg_is_opaque_comparator(arg, types) {
                Some(NativeCollectionNewTarget::PriorityQueueComparatorUnsupported { kind })
            } else {
                Some(NativeCollectionNewTarget::PriorityQueueComparator { kind })
            }
        }
        [_] => Some(NativeCollectionNewTarget::FromSingleArg { kind }),
        _ => None,
    }
}

fn native_collection_constructor_kind_with_facts(
    ty: &JavaType,
    facts: &FactSet,
) -> Option<crate::stdlib::JavaCollectionKind> {
    crate::stdlib::native_collection_constructor_kind_for_type(ty).or_else(|| {
        EffectiveFacts::new(facts)
            .sequence_element_type_for(ty)
            .map(|_| crate::stdlib::JavaCollectionKind::List)
    })
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
            plan_native_collection_receiver_call(receiver, name, None, types),
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

fn is_priority_queue_comparator_constructor_arg(
    arg: &Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> bool {
    let arg_ty = infer_expr_type(arg, types);
    match &arg_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            if crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Comparator")
            {
                return true;
            }
        }
        _ => {}
    }
    facts
        .hierarchy_subtype_policy(
            &arg_ty,
            &JavaType::Named {
                name: "Comparator".to_string(),
                args: Vec::new(),
            },
        )
        .reaches()
}

fn priority_queue_comparator_constructor_arg_is_opaque_comparator(
    arg: &Expr,
    types: &TypeEnv,
) -> bool {
    if is_anonymous_priority_queue_comparator_arg(arg) {
        return false;
    }
    let arg_ty = infer_expr_type(arg, types);
    match &arg_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == "Comparator")
        }
        _ => false,
    }
}

fn priority_queue_comparator_constructor_arg_is_callback_param(
    arg: &Expr,
    types: &TypeEnv,
) -> bool {
    let Expr::Var(name) = arg else {
        return false;
    };
    (types.is_param(name) || types.is_callable_local(name))
        && priority_queue_comparator_constructor_arg_is_opaque_comparator(arg, types)
}

fn priority_queue_comparator_constructor_arg_is_comparator_object_binding(
    arg: &Expr,
    types: &TypeEnv,
) -> bool {
    let comparator_object_binding = match arg {
        Expr::Var(name) => {
            types.is_comparator_object_local(name) || types.is_comparator_object_field(name)
        }
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            types.is_comparator_object_field(field)
        }
        _ => false,
    };
    comparator_object_binding
        && priority_queue_comparator_constructor_arg_is_opaque_comparator(arg, types)
}

fn is_anonymous_priority_queue_comparator_arg(arg: &Expr) -> bool {
    let Expr::New {
        ty,
        args,
        body: Some(_),
        ..
    } = arg
    else {
        return false;
    };
    args.is_empty() && concrete_type_simple_name_is(ty, "Comparator")
}

fn concrete_type_simple_name_is(ty: &JavaType, expected: &str) -> bool {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::canonical_external_receiver_type_key(name)
                .rsplit('.')
                .next()
                .is_some_and(|simple| simple == expected)
        }
        _ => false,
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
    Some(ExternalCrateNewTarget::new(
        pack.rust_target.clone()?,
        mapping
            .as_ref()
            .and_then(|mapping| mapping.rust_type_name.clone())
            .unwrap_or_else(|| {
                type_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(&type_name)
                    .to_string()
            }),
        mapping
            .map(|mapping| mapping.rust_name)
            .unwrap_or_else(|| rust_name.to_string()),
    ))
}

fn java_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::String => Some("String"),
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

fn resolved_call_signature_arg_shapes(
    call: &ResolvedCall,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Option<Vec<RustSlotShape>> {
    let signature =
        signatures.method_signature_by_rust_name(&call.declaring_type, &call.rust_name)?;
    if signature.params().len() != arg_count {
        return None;
    }
    Some(
        signature
            .params()
            .iter()
            .map(|param| param.shape().clone())
            .collect(),
    )
}

fn resolved_call_signature_arg_shapes_by_java_key(
    call: &ResolvedCall,
    source_name: &str,
    arg_count: usize,
    signatures: &SignaturePlan,
) -> Option<Vec<RustSlotShape>> {
    if call.arg_target_types.len() != arg_count {
        return None;
    }
    let signature = signatures.resolved_source_method_signature(call, source_name)?;
    if signature.params().len() != arg_count {
        return None;
    }
    Some(
        signature
            .params()
            .iter()
            .map(|param| param.shape().clone())
            .collect(),
    )
}

fn resolved_call_signature_arg_shapes_by_args(
    call: &ResolvedCall,
    source_name: &str,
    args: &[Expr],
    signatures: &SignaturePlan,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    let arg_types = args
        .iter()
        .map(|arg| crate::types::infer_expr_type(arg, types))
        .collect::<Vec<_>>();
    let (_, signature) = signatures
        .method_signature_by_base_name_and_arg_types_with_facts(
            &call.declaring_type,
            source_name,
            &arg_types,
            facts,
        )
        .or_else(|| {
            signatures.method_signature_by_base_name_and_arg_types(
                &call.declaring_type,
                source_name,
                &arg_types,
            )
        })
        .or_else(|| {
            signatures.method_signature_by_base_name_and_arity(
                &call.declaring_type,
                source_name,
                args.len(),
            )
        })?;
    if signature.params().len() != args.len() {
        return None;
    }
    Some(
        signature
            .params()
            .iter()
            .map(|param| param.shape().clone())
            .collect(),
    )
}

fn resolved_call_boundary_arg_shape(call: &ResolvedCall, index: usize) -> RustSlotShape {
    let java_ty = call
        .arg_target_types
        .get(index)
        .cloned()
        .or_else(|| {
            call.arg_coercions
                .iter()
                .find(|coercion| coercion.arg_index == index)
                .map(|coercion| coercion.target.clone())
        })
        .unwrap_or_else(|| JavaType::Unknown(format!("{}:arg{index}", call.rust_name)));
    RustSlotShape::for_storage(java_ty)
}

pub(crate) fn resolved_call_arg_target_type(
    call: &ResolvedCall,
    index: usize,
) -> Option<&JavaType> {
    call.arg_target_types.get(index).or_else(|| {
        call.arg_coercions
            .iter()
            .find(|coercion| coercion.arg_index == index)
            .map(|coercion| &coercion.target)
    })
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
        if let Some(shapes) =
            native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
        {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) =
            resolved_call_signature_arg_shapes_by_java_key(call, name, arg_count, signatures)
        {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) = resolved_call_signature_arg_shapes(call, arg_count, signatures) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(plan) =
            facade_abi_call_arg_plan_for_resolved(call, name, arg_count, None, facts, types)
        {
            return plan;
        }
        if let Some(shapes) = external_call_arg_shapes_for_resolved(call, name, arg_count, facts) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) = external_call_arg_shapes(receiver, name, arg_count, facts, types) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        return ArgumentLoweringPlan::Shaped(resolved_call_arg_shapes(call, arg_count, signatures));
    }

    external_call_arg_shapes(receiver, name, arg_count, facts, types)
        .map(ArgumentLoweringPlan::Shaped)
        .or_else(|| facade_abi_call_arg_plan(receiver, name, arg_count, None, facts, types))
        .or_else(|| {
            native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
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
        if let Some(shapes) =
            native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
        {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) =
            resolved_call_signature_arg_shapes_by_java_key(call, name, arg_count, signatures)
        {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) = resolved_call_signature_arg_shapes(call, arg_count, signatures) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) =
            resolved_call_signature_arg_shapes_by_args(call, name, args, signatures, facts, types)
        {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(plan) =
            facade_abi_call_arg_plan_for_resolved(call, name, arg_count, Some(args), facts, types)
        {
            return plan;
        }
        if let Some(shapes) = external_call_arg_shapes_for_resolved(call, name, arg_count, facts) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        if let Some(shapes) = external_call_arg_shapes(receiver, name, arg_count, facts, types) {
            return ArgumentLoweringPlan::Shaped(shapes);
        }
        return ArgumentLoweringPlan::Shaped(resolved_call_arg_shapes(call, arg_count, signatures));
    }

    external_call_arg_shapes_with_args(receiver, name, args, facts, types)
        .map(ArgumentLoweringPlan::Shaped)
        .or_else(|| facade_abi_call_arg_plan(receiver, name, arg_count, Some(args), facts, types))
        .or_else(|| {
            native_collection_arg_shapes_with_facts(receiver, name, arg_count, types, facts)
                .map(ArgumentLoweringPlan::Shaped)
        })
        .or_else(|| {
            conservative_external_mutator_arg_shapes(receiver, name, args, signatures, facts, types)
                .map(ArgumentLoweringPlan::Shaped)
        })
        .unwrap_or(ArgumentLoweringPlan::Unshaped { arg_count })
}

pub fn native_collection_arg_shapes_with_facts(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let receiver = receiver?;
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    let method = if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_peek_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueuePeek
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_poll_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueuePoll
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_read_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueElement
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_tail_read_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueLastElement
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_remove_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueRemove
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_tail_remove_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueLastRemove
    } else if matches!(
        crate::stdlib::native_collection_receiver_kind_for_type(&receiver_ty),
        Some(crate::stdlib::JavaCollectionKind::List)
    ) && name == "add"
        && arg_count == 2
    {
        NativeCollectionMethod::ListInsert
    } else {
        match plan_native_collection_receiver_call(receiver, name, None, types)? {
            CallTargetPlan::NativeCollectionMethod { method, .. } => method,
            _ => return None,
        }
    };
    native_collection_method_arg_shapes(&receiver_ty, method, arg_count, facts)
}

pub(crate) fn native_collection_method_arg_shapes(
    receiver_ty: &JavaType,
    method: NativeCollectionMethod,
    arg_count: usize,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_ty = effective_native_collection_receiver_type(receiver_ty);
    let shapes = match method {
        NativeCollectionMethod::ListAdd
        | NativeCollectionMethod::PriorityQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueOffer
        | NativeCollectionMethod::ArrayBlockingQueuePut
        | NativeCollectionMethod::SetInsert
            if arg_count == 1 =>
        {
            vec![collection_element_arg_shape_with_effective_facts(
                &receiver_ty,
                &effective_facts,
            )?]
        }
        NativeCollectionMethod::ListInsert if arg_count == 2 => {
            vec![
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                collection_element_arg_shape_with_effective_facts(&receiver_ty, &effective_facts)?,
            ]
        }
        NativeCollectionMethod::AddAll
        | NativeCollectionMethod::RemoveAll
        | NativeCollectionMethod::ContainsAll
        | NativeCollectionMethod::RetainAll
        | NativeCollectionMethod::Extend
            if arg_count == 1 =>
        {
            vec![storage_arg_shape_with_effective_facts(
                receiver_ty.clone(),
                &effective_facts,
            )]
        }
        NativeCollectionMethod::ListHashCode
        | NativeCollectionMethod::SetHashCode
        | NativeCollectionMethod::MapHashCode
            if arg_count == 0 =>
        {
            Vec::new()
        }
        NativeCollectionMethod::MapInsert | NativeCollectionMethod::MapPutIfAbsent
            if arg_count == 2 =>
        {
            let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![
                storage_arg_shape_with_effective_facts(key_ty.clone(), &effective_facts),
                storage_arg_shape_with_effective_facts(value_ty.clone(), &effective_facts),
            ]
        }
        NativeCollectionMethod::MapGetOrDefault if arg_count == 2 => {
            let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![
                borrowed_sequence_element_shape(key_ty.clone()),
                storage_arg_shape_with_effective_facts(value_ty.clone(), &effective_facts),
            ]
        }
        NativeCollectionMethod::ListGet if arg_count == 1 => {
            vec![RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            ))]
        }
        NativeCollectionMethod::ListSubList if arg_count == 2 => {
            vec![
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
            ]
        }
        NativeCollectionMethod::ListIndexOf if arg_count == 1 => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(element_ty.clone())]
        }
        NativeCollectionMethod::ListSet if arg_count == 2 => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            vec![
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                collection_element_arg_shape_with_effective_facts(&receiver_ty, &effective_facts)
                    .unwrap_or_else(|| {
                        storage_arg_shape_with_effective_facts(element_ty.clone(), &effective_facts)
                    }),
            ]
        }
        NativeCollectionMethod::ListRemoveIndex if arg_count == 1 => {
            vec![RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            ))]
        }
        NativeCollectionMethod::ListRemoveObject if arg_count == 1 => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(element_ty.clone())]
        }
        NativeCollectionMethod::QueueElement
        | NativeCollectionMethod::QueuePeek
        | NativeCollectionMethod::QueuePoll
        | NativeCollectionMethod::QueueRemove
            if arg_count == 0 =>
        {
            Vec::new()
        }
        NativeCollectionMethod::Contains if arg_count == 1 => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(element_ty.clone())]
        }
        NativeCollectionMethod::SetRemove if arg_count == 1 => {
            let element_ty = crate::types::sequence_element_type(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(element_ty.clone())]
        }
        NativeCollectionMethod::MapGet
        | NativeCollectionMethod::MapRemove
        | NativeCollectionMethod::MapContainsKey
            if arg_count == 1 =>
        {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(key_ty.clone())]
        }
        NativeCollectionMethod::MapContainsValue if arg_count == 1 => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(value_ty.clone())]
        }
        NativeCollectionMethod::MapLowerKey
        | NativeCollectionMethod::MapFloorKey
        | NativeCollectionMethod::MapCeilingKey
        | NativeCollectionMethod::MapHigherKey
            if arg_count == 1 =>
        {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            vec![borrowed_sequence_element_shape(key_ty.clone())]
        }
        NativeCollectionMethod::MapValues if arg_count == 0 => Vec::new(),
        NativeCollectionMethod::MapFirstKey | NativeCollectionMethod::MapLastKey
            if arg_count == 0 =>
        {
            Vec::new()
        }
        NativeCollectionMethod::ToArray if matches!(arg_count, 0 | 1) => Vec::new(),
        NativeCollectionMethod::Iterator if arg_count == 0 => Vec::new(),
        NativeCollectionMethod::ListIterator if arg_count == 0 => Vec::new(),
        NativeCollectionMethod::ListIterator if arg_count == 1 => {
            vec![RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            ))]
        }
        _ => return None,
    };
    Some(shapes)
}

fn collection_element_arg_shape_with_effective_facts(
    receiver_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Option<RustSlotShape> {
    let element_ty = crate::types::sequence_element_type(receiver_ty)
        .or_else(|| raw_sequence_receiver_erased_element_type(receiver_ty))?;
    Some(storage_arg_shape_with_effective_facts(element_ty, facts))
}

pub(crate) fn raw_sequence_receiver_erased_element_type(
    receiver_ty: &JavaType,
) -> Option<JavaType> {
    let JavaType::Named { name, args } = receiver_ty else {
        return None;
    };
    if !args.is_empty()
        || !crate::stdlib::java_collection_kind_for_name(name).is_some_and(|kind| {
            kind == crate::stdlib::JavaCollectionKind::List || kind.is_set_like()
        })
    {
        return None;
    }
    Some(JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    })
}

pub(crate) fn plan_native_collection_extend_arg(
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

pub(crate) fn external_call_arg_shapes(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    external_call_arg_shapes_core(receiver, name, arg_count, None, facts, types)
}

fn external_call_arg_shapes_with_args(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    external_call_arg_shapes_core(
        receiver,
        name,
        args.len(),
        Some(&inferred_argument_java_types(args, types)),
        facts,
        types,
    )
}

fn external_call_arg_shapes_core(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    arg_types: Option<&[JavaType]>,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_name =
        external_call_argument_receiver_key(receiver, name, arg_count, facts, types);
    if name == "format"
        && receiver_name
            .as_deref()
            .is_some_and(is_string_formatting_receiver)
    {
        return None;
    }
    let receiver_ty = receiver.map(|receiver| infer_expr_type(receiver, types));
    let mut saw_fact = false;
    let shapes = (0..arg_count)
        .map(|index| {
            if let Some(effect) = facts.external_api_argument_effect_for_receiver_with_arity(
                receiver_name.as_deref(),
                name,
                Some(arg_count),
                index,
            ) {
                saw_fact = true;
                argument_shape_from_external_effect(effect, receiver_ty.as_ref(), &effective_facts)
            } else if let Some(effect) =
                crate::external_packs::external_api_argument_effect_for_receiver_with_arity(
                    receiver_name.as_deref(),
                    name,
                    Some(arg_count),
                    index,
                )
            {
                saw_fact = true;
                argument_shape_from_external_effect(&effect, receiver_ty.as_ref(), &effective_facts)
            } else {
                unspecified_external_argument_shape(
                    name,
                    index,
                    arg_types.and_then(|arg_types| arg_types.get(index).cloned()),
                )
            }
        })
        .collect();
    saw_fact.then_some(shapes)
}

fn unspecified_external_argument_shape(
    name: &str,
    index: usize,
    java_ty: Option<JavaType>,
) -> RustSlotShape {
    let java_ty = java_ty.unwrap_or_else(|| JavaType::Unknown(format!("{name}:arg{index}")));
    let mut shape = RustSlotShape::conservative(java_ty.clone());
    if java_ty == JavaType::String {
        apply_borrowed_string_content_projection(&mut shape);
    } else if matches!(java_ty, JavaType::Array(_)) {
        apply_borrowed_sequence_content_projection(&mut shape);
    } else if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) {
        shape.ownership = OwnershipShape::SharedBorrow;
    }
    shape
}

fn facade_abi_call_arg_plan(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    args: Option<&[Expr]>,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<ArgumentLoweringPlan> {
    let receiver_name =
        external_call_argument_receiver_key(receiver, name, arg_count, facts, types)?;
    let key = CallableKey::method(
        receiver_name,
        name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let abi = if let Some(args) = args {
        let arg_types = inferred_argument_java_types(args, types);
        facts
            .facade_abi_for_call(&key, &arg_types)
            .or_else(|| unresolved_primitive_facade_abi_fallback(facts, &key, &arg_types))
    } else {
        facts.facade_abi_for_external_method(key.owner_type(), key.member_name(), key.arity())
    }?;
    let collection_arg_types =
        facade_abi_native_collection_arg_java_types(receiver, name, arg_count, types);
    facade_abi_argument_lowering_plan(
        abi,
        arg_count,
        args,
        types,
        collection_arg_types.as_deref(),
        facts,
    )
}

fn facade_abi_call_arg_plan_for_resolved(
    call: &ResolvedCall,
    source_name: &str,
    arg_count: usize,
    args: Option<&[Expr]>,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<ArgumentLoweringPlan> {
    let key = CallableKey::resolved_method(call, Staticness::Unknown, arg_count);
    let source_key = CallableKey::method(
        call.declaring_type.clone(),
        source_name.to_string(),
        Staticness::Unknown,
        arg_count,
    );
    let abi = if let Some(args) = args {
        let arg_types = inferred_argument_java_types(args, types);
        facts
            .facade_abi_for_call(&key, &arg_types)
            .or_else(|| facts.facade_abi_for_call(&source_key, &arg_types))
            .or_else(|| unresolved_primitive_facade_abi_fallback(facts, &key, &arg_types))
            .or_else(|| unresolved_primitive_facade_abi_fallback(facts, &source_key, &arg_types))
    } else {
        facts
            .facade_abi_for_key(&key)
            .or_else(|| facts.facade_abi_for_key(&source_key))
    };
    abi.and_then(|abi| facade_abi_argument_lowering_plan(abi, arg_count, args, types, None, facts))
}

pub(crate) fn facade_abi_argument_plan_for_rust_path(
    rust_path: &str,
    arg_count: usize,
    args: Option<&[Expr]>,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<ArgumentLoweringPlan> {
    facts
        .facade_abi_facts
        .iter()
        .rev()
        .find(|abi| abi.rust_path == rust_path && abi.args.len() == arg_count)
        .and_then(|abi| facade_abi_argument_lowering_plan(abi, arg_count, args, types, None, facts))
}

pub(crate) fn facade_abi_argument_plan_for_rust_method_name(
    rust_method: &str,
    arg_count: usize,
    args: Option<&[Expr]>,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<ArgumentLoweringPlan> {
    facts
        .facade_abi_facts
        .iter()
        .rev()
        .find(|abi| {
            abi.args.len() == arg_count
                && abi.rust_path.rsplit("::").next().unwrap_or(&abi.rust_path) == rust_method
        })
        .and_then(|abi| facade_abi_argument_lowering_plan(abi, arg_count, args, types, None, facts))
}

fn unresolved_primitive_facade_abi_fallback<'a>(
    facts: &'a FactSet,
    key: &CallableKey,
    arg_types: &[JavaType],
) -> Option<&'a FacadeAbiFact> {
    if arg_types.len() != 1 || !matches!(arg_types.first(), Some(JavaType::Unknown(_))) {
        return None;
    }
    facts.facade_abi_facts.iter().rev().find(|fact| {
        external_method_receiver_matches(fact.key.owner_type(), key.owner_type())
            && fact.key.member_name() == key.member_name()
            && fact.key.kind() == key.kind()
            && (fact.key.staticness() == key.staticness()
                || fact.key.staticness() == Staticness::Unknown
                || key.staticness() == Staticness::Unknown)
            && fact.key.arity() == key.arity()
            && fact.args.len() == 1
            && fact.rust_path.rsplit("::").next() == Some("write_int")
            && matches!(
                fact.key.params(),
                [TypePattern::Exact(type_key)]
                    if matches!(type_key.as_str(), "byte" | "short" | "char" | "int")
            )
    })
}

fn facade_abi_for_receiver_call<'a>(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    facts: &'a FactSet,
    external_index: &ExternalFactLookupIndex<'_>,
    types: &TypeEnv,
) -> Option<&'a FacadeAbiFact> {
    let receiver_name = external_receiver_expr_type_key_with_index(
        receiver,
        name,
        facts,
        external_index,
        types,
        None,
    )?;
    let arg_types = inferred_argument_java_types(args, types);
    facts.facade_abi_for_external_call(&receiver_name, name, args.len(), &arg_types)
}

fn facade_abi_argument_lowering_plan(
    abi: &FacadeAbiFact,
    arg_count: usize,
    args: Option<&[Expr]>,
    types: &TypeEnv,
    arg_java_types: Option<&[JavaType]>,
    facts: &FactSet,
) -> Option<ArgumentLoweringPlan> {
    if abi.args.len() != arg_count {
        return None;
    }
    let effective_facts = EffectiveFacts::new(facts);
    let mut shapes = Vec::with_capacity(arg_count);
    let mut slots = Vec::with_capacity(arg_count);
    let mut has_explicit_slot = false;
    for (index, slot) in abi.args.iter().enumerate() {
        let java_ty = arg_java_types
            .and_then(|arg_types| arg_types.get(index).cloned())
            .or_else(|| inferred_argument_java_type(args, index, types))
            .map(|java_ty| facade_abi_effective_argument_java_type(abi, index, java_ty))
            .unwrap_or_else(|| JavaType::Unknown("Object".to_string()));
        match slot {
            FacadeSlot::Index => {
                has_explicit_slot = true;
                slots.push(index_slot());
            }
            _ => {
                let shape = rust_slot_shape_from_facade_arg_slot(slot, java_ty, &effective_facts);
                slots.push(argument_slot(shape.clone()));
                shapes.push(shape);
            }
        }
    }
    if has_explicit_slot {
        Some(ArgumentLoweringPlan::Slotted(slots))
    } else {
        Some(ArgumentLoweringPlan::Shaped(shapes))
    }
}

fn facade_abi_effective_argument_java_type(
    abi: &FacadeAbiFact,
    index: usize,
    java_ty: JavaType,
) -> JavaType {
    if index == 0
        && abi.rust_path.rsplit("::").next().unwrap_or(&abi.rust_path) == "write_int"
        && matches!(
            java_ty,
            JavaType::Primitive(PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Char)
        )
    {
        JavaType::Primitive(PrimitiveType::Int)
    } else {
        java_ty
    }
}

fn facade_abi_native_collection_arg_java_types(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    types: &TypeEnv,
) -> Option<Vec<JavaType>> {
    let receiver = receiver?;
    let receiver_ty = infer_expr_type(receiver, types);
    let receiver_ty = effective_native_collection_receiver_type(&receiver_ty);
    let method = if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_peek_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueuePeek
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_poll_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueuePoll
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_read_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueElement
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_tail_read_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueLastElement
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_head_remove_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueRemove
    } else if native_queue_receiver_kind(&receiver_ty).is_some()
        && is_queue_tail_remove_method(name)
        && arg_count == 0
    {
        NativeCollectionMethod::QueueLastRemove
    } else if matches!(
        crate::stdlib::native_collection_receiver_kind_for_type(&receiver_ty),
        Some(crate::stdlib::JavaCollectionKind::List)
    ) && name == "add"
        && arg_count == 2
    {
        NativeCollectionMethod::ListInsert
    } else {
        match plan_native_collection_receiver_call(receiver, name, None, types)? {
            CallTargetPlan::NativeCollectionMethod { method, .. } => method,
            _ => return None,
        }
    };
    match method {
        NativeCollectionMethod::ListAdd
        | NativeCollectionMethod::PriorityQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueAdd
        | NativeCollectionMethod::ArrayBlockingQueueOffer
        | NativeCollectionMethod::ArrayBlockingQueuePut
        | NativeCollectionMethod::SetInsert
            if arg_count == 1 =>
        {
            crate::types::sequence_mutation_element_type(&receiver_ty)
                .map(|element_ty| vec![element_ty])
        }
        NativeCollectionMethod::ListInsert if arg_count == 2 => {
            crate::types::sequence_mutation_element_type(&receiver_ty)
                .map(|element_ty| vec![JavaType::Primitive(PrimitiveType::Int), element_ty])
        }
        NativeCollectionMethod::AddAll
        | NativeCollectionMethod::RemoveAll
        | NativeCollectionMethod::ContainsAll
        | NativeCollectionMethod::RetainAll
        | NativeCollectionMethod::Extend
            if arg_count == 1 =>
        {
            None
        }
        NativeCollectionMethod::ListHashCode
        | NativeCollectionMethod::SetHashCode
        | NativeCollectionMethod::MapHashCode
            if arg_count == 0 =>
        {
            Some(Vec::new())
        }
        NativeCollectionMethod::MapInsert | NativeCollectionMethod::MapPutIfAbsent
            if arg_count == 2 =>
        {
            let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(vec![key_ty.clone(), value_ty.clone()])
        }
        NativeCollectionMethod::MapGetOrDefault if arg_count == 2 => {
            let (key_ty, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(vec![key_ty.clone(), value_ty.clone()])
        }
        NativeCollectionMethod::ListGet if arg_count == 1 => {
            Some(vec![JavaType::Primitive(PrimitiveType::Int)])
        }
        NativeCollectionMethod::ListSubList if arg_count == 2 => Some(vec![
            JavaType::Primitive(PrimitiveType::Int),
            JavaType::Primitive(PrimitiveType::Int),
        ]),
        NativeCollectionMethod::ListIndexOf if arg_count == 1 => {
            crate::types::sequence_element_type(&receiver_ty).map(|element_ty| vec![element_ty])
        }
        NativeCollectionMethod::ListSet if arg_count == 2 => {
            crate::types::sequence_mutation_element_type(&receiver_ty)
                .map(|element_ty| vec![JavaType::Primitive(PrimitiveType::Int), element_ty])
        }
        NativeCollectionMethod::ListRemoveIndex if arg_count == 1 => {
            Some(vec![JavaType::Primitive(PrimitiveType::Int)])
        }
        NativeCollectionMethod::ListRemoveObject if arg_count == 1 => {
            crate::types::sequence_element_type(&receiver_ty).map(|element_ty| vec![element_ty])
        }
        NativeCollectionMethod::QueueElement
        | NativeCollectionMethod::QueuePeek
        | NativeCollectionMethod::QueuePoll
        | NativeCollectionMethod::QueueRemove
            if arg_count == 0 =>
        {
            Some(Vec::new())
        }
        NativeCollectionMethod::Contains if arg_count == 1 => {
            crate::types::sequence_element_type(&receiver_ty).map(|element_ty| vec![element_ty])
        }
        NativeCollectionMethod::SetRemove if arg_count == 1 => {
            crate::types::sequence_element_type(&receiver_ty).map(|element_ty| vec![element_ty])
        }
        NativeCollectionMethod::MapGet
        | NativeCollectionMethod::MapRemove
        | NativeCollectionMethod::MapContainsKey
            if arg_count == 1 =>
        {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(vec![key_ty.clone()])
        }
        NativeCollectionMethod::MapContainsValue if arg_count == 1 => {
            let (_, value_ty) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(vec![value_ty.clone()])
        }
        NativeCollectionMethod::MapLowerKey
        | NativeCollectionMethod::MapFloorKey
        | NativeCollectionMethod::MapCeilingKey
        | NativeCollectionMethod::MapHigherKey
            if arg_count == 1 =>
        {
            let (key_ty, _) = crate::stdlib::java_map_type_args(&receiver_ty)?;
            Some(vec![key_ty.clone()])
        }
        NativeCollectionMethod::MapValues if arg_count == 0 => Some(Vec::new()),
        NativeCollectionMethod::MapFirstKey | NativeCollectionMethod::MapLastKey
            if arg_count == 0 =>
        {
            Some(Vec::new())
        }
        NativeCollectionMethod::ToArray if matches!(arg_count, 0 | 1) => Some(if arg_count == 0 {
            Vec::new()
        } else {
            vec![receiver_ty]
        }),
        NativeCollectionMethod::Iterator if arg_count == 0 => Some(Vec::new()),
        NativeCollectionMethod::ListIterator if arg_count == 0 => Some(Vec::new()),
        NativeCollectionMethod::ListIterator if arg_count == 1 => {
            Some(vec![JavaType::Primitive(PrimitiveType::Int)])
        }
        _ => None,
    }
}

fn inferred_argument_java_type(
    args: Option<&[Expr]>,
    index: usize,
    types: &TypeEnv,
) -> Option<JavaType> {
    args?.get(index).map(|arg| infer_expr_type(arg, types))
}

fn inferred_argument_java_types(args: &[Expr], types: &TypeEnv) -> Vec<JavaType> {
    args.iter().map(|arg| infer_expr_type(arg, types)).collect()
}

pub(crate) fn external_call_arg_shapes_for_resolved(
    call: &ResolvedCall,
    source_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let staticness = match call.kind {
        CallKind::Instance => Staticness::Instance,
        CallKind::Static => Staticness::Static,
    };
    let key = CallableKey::resolved_method(call, staticness, arg_count);
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_ty = JavaType::Named {
        name: call.declaring_type.clone(),
        args: Vec::new(),
    };
    let mut saw_fact = false;
    let shapes = (0..arg_count)
        .map(|index| {
            if let Some(effect) = facts
                .external_api_argument_effect_for_key(&key, index)
                .or_else(|| {
                    facts.external_api_argument_effect_for_receiver_with_arity(
                        Some(&call.declaring_type),
                        source_name,
                        Some(arg_count),
                        index,
                    )
                })
                .or_else(|| {
                    facts.external_api_argument_effect_for_receiver_with_arity(
                        Some(&call.declaring_type),
                        &call.rust_name,
                        Some(arg_count),
                        index,
                    )
                })
            {
                saw_fact = true;
                argument_shape_from_external_effect(effect, Some(&receiver_ty), &effective_facts)
            } else {
                resolved_call_boundary_arg_shape(call, index)
            }
        })
        .collect();
    saw_fact.then_some(shapes)
}

fn is_string_formatting_receiver(receiver: &str) -> bool {
    matches!(
        receiver.rsplit('.').next().unwrap_or(receiver),
        "DecimalFormat" | "NumberFormat" | "DateFormat" | "MessageFormat" | "Format"
    )
}

pub fn external_constructor_arg_shapes(
    ty: &JavaType,
    rust_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> Option<Vec<RustSlotShape>> {
    let effective_facts = EffectiveFacts::new(facts);
    let receiver_name = java_type_name(ty)?;
    if arg_count == 1
        && rust_name == "new"
        && receiver_name
            .rsplit('.')
            .next()
            .is_some_and(|simple| simple == "File")
    {
        return Some(vec![RustSlotShape::for_storage(JavaType::String)]);
    }
    let mut saw_fact = false;
    let shapes = (0..arg_count)
        .map(|index| {
            if let Some(effect) =
                facts.external_constructor_argument_effect_for(receiver_name, arg_count, index)
            {
                saw_fact = true;
                argument_shape_from_external_constructor_effect(effect, &effective_facts)
            } else if let Some(effect) = facts.external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_name),
                rust_name,
                Some(arg_count),
                index,
            ) {
                saw_fact = true;
                argument_shape_from_external_effect(effect, Some(ty), &effective_facts)
            } else if let Some(effect) =
                crate::external_packs::external_api_argument_effect_for_receiver_with_arity(
                    Some(receiver_name),
                    rust_name,
                    Some(arg_count),
                    index,
                )
            {
                saw_fact = true;
                argument_shape_from_external_effect(&effect, Some(ty), &effective_facts)
            } else {
                RustSlotShape::conservative(JavaType::Unknown(format!("{rust_name}:arg{index}")))
            }
        })
        .collect();
    saw_fact
        .then_some(shapes)
        .or_else(|| external_constructor_arg_shapes(ty, rust_name, arg_count, facts))
}

pub fn external_constructor_arg_shapes_with_args(
    ty: &JavaType,
    rust_name: &str,
    args: &[Expr],
    signatures: &SignaturePlan,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<Vec<RustSlotShape>> {
    let arg_count = args.len();
    let receiver_name = java_type_name(ty)?;
    let effective_facts = EffectiveFacts::new(facts);
    let mut saw_fact = false;
    let shapes = args
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            let actual_arg_type = infer_arg_storage_java_type(arg, signatures, types);
            if receiver_name == "String" && args.len() == 1 && actual_arg_type == JavaType::String {
                saw_fact = true;
                RustSlotShape::for_storage(JavaType::String)
            } else if let Some(effect) =
                crate::external_packs::java_platform_constructor_argument_effect_for_actual_arg(
                    receiver_name,
                    arg_count,
                    index,
                    &actual_arg_type,
                )
            {
                saw_fact = true;
                argument_shape_from_external_constructor_effect(&effect, &effective_facts)
            } else if let Some(effect) =
                facts.external_constructor_argument_effect_for(receiver_name, arg_count, index)
            {
                saw_fact = true;
                argument_shape_from_external_constructor_effect(effect, &effective_facts)
            } else if let Some(effect) =
                crate::external_packs::java_platform_constructor_argument_effect_for(
                    receiver_name,
                    arg_count,
                    index,
                )
            {
                saw_fact = true;
                argument_shape_from_external_constructor_effect(&effect, &effective_facts)
            } else if let Some(effect) = facts.external_api_argument_effect_for_receiver_with_arity(
                Some(receiver_name),
                rust_name,
                Some(arg_count),
                index,
            ) {
                saw_fact = true;
                argument_shape_from_external_effect(effect, Some(ty), &effective_facts)
            } else if let Some(effect) =
                crate::external_packs::external_api_argument_effect_for_receiver_with_arity(
                    Some(receiver_name),
                    rust_name,
                    Some(arg_count),
                    index,
                )
            {
                saw_fact = true;
                argument_shape_from_external_effect(&effect, Some(ty), &effective_facts)
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
        false,
        facts,
    )
}

fn external_call_argument_receiver_key(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<String> {
    receiver
        .and_then(|receiver| static_call_argument_owner_key(receiver, types))
        .or_else(|| {
            receiver.and_then(|receiver| {
                static_utility_argument_owner_key(receiver, name, arg_count, facts, types)
            })
        })
        .or_else(|| {
            receiver.and_then(|receiver| {
                let receiver_ty = infer_expr_type(receiver, types);
                let direct_receiver = java_type_name(&receiver_ty).map(str::to_string);
                if direct_receiver.as_ref().is_some_and(|owner| {
                    facts
                        .facade_abi_for_external_method(owner, name, arg_count)
                        .is_some()
                }) {
                    return direct_receiver;
                }
                let receiver_ty =
                    crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
                java_type_name(&receiver_ty).map(str::to_string)
            })
        })
        .or_else(|| {
            static_member_import_owner_for_call(name, types)
                .map(crate::stdlib::canonical_external_receiver_type_key)
        })
}

fn static_utility_argument_owner_key(
    receiver: &Expr,
    name: &str,
    arg_count: usize,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<String> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if types.local_type(receiver_name).is_some() || types.field_type(receiver_name).is_some() {
        return None;
    }
    let query_owner = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    if crate::stdlib::is_known_object_inherited_method(name, arg_count) {
        return None;
    }
    if facts
        .external_api_effect_for_receiver_with_arity(Some(&query_owner), name, Some(arg_count))
        .is_some()
        || crate::external_packs::external_api_effect_for_receiver_with_arity(
            Some(&query_owner),
            name,
            Some(arg_count),
        )
        .is_some()
    {
        return Some(query_owner);
    }
    if crate::stdlib::external_api_method_role(Some(&query_owner), name, arg_count)
        != ExternalApiRole::Unknown
        && (unproven_simple_receiver_can_be_static_owner(receiver_name)
            || query_owner != receiver_name.as_str())
    {
        return Some(query_owner);
    }
    let rust_owner = upper_camel_from_snake_or_preserved(receiver_name);
    facts
        .facade_abi_for_external_method(&rust_owner, name, arg_count)
        .map(|_| rust_owner)
}

fn static_call_argument_owner_key(receiver: &Expr, types: &TypeEnv) -> Option<String> {
    if let Some(owner) = imported_static_owner_for_call(receiver, types) {
        return Some(crate::stdlib::canonical_external_receiver_type_key(owner));
    }
    crate::stdlib::qualified_static_owner_java_name(receiver)
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
    static_field_shape_for_expr(arg, signatures, types)
        .map(|shape| shape.java_ty.clone())
        .unwrap_or(inferred)
}

fn static_field_shape_for_expr<'a>(
    arg: &Expr,
    signatures: &'a SignaturePlan,
    types: &TypeEnv,
) -> Option<&'a RustSlotShape> {
    let Expr::FieldAccess(receiver, field) = arg else {
        return None;
    };
    let Expr::Var(owner) = receiver.as_ref() else {
        return None;
    };
    let owner_upper = upper_camel_from_snake_or_preserved(owner);
    let field_snake = crate::parser::to_snake_case(field);
    let mut owners = vec![owner.as_str(), owner_upper.as_str()];
    if let Some(alias) = types.static_type_alias(owner) {
        owners.push(alias);
    }
    owners.into_iter().find_map(|owner| {
        signatures
            .field_shape(owner, field)
            .or_else(|| signatures.field_shape(owner, &field_snake))
    })
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
        external_argument_effect_allows_nullable_borrowed_slot(effect),
        facts,
    )
}

fn external_argument_effect_allows_nullable_borrowed_slot(
    effect: &ExternalApiArgumentEffectFact,
) -> bool {
    effect.parameter_index == 0
        && matches!(effect.method_name.as_str(), "writeObject" | "write_object")
        && effect
            .receiver_type
            .as_deref()
            .map(crate::stdlib::canonical_external_receiver_type_key)
            .and_then(|receiver| receiver.rsplit('.').next().map(str::to_string))
            .is_some_and(|simple| {
                matches!(
                    simple.as_str(),
                    "ObjectOutputStream" | "JavaIoObjectOutputStream"
                )
            })
}

fn argument_shape_from_external_metadata(
    parameter_type: Option<JavaType>,
    parameter_nullability: NullabilityState,
    argument_mutability: ReceiverMutabilityFact,
    alias_policy: ExternalArgumentAliasPolicy,
    keep_concrete_unknown: bool,
    allow_nullable_borrowed_slot: bool,
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

    let nullable_argument = crate::types::optional_element_type(&shape.java_ty).is_some();
    let allow_alias_borrow = !nullable_argument || allow_nullable_borrowed_slot;
    match alias_policy {
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow && base_java_ty == JavaType::String =>
        {
            apply_borrowed_string_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow && matches!(base_java_ty, JavaType::Array(_)) =>
        {
            apply_borrowed_sequence_content_projection(&mut shape);
        }
        ExternalArgumentAliasPolicy::Borrowed
            if allow_alias_borrow
                && !matches!(base_java_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            shape.ownership = OwnershipShape::SharedBorrow;
        }
        ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes => {
            if !matches!(base_java_ty, JavaType::Primitive(_) | JavaType::Void) {
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
    if !facts.is_final_type(receiver_ty) {
        return false;
    }
    if crate::stdlib::java_collection_kind_for_type(receiver_ty).is_some() {
        return true;
    }
    let Some(receiver_name) = java_type_name(receiver_ty) else {
        return false;
    };
    facts
        .external_library_pack_for_type(receiver_name)
        .is_some()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::ExternalApiEffectFact;
    use crate::java_ir::Literal;
    use crate::types::optional_java_type;

    #[test]
    fn facade_abi_argument_plan_prefers_abi_slot_over_argument_effect() {
        let mut facts = FactSet::default();
        facts.facade_abi_facts.push(FacadeAbiFact {
            key: CallableKey::method_with_params(
                "FacadeOwner",
                "accept",
                Staticness::Unknown,
                vec![TypePattern::exact("String")],
            ),
            rust_path: "facade::FacadeOwner::accept".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: vec![FacadeSlot::Owned],
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::Plain,
            borrow_source: None,
            borrowed_return_recovery: None,
            provenance: FactProvenance::UserOverride,
        });
        facts
            .external_api_argument_effect_facts
            .push(ExternalApiArgumentEffectFact {
                method_name: "accept".to_string(),
                receiver_type: Some("FacadeOwner".to_string()),
                arg_count: Some(1),
                parameter_index: 0,
                parameter_type: Some(JavaType::String),
                parameter_nullability: NullabilityState::DefinitelyNonNull,
                argument_mutability: ReceiverMutabilityFact::ReadOnly,
                alias_policy: ExternalArgumentAliasPolicy::Borrowed,
                provenance: FactProvenance::UserOverride,
            });
        let args = vec![Expr::Var("value".to_string())];
        let mut types = TypeEnv::new();
        types.insert_local("value", JavaType::String);

        let plan = facade_abi_argument_plan_for_rust_path(
            "facade::FacadeOwner::accept",
            1,
            Some(&args),
            &types,
            &facts,
        )
        .expect("facade ABI should plan argument slots");
        let ArgumentLoweringPlan::Shaped(shapes) = plan else {
            panic!("owned facade argument should produce shaped plan");
        };
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].java_ty, JavaType::String);
        assert_eq!(shapes[0].ownership, OwnershipShape::Owned);
        assert_eq!(shapes[0].view, ViewShape::Native);
    }

    fn resolved_instance_call(owner: &str, rust_name: &str) -> ResolvedCall {
        ResolvedCall {
            declaring_type: owner.to_string(),
            rust_name: rust_name.to_string(),
            kind: CallKind::Instance,
            return_type: JavaType::Void,
            arg_target_types: Vec::new(),
            arg_coercions: Vec::new(),
        }
    }

    #[test]
    fn native_system_print_statement_refuses_source_resolved_owner() {
        let call = Expr::Call {
            receiver: Some(Box::new(Expr::FieldAccess(
                Box::new(Expr::Var("System".to_string())),
                "out".to_string(),
            ))),
            name: "println".to_string(),
            args: vec![Expr::Literal(Literal::String("x".to_string()))],
            resolved: Some(resolved_instance_call("Sink", "println")),
        };

        assert_eq!(
            plan_call_statement(&call, &FactSet::default(), &TypeEnv::default()),
            None
        );
    }

    #[test]
    fn native_system_print_statement_keeps_jdk_print_stream_owner() {
        let call = Expr::Call {
            receiver: Some(Box::new(Expr::FieldAccess(
                Box::new(Expr::Var("System".to_string())),
                "out".to_string(),
            ))),
            name: "println".to_string(),
            args: vec![Expr::Literal(Literal::String("x".to_string()))],
            resolved: Some(resolved_instance_call("PrintStream", "println")),
        };

        assert!(matches!(
            plan_call_statement(&call, &FactSet::default(), &TypeEnv::default()),
            Some(CallStatementPlan::NativeSystemPrintStream {
                stream: NativeSystemStream::Out,
                method: NativePrintStreamMethod::Println,
                ..
            })
        ));
    }

    #[test]
    fn external_call_thrown_effect_receiver_lookup_uses_rust_method_name() {
        let parsed = crate::external_packs::parse_external_pack_manifest(
            r#"
            external-pack dep.io dep.io rust-crate-mapping mapped target=dep_io
            external-pack-exception dep.io dep.io.DepIoError checked
            external-pack-api dep.io dep.io.Reader readRecord mapped getter read-only definitely-non-null args=0 return=dep.io.Record throws=dep.io.DepIoError rust=read_record
            "#,
        );
        assert!(
            parsed.diagnostics().is_empty(),
            "manifest parse should be clean: {:?}",
            parsed.diagnostics()
        );

        assert!(external_call_has_thrown_effect_for_receiver(
            parsed.facts(),
            "dep.io.Reader",
            "read_record",
            0,
        ));
    }

    #[test]
    fn erased_object_facade_method_return_uses_unique_typed_erased_facade() {
        let mut facts = FactSet::default();
        facts
            .hierarchy_overrides
            .push(crate::facts::HierarchyOverride {
                type_name: "ExternalFactory.Product.Builder".to_string(),
                strategy: HierarchyStrategy::ErasedObject,
                provenance: FactProvenance::ExternalLibraryPack,
            });
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: "toBuilder".to_string(),
            receiver_type: Some("Product".to_string()),
            arg_count: Some(0),
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::Named {
                name: "Product.Builder".to_string(),
                args: Vec::new(),
            }),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });

        assert!(matches!(
            erased_object_facade_method_return_lowered_type("toBuilder", 0, &facts),
            Some(LoweredType::RustOwned { .. })
        ));
    }

    #[test]
    fn object_receiver_external_call_lowered_type_uses_erased_facade_method_return() {
        let mut facts = FactSet::default();
        facts
            .hierarchy_overrides
            .push(crate::facts::HierarchyOverride {
                type_name: "ExternalFactory.Product.Builder".to_string(),
                strategy: HierarchyStrategy::ErasedObject,
                provenance: FactProvenance::ExternalLibraryPack,
            });
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: "toBuilder".to_string(),
            receiver_type: Some("Product".to_string()),
            arg_count: Some(0),
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::Named {
                name: "Product.Builder".to_string(),
                args: Vec::new(),
            }),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
        let external_index = facts.external_lookup_index();
        let mut types = TypeEnv::new();
        types.insert_local(
            "value",
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
        );

        assert!(matches!(
            external_call_lowered_type_with_external_index(
                Some(&Expr::Var("value".to_string())),
                "toBuilder",
                0,
                &facts,
                &external_index,
                &types,
                None,
            ),
            Some(LoweredType::RustOwned { .. })
        ));
    }

    #[test]
    fn external_call_result_wrapper_follows_facade_abi_policy() {
        let mut facts = FactSet::default();
        facts.external_api_effect_facts.push(ExternalApiEffectFact {
            method_name: "read".to_string(),
            receiver_type: Some("Reader".to_string()),
            arg_count: Some(0),
            parameter_patterns: None,
            role: ExternalApiRole::Getter,
            receiver_mutability: ReceiverMutabilityFact::ReadOnly,
            return_type: Some(JavaType::String),
            return_nullability: NullabilityState::DefinitelyNonNull,
            thrown: Vec::new(),
            provenance: FactProvenance::ExternalLibraryPack,
        });
        facts.facade_abi_facts.push(FacadeAbiFact {
            key: CallableKey::method("Reader", "read", Staticness::Unknown, 0),
            rust_path: "reader_facade::Reader::read".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: Vec::new(),
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::ResultWrapped,
            borrow_source: None,
            borrowed_return_recovery: None,
            provenance: FactProvenance::ExternalLibraryPack,
        });
        let mut types = TypeEnv::new();
        types.insert_local(
            "reader",
            JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            },
        );

        assert!(external_call_returns_rust_result(
            Some(&Expr::Var("reader".to_string())),
            "read",
            0,
            &facts,
            &types,
            Some("Owner"),
        ));

        facts.external_api_effect_facts[0].method_name = "getName".to_string();
        facts.external_api_effect_facts[0].thrown = vec![JavaType::Named {
            name: "IOException".to_string(),
            args: Vec::new(),
        }];
        facts.facade_abi_facts[0] = FacadeAbiFact {
            key: CallableKey::method("Reader", "getName", Staticness::Unknown, 0),
            rust_path: "reader_facade::Reader::get_name".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: Vec::new(),
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::Plain,
            borrow_source: None,
            borrowed_return_recovery: None,
            provenance: FactProvenance::ExternalLibraryPack,
        };

        assert!(!external_call_returns_rust_result(
            Some(&Expr::Var("reader".to_string())),
            "getName",
            0,
            &facts,
            &types,
            Some("Owner"),
        ));
    }

    #[test]
    fn stdlib_external_calls_report_result_wrappers_for_fallible_facades() {
        let facts = crate::stdlib::rust_stdlib_signature_facts();
        let mut types = TypeEnv::new();
        types.insert_static_type_alias("Files", "java.nio.file.Files");
        types.insert_local(
            "path",
            JavaType::Named {
                name: "Path".to_string(),
                args: Vec::new(),
            },
        );
        assert!(external_call_returns_rust_result(
            Some(&Expr::Var("Files".to_string())),
            "readAllLines",
            1,
            &facts,
            &types,
            Some("UseFilesReadAbi"),
        ));

        types.insert_local(
            "input",
            JavaType::Named {
                name: "InputStream".to_string(),
                args: Vec::new(),
            },
        );
        types.insert_local(
            "output",
            JavaType::Named {
                name: "BufferedOutputStream".to_string(),
                args: Vec::new(),
            },
        );
        assert!(external_call_returns_rust_result(
            Some(&Expr::Var("input".to_string())),
            "read",
            3,
            &facts,
            &types,
            None,
        ));
        assert!(external_call_returns_rust_result(
            Some(&Expr::Var("output".to_string())),
            "write",
            3,
            &facts,
            &types,
            None,
        ));
        assert!(external_call_returns_rust_result(
            Some(&Expr::Var("output".to_string())),
            "write",
            1,
            &facts,
            &types,
            None,
        ));
    }

    #[test]
    fn facade_receiver_abi_maps_to_receiver_slot_shapes() {
        let facts = FactSet::default();
        let reader_ty = JavaType::Named {
            name: "Reader".to_string(),
            args: Vec::new(),
        };

        let borrowed =
            facade_receiver_slot_plan(ReceiverAbi::BorrowedSelf, reader_ty.clone(), &facts);
        assert_eq!(borrowed.receiver_abi(), ReceiverAbi::BorrowedSelf);
        let Some(Slot::Argument(borrowed_shape)) = borrowed.slot() else {
            panic!("borrowed receiver should have an argument slot");
        };
        assert_eq!(borrowed_shape.ownership, OwnershipShape::SharedBorrow);
        assert_eq!(borrowed_shape.java_ty, reader_ty);

        let mutable = facade_receiver_slot_plan(
            ReceiverAbi::MutBorrowedSelf,
            JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            },
            &facts,
        );
        assert_eq!(mutable.receiver_abi(), ReceiverAbi::MutBorrowedSelf);
        let Some(Slot::Argument(mutable_shape)) = mutable.slot() else {
            panic!("mutable receiver should have an argument slot");
        };
        assert_eq!(mutable_shape.ownership, OwnershipShape::MutableBorrow);
        assert_eq!(mutable_shape.mutability, MutabilityShape::Mutable);

        let associated = facade_receiver_slot_plan(
            ReceiverAbi::AssociatedFunction,
            JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            },
            &facts,
        );
        assert_eq!(associated.receiver_abi(), ReceiverAbi::AssociatedFunction);
        assert_eq!(associated.slot(), None);
    }

    #[test]
    fn optional_receiver_slot_plan_maps_mutability_and_projection() {
        let facts = FactSet::default();
        let reader_ty = JavaType::Named {
            name: "Reader".to_string(),
            args: Vec::new(),
        };

        let shared = optional_receiver_slot_plan(reader_ty.clone(), false, true, &facts);
        assert_eq!(shared.receiver_abi(), ReceiverAbi::BorrowedSelf);
        assert_eq!(
            shared.projection(),
            OptionalReceiverProjection::SharedUnchecked
        );
        let Slot::Argument(shared_shape) = shared.slot() else {
            panic!("shared optional receiver should have an argument slot");
        };
        assert_eq!(shared_shape.java_ty, reader_ty);
        assert_eq!(shared_shape.ownership, OwnershipShape::SharedBorrow);
        assert_eq!(shared.element_ownership(), OwnershipShape::SharedBorrow);
        let projection_plan = shared.projection_plan_for_option_shape(
            RustSlotShape::conservative(optional_java_type(reader_ty.clone())),
            &facts,
        );
        let element_shape = projection_plan
            .element_shape()
            .expect("shared optional receiver should plan an optional element borrow");
        assert_eq!(element_shape.java_ty, reader_ty);
        assert_eq!(element_shape.ownership, OwnershipShape::SharedBorrow);

        let mutable = optional_receiver_slot_plan(
            JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            },
            true,
            false,
            &facts,
        );
        assert_eq!(mutable.receiver_abi(), ReceiverAbi::MutBorrowedSelf);
        assert_eq!(mutable.projection(), OptionalReceiverProjection::Mutable);
        let Slot::Argument(mutable_shape) = mutable.slot() else {
            panic!("mutable optional receiver should have an argument slot");
        };
        assert_eq!(mutable_shape.ownership, OwnershipShape::MutableBorrow);
        assert_eq!(mutable_shape.mutability, MutabilityShape::Mutable);
        assert_eq!(mutable.element_ownership(), OwnershipShape::MutableBorrow);
        let projection_plan = mutable.projection_plan_for_option_shape(
            RustSlotShape::conservative(optional_java_type(JavaType::Named {
                name: "Reader".to_string(),
                args: Vec::new(),
            })),
            &facts,
        );
        let element_shape = projection_plan
            .element_shape()
            .expect("mutable optional receiver should plan an optional element borrow");
        assert_eq!(element_shape.ownership, OwnershipShape::MutableBorrow);

        let borrowed = optional_borrowed_receiver_slot_plan(reader_ty.clone(), false, &facts);
        assert_eq!(borrowed.receiver_abi(), ReceiverAbi::BorrowedSelf);
        assert_eq!(borrowed.projection(), OptionalReceiverProjection::Borrowed);
        assert_eq!(borrowed.element_ownership(), OwnershipShape::SharedBorrow);
        let Slot::Argument(borrowed_shape) = borrowed.slot() else {
            panic!("borrowed optional receiver should have an argument slot");
        };
        assert_eq!(borrowed_shape.java_ty, reader_ty);
        assert_eq!(borrowed_shape.ownership, OwnershipShape::SharedBorrow);
    }

    #[test]
    fn native_queue_receiver_kind_is_the_single_queue_capability_answer() {
        let queue_kind = |name: &str| {
            native_queue_receiver_kind(&JavaType::Named {
                name: name.to_string(),
                args: Vec::new(),
            })
        };
        assert_eq!(
            queue_kind("java.util.PriorityQueue"),
            Some(NativeQueueReceiverKind::PriorityQueue)
        );
        assert_eq!(
            queue_kind("java.util.concurrent.ArrayBlockingQueue"),
            Some(NativeQueueReceiverKind::ArrayBlockingQueue)
        );
        assert_eq!(
            queue_kind("java.util.ArrayDeque"),
            Some(NativeQueueReceiverKind::ArrayDeque)
        );
        assert_eq!(
            queue_kind("java.util.LinkedList"),
            Some(NativeQueueReceiverKind::LinkedList)
        );
        assert_eq!(queue_kind("java.util.ArrayList"), None);

        assert!(
            !NativeQueueReceiverKind::PriorityQueue.offer_always_inserts(),
            "PriorityQueue offer has ordering semantics, not the generic alias"
        );
        assert!(
            !NativeQueueReceiverKind::ArrayBlockingQueue.offer_always_inserts(),
            "bounded offer needs capacity support"
        );
        assert!(NativeQueueReceiverKind::ArrayDeque.offer_always_inserts());
        assert!(NativeQueueReceiverKind::LinkedList.offer_always_inserts());

        let receiver = Expr::Var("queue".to_string());
        let args = vec![Expr::Literal(Literal::Int(1))];
        let mut types = TypeEnv::new();
        types.insert_local(
            "queue",
            JavaType::Named {
                name: "java.util.PriorityQueue".to_string(),
                args: vec![JavaType::Primitive(PrimitiveType::Int)],
            },
        );
        assert!(matches!(
            plan_native_collection_receiver_call(&receiver, "add", Some(&args), &types),
            Some(CallTargetPlan::NativeCollectionMethod {
                method: NativeCollectionMethod::PriorityQueueAdd,
                ..
            })
        ));
        assert!(matches!(
            plan_native_collection_receiver_call(&receiver, "offer", Some(&args), &types),
            Some(CallTargetPlan::NativeCollectionMethod {
                method: NativeCollectionMethod::PriorityQueueAdd,
                ..
            })
        ));
        assert!(matches!(
            plan_native_collection_receiver_call(&receiver, "poll", Some(&[]), &types),
            Some(CallTargetPlan::NativeCollectionMethod {
                method: NativeCollectionMethod::QueuePoll,
                ..
            })
        ));
        let receiver_ty = JavaType::Named {
            name: "java.util.PriorityQueue".to_string(),
            args: vec![JavaType::Primitive(PrimitiveType::Int)],
        };
        let comparator_arg = vec![Expr::Var("comparator".to_string())];
        let comparator_ty = JavaType::Named {
            name: "java.util.Comparator".to_string(),
            args: vec![JavaType::Primitive(PrimitiveType::Int)],
        };
        types.insert_local("comparator", comparator_ty.clone());
        assert!(matches!(
            plan_native_collection_constructor_for_args(
                &receiver_ty,
                &comparator_arg,
                &types,
                &FactSet::default(),
            ),
            Some(NativeCollectionNewTarget::PriorityQueueComparatorUnsupported { .. })
        ));
        types.mark_callable_local("comparator");
        assert!(matches!(
            plan_native_collection_constructor_for_args(
                &receiver_ty,
                &comparator_arg,
                &types,
                &FactSet::default(),
            ),
            Some(NativeCollectionNewTarget::PriorityQueueComparatorCallback { .. })
        ));
        types.insert_param("param_comparator", comparator_ty);
        let comparator_param_arg = vec![Expr::Var("param_comparator".to_string())];
        assert!(matches!(
            plan_native_collection_constructor_for_args(
                &receiver_ty,
                &comparator_param_arg,
                &types,
                &FactSet::default(),
            ),
            Some(NativeCollectionNewTarget::PriorityQueueComparatorCallback { .. })
        ));
        assert!(matches!(
            native_collection_method_arg_shapes(
                &receiver_ty,
                NativeCollectionMethod::QueuePoll,
                0,
                &FactSet::default(),
            )
            .as_deref(),
            Some([])
        ));

        types.insert_local(
            "blocking",
            JavaType::Named {
                name: "java.util.concurrent.ArrayBlockingQueue".to_string(),
                args: vec![JavaType::String],
            },
        );
        let blocking = Expr::Var("blocking".to_string());
        let value_arg = vec![Expr::Literal(Literal::String("x".to_string()))];
        assert!(matches!(
            plan_native_collection_receiver_call(&blocking, "put", Some(&value_arg), &types),
            Some(CallTargetPlan::NativeCollectionMethod {
                method: NativeCollectionMethod::ArrayBlockingQueuePut,
                ..
            })
        ));
    }

    #[test]
    fn resolved_constructor_name_is_a_member_not_a_rendered_path() {
        assert_eq!(constructor_member_rust_name("new"), "new");
        assert_eq!(constructor_member_rust_name("owner::new_1"), "new_1");
        assert_eq!(
            constructor_member_rust_name("std::rc::Rc::<Wrapper>::new"),
            "new"
        );
    }

    #[test]
    fn fallback_rust_name_adapters_preserve_overload_boundaries() {
        let receiver = crate::java_ir::Expr::Var("out".to_string());
        let overload_safe_target = apply_fallback_receiver_rust_name(
            CallExprPlan::Target(CallTargetPlan::FallbackReceiver {
                receiver: &receiver,
                rust_name: "println_1".to_string(),
            }),
            Some("println".to_string()),
        );
        assert!(matches!(
            overload_safe_target,
            CallExprPlan::Target(CallTargetPlan::FallbackReceiver {
                rust_name,
                ..
            }) if rust_name == "println_1"
        ));

        let typed_static_overload = apply_fallback_static_owner_rust_name(
            CallExprPlan::Target(CallTargetPlan::StaticOwner {
                owner: "Util".to_string(),
                rust_name: "pick_1".to_string(),
            }),
            Some("pick_int".to_string()),
        );
        assert!(matches!(
            typed_static_overload,
            CallExprPlan::Target(CallTargetPlan::StaticOwner { rust_name, .. })
                if rust_name == "pick_int"
        ));

        let bare_static_overload = apply_fallback_static_owner_rust_name(
            CallExprPlan::Target(CallTargetPlan::StaticOwner {
                owner: "Util".to_string(),
                rust_name: "pick_1".to_string(),
            }),
            Some("pick".to_string()),
        );
        assert!(matches!(
            bare_static_overload,
            CallExprPlan::Target(CallTargetPlan::StaticOwner { rust_name, .. })
                if rust_name == "pick_1"
        ));

        let fallback_self = apply_fallback_self_rust_name(
            CallExprPlan::Target(CallTargetPlan::InstanceSelf { rust_name: "read" }),
            Some("read_1".to_string()),
        );
        assert!(matches!(
            fallback_self,
            CallExprPlan::Target(CallTargetPlan::FallbackSelf { rust_name })
                if rust_name == "read_1"
        ));
    }

    #[test]
    fn platform_display_type_to_string_uses_object_display_plan() {
        let receiver = Expr::Var("value".to_string());
        let mut types = TypeEnv::new();
        types.insert_local(
            "value",
            JavaType::Named {
                name: "java.util.Random".to_string(),
                args: Vec::new(),
            },
        );

        assert!(matches!(
            plan_call_target_with_facts(
                Some(&receiver),
                "toString",
                None,
                &FactSet::default(),
                &types
            ),
            CallTargetPlan::NativeJavaInstance {
                method: NativeJavaInstanceMethod::ObjectToString,
                ..
            }
        ));
    }

    #[test]
    fn fallback_static_owner_receiver_uses_type_spelling_not_value_module() {
        assert!(matches!(
            plan_call_target_with_facts_and_arity(
                Some(&Expr::Var("product".to_string())),
                "newBuilder",
                0,
                None,
                &FactSet::default(),
                &TypeEnv::default(),
            ),
            CallTargetPlan::StaticOwner {
                ref owner,
                ref rust_name,
            } if owner == "Product" && rust_name == "new_builder"
        ));
    }

    #[test]
    fn native_java_instance_receiver_type_hint_names_string_receivers() {
        assert_eq!(
            native_java_instance_receiver_type_hint("trim", 0),
            Some(JavaType::String)
        );
        assert_eq!(native_java_instance_receiver_type_hint("trim", 1), None);
    }
}
