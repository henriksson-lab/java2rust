use crate::effective_facts::EffectiveFacts;
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, infer_assignment_target_shape_with_context,
    infer_expr_shape_with_context,
};
use crate::facts::{FactSet, OwnedRecoveryCapability};
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::shape_projection::{borrowed_sequence_element_shape, project_java_value_shape};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{list_java_type, optional_java_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    Value,
    Condition,
    Index,
    Assignment(RustSlotShape),
    Return(RustSlotShape),
    Argument(RustSlotShape),
    StringConcatSegment,
}

pub fn assignment_slot(shape: RustSlotShape) -> Slot {
    Slot::Assignment(shape)
}

pub fn argument_slot(shape: RustSlotShape) -> Slot {
    Slot::Argument(shape)
}

pub fn condition_slot() -> Slot {
    Slot::Condition
}

pub fn index_slot() -> Slot {
    Slot::Index
}

pub fn assignment_slot_for_target(
    target: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Slot {
    infer_assignment_target_shape_with_context(target, env, ctx)
        .map(assignment_slot)
        .unwrap_or(Slot::Value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentValuePlan {
    pub slot: Slot,
}

pub fn assignment_value_plan_for_shape(shape: &RustSlotShape) -> AssignmentValuePlan {
    AssignmentValuePlan {
        slot: assignment_slot(shape.clone()),
    }
}

pub fn assignment_value_plan_for_target(
    target: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> AssignmentValuePlan {
    AssignmentValuePlan {
        slot: assignment_slot_for_target(target, env, ctx),
    }
}

pub fn return_slot(shape: Option<&RustSlotShape>) -> Option<Slot> {
    shape.cloned().map(Slot::Return)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnValuePlan {
    Slot(Slot),
    Raw,
}

pub fn return_value_plan(shape: Option<&RustSlotShape>) -> ReturnValuePlan {
    return_slot(shape)
        .map(ReturnValuePlan::Slot)
        .unwrap_or(ReturnValuePlan::Raw)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxSlotCoercionPolicy {
    Identity,
    IndexToUsize,
    ToStringForConcat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewCoercionPolicy {
    NoViewChange,
    BorrowStringView,
    OwnStringFromView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloneRecoveryPolicy {
    AuditRequired,
    ProvenCloneCapability { java_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipCoercionFallbackPolicy {
    CompileError,
    BorrowedToOwnedNonCloneable { java_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipCoercionPolicy {
    NoOwnershipChange,
    BorrowOwnedValue,
    RecoverOwnedCopyValue,
    RecoverOwnedWithClone {
        policy: CloneRecoveryPolicy,
    },
    UnsupportedRecovery {
        actual_ownership: OwnershipShape,
        target_ownership: OwnershipShape,
        policy: OwnershipCoercionFallbackPolicy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidCastPolicy {
    Panic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfFallbackPolicy {
    CompileError,
    MissingRuntimeTarget { java_ty: JavaType },
    ErasedObjectToDynTrait { trait_name: String },
    ErasedObjectToBoxedDynTrait { trait_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchCoercionFallbackPolicy {
    CompileError,
    MissingRuntimeTarget {
        java_ty: JavaType,
    },
    BorrowedDynToOwnedBoxedDyn {
        trait_name: String,
        recovery: BorrowedDynToOwnedBoxedDynRecoveryPolicy,
    },
    ErasedObjectToDynTrait {
        trait_name: String,
    },
    ErasedObjectToBoxedDynTrait {
        trait_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedDynToOwnedBoxedDynRecoveryPolicy {
    NoOwnershipProof,
    CloneBoxAdapterRequired { policy: CloneRecoveryPolicy },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchCoercionPolicy {
    NoDispatchChange,
    WrapClosedEnum {
        enum_name: String,
        variant_name: String,
    },
    BorrowDynTrait {
        trait_name: String,
    },
    BorrowBoxedDynTrait {
        trait_name: String,
    },
    BoxDynTrait {
        trait_name: String,
    },
    EraseObject,
    DowncastClosedEnum {
        enum_name: String,
        variant_name: String,
        invalid_cast: InvalidCastPolicy,
    },
    DowncastDynTrait {
        target_type: String,
        invalid_cast: InvalidCastPolicy,
    },
    DowncastBoxedDynTrait {
        target_type: String,
        invalid_cast: InvalidCastPolicy,
    },
    DowncastErasedObject {
        target_type: String,
        invalid_cast: InvalidCastPolicy,
    },
    Unsupported {
        actual_dispatch: DispatchShape,
        target_dispatch: DispatchShape,
        policy: DispatchCoercionFallbackPolicy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionalShapeCoercionPolicy {
    Wrap {
        element_ty: JavaType,
        element_plan: Box<ShapeCoercionPlan>,
    },
    Unwrap {
        element_ty: JavaType,
        element_plan: Box<ShapeCoercionPlan>,
    },
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionOwnedRecoveryCapabilityPolicy {
    pub java_ty: JavaType,
    pub recovery: OwnedRecoveryCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedElementRecoveryFallbackPolicy {
    CompileError,
    NonCloneableElement { java_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedElementRecoveryPolicy {
    StringViewToOwned,
    CopyElements,
    CloneElements {
        policy: CloneRecoveryPolicy,
    },
    Unsupported {
        policy: BorrowedElementRecoveryFallbackPolicy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalElementTargetMatchPolicy {
    pub target: RustSlotShape,
    pub option_shape: RustSlotShape,
    pub element_shape: RustSlotShape,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecElementTargetMatchPolicy {
    pub target: RustSlotShape,
    pub vec_shape: RustSlotShape,
    pub element_shape: RustSlotShape,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredType {
    Slot(Box<RustSlotShape>),
    OptionalElementBorrow {
        option_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
    VecElementBorrow {
        vec_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
}

impl LoweredType {
    pub fn from_java(java_ty: JavaType) -> Self {
        Self::Slot(Box::new(RustSlotShape::conservative(java_ty)))
    }

    pub fn optional_element_borrow(element_ty: JavaType) -> Self {
        Self::optional_element_borrow_shape(borrowed_sequence_element_shape(element_ty))
    }

    pub fn optional_element_borrow_shape(element_shape: RustSlotShape) -> Self {
        let option_shape =
            RustSlotShape::conservative(optional_java_type(element_shape.java_ty.clone()));
        Self::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn vec_element_borrow(element_shape: RustSlotShape) -> Self {
        let vec_shape = RustSlotShape::conservative(list_java_type(element_shape.java_ty.clone()));
        Self::VecElementBorrow {
            vec_shape: Box::new(vec_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn collected_sequence_from_element_shape(element_shape: RustSlotShape) -> Self {
        if element_shape.ownership == OwnershipShape::SharedBorrow {
            Self::vec_element_borrow(element_shape)
        } else {
            Self::from_java(list_java_type(element_shape.java_ty))
        }
    }

    pub fn java_ty(&self) -> Option<&JavaType> {
        match self {
            LoweredType::Slot(shape) => Some(&shape.java_ty),
            LoweredType::OptionalElementBorrow { option_shape, .. } => Some(&option_shape.java_ty),
            LoweredType::VecElementBorrow { vec_shape, .. } => Some(&vec_shape.java_ty),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedElementCarrier {
    Option,
    Vec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustNominalTypeNamePolicy {
    pub java_ty: JavaType,
    pub target_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTypeTargetPolicy {
    Nominal { target_type: String },
    MissingNominalTarget { java_ty: JavaType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveWideningPolicy {
    pub from: PrimitiveType,
    pub to: PrimitiveType,
    pub from_rank: u8,
    pub to_rank: u8,
    pub widens: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveRankPolicy {
    pub primitive: PrimitiveType,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionContentPolicy {
    pub java_ty: JavaType,
    pub string_content: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionShapeContentPolicy {
    pub java_ty: JavaType,
    pub string_content: bool,
}

pub fn rust_nominal_type_name_policy(java_ty: &JavaType) -> RustNominalTypeNamePolicy {
    let target_type = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.clone()),
        JavaType::String => Some("String".to_string()),
        _ => None,
    };
    RustNominalTypeNamePolicy {
        java_ty: java_ty.clone(),
        target_type,
    }
}

pub fn runtime_type_target_policy(java_ty: &JavaType) -> RuntimeTypeTargetPolicy {
    match rust_nominal_type_name_policy(java_ty).target_type {
        Some(target_type) => RuntimeTypeTargetPolicy::Nominal { target_type },
        None => RuntimeTypeTargetPolicy::MissingNominalTarget {
            java_ty: java_ty.clone(),
        },
    }
}

pub fn primitive_widening_policy(
    actual: &JavaType,
    target: &JavaType,
) -> Option<PrimitiveWideningPolicy> {
    let (JavaType::Primitive(from), JavaType::Primitive(to)) = (actual, target) else {
        return None;
    };
    let from_rank = primitive_rank_policy(*from).rank;
    let to_rank = primitive_rank_policy(*to).rank;
    Some(PrimitiveWideningPolicy {
        from: *from,
        to: *to,
        from_rank,
        to_rank,
        widens: from != to && from_rank < to_rank,
    })
}

pub fn primitive_rank_policy(primitive: PrimitiveType) -> PrimitiveRankPolicy {
    let rank = match primitive {
        PrimitiveType::Boolean => 0,
        PrimitiveType::Byte => 1,
        PrimitiveType::Short | PrimitiveType::Char => 2,
        PrimitiveType::Int => 3,
        PrimitiveType::Long => 4,
        PrimitiveType::Float => 5,
        PrimitiveType::Double => 6,
    };
    PrimitiveRankPolicy { primitive, rank }
}

pub fn coercion_content_policy(java_ty: &JavaType, facts: &FactSet) -> CoercionContentPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    coercion_content_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn coercion_content_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> CoercionContentPolicy {
    CoercionContentPolicy {
        java_ty: java_ty.clone(),
        string_content: facts.is_string_like_type(java_ty),
    }
}

pub fn coercion_shape_content_policy(
    shape: &RustSlotShape,
    facts: &FactSet,
) -> CoercionShapeContentPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    coercion_shape_content_policy_with_effective_facts(shape, &effective_facts)
}

pub fn coercion_shape_content_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> CoercionShapeContentPolicy {
    let content = coercion_content_policy_with_effective_facts(&shape.java_ty, facts);
    CoercionShapeContentPolicy {
        java_ty: content.java_ty,
        string_content: content.string_content,
    }
}

pub fn view_coercion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> ViewCoercionPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    view_coercion_policy_with_effective_facts(actual, target, &effective_facts)
}

pub fn view_coercion_policy_with_effective_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &EffectiveFacts,
) -> ViewCoercionPolicy {
    if !coercion_shape_content_policy_with_effective_facts(actual, facts).string_content
        || !coercion_shape_content_policy_with_effective_facts(target, facts).string_content
    {
        return ViewCoercionPolicy::NoViewChange;
    }
    if actual.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::SharedBorrow
        && target.view == ViewShape::Str
    {
        return ViewCoercionPolicy::BorrowStringView;
    }
    if actual.ownership == OwnershipShape::SharedBorrow
        && actual.view == ViewShape::Str
        && target.ownership == OwnershipShape::Owned
        && target.view == ViewShape::Native
    {
        return ViewCoercionPolicy::OwnStringFromView;
    }
    ViewCoercionPolicy::NoViewChange
}

pub fn coercion_owned_recovery_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> CoercionOwnedRecoveryCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    coercion_owned_recovery_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn coercion_owned_recovery_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> CoercionOwnedRecoveryCapabilityPolicy {
    CoercionOwnedRecoveryCapabilityPolicy {
        java_ty: java_ty.clone(),
        recovery: facts.owned_recovery_capability_for(java_ty),
    }
}

pub fn ownership_coercion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> OwnershipCoercionPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    ownership_coercion_policy_with_effective_facts(actual, target, &effective_facts)
}

pub fn ownership_coercion_policy_with_effective_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &EffectiveFacts,
) -> OwnershipCoercionPolicy {
    if actual.ownership == target.ownership {
        return OwnershipCoercionPolicy::NoOwnershipChange;
    }
    if actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::SharedBorrow
        && actual.view == target.view
        && actual.dispatch == target.dispatch
    {
        return OwnershipCoercionPolicy::BorrowOwnedValue;
    }
    if actual.java_ty == target.java_ty
        && actual.dispatch == target.dispatch
        && actual.view == target.view
        && actual.ownership == OwnershipShape::SharedBorrow
        && target.ownership == OwnershipShape::Owned
    {
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        ownership_recovery_policy_from_capability(capability.recovery, actual, target)
    } else {
        OwnershipCoercionPolicy::NoOwnershipChange
    }
}

fn ownership_recovery_policy_from_capability(
    capability: OwnedRecoveryCapability,
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> OwnershipCoercionPolicy {
    match capability {
        OwnedRecoveryCapability::Copy => OwnershipCoercionPolicy::RecoverOwnedCopyValue,
        OwnedRecoveryCapability::ExplicitClone { java_ty } => {
            OwnershipCoercionPolicy::RecoverOwnedWithClone {
                policy: clone_recovery_policy_from_capability(
                    OwnedRecoveryCapability::ExplicitClone { java_ty },
                ),
            }
        }
        OwnedRecoveryCapability::ConservativeClone => {
            OwnershipCoercionPolicy::RecoverOwnedWithClone {
                policy: clone_recovery_policy_from_capability(
                    OwnedRecoveryCapability::ConservativeClone,
                ),
            }
        }
        OwnedRecoveryCapability::NonCloneable => OwnershipCoercionPolicy::UnsupportedRecovery {
            actual_ownership: actual.ownership,
            target_ownership: target.ownership,
            policy: OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable {
                java_ty: actual.java_ty.clone(),
            },
        },
    }
}

pub fn clone_recovery_policy_from_capability(
    capability: OwnedRecoveryCapability,
) -> CloneRecoveryPolicy {
    match capability {
        OwnedRecoveryCapability::ExplicitClone { java_ty } => {
            CloneRecoveryPolicy::ProvenCloneCapability { java_ty }
        }
        OwnedRecoveryCapability::ConservativeClone => CloneRecoveryPolicy::AuditRequired,
        OwnedRecoveryCapability::Copy | OwnedRecoveryCapability::NonCloneable => {
            CloneRecoveryPolicy::AuditRequired
        }
    }
}

pub fn optional_shape_coercion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> OptionalShapeCoercionPolicy {
    optional_shape_coercion_policy_with_facts(actual, target, &FactSet::default())
}

pub fn optional_shape_coercion_policy_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> OptionalShapeCoercionPolicy {
    if actual.dispatch != target.dispatch || actual.view != target.view {
        return OptionalShapeCoercionPolicy::NoChange;
    }
    if let Some(element_ty) = target.optional_element_type()
        && element_ty == actual.java_ty
    {
        let target_element = optional_element_slot_shape(target, element_ty);
        return OptionalShapeCoercionPolicy::Wrap {
            element_ty: actual.java_ty.clone(),
            element_plan: Box::new(shape_coercion_plan_with_facts(
                actual,
                &target_element,
                facts,
            )),
        };
    }
    if let Some(element_ty) = actual.optional_element_type()
        && element_ty == target.java_ty
    {
        let actual_element = optional_element_slot_shape(actual, element_ty);
        return OptionalShapeCoercionPolicy::Unwrap {
            element_ty: target.java_ty.clone(),
            element_plan: Box::new(shape_coercion_plan_with_facts(
                &actual_element,
                target,
                facts,
            )),
        };
    }
    OptionalShapeCoercionPolicy::NoChange
}

fn optional_element_slot_shape(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
) -> RustSlotShape {
    let mut element_shape = option_shape.clone();
    element_shape.java_ty = element_ty;
    element_shape
}

pub fn optional_element_target_match_policy(
    target: &RustSlotShape,
    option_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> OptionalElementTargetMatchPolicy {
    OptionalElementTargetMatchPolicy {
        target: target.clone(),
        option_shape: option_shape.clone(),
        element_shape: element_shape.clone(),
        matches: target.java_ty == option_shape.java_ty && target.is_optional_of(element_shape),
    }
}

pub fn vec_element_target_match_policy(
    target: &RustSlotShape,
    vec_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> VecElementTargetMatchPolicy {
    VecElementTargetMatchPolicy {
        target: target.clone(),
        vec_shape: vec_shape.clone(),
        element_shape: element_shape.clone(),
        matches: target.java_ty == vec_shape.java_ty && target.is_sequence_of(element_shape),
    }
}

pub fn borrowed_element_recovery_policy(
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> BorrowedElementRecoveryPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_element_recovery_policy_with_effective_facts(element_shape, &effective_facts)
}

pub fn borrowed_element_recovery_policy_with_effective_facts(
    element_shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> BorrowedElementRecoveryPolicy {
    if coercion_shape_content_policy_with_effective_facts(element_shape, facts).string_content
        && element_shape.ownership == OwnershipShape::SharedBorrow
        && element_shape.view == ViewShape::Str
    {
        BorrowedElementRecoveryPolicy::StringViewToOwned
    } else {
        let capability = coercion_owned_recovery_capability_policy_with_effective_facts(
            &element_shape.java_ty,
            facts,
        );
        borrowed_element_recovery_policy_from_capability(capability.recovery, element_shape)
    }
}

fn borrowed_element_recovery_policy_from_capability(
    capability: OwnedRecoveryCapability,
    element_shape: &RustSlotShape,
) -> BorrowedElementRecoveryPolicy {
    match capability {
        OwnedRecoveryCapability::Copy => BorrowedElementRecoveryPolicy::CopyElements,
        OwnedRecoveryCapability::ExplicitClone { java_ty } => {
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: clone_recovery_policy_from_capability(
                    OwnedRecoveryCapability::ExplicitClone { java_ty },
                ),
            }
        }
        OwnedRecoveryCapability::ConservativeClone => {
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: clone_recovery_policy_from_capability(
                    OwnedRecoveryCapability::ConservativeClone,
                ),
            }
        }
        OwnedRecoveryCapability::NonCloneable => BorrowedElementRecoveryPolicy::Unsupported {
            policy: BorrowedElementRecoveryFallbackPolicy::NonCloneableElement {
                java_ty: element_shape.java_ty.clone(),
            },
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedElementAdapterPolicy {
    pub carrier: BorrowedElementCarrier,
    pub element_shape: RustSlotShape,
    pub recovery: BorrowedElementRecoveryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedElementSlotAdapterPolicy {
    pub target: RustSlotShape,
    pub carrier_shape: RustSlotShape,
    pub element_shape: RustSlotShape,
    pub carrier: BorrowedElementCarrier,
    pub matches: bool,
    pub adapter: Option<BorrowedElementAdapterPolicy>,
}

pub fn borrowed_element_adapter_policy(
    carrier: BorrowedElementCarrier,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> BorrowedElementAdapterPolicy {
    BorrowedElementAdapterPolicy {
        carrier,
        element_shape: element_shape.clone(),
        recovery: borrowed_element_recovery_policy(element_shape, facts),
    }
}

pub fn borrowed_element_slot_adapter_policy(
    carrier: BorrowedElementCarrier,
    target: &RustSlotShape,
    carrier_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> BorrowedElementSlotAdapterPolicy {
    let matches = match carrier {
        BorrowedElementCarrier::Option => {
            optional_element_target_match_policy(target, carrier_shape, element_shape).matches
        }
        BorrowedElementCarrier::Vec => {
            vec_element_target_match_policy(target, carrier_shape, element_shape).matches
        }
    };
    let adapter = matches.then(|| borrowed_element_adapter_policy(carrier, element_shape, facts));
    BorrowedElementSlotAdapterPolicy {
        target: target.clone(),
        carrier_shape: carrier_shape.clone(),
        element_shape: element_shape.clone(),
        carrier,
        matches,
        adapter,
    }
}

pub fn dispatch_coercion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> DispatchCoercionPolicy {
    dispatch_coercion_policy_with_facts(actual, target, &FactSet::default())
}

pub fn dispatch_coercion_policy_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> DispatchCoercionPolicy {
    if actual.dispatch == target.dispatch && actual.java_ty == target.java_ty {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::Concrete, DispatchShape::Concrete) => {
            DispatchCoercionPolicy::NoDispatchChange
        }
        (DispatchShape::Concrete, DispatchShape::ClosedEnum { enum_name }) => {
            rust_nominal_type_name_policy(&actual.java_ty)
                .target_type
                .map(|variant_name| DispatchCoercionPolicy::WrapClosedEnum {
                    enum_name: enum_name.clone(),
                    variant_name,
                })
                .unwrap_or_else(|| unsupported_dispatch_coercion_policy(actual, target))
        }
        (DispatchShape::Concrete, DispatchShape::DynTrait { trait_name }) => {
            DispatchCoercionPolicy::BorrowDynTrait {
                trait_name: trait_name.clone(),
            }
        }
        (
            DispatchShape::BoxedDynTrait {
                trait_name: actual_trait,
            },
            DispatchShape::DynTrait {
                trait_name: target_trait,
            },
        ) if actual_trait == target_trait => DispatchCoercionPolicy::BorrowBoxedDynTrait {
            trait_name: target_trait.clone(),
        },
        (DispatchShape::Concrete, DispatchShape::BoxedDynTrait { trait_name }) => {
            DispatchCoercionPolicy::BoxDynTrait {
                trait_name: trait_name.clone(),
            }
        }
        (
            DispatchShape::DynTrait {
                trait_name: actual_trait,
            },
            DispatchShape::BoxedDynTrait {
                trait_name: target_trait,
            },
        ) if actual_trait == target_trait => DispatchCoercionPolicy::Unsupported {
            actual_dispatch: actual.dispatch.clone(),
            target_dispatch: target.dispatch.clone(),
            policy: borrowed_dyn_to_owned_boxed_dyn_fallback_policy(
                target_trait.clone(),
                &actual.java_ty,
                facts,
            ),
        },
        (DispatchShape::Concrete, DispatchShape::ErasedObject) => {
            DispatchCoercionPolicy::EraseObject
        }
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastClosedEnum {
                        enum_name: enum_name.clone(),
                        variant_name: target_type,
                        invalid_cast: InvalidCastPolicy::Panic,
                    }
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    unsupported_dispatch_coercion_policy_with_fallback(
                        actual,
                        target,
                        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty },
                    )
                }
            }
        }
        (DispatchShape::DynTrait { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastDynTrait {
                        target_type,
                        invalid_cast: InvalidCastPolicy::Panic,
                    }
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    unsupported_dispatch_coercion_policy_with_fallback(
                        actual,
                        target,
                        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty },
                    )
                }
            }
        }
        (DispatchShape::BoxedDynTrait { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastBoxedDynTrait {
                        target_type,
                        invalid_cast: InvalidCastPolicy::Panic,
                    }
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    unsupported_dispatch_coercion_policy_with_fallback(
                        actual,
                        target,
                        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty },
                    )
                }
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastErasedObject {
                        target_type,
                        invalid_cast: InvalidCastPolicy::Panic,
                    }
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    unsupported_dispatch_coercion_policy_with_fallback(
                        actual,
                        target,
                        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty },
                    )
                }
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::DynTrait { trait_name }) => {
            DispatchCoercionPolicy::Unsupported {
                actual_dispatch: actual.dispatch.clone(),
                target_dispatch: target.dispatch.clone(),
                policy: DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait {
                    trait_name: trait_name.clone(),
                },
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::BoxedDynTrait { trait_name }) => {
            DispatchCoercionPolicy::Unsupported {
                actual_dispatch: actual.dispatch.clone(),
                target_dispatch: target.dispatch.clone(),
                policy: DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait {
                    trait_name: trait_name.clone(),
                },
            }
        }
        _ => unsupported_dispatch_coercion_policy(actual, target),
    }
}

fn borrowed_dyn_to_owned_boxed_dyn_fallback_policy(
    trait_name: String,
    java_ty: &JavaType,
    facts: &FactSet,
) -> DispatchCoercionFallbackPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    borrowed_dyn_to_owned_boxed_dyn_fallback_policy_with_effective_facts(
        trait_name,
        java_ty,
        &effective_facts,
    )
}

fn borrowed_dyn_to_owned_boxed_dyn_fallback_policy_with_effective_facts(
    trait_name: String,
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> DispatchCoercionFallbackPolicy {
    let capability = coercion_owned_recovery_capability_policy_with_effective_facts(java_ty, facts);
    let recovery = match capability.recovery {
        OwnedRecoveryCapability::ExplicitClone { java_ty } => {
            BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::ProvenCloneCapability { java_ty },
            }
        }
        OwnedRecoveryCapability::ConservativeClone => {
            BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired {
                policy: CloneRecoveryPolicy::AuditRequired,
            }
        }
        OwnedRecoveryCapability::Copy | OwnedRecoveryCapability::NonCloneable => {
            BorrowedDynToOwnedBoxedDynRecoveryPolicy::NoOwnershipProof
        }
    };
    DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
        trait_name,
        recovery,
    }
}

fn unsupported_dispatch_coercion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> DispatchCoercionPolicy {
    unsupported_dispatch_coercion_policy_with_fallback(
        actual,
        target,
        DispatchCoercionFallbackPolicy::CompileError,
    )
}

fn unsupported_dispatch_coercion_policy_with_fallback(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    policy: DispatchCoercionFallbackPolicy,
) -> DispatchCoercionPolicy {
    DispatchCoercionPolicy::Unsupported {
        actual_dispatch: actual.dispatch.clone(),
        target_dispatch: target.dispatch.clone(),
        policy,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeCoercionPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
    pub dispatch: DispatchCoercionPolicy,
    pub view: ViewCoercionPolicy,
    pub optional: OptionalShapeCoercionPolicy,
    pub ownership: OwnershipCoercionPolicy,
    pub primitive_widening: Option<PrimitiveWideningPolicy>,
}

pub fn shape_coercion_plan(actual: &RustSlotShape, target: &RustSlotShape) -> ShapeCoercionPlan {
    shape_coercion_plan_with_facts(actual, target, &FactSet::default())
}

pub fn shape_coercion_plan_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> ShapeCoercionPlan {
    ShapeCoercionPlan {
        actual: actual.clone(),
        target: target.clone(),
        dispatch: dispatch_coercion_policy_with_facts(actual, target, facts),
        view: view_coercion_policy(actual, target, facts),
        optional: optional_shape_coercion_policy_with_facts(actual, target, facts),
        ownership: ownership_coercion_policy(actual, target, facts),
        primitive_widening: primitive_widening_policy(&actual.java_ty, &target.java_ty)
            .filter(|policy| policy.widens),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCoercionPlan {
    pub actual: JavaType,
    pub slot: Slot,
    pub content: CoercionContentPolicy,
    pub policy: SyntaxSlotCoercionPolicy,
}

pub fn syntax_coercion_plan(actual: &JavaType, slot: &Slot) -> SyntaxCoercionPlan {
    syntax_coercion_plan_with_facts(actual, slot, &FactSet::default())
}

pub fn syntax_coercion_plan_with_facts(
    actual: &JavaType,
    slot: &Slot,
    facts: &FactSet,
) -> SyntaxCoercionPlan {
    let content = coercion_content_policy(actual, facts);
    let policy = syntax_slot_coercion_policy_from_content(slot, &content);
    SyntaxCoercionPlan {
        actual: actual.clone(),
        slot: slot.clone(),
        content,
        policy,
    }
}

pub fn syntax_slot_coercion_policy(
    actual: &JavaType,
    slot: &Slot,
    facts: &FactSet,
) -> SyntaxSlotCoercionPolicy {
    let content = coercion_content_policy(actual, facts);
    syntax_slot_coercion_policy_from_content(slot, &content)
}

fn syntax_slot_coercion_policy_from_content(
    slot: &Slot,
    content: &CoercionContentPolicy,
) -> SyntaxSlotCoercionPolicy {
    match slot {
        Slot::Index => SyntaxSlotCoercionPolicy::IndexToUsize,
        Slot::StringConcatSegment if !content.string_content => {
            SyntaxSlotCoercionPolicy::ToStringForConcat
        }
        _ => SyntaxSlotCoercionPolicy::Identity,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfCheck {
    ClosedEnumVariant {
        enum_name: String,
        variant_name: String,
    },
    DynTraitType {
        target_type: String,
    },
    BoxedDynTraitType {
        target_type: String,
    },
    ErasedObjectType {
        target_type: String,
    },
    AlwaysTrue,
    Unsupported {
        actual_dispatch: DispatchShape,
        target_dispatch: DispatchShape,
        policy: InstanceOfFallbackPolicy,
    },
}

pub fn plan_instanceof_check(actual: &RustSlotShape, target: &RustSlotShape) -> InstanceOfCheck {
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::ClosedEnum { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::ClosedEnumVariant {
                        enum_name: closed_enum_name(actual).to_string(),
                        variant_name: target_type,
                    };
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    return unsupported_instanceof_check(
                        actual,
                        target,
                        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
                    );
                }
            }
        }
        (DispatchShape::DynTrait { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::DynTraitType { target_type };
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    return unsupported_instanceof_check(
                        actual,
                        target,
                        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
                    );
                }
            }
        }
        (DispatchShape::BoxedDynTrait { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::BoxedDynTraitType { target_type };
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    return unsupported_instanceof_check(
                        actual,
                        target,
                        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
                    );
                }
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::ErasedObjectType { target_type };
                }
                RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                    return unsupported_instanceof_check(
                        actual,
                        target,
                        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
                    );
                }
            }
        }
        (DispatchShape::Concrete, DispatchShape::Concrete) if actual.java_ty == target.java_ty => {
            return InstanceOfCheck::AlwaysTrue;
        }
        (DispatchShape::ErasedObject, DispatchShape::DynTrait { trait_name }) => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::ErasedObjectToDynTrait {
                    trait_name: trait_name.clone(),
                },
            );
        }
        (DispatchShape::ErasedObject, DispatchShape::BoxedDynTrait { trait_name }) => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait {
                    trait_name: trait_name.clone(),
                },
            );
        }
        _ => {}
    }
    unsupported_instanceof_check(actual, target, InstanceOfFallbackPolicy::CompileError)
}

fn unsupported_instanceof_check(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    policy: InstanceOfFallbackPolicy,
) -> InstanceOfCheck {
    InstanceOfCheck::Unsupported {
        actual_dispatch: actual.dispatch.clone(),
        target_dispatch: target.dispatch.clone(),
        policy,
    }
}

fn closed_enum_name(shape: &RustSlotShape) -> &str {
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => enum_name,
        _ => unreachable!("closed_enum_name called for non-enum dispatch"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotConversionPlan {
    Shape(ShapeCoercionPlan),
    Syntax(SyntaxCoercionPlan),
    BorrowedElement(BorrowedElementSlotAdapterPolicy),
    Identity { actual: LoweredType, slot: Slot },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastConversionPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
    pub coercion: ShapeCoercionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceOfPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
    pub check: InstanceOfCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotExprConversionPlan {
    pub actual: RustSlotShape,
    pub slot: Slot,
    pub conversion: SlotConversionPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSlotExprConversionPlan {
    pub actual: LoweredType,
    pub slot: Slot,
    pub conversion: SlotConversionPlan,
}

pub fn plan_cast_conversion(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> CastConversionPlan {
    let actual = infer_expr_shape_with_context(expr, env, ctx);
    let target = project_java_value_shape(ctx.representations, target_ty.clone());
    let default_facts = FactSet::default();
    let facts = ctx.facts.unwrap_or(&default_facts);
    let coercion = shape_coercion_plan_with_facts(&actual, &target, facts);

    CastConversionPlan {
        actual,
        target,
        coercion,
    }
}

pub fn plan_instanceof(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> InstanceOfPlan {
    let actual = infer_expr_shape_with_context(expr, env, ctx);
    let target = project_java_value_shape(ctx.representations, target_ty.clone());
    let check = plan_instanceof_check(&actual, &target);

    InstanceOfPlan {
        actual,
        target,
        check,
    }
}

pub fn plan_slot_conversion(
    actual: RustSlotShape,
    slot: Slot,
    facts: &FactSet,
) -> SlotExprConversionPlan {
    let conversion = slot_conversion_plan_with_facts(&actual, &slot, facts);
    SlotExprConversionPlan {
        actual,
        slot,
        conversion,
    }
}

pub fn slot_conversion_plan(actual: &RustSlotShape, slot: &Slot) -> SlotConversionPlan {
    slot_conversion_plan_with_facts(actual, slot, &FactSet::default())
}

pub fn slot_conversion_plan_with_facts(
    actual: &RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> SlotConversionPlan {
    match slot {
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            SlotConversionPlan::Shape(shape_coercion_plan_with_facts(actual, target, facts))
        }
        _ => SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts(
            &actual.java_ty,
            slot,
            facts,
        )),
    }
}

pub fn actual_shape_for_slot_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    if let Expr::Cast { ty, .. } = expr {
        project_java_value_shape(ctx.representations, ty.clone())
    } else {
        infer_expr_shape_with_context(expr, env, ctx)
    }
}

pub fn plan_lowered_slot_conversion(
    actual: &LoweredType,
    slot: Slot,
    facts: &FactSet,
) -> LoweredSlotExprConversionPlan {
    let conversion = lowered_slot_conversion_plan_with_facts(actual, &slot, facts);
    LoweredSlotExprConversionPlan {
        actual: actual.clone(),
        slot,
        conversion,
    }
}

pub fn lowered_slot_conversion_plan(actual: &LoweredType, slot: &Slot) -> SlotConversionPlan {
    lowered_slot_conversion_plan_with_facts(actual, slot, &FactSet::default())
}

pub fn lowered_slot_conversion_plan_with_facts(
    actual: &LoweredType,
    slot: &Slot,
    facts: &FactSet,
) -> SlotConversionPlan {
    match (actual, slot) {
        (LoweredType::Slot(actual), _) => slot_conversion_plan_with_facts(actual, slot, facts),
        (
            LoweredType::OptionalElementBorrow {
                option_shape,
                element_shape,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) => SlotConversionPlan::BorrowedElement(borrowed_element_slot_adapter_policy(
            BorrowedElementCarrier::Option,
            target,
            option_shape,
            element_shape,
            facts,
        )),
        (
            LoweredType::VecElementBorrow {
                vec_shape,
                element_shape,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) => SlotConversionPlan::BorrowedElement(borrowed_element_slot_adapter_policy(
            BorrowedElementCarrier::Vec,
            target,
            vec_shape,
            element_shape,
            facts,
        )),
        _ => SlotConversionPlan::Identity {
            actual: actual.clone(),
            slot: slot.clone(),
        },
    }
}
