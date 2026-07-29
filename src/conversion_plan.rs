use crate::effective_facts::EffectiveFacts;
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, infer_assignment_target_shape_with_context,
    infer_expr_shape_with_context,
};
use crate::facts::{
    BorrowedReturnRecovery, ExternalConversionAdapterKind, FactSet, HierarchyStrategy,
    OwnedRecoveryCapability,
};
use crate::java_ir::{Expr, JavaCoercionEvidence, JavaCoercionKind, JavaType, PrimitiveType};
use crate::shape_projection::{borrowed_sequence_element_shape, project_java_value_shape};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{is_numeric_primitive_type, list_java_type, optional_java_type, set_java_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slot {
    Value,
    Condition,
    Index,
    Assignment(RustSlotShape),
    Return(RustSlotShape),
    Argument(RustSlotShape),
    StringConcatSegment,
    DisplaySegment,
}

pub fn assignment_slot(shape: RustSlotShape) -> Slot {
    Slot::Assignment(shape)
}

pub fn storage_assignment_slot_for_java_type(java_ty: JavaType) -> Slot {
    assignment_slot(storage_shape_for_java_type(java_ty))
}

pub fn storage_shape_for_java_type(java_ty: JavaType) -> RustSlotShape {
    RustSlotShape::for_storage(java_ty)
}

pub fn effective_stream_item_shape(shape: &RustSlotShape, item_is_borrowed: bool) -> RustSlotShape {
    let mut shape = shape.clone();
    shape.ownership = if item_is_borrowed {
        OwnershipShape::SharedBorrow
    } else {
        crate::slot_shape::conservative_ownership(&shape.java_ty)
    };
    shape
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentStatementPlan {
    ClearOptionalTarget,
    AssignValue,
}

pub fn assignment_statement_plan_for_target(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> AssignmentStatementPlan {
    if matches!(value, Expr::Literal(crate::java_ir::Literal::Null))
        && infer_assignment_target_shape_with_context(target, env, ctx)
            .is_some_and(|shape| shape.optional_element_type().is_some())
    {
        AssignmentStatementPlan::ClearOptionalTarget
    } else {
        AssignmentStatementPlan::AssignValue
    }
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
    ErasedObjectToJavaDisplayString,
    OptionalToJavaDisplayString,
    OptionalToJavaDisplayValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewCoercionPolicy {
    NoViewChange,
    BorrowStringView,
    OwnStringFromView,
    BorrowSequenceSliceView,
    BorrowSequenceMutSliceView,
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
    BorrowedToFacadeOwnedExternalHandle { java_ty: JavaType },
    BorrowedToFfiOwnedHandle { java_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipCoercionPolicy {
    NoOwnershipChange,
    BorrowOwnedValue,
    BorrowOwnedMutably,
    RecoverOwnedCopyValue,
    RecoverErasedObjectClone {
        sequence: bool,
    },
    RecoverOwnedSequenceWithToVec {
        policy: CloneRecoveryPolicy,
    },
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
pub struct ConversionDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidCastPolicy {
    Panic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaCastSemanticsPolicy {
    Identity,
    PrimitiveNumericCast {
        from: PrimitiveType,
        to: PrimitiveType,
        widening: Option<PrimitiveWideningPolicy>,
    },
    ReferenceUpcast,
    CheckedReferenceDowncast {
        invalid_cast: InvalidCastPolicy,
    },
    UncheckedGenericCast {
        actual: JavaType,
        target: JavaType,
    },
    RawOrWildcardCast {
        actual: JavaType,
        target: JavaType,
    },
    ImpossibleCast {
        actual: JavaType,
        target: JavaType,
    },
    OptionalResultCast {
        actual: JavaType,
        target: JavaType,
    },
    DiagnosticOnlyFallback {
        diagnostics: Vec<ConversionDiagnostic>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaTypeConversionKind {
    Identity,
    PrimitiveWidening {
        from: PrimitiveType,
        to: PrimitiveType,
    },
    Boxing,
    Unboxing,
    BoxingWidening,
    ReferenceWidening,
    GenericInference,
    NullToReference,
    LambdaAdaptation,
    MethodReferenceAdaptation,
    UnknownFallback,
    Cast(JavaCastSemanticsPolicy),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaTypeConversionPlan {
    pub actual: JavaType,
    pub target: JavaType,
    pub kind: JavaTypeConversionKind,
}

pub fn java_type_conversion_plan_from_evidence(
    evidence: &JavaCoercionEvidence,
) -> JavaTypeConversionPlan {
    JavaTypeConversionPlan {
        actual: evidence.actual.clone(),
        target: evidence.target.clone(),
        kind: java_type_conversion_kind_from_evidence(evidence),
    }
}

pub fn java_type_conversion_kind_from_evidence(
    evidence: &JavaCoercionEvidence,
) -> JavaTypeConversionKind {
    match evidence.kind {
        JavaCoercionKind::Identity => JavaTypeConversionKind::Identity,
        JavaCoercionKind::PrimitiveWidening => {
            if let (JavaType::Primitive(from), JavaType::Primitive(to)) =
                (&evidence.actual, &evidence.target)
            {
                JavaTypeConversionKind::PrimitiveWidening {
                    from: *from,
                    to: *to,
                }
            } else {
                JavaTypeConversionKind::UnknownFallback
            }
        }
        JavaCoercionKind::Boxing => JavaTypeConversionKind::Boxing,
        JavaCoercionKind::Unboxing => JavaTypeConversionKind::Unboxing,
        JavaCoercionKind::BoxingWidening => JavaTypeConversionKind::BoxingWidening,
        JavaCoercionKind::ReferenceWidening => JavaTypeConversionKind::ReferenceWidening,
        JavaCoercionKind::GenericInference => JavaTypeConversionKind::GenericInference,
        JavaCoercionKind::NullToReference => JavaTypeConversionKind::NullToReference,
        JavaCoercionKind::LambdaAdaptation => JavaTypeConversionKind::LambdaAdaptation,
        JavaCoercionKind::MethodReferenceAdaptation => {
            JavaTypeConversionKind::MethodReferenceAdaptation
        }
        JavaCoercionKind::UnknownFallback => JavaTypeConversionKind::UnknownFallback,
    }
}

pub fn java_cast_type_conversion_plan(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> JavaTypeConversionPlan {
    let coercion = shape_coercion_plan_with_facts(actual, target, facts);
    JavaTypeConversionPlan {
        actual: actual.java_ty.clone(),
        target: target.java_ty.clone(),
        kind: JavaTypeConversionKind::Cast(java_cast_semantics_policy_with_facts(
            actual, target, &coercion, facts,
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastAndInstanceOfPlanningBoundary {
    pub casts_use_representation_shapes: bool,
    pub instanceof_uses_representation_shapes: bool,
    pub hierarchy_reasoning_before_lowering: bool,
    pub rendering_uses_conversion_plans_only: bool,
}

pub fn cast_and_instanceof_planning_boundary() -> CastAndInstanceOfPlanningBoundary {
    CastAndInstanceOfPlanningBoundary {
        casts_use_representation_shapes: true,
        instanceof_uses_representation_shapes: true,
        hierarchy_reasoning_before_lowering: true,
        rendering_uses_conversion_plans_only: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfFallbackPolicy {
    CompileError,
    PrimitiveTarget { java_ty: JavaType },
    RawOrWildcardTarget { java_ty: JavaType },
    MissingRuntimeTarget { java_ty: JavaType },
    ErasedObjectToDynTrait { trait_name: String },
    ErasedObjectToBoxedDynTrait { trait_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfTargetPolicy {
    ConcreteRuntimeTarget { target_type: String },
    ClosedEnumTarget { enum_name: String },
    DynTraitTarget { trait_name: String },
    BoxedDynTraitTarget { trait_name: String },
    ErasedObjectTarget,
    PrimitiveTarget { java_ty: JavaType },
    RawOrWildcardTarget { java_ty: JavaType },
    MissingRuntimeTarget { java_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasedObjectRuntimeContract {
    pub rust_type: &'static str,
    pub erase_operation: &'static str,
    pub concrete_downcast_operation: &'static str,
    pub concrete_instanceof_operation: &'static str,
    pub dyn_trait_recovery: DispatchCoercionFallbackPolicy,
    pub boxed_dyn_trait_recovery: DispatchCoercionFallbackPolicy,
    pub dyn_trait_instanceof: InstanceOfFallbackPolicy,
    pub boxed_dyn_trait_instanceof: InstanceOfFallbackPolicy,
}

pub fn erased_object_runtime_contract(trait_name: &str) -> ErasedObjectRuntimeContract {
    ErasedObjectRuntimeContract {
        rust_type: "Box<dyn std::any::Any>",
        erase_operation: "Box::new(value) as Box<dyn std::any::Any>",
        concrete_downcast_operation: "*value.downcast::<Target>().expect(\"invalid Java cast\")",
        concrete_instanceof_operation: "value.is::<Target>()",
        dyn_trait_recovery: DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait {
            trait_name: trait_name.to_string(),
        },
        boxed_dyn_trait_recovery: DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait {
            trait_name: trait_name.to_string(),
        },
        dyn_trait_instanceof: InstanceOfFallbackPolicy::ErasedObjectToDynTrait {
            trait_name: trait_name.to_string(),
        },
        boxed_dyn_trait_instanceof: InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait {
            trait_name: trait_name.to_string(),
        },
    }
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
pub enum DynCloneBoxAdapterRequirement {
    ProvenAdapter {
        trait_name: String,
    },
    RequiresGeneratedAdapter {
        trait_name: String,
        policy: CloneRecoveryPolicy,
    },
    UnsupportedNonCloneable {
        trait_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynCloneBoxAdapterAvailability {
    Available,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynCloneBoxAdapterGenerationPolicy {
    AlreadyAvailable {
        trait_name: String,
    },
    GenerateAdapter {
        trait_name: String,
        policy: CloneRecoveryPolicy,
    },
    DeferredUnsupported {
        trait_name: String,
        reason: DynCloneBoxAdapterDeferralReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynCloneBoxAdapterDeferralReason {
    NoOwnershipProof,
    GenerationNotImplemented { policy: CloneRecoveryPolicy },
}

pub fn dyn_clone_box_adapter_requirement(
    trait_name: &str,
    recovery: &BorrowedDynToOwnedBoxedDynRecoveryPolicy,
) -> DynCloneBoxAdapterRequirement {
    dyn_clone_box_adapter_requirement_with_availability(
        trait_name,
        recovery,
        DynCloneBoxAdapterAvailability::Missing,
    )
}

pub fn dyn_clone_box_adapter_requirement_with_availability(
    trait_name: &str,
    recovery: &BorrowedDynToOwnedBoxedDynRecoveryPolicy,
    availability: DynCloneBoxAdapterAvailability,
) -> DynCloneBoxAdapterRequirement {
    if matches!(availability, DynCloneBoxAdapterAvailability::Available) {
        return DynCloneBoxAdapterRequirement::ProvenAdapter {
            trait_name: trait_name.to_string(),
        };
    }
    match recovery {
        BorrowedDynToOwnedBoxedDynRecoveryPolicy::NoOwnershipProof => {
            DynCloneBoxAdapterRequirement::UnsupportedNonCloneable {
                trait_name: trait_name.to_string(),
            }
        }
        BorrowedDynToOwnedBoxedDynRecoveryPolicy::CloneBoxAdapterRequired { policy } => {
            DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter {
                trait_name: trait_name.to_string(),
                policy: policy.clone(),
            }
        }
    }
}

pub fn dyn_clone_box_adapter_generation_policy(
    requirement: &DynCloneBoxAdapterRequirement,
) -> DynCloneBoxAdapterGenerationPolicy {
    match requirement {
        DynCloneBoxAdapterRequirement::ProvenAdapter { trait_name } => {
            DynCloneBoxAdapterGenerationPolicy::AlreadyAvailable {
                trait_name: trait_name.clone(),
            }
        }
        DynCloneBoxAdapterRequirement::RequiresGeneratedAdapter { trait_name, policy } => {
            DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported {
                trait_name: trait_name.clone(),
                reason: DynCloneBoxAdapterDeferralReason::GenerationNotImplemented {
                    policy: policy.clone(),
                },
            }
        }
        DynCloneBoxAdapterRequirement::UnsupportedNonCloneable { trait_name } => {
            DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported {
                trait_name: trait_name.clone(),
                reason: DynCloneBoxAdapterDeferralReason::NoOwnershipProof,
            }
        }
    }
}

pub fn dyn_clone_box_adapter_generation_policy_with_availability(
    trait_name: &str,
    recovery: &BorrowedDynToOwnedBoxedDynRecoveryPolicy,
    availability: DynCloneBoxAdapterAvailability,
) -> DynCloneBoxAdapterGenerationPolicy {
    dyn_clone_box_adapter_generation_policy(&dyn_clone_box_adapter_requirement_with_availability(
        trait_name,
        recovery,
        availability,
    ))
}

pub fn dyn_clone_box_adapter_deferral_message(
    trait_name: &str,
    reason: DynCloneBoxAdapterDeferralReason,
) -> ConversionDiagnostic {
    match reason {
        DynCloneBoxAdapterDeferralReason::NoOwnershipProof => ConversionDiagnostic {
            code: "unsupported_borrowed_dyn_to_owned_boxed_dyn",
            message: format!(
                "unsupported Java borrowed dyn trait to owned boxed dyn recovery for {trait_name}; no ownership proof is available"
            ),
        },
        DynCloneBoxAdapterDeferralReason::GenerationNotImplemented { policy } => {
            let proof = match policy {
                CloneRecoveryPolicy::ProvenCloneCapability { .. } => "explicit clone proof exists",
                CloneRecoveryPolicy::AuditRequired => "only conservative clone proof exists",
            };
            ConversionDiagnostic {
                code: "unsupported_borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter_deferred",
                message: format!(
                    "unsupported Java borrowed dyn trait to owned boxed dyn recovery for {trait_name}; {proof}, but dyn clone-box adapter generation is explicitly deferred"
                ),
            }
        }
    }
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
    BorrowConcreteFacade {
        target_type: String,
    },
    ExternalInto {
        from_type: JavaType,
        to_type: JavaType,
    },
    BoxDynTrait {
        trait_name: String,
    },
    CloneBoxDynTrait {
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
    MapElement {
        actual_element_ty: JavaType,
        target_element_ty: JavaType,
        element_plan: Box<ShapeCoercionPlan>,
    },
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSlotConstructionPlan {
    pub target: RustSlotShape,
    pub element_shape: RustSlotShape,
    pub element_plan: ShapeCoercionPlan,
    pub wrapper_ownership: OwnershipShape,
}

pub fn optional_slot_construction_plan(
    target: &RustSlotShape,
    element_ty: JavaType,
) -> OptionalSlotConstructionPlan {
    optional_slot_construction_plan_with_facts(target, element_ty, &FactSet::default())
}

pub fn optional_slot_construction_plan_with_facts(
    target: &RustSlotShape,
    element_ty: JavaType,
    facts: &FactSet,
) -> OptionalSlotConstructionPlan {
    let mut element_shape = target.clone();
    element_shape.java_ty = element_ty;
    optional_slot_construction_plan_with_actual_element(&element_shape, target, facts)
        .expect("target must be an optional slot")
}

pub fn optional_slot_construction_plan_with_actual_element(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> Option<OptionalSlotConstructionPlan> {
    let target_element_ty = target.optional_element_type()?;
    let target_element = optional_element_slot_shape_with_facts(target, target_element_ty, facts);
    let element_plan = shape_coercion_plan_with_facts(actual_element_shape, &target_element, facts);
    Some(OptionalSlotConstructionPlan {
        target: target.clone(),
        element_shape: actual_element_shape.clone(),
        element_plan,
        wrapper_ownership: OwnershipShape::Owned,
    })
}

pub fn optional_slot_construction_plan_for_actual(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> OptionalSlotConstructionPlan {
    optional_slot_construction_plan_with_actual_element(actual_element_shape, target, facts)
        .expect("target must be an optional slot")
}

impl OptionalShapeCoercionPolicy {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            OptionalShapeCoercionPolicy::Wrap { element_plan, .. }
            | OptionalShapeCoercionPolicy::Unwrap { element_plan, .. }
            | OptionalShapeCoercionPolicy::MapElement { element_plan, .. } => {
                element_plan.unsupported_diagnostics()
            }
            OptionalShapeCoercionPolicy::NoChange => Vec::new(),
        }
    }
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
    BorrowElement,
    StringViewToOwned,
    CopyElements,
    CloneElements {
        policy: CloneRecoveryPolicy,
    },
    Unsupported {
        policy: BorrowedElementRecoveryFallbackPolicy,
    },
}

impl BorrowedElementRecoveryPolicy {
    pub fn unsupported_diagnostics(
        &self,
        element_shape: &RustSlotShape,
    ) -> Vec<ConversionDiagnostic> {
        match self {
            BorrowedElementRecoveryPolicy::BorrowElement => Vec::new(),
            BorrowedElementRecoveryPolicy::Unsupported { policy } => {
                vec![borrowed_element_recovery_fallback_diagnostic(
                    policy,
                    element_shape,
                )]
            }
            _ => Vec::new(),
        }
    }
}

fn borrowed_element_recovery_fallback_diagnostic(
    policy: &BorrowedElementRecoveryFallbackPolicy,
    element_shape: &RustSlotShape,
) -> ConversionDiagnostic {
    match policy {
        BorrowedElementRecoveryFallbackPolicy::CompileError => ConversionDiagnostic {
            code: "unsupported_borrowed_element_recovery",
            message: format!(
                "unsupported borrowed Java element recovery for element shape {element_shape:?}"
            ),
        },
        BorrowedElementRecoveryFallbackPolicy::NonCloneableElement { java_ty } => {
            ConversionDiagnostic {
                code: "unsupported_borrowed_element_non_cloneable",
                message: format!(
                    "unsupported Java borrowed element ownership recovery for non-cloneable type {java_ty:?}"
                ),
            }
        }
    }
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
pub struct SetElementTargetMatchPolicy {
    pub target: RustSlotShape,
    pub set_shape: RustSlotShape,
    pub element_shape: RustSlotShape,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredType {
    Slot(Box<RustSlotShape>),
    RustScalar {
        java_shape: Box<RustSlotShape>,
        rust_type_name: String,
    },
    RustBorrowed {
        java_shape: Box<RustSlotShape>,
        rust_type_name: String,
        recovery: Option<RustBorrowedFacadeReturnRecovery>,
    },
    RustUnsignedByteArray {
        java_shape: Box<RustSlotShape>,
        optional: bool,
    },
    OptionalElementBorrow {
        option_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
        recovery: Option<RustBorrowedFacadeReturnRecovery>,
    },
    VecElementBorrow {
        vec_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
    SetElementBorrow {
        set_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
}

impl LoweredType {
    pub fn from_java(java_ty: JavaType) -> Self {
        Self::Slot(Box::new(RustSlotShape::conservative(java_ty)))
    }

    pub fn rust_scalar_return(java_ty: JavaType, rust_type_name: String) -> Self {
        Self::RustScalar {
            java_shape: Box::new(RustSlotShape::conservative(java_ty)),
            rust_type_name,
        }
    }

    pub fn rust_borrowed_return(
        java_ty: JavaType,
        rust_type_name: String,
        recovery: Option<RustBorrowedFacadeReturnRecovery>,
    ) -> Self {
        let mut java_shape = RustSlotShape::conservative(java_ty);
        java_shape.ownership = OwnershipShape::SharedBorrow;
        Self::RustBorrowed {
            java_shape: Box::new(java_shape),
            rust_type_name,
            recovery,
        }
    }

    pub fn rust_unsigned_byte_array_return(java_ty: JavaType, optional: bool) -> Self {
        Self::RustUnsignedByteArray {
            java_shape: Box::new(RustSlotShape::conservative(java_ty)),
            optional,
        }
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
            recovery: None,
        }
    }

    pub fn optional_element_borrow_from_option_shape(
        option_shape: RustSlotShape,
        element_ty: JavaType,
        element_ownership: Option<OwnershipShape>,
        facts: &FactSet,
    ) -> Option<Self> {
        if option_shape.optional_element_type() != Some(element_ty.clone()) {
            return None;
        }
        let mut element_shape =
            optional_element_slot_shape_with_facts(&option_shape, element_ty, facts);
        if let Some(element_ownership) = element_ownership {
            element_shape.ownership = element_ownership;
        }
        Some(Self::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery: None,
        })
    }

    pub fn optional_rust_borrowed_return(
        element_ty: JavaType,
        recovery: Option<RustBorrowedFacadeReturnRecovery>,
    ) -> Self {
        let element_shape = borrowed_sequence_element_shape(element_ty);
        let option_shape =
            RustSlotShape::conservative(optional_java_type(element_shape.java_ty.clone()));
        Self::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
            recovery,
        }
    }

    pub fn vec_element_borrow(element_shape: RustSlotShape) -> Self {
        let vec_shape = RustSlotShape::conservative(list_java_type(element_shape.java_ty.clone()));
        Self::VecElementBorrow {
            vec_shape: Box::new(vec_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn set_element_borrow(element_shape: RustSlotShape) -> Self {
        let set_shape = RustSlotShape::conservative(set_java_type(element_shape.java_ty.clone()));
        Self::SetElementBorrow {
            set_shape: Box::new(set_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn collected_list_from_element_shape(element_shape: RustSlotShape) -> Self {
        if element_shape.ownership == OwnershipShape::SharedBorrow {
            Self::vec_element_borrow(element_shape)
        } else {
            Self::from_java(list_java_type(element_shape.java_ty))
        }
    }

    pub fn collected_set_from_element_shape(element_shape: RustSlotShape) -> Self {
        if element_shape.ownership == OwnershipShape::SharedBorrow {
            Self::set_element_borrow(element_shape)
        } else {
            Self::from_java(set_java_type(element_shape.java_ty))
        }
    }

    pub fn collected_sequence_from_element_shape(element_shape: RustSlotShape) -> Self {
        Self::collected_list_from_element_shape(element_shape)
    }

    pub fn java_ty(&self) -> Option<&JavaType> {
        match self {
            LoweredType::Slot(shape) => Some(&shape.java_ty),
            LoweredType::RustScalar { java_shape, .. } => Some(&java_shape.java_ty),
            LoweredType::RustBorrowed { java_shape, .. } => Some(&java_shape.java_ty),
            LoweredType::RustUnsignedByteArray { java_shape, .. } => Some(&java_shape.java_ty),
            LoweredType::OptionalElementBorrow { option_shape, .. } => Some(&option_shape.java_ty),
            LoweredType::VecElementBorrow { vec_shape, .. } => Some(&vec_shape.java_ty),
            LoweredType::SetElementBorrow { set_shape, .. } => Some(&set_shape.java_ty),
        }
    }

    pub fn into_slot_shape(self) -> RustSlotShape {
        match self {
            LoweredType::Slot(shape)
            | LoweredType::RustScalar {
                java_shape: shape, ..
            }
            | LoweredType::RustBorrowed {
                java_shape: shape, ..
            }
            | LoweredType::RustUnsignedByteArray {
                java_shape: shape, ..
            }
            | LoweredType::OptionalElementBorrow {
                option_shape: shape,
                ..
            }
            | LoweredType::VecElementBorrow {
                vec_shape: shape, ..
            }
            | LoweredType::SetElementBorrow {
                set_shape: shape, ..
            } => *shape,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustScalarSlotCastPolicy {
    pub rust_type_name: String,
    pub java_semantic_shape: RustSlotShape,
    pub target: RustSlotShape,
    pub target_primitive: PrimitiveType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustBorrowedFacadeReturnPolicy {
    pub rust_type_name: String,
    pub java_semantic_shape: RustSlotShape,
    pub target: RustSlotShape,
    pub recovery: RustBorrowedFacadeReturnRecovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustBorrowedFacadeReturnRecovery {
    Clone,
    ToPathBuf,
}

impl From<BorrowedReturnRecovery> for RustBorrowedFacadeReturnRecovery {
    fn from(recovery: BorrowedReturnRecovery) -> Self {
        match recovery {
            BorrowedReturnRecovery::Clone => Self::Clone,
            BorrowedReturnRecovery::ToPathBuf => Self::ToPathBuf,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustUnsignedByteArrayReturnPolicy {
    pub java_semantic_shape: RustSlotShape,
    pub target: RustSlotShape,
    pub optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedElementCarrier {
    Option,
    Vec,
    Set,
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
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
            crate::stdlib::boxed_primitive_type(&canonical)
                .map(boxed_primitive_runtime_rust_type)
                .or_else(|| Some(crate::slot_render::rust_name_for_java_named_type(name)))
        }
        JavaType::String => Some("String".to_string()),
        _ => None,
    };
    RustNominalTypeNamePolicy {
        java_ty: java_ty.clone(),
        target_type,
    }
}

fn boxed_primitive_runtime_rust_type(primitive: PrimitiveType) -> String {
    match primitive {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Byte => "i8",
        PrimitiveType::Short => "i16",
        PrimitiveType::Char => "char",
        PrimitiveType::Int => "i32",
        PrimitiveType::Long => "i64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
    }
    .to_string()
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

pub fn java_cast_semantics_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    coercion: &ShapeCoercionPlan,
) -> JavaCastSemanticsPolicy {
    java_cast_semantics_policy_with_facts(actual, target, coercion, &FactSet::default())
}

pub fn java_cast_semantics_policy_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    coercion: &ShapeCoercionPlan,
    facts: &FactSet,
) -> JavaCastSemanticsPolicy {
    if actual.java_ty == target.java_ty && actual.dispatch == target.dispatch {
        return JavaCastSemanticsPolicy::Identity;
    }
    let capability = facts.cast_type_capability_policy(&actual.java_ty, &target.java_ty);
    if capability.optional_or_result {
        return JavaCastSemanticsPolicy::OptionalResultCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if capability.raw_or_wildcard {
        return JavaCastSemanticsPolicy::RawOrWildcardCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if capability.unchecked_generic {
        return JavaCastSemanticsPolicy::UncheckedGenericCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if let (JavaType::Primitive(from), JavaType::Primitive(to)) = (&actual.java_ty, &target.java_ty)
    {
        if is_numeric_primitive_type(*from) && is_numeric_primitive_type(*to) {
            return JavaCastSemanticsPolicy::PrimitiveNumericCast {
                from: *from,
                to: *to,
                widening: primitive_widening_policy(&actual.java_ty, &target.java_ty),
            };
        }
        return JavaCastSemanticsPolicy::ImpossibleCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if matches!(actual.java_ty, JavaType::Primitive(_))
        || matches!(target.java_ty, JavaType::Primitive(_))
    {
        return JavaCastSemanticsPolicy::ImpossibleCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if coercion.unsupported_diagnostics().is_empty() {
        return match &coercion.dispatch {
            DispatchCoercionPolicy::DowncastClosedEnum { invalid_cast, .. }
            | DispatchCoercionPolicy::DowncastDynTrait { invalid_cast, .. }
            | DispatchCoercionPolicy::DowncastBoxedDynTrait { invalid_cast, .. }
            | DispatchCoercionPolicy::DowncastErasedObject { invalid_cast, .. } => {
                JavaCastSemanticsPolicy::CheckedReferenceDowncast {
                    invalid_cast: invalid_cast.clone(),
                }
            }
            DispatchCoercionPolicy::WrapClosedEnum { .. }
            | DispatchCoercionPolicy::BorrowDynTrait { .. }
            | DispatchCoercionPolicy::BoxDynTrait { .. }
            | DispatchCoercionPolicy::EraseObject => JavaCastSemanticsPolicy::ReferenceUpcast,
            _ => JavaCastSemanticsPolicy::Identity,
        };
    }
    JavaCastSemanticsPolicy::DiagnosticOnlyFallback {
        diagnostics: coercion.unsupported_diagnostics(),
    }
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
    if actual.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::SharedBorrow
        && target.view == ViewShape::Slice
        && crate::types::two_dimensional_array_element_type(&actual.java_ty).is_none()
        && sequence_view_element_types_match(&actual.java_ty, &target.java_ty)
    {
        return ViewCoercionPolicy::BorrowSequenceSliceView;
    }
    if actual.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::MutableBorrow
        && target.view == ViewShape::Slice
        && crate::types::two_dimensional_array_element_type(&actual.java_ty).is_none()
        && sequence_view_element_types_match(&actual.java_ty, &target.java_ty)
    {
        return ViewCoercionPolicy::BorrowSequenceMutSliceView;
    }
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

fn sequence_view_element_types_match(actual: &JavaType, target: &JavaType) -> bool {
    let Some(actual_element) = sequence_view_element_type(actual) else {
        return false;
    };
    let Some(target_element) = sequence_view_element_type(target) else {
        return false;
    };
    actual_element == target_element
}

fn sequence_view_element_type(java_ty: &JavaType) -> Option<&JavaType> {
    match java_ty {
        JavaType::Array(element) => Some(element.as_ref()),
        JavaType::Named { args, .. }
            if crate::stdlib::java_collection_kind_for_type(java_ty).is_some() =>
        {
            args.first()
        }
        _ => None,
    }
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
    if actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::MutableBorrow
        && actual.view == target.view
        && actual.dispatch == target.dispatch
    {
        return OwnershipCoercionPolicy::BorrowOwnedMutably;
    }
    if actual.java_ty == target.java_ty
        && actual.dispatch == target.dispatch
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && actual.view == ViewShape::Slice
        && target.ownership == OwnershipShape::Owned
        && target.view == ViewShape::Native
        && crate::types::sequence_element_type(&actual.java_ty).is_some()
    {
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        return OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec {
            policy: clone_recovery_policy_from_capability(capability.recovery),
        };
    }
    if actual.java_ty == target.java_ty
        && actual.dispatch == target.dispatch
        && actual.view == target.view
        && actual.ownership == OwnershipShape::SharedBorrow
        && target.ownership == OwnershipShape::Owned
    {
        if let Some(sequence) = erased_object_clone_recovery_sequence(actual, target) {
            return OwnershipCoercionPolicy::RecoverErasedObjectClone { sequence };
        }
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        if crate::types::sequence_element_type(&actual.java_ty).is_some() {
            return OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec {
                policy: clone_recovery_policy_from_capability(capability.recovery),
            };
        }
        ownership_recovery_policy_from_capability(capability.recovery, actual, target)
    } else if actual.dispatch == DispatchShape::Concrete
        && matches!(target.dispatch, DispatchShape::ClosedEnum { .. })
        && actual.ownership == OwnershipShape::SharedBorrow
        && target.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.view == ViewShape::Native
    {
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        ownership_recovery_policy_from_capability(capability.recovery, actual, target)
    } else if actual.ownership == OwnershipShape::SharedBorrow
        && target.ownership == OwnershipShape::Owned
        && matches!(target.dispatch, DispatchShape::ErasedObject)
        && !matches!(actual.java_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        if let Some(sequence) = erased_object_clone_recovery_sequence(actual, target) {
            return OwnershipCoercionPolicy::RecoverErasedObjectClone { sequence };
        }
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
        OwnedRecoveryCapability::Copy | OwnedRecoveryCapability::ImmutableSharedHandle { .. } => {
            OwnershipCoercionPolicy::RecoverOwnedCopyValue
        }
        OwnedRecoveryCapability::ExplicitClone { java_ty }
        | OwnedRecoveryCapability::CloneBoxAdapter { java_ty }
        | OwnedRecoveryCapability::FactoryCopy { java_ty } => {
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
        OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. } => {
            OwnershipCoercionPolicy::UnsupportedRecovery {
                actual_ownership: actual.ownership,
                target_ownership: target.ownership,
                policy: OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle {
                    java_ty: actual.java_ty.clone(),
                },
            }
        }
        OwnedRecoveryCapability::FfiOwnedHandle { .. } => {
            OwnershipCoercionPolicy::UnsupportedRecovery {
                actual_ownership: actual.ownership,
                target_ownership: target.ownership,
                policy: OwnershipCoercionFallbackPolicy::BorrowedToFfiOwnedHandle {
                    java_ty: actual.java_ty.clone(),
                },
            }
        }
    }
}

pub fn clone_recovery_policy_from_capability(
    capability: OwnedRecoveryCapability,
) -> CloneRecoveryPolicy {
    match capability {
        OwnedRecoveryCapability::ExplicitClone { java_ty }
        | OwnedRecoveryCapability::CloneBoxAdapter { java_ty }
        | OwnedRecoveryCapability::FactoryCopy { java_ty } => {
            CloneRecoveryPolicy::ProvenCloneCapability { java_ty }
        }
        OwnedRecoveryCapability::ConservativeClone => CloneRecoveryPolicy::AuditRequired,
        OwnedRecoveryCapability::Copy
        | OwnedRecoveryCapability::ImmutableSharedHandle { .. }
        | OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. }
        | OwnedRecoveryCapability::FfiOwnedHandle { .. }
        | OwnedRecoveryCapability::NonCloneable => CloneRecoveryPolicy::AuditRequired,
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
    if let Some(element_ty) = target.optional_element_type()
        && actual.optional_element_type().is_none()
    {
        let target_element = optional_element_slot_shape_with_facts(target, element_ty, facts);
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
        && target.optional_element_type().is_none()
    {
        let actual_element = optional_element_slot_shape_with_facts(actual, element_ty, facts);
        return OptionalShapeCoercionPolicy::Unwrap {
            element_ty: target.java_ty.clone(),
            element_plan: Box::new(shape_coercion_plan_with_facts(
                &actual_element,
                target,
                facts,
            )),
        };
    }
    if let (Some(actual_element_ty), Some(target_element_ty)) = (
        actual.optional_element_type(),
        target.optional_element_type(),
    ) {
        let actual_element =
            optional_element_slot_shape_with_facts(actual, actual_element_ty.clone(), facts);
        let target_element =
            optional_element_slot_shape_with_facts(target, target_element_ty.clone(), facts);
        if actual_element != target_element {
            return OptionalShapeCoercionPolicy::MapElement {
                actual_element_ty,
                target_element_ty,
                element_plan: Box::new(shape_coercion_plan_with_facts(
                    &actual_element,
                    &target_element,
                    facts,
                )),
            };
        }
    }
    OptionalShapeCoercionPolicy::NoChange
}

pub(crate) fn optional_element_slot_shape(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
) -> RustSlotShape {
    optional_element_slot_shape_with_facts(option_shape, element_ty, &FactSet::default())
}

pub(crate) fn optional_element_slot_shape_with_facts(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    facts: &FactSet,
) -> RustSlotShape {
    let mut element_shape = option_shape.clone();
    if java_type_is_erased_object_like(&element_ty) {
        let projected = project_java_value_shape(None, element_ty);
        element_shape.java_ty = projected.java_ty;
        element_shape.dispatch = projected.dispatch;
        element_shape.view = projected.view;
        element_shape.type_identity = projected.type_identity;
    } else if let JavaType::Named { name, args } = &element_ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        let trait_name = name.clone();
        element_shape.java_ty = element_ty;
        element_shape.dispatch = DispatchShape::BoxedDynTrait { trait_name };
        element_shape.view = ViewShape::DynTraitView;
        element_shape.type_identity =
            crate::slot_shape::TypeIdentity::from_java_type(element_shape.java_ty.clone());
    } else if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        EffectiveFacts::new(facts).object_representation_override_strategy_for(&element_ty)
    {
        element_shape.java_ty = element_ty;
        element_shape.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        };
        element_shape.view = ViewShape::DynTraitView;
        element_shape.type_identity =
            crate::slot_shape::TypeIdentity::from_java_type(element_shape.java_ty.clone());
    } else {
        element_shape.java_ty = element_ty;
    }
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

pub fn set_element_target_match_policy(
    target: &RustSlotShape,
    set_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> SetElementTargetMatchPolicy {
    SetElementTargetMatchPolicy {
        target: target.clone(),
        set_shape: set_shape.clone(),
        element_shape: element_shape.clone(),
        matches: target.java_ty == set_shape.java_ty && target.is_sequence_of(element_shape),
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
    } else if matches!(
        &element_shape.java_ty,
        JavaType::Named { name, .. } if crate::stdlib::boxed_primitive_type(name).is_some()
    ) {
        BorrowedElementRecoveryPolicy::CopyElements
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
        OwnedRecoveryCapability::Copy | OwnedRecoveryCapability::ImmutableSharedHandle { .. } => {
            BorrowedElementRecoveryPolicy::CopyElements
        }
        OwnedRecoveryCapability::ExplicitClone { java_ty }
        | OwnedRecoveryCapability::CloneBoxAdapter { java_ty }
        | OwnedRecoveryCapability::FactoryCopy { java_ty } => {
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
        OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. }
        | OwnedRecoveryCapability::FfiOwnedHandle { .. } => {
            BorrowedElementRecoveryPolicy::Unsupported {
                policy: BorrowedElementRecoveryFallbackPolicy::NonCloneableElement {
                    java_ty: element_shape.java_ty.clone(),
                },
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedElementAdapterPolicy {
    pub carrier: BorrowedElementCarrier,
    pub element_shape: RustSlotShape,
    pub recovery: BorrowedElementRecoveryPolicy,
    pub unwrap_carrier: bool,
}

impl BorrowedElementAdapterPolicy {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.recovery.unsupported_diagnostics(&self.element_shape)
    }
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

impl BorrowedElementSlotAdapterPolicy {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.adapter
            .as_ref()
            .map(BorrowedElementAdapterPolicy::unsupported_diagnostics)
            .unwrap_or_default()
    }
}

pub fn borrowed_element_adapter_policy(
    carrier: BorrowedElementCarrier,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> BorrowedElementAdapterPolicy {
    borrowed_element_adapter_policy_with_unwrap(carrier, element_shape, facts, false)
}

pub fn borrowed_element_adapter_policy_with_unwrap(
    carrier: BorrowedElementCarrier,
    element_shape: &RustSlotShape,
    facts: &FactSet,
    unwrap_carrier: bool,
) -> BorrowedElementAdapterPolicy {
    BorrowedElementAdapterPolicy {
        carrier,
        element_shape: element_shape.clone(),
        recovery: borrowed_element_recovery_policy(element_shape, facts),
        unwrap_carrier,
    }
}

pub fn borrowed_element_slot_adapter_policy(
    carrier: BorrowedElementCarrier,
    target: &RustSlotShape,
    carrier_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> BorrowedElementSlotAdapterPolicy {
    let carrier_matches = match carrier {
        BorrowedElementCarrier::Option => {
            optional_element_target_match_policy(target, carrier_shape, element_shape).matches
        }
        BorrowedElementCarrier::Vec => {
            vec_element_target_match_policy(target, carrier_shape, element_shape).matches
        }
        BorrowedElementCarrier::Set => {
            set_element_target_match_policy(target, carrier_shape, element_shape).matches
        }
    };
    let element_matches = matches!(carrier, BorrowedElementCarrier::Option)
        && target.optional_element_type().is_none()
        && borrowed_element_target_matches_element(target, element_shape, facts);
    let matches = carrier_matches || element_matches;
    let exact_optional_element_match = matches!(carrier, BorrowedElementCarrier::Option)
        && target.optional_element_type().is_some_and(|element_ty| {
            optional_element_slot_shape_matches(target, element_ty, element_shape)
        });
    let optional_element_view_projection =
        optional_element_borrow_target_projection(carrier, target, element_shape, facts);
    let adapter = if matches && !exact_optional_element_match {
        Some(
            if optional_element_view_projection
                || (element_matches
                    && matches!(
                        target.ownership,
                        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                    ))
            {
                BorrowedElementAdapterPolicy {
                    carrier,
                    element_shape: element_shape.clone(),
                    recovery: BorrowedElementRecoveryPolicy::BorrowElement,
                    unwrap_carrier: element_matches,
                }
            } else {
                borrowed_element_adapter_policy_with_unwrap(
                    carrier,
                    element_shape,
                    facts,
                    element_matches,
                )
            },
        )
    } else {
        None
    };
    BorrowedElementSlotAdapterPolicy {
        target: target.clone(),
        carrier_shape: carrier_shape.clone(),
        element_shape: element_shape.clone(),
        carrier,
        matches,
        adapter,
    }
}

fn optional_element_borrow_target_projection(
    carrier: BorrowedElementCarrier,
    target: &RustSlotShape,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> bool {
    if !matches!(carrier, BorrowedElementCarrier::Option) {
        return false;
    }
    let Some(target_element_ty) = target.optional_element_type() else {
        return false;
    };
    let target_element = optional_element_slot_shape(target, target_element_ty);
    matches!(
        target_element.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && borrowed_element_target_matches_element(&target_element, element_shape, facts)
        && (target_element.ownership != element_shape.ownership
            || target_element.view != element_shape.view
            || target_element.dispatch != element_shape.dispatch)
}

fn optional_element_slot_shape_matches(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    actual_element_shape: &RustSlotShape,
) -> bool {
    let target_element = optional_element_slot_shape(option_shape, element_ty);
    target_element.java_ty == actual_element_shape.java_ty
        && target_element.ownership == actual_element_shape.ownership
        && target_element.view == actual_element_shape.view
        && target_element.dispatch == actual_element_shape.dispatch
}

pub(crate) fn borrowed_element_target_matches_element(
    target: &RustSlotShape,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> bool {
    target.java_ty == element_shape.java_ty
        || boxed_primitive_element_matches_target(&element_shape.java_ty, &target.java_ty)
        || facts
            .hierarchy_subtype_policy(&element_shape.java_ty, &target.java_ty)
            .reaches
        || external_conversion_adapter_policy(element_shape, target, facts).is_some()
        || known_structural_element_matches_target(&element_shape.java_ty, &target.java_ty)
}

fn boxed_primitive_element_matches_target(element_ty: &JavaType, target_ty: &JavaType) -> bool {
    match (element_ty, target_ty) {
        (JavaType::Named { name, args }, JavaType::Primitive(target_primitive))
            if args.is_empty() =>
        {
            crate::stdlib::boxed_primitive_type(name) == Some(*target_primitive)
        }
        _ => false,
    }
}

fn known_structural_element_matches_target(element_ty: &JavaType, target_ty: &JavaType) -> bool {
    let (JavaType::Named { name: element, .. }, JavaType::Named { name: target, .. }) =
        (element_ty, target_ty)
    else {
        return false;
    };
    crate::stdlib::is_known_java_subtype(element, target)
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
    if (actual.java_ty == target.java_ty || matches!(actual.java_ty, JavaType::Unknown(_)))
        && matches!(
            runtime_type_target_policy(&target.java_ty),
            RuntimeTypeTargetPolicy::MissingNominalTarget { .. }
        )
    {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if matches!(actual.java_ty, JavaType::Unknown(_)) && target.optional_element_type().is_some() {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if matches!(actual.dispatch, DispatchShape::ErasedObject)
        && java_type_is_erased_object_like(&target.java_ty)
    {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::Concrete, DispatchShape::Concrete) => {
            if let Some(adapter) = external_conversion_adapter_policy(actual, target, facts) {
                adapter
            } else if actual.java_ty != target.java_ty
                && target.ownership == OwnershipShape::SharedBorrow
                && borrowed_element_target_matches_element(target, actual, facts)
                && let Some(target_type) =
                    rust_nominal_type_name_policy(&target.java_ty).target_type
            {
                DispatchCoercionPolicy::BorrowConcreteFacade { target_type }
            } else {
                DispatchCoercionPolicy::NoDispatchChange
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::ErasedObject) => {
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
            if external_crate_mapped_dyn_trait_boundary(trait_name, facts) {
                return DispatchCoercionPolicy::NoDispatchChange;
            }
            if actual.ownership == OwnershipShape::SharedBorrow
                && actual.java_ty == target.java_ty
                && object_safe_trait_boundary(&target.java_ty, facts)
            {
                return DispatchCoercionPolicy::BorrowBoxedDynTrait {
                    trait_name: trait_name.clone(),
                };
            }
            DispatchCoercionPolicy::BorrowDynTrait {
                trait_name: trait_name.clone(),
            }
        }
        (DispatchShape::ClosedEnum { .. }, DispatchShape::DynTrait { trait_name }) => {
            if actual.java_ty != target.java_ty
                && facts
                    .hierarchy_subtype_policy(&actual.java_ty, &target.java_ty)
                    .reaches
            {
                DispatchCoercionPolicy::BorrowDynTrait {
                    trait_name: trait_name.clone(),
                }
            } else {
                unsupported_dispatch_coercion_policy(actual, target)
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
            if external_crate_mapped_dyn_trait_boundary(trait_name, facts) {
                return DispatchCoercionPolicy::EraseObject;
            }
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
        ) if actual_trait == target_trait => DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: target_trait.clone(),
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

fn external_crate_mapped_dyn_trait_boundary(trait_name: &str, facts: &FactSet) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    let java_ty = JavaType::Named {
        name: trait_name.to_string(),
        args: Vec::new(),
    };
    matches!(
        effective_facts.object_representation_override_strategy_for(&java_ty),
        Some(HierarchyStrategy::DynTrait { .. })
    ) && effective_facts
        .external_library_pack_for_type(&crate::stdlib::canonical_external_receiver_type_key(
            trait_name,
        ))
        .is_some_and(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
}

fn object_safe_trait_boundary(java_ty: &JavaType, facts: &FactSet) -> bool {
    match java_ty {
        JavaType::Named { name, args } if args.is_empty() => {
            facts.object_safe_trait_fact_for_name(name) == Some(true)
        }
        _ => false,
    }
}

fn external_conversion_adapter_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> Option<DispatchCoercionPolicy> {
    let from_type = external_conversion_adapter_type_name(&actual.java_ty)?;
    let to_type = external_conversion_adapter_type_name(&target.java_ty)?;
    let adapter = facts
        .external_conversion_adapter_for(from_type, to_type)
        .cloned()
        .or_else(|| crate::external_packs::external_conversion_adapter_for(from_type, to_type))?;
    match adapter.kind {
        ExternalConversionAdapterKind::Into => Some(DispatchCoercionPolicy::ExternalInto {
            from_type: actual.java_ty.clone(),
            to_type: target.java_ty.clone(),
        }),
    }
}

fn external_conversion_adapter_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } => Some(name),
        JavaType::Unknown(name) => Some(name),
        _ => None,
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

impl ShapeCoercionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        let mut diagnostics = Vec::new();
        diagnostics.extend(self.dispatch.unsupported_diagnostics());
        diagnostics.extend(self.ownership.unsupported_diagnostics());
        diagnostics.extend(self.optional.unsupported_diagnostics());
        diagnostics
    }
}

impl OwnershipCoercionPolicy {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            OwnershipCoercionPolicy::UnsupportedRecovery {
                actual_ownership,
                target_ownership,
                policy,
            } => vec![ownership_coercion_fallback_diagnostic(
                policy,
                *actual_ownership,
                *target_ownership,
            )],
            _ => Vec::new(),
        }
    }
}

fn ownership_coercion_fallback_diagnostic(
    policy: &OwnershipCoercionFallbackPolicy,
    actual_ownership: OwnershipShape,
    target_ownership: OwnershipShape,
) -> ConversionDiagnostic {
    match policy {
        OwnershipCoercionFallbackPolicy::CompileError => ConversionDiagnostic {
            code: "unsupported_ownership_coercion",
            message: format!(
                "unsupported Java ownership coercion: actual={actual_ownership:?}, target={target_ownership:?}"
            ),
        },
        OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable { java_ty } => {
            ConversionDiagnostic {
                code: "unsupported_borrowed_to_owned_non_cloneable",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for non-cloneable type {java_ty:?}"
                ),
            }
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle { java_ty } => {
            ConversionDiagnostic {
                code: "unsupported_borrowed_to_facade_owned_external_handle",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for facade-owned external handle {java_ty:?}; use an explicit facade ownership transfer"
                ),
            }
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFfiOwnedHandle { java_ty } => {
            ConversionDiagnostic {
                code: "unsupported_borrowed_to_ffi_owned_handle",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for FFI-owned handle {java_ty:?}; use an explicit FFI ownership transfer"
                ),
            }
        }
    }
}

impl DispatchCoercionPolicy {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            DispatchCoercionPolicy::Unsupported {
                actual_dispatch,
                target_dispatch,
                policy,
            } => vec![dispatch_coercion_fallback_diagnostic(
                policy,
                actual_dispatch,
                target_dispatch,
            )],
            _ => Vec::new(),
        }
    }
}

fn dispatch_coercion_fallback_diagnostic(
    policy: &DispatchCoercionFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> ConversionDiagnostic {
    match policy {
        DispatchCoercionFallbackPolicy::CompileError => ConversionDiagnostic {
            code: "unsupported_dispatch_coercion",
            message: format!(
                "unsupported Java dispatch coercion: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        },
        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty } => ConversionDiagnostic {
            code: "unsupported_dispatch_missing_runtime_target",
            message: format!(
                "unsupported Java dispatch coercion: target type {java_ty:?} has no concrete runtime type"
            ),
        },
        DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
            trait_name,
            recovery,
        } => borrowed_dyn_to_owned_boxed_dyn_diagnostic(trait_name, recovery),
        DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait { trait_name } => {
            ConversionDiagnostic {
                code: "unsupported_erased_object_to_dyn_trait",
                message: format!(
                    "unsupported Java erased object to borrowed dyn trait recovery for {trait_name}; a concrete downcast target is required"
                ),
            }
        }
        DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            ConversionDiagnostic {
                code: "unsupported_erased_object_to_boxed_dyn_trait",
                message: format!(
                    "unsupported Java erased object to owned boxed dyn trait recovery for {trait_name}; a concrete downcast target and ownership proof are required"
                ),
            }
        }
    }
}

fn borrowed_dyn_to_owned_boxed_dyn_diagnostic(
    trait_name: &str,
    recovery: &BorrowedDynToOwnedBoxedDynRecoveryPolicy,
) -> ConversionDiagnostic {
    let requirement = dyn_clone_box_adapter_requirement(trait_name, recovery);
    match dyn_clone_box_adapter_generation_policy(&requirement) {
        DynCloneBoxAdapterGenerationPolicy::AlreadyAvailable { trait_name } => {
            ConversionDiagnostic {
                code: "borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter_proven",
                message: format!(
                    "Java borrowed dyn trait to owned boxed dyn recovery for {trait_name} has a proven clone-box adapter"
                ),
            }
        }
        DynCloneBoxAdapterGenerationPolicy::GenerateAdapter { trait_name, policy } => {
            let proof = match policy {
                CloneRecoveryPolicy::ProvenCloneCapability { .. } => "explicit clone proof exists",
                CloneRecoveryPolicy::AuditRequired => "only conservative clone proof exists",
            };
            ConversionDiagnostic {
                code: "borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter_required",
                message: format!(
                    "Java borrowed dyn trait to owned boxed dyn recovery for {trait_name}; {proof}, and dyn clone-box adapter generation is required"
                ),
            }
        }
        DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported { trait_name, reason } => {
            dyn_clone_box_adapter_deferral_message(&trait_name, reason)
        }
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
    let policy = syntax_slot_coercion_policy_from_content(actual, slot, &content);
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
    syntax_slot_coercion_policy_from_content(actual, slot, &content)
}

fn syntax_slot_coercion_policy_from_content(
    actual: &JavaType,
    slot: &Slot,
    content: &CoercionContentPolicy,
) -> SyntaxSlotCoercionPolicy {
    match slot {
        Slot::Index => SyntaxSlotCoercionPolicy::IndexToUsize,
        Slot::StringConcatSegment | Slot::DisplaySegment
            if java_type_is_erased_object_display_like(actual) =>
        {
            SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if java_type_needs_java_object_display_fallback(actual, content) =>
        {
            SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString
        }
        Slot::StringConcatSegment if !content.string_content => {
            SyntaxSlotCoercionPolicy::ToStringForConcat
        }
        _ => SyntaxSlotCoercionPolicy::Identity,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfCheck {
    JavaSerializableMarker,
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
    OptionalErasedObjectType {
        target_type: String,
    },
    AlwaysTrue,
    Unsupported {
        actual_dispatch: DispatchShape,
        target_dispatch: DispatchShape,
        policy: InstanceOfFallbackPolicy,
    },
}

impl InstanceOfCheck {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            InstanceOfCheck::Unsupported {
                actual_dispatch,
                target_dispatch,
                policy,
            } => vec![instanceof_fallback_diagnostic(
                policy,
                actual_dispatch,
                target_dispatch,
            )],
            _ => Vec::new(),
        }
    }
}

fn instanceof_fallback_diagnostic(
    policy: &InstanceOfFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> ConversionDiagnostic {
    match policy {
        InstanceOfFallbackPolicy::CompileError => ConversionDiagnostic {
            code: "unsupported_instanceof_shape",
            message: format!(
                "unsupported Java instanceof shape: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        },
        InstanceOfFallbackPolicy::PrimitiveTarget { java_ty } => ConversionDiagnostic {
            code: "unsupported_instanceof_primitive_target",
            message: format!(
                "unsupported Java instanceof check: primitive target type {java_ty:?}"
            ),
        },
        InstanceOfFallbackPolicy::RawOrWildcardTarget { java_ty } => ConversionDiagnostic {
            code: "unsupported_instanceof_raw_or_wildcard_target",
            message: format!(
                "unsupported Java instanceof check: raw or wildcard target type {java_ty:?}"
            ),
        },
        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty } => ConversionDiagnostic {
            code: "unsupported_instanceof_missing_runtime_target",
            message: format!(
                "unsupported Java instanceof check: target type {java_ty:?} has no concrete runtime type"
            ),
        },
        InstanceOfFallbackPolicy::ErasedObjectToDynTrait { trait_name } => ConversionDiagnostic {
            code: "unsupported_erased_object_instanceof_dyn_trait",
            message: format!(
                "unsupported Java erased object instanceof borrowed dyn trait check for {trait_name}; a concrete runtime target is required"
            ),
        },
        InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            ConversionDiagnostic {
                code: "unsupported_erased_object_instanceof_boxed_dyn_trait",
                message: format!(
                    "unsupported Java erased object instanceof owned boxed dyn trait check for {trait_name}; a concrete runtime target is required"
                ),
            }
        }
    }
}

pub fn plan_instanceof_check(actual: &RustSlotShape, target: &RustSlotShape) -> InstanceOfCheck {
    plan_instanceof_check_with_facts(actual, target, &FactSet::default())
}

pub fn instanceof_target_policy(target: &RustSlotShape, facts: &FactSet) -> InstanceOfTargetPolicy {
    let capability = facts.instanceof_target_capability_policy(&target.java_ty);
    if capability.primitive_target {
        return InstanceOfTargetPolicy::PrimitiveTarget {
            java_ty: target.java_ty.clone(),
        };
    }
    if capability.raw_or_wildcard_target {
        return InstanceOfTargetPolicy::RawOrWildcardTarget {
            java_ty: target.java_ty.clone(),
        };
    }
    match &target.dispatch {
        DispatchShape::Concrete => match runtime_type_target_policy(&target.java_ty) {
            RuntimeTypeTargetPolicy::Nominal { target_type } => {
                InstanceOfTargetPolicy::ConcreteRuntimeTarget { target_type }
            }
            RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                InstanceOfTargetPolicy::MissingRuntimeTarget { java_ty }
            }
        },
        DispatchShape::ClosedEnum { enum_name } => InstanceOfTargetPolicy::ClosedEnumTarget {
            enum_name: enum_name.clone(),
        },
        DispatchShape::DynTrait { trait_name } => InstanceOfTargetPolicy::DynTraitTarget {
            trait_name: trait_name.clone(),
        },
        DispatchShape::BoxedDynTrait { trait_name } => {
            InstanceOfTargetPolicy::BoxedDynTraitTarget {
                trait_name: trait_name.clone(),
            }
        }
        DispatchShape::ErasedObject => InstanceOfTargetPolicy::ErasedObjectTarget,
    }
}

pub fn plan_instanceof_check_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> InstanceOfCheck {
    if is_java_serializable_marker_type(&target.java_ty) {
        return InstanceOfCheck::JavaSerializableMarker;
    }
    match instanceof_target_policy(target, facts) {
        InstanceOfTargetPolicy::PrimitiveTarget { java_ty } => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::PrimitiveTarget { java_ty },
            );
        }
        InstanceOfTargetPolicy::RawOrWildcardTarget { java_ty } => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::RawOrWildcardTarget { java_ty },
            );
        }
        InstanceOfTargetPolicy::MissingRuntimeTarget { java_ty } => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
            );
        }
        _ => {}
    }
    if actual.java_ty == target.java_ty {
        return InstanceOfCheck::AlwaysTrue;
    }
    if hierarchy_instanceof_policy(actual, target, facts).always_true {
        return InstanceOfCheck::AlwaysTrue;
    }
    if let Some(element_ty) = actual.optional_element_type() {
        let element_shape = optional_element_slot_shape(actual, element_ty);
        if matches!(element_shape.dispatch, DispatchShape::ErasedObject)
            && matches!(target.dispatch, DispatchShape::Concrete)
        {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::OptionalErasedObjectType { target_type };
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
    }
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

fn is_java_serializable_marker_type(ty: &JavaType) -> bool {
    matches!(
        ty,
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Serializable"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyInstanceOfPolicy {
    pub actual: JavaType,
    pub target: JavaType,
    pub always_true: bool,
}

pub fn hierarchy_instanceof_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> HierarchyInstanceOfPolicy {
    let subtype = facts.hierarchy_subtype_policy(&actual.java_ty, &target.java_ty);
    HierarchyInstanceOfPolicy {
        actual: actual.java_ty.clone(),
        target: target.java_ty.clone(),
        always_true: subtype.reaches,
    }
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
    OptionalConstruction(OptionalSlotConstructionPlan),
    BorrowedElement(BorrowedElementSlotAdapterPolicy),
    RustScalarCast(RustScalarSlotCastPolicy),
    RustBorrowedFacadeReturn(RustBorrowedFacadeReturnPolicy),
    OptionalRustBorrowedFacadeReturn(RustBorrowedFacadeReturnPolicy),
    RustUnsignedByteArrayReturn(RustUnsignedByteArrayReturnPolicy),
    Identity { actual: LoweredType, slot: Slot },
}

impl SlotConversionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            SlotConversionPlan::Shape(plan) => plan.unsupported_diagnostics(),
            SlotConversionPlan::OptionalConstruction(plan) => {
                plan.element_plan.unsupported_diagnostics()
            }
            SlotConversionPlan::BorrowedElement(policy) => policy.unsupported_diagnostics(),
            SlotConversionPlan::Syntax(_)
            | SlotConversionPlan::RustScalarCast(_)
            | SlotConversionPlan::RustBorrowedFacadeReturn(_)
            | SlotConversionPlan::OptionalRustBorrowedFacadeReturn(_)
            | SlotConversionPlan::RustUnsignedByteArrayReturn(_)
            | SlotConversionPlan::Identity { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastConversionPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
    pub semantics: JavaCastSemanticsPolicy,
    pub coercion: ShapeCoercionPlan,
}

impl CastConversionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.coercion.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceOfPlan {
    pub actual: RustSlotShape,
    pub target: RustSlotShape,
    pub check: InstanceOfCheck,
}

impl InstanceOfPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.check.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotExprConversionPlan {
    pub actual: RustSlotShape,
    pub slot: Slot,
    pub conversion: SlotConversionPlan,
}

impl SlotExprConversionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.conversion.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSlotExprConversionPlan {
    pub actual: LoweredType,
    pub slot: Slot,
    pub conversion: SlotConversionPlan,
}

impl LoweredSlotExprConversionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.conversion.unsupported_diagnostics()
    }
}

pub fn receiver_method_autoref_satisfies_facade_slot(
    receiver_abi: crate::facts::ReceiverAbi,
    plan: &LoweredSlotExprConversionPlan,
) -> bool {
    let SlotConversionPlan::Shape(shape_plan) = &plan.conversion else {
        return false;
    };
    matches!(
        (&shape_plan.dispatch, &shape_plan.view, &shape_plan.optional),
        (
            DispatchCoercionPolicy::NoDispatchChange,
            ViewCoercionPolicy::NoViewChange,
            OptionalShapeCoercionPolicy::NoChange
        )
    ) && shape_plan.primitive_widening.is_none()
        && matches!(
            (receiver_abi, &shape_plan.ownership),
            (
                crate::facts::ReceiverAbi::BorrowedSelf,
                OwnershipCoercionPolicy::BorrowOwnedValue
            ) | (
                crate::facts::ReceiverAbi::MutBorrowedSelf,
                OwnershipCoercionPolicy::BorrowOwnedMutably
            )
        )
}

pub fn receiver_method_autoref_satisfies_optional_slot(
    plan: &LoweredSlotExprConversionPlan,
) -> bool {
    let SlotConversionPlan::Shape(shape_plan) = &plan.conversion else {
        return false;
    };
    matches!(
        shape_plan.ownership,
        OwnershipCoercionPolicy::NoOwnershipChange
    ) && matches!(shape_plan.optional, OptionalShapeCoercionPolicy::NoChange)
        && shape_plan.primitive_widening.is_none()
}

pub fn plan_cast_conversion(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> CastConversionPlan {
    let actual = infer_expr_shape_with_context(expr, env, ctx);
    plan_cast_conversion_from_actual(actual, target_ty, ctx)
}

pub fn plan_cast_conversion_from_actual(
    actual: RustSlotShape,
    target_ty: &JavaType,
    ctx: ExprShapeContext<'_>,
) -> CastConversionPlan {
    let mut target = project_java_value_shape(ctx.representations, target_ty.clone());
    if java_type_is_erased_object_like(target_ty)
        && matches!(actual.dispatch, DispatchShape::ErasedObject)
    {
        target = actual.clone();
    }
    if same_java_type_cast_should_preserve_actual_shape(&actual, &target) {
        target = actual.clone();
    }
    let default_facts = FactSet::default();
    let facts = ctx.facts.unwrap_or(&default_facts);
    let coercion = shape_coercion_plan_with_facts(&actual, &target, facts);
    let semantics = java_cast_semantics_policy_with_facts(&actual, &target, &coercion, facts);

    CastConversionPlan {
        actual,
        target,
        semantics,
        coercion,
    }
}

fn same_java_type_cast_should_preserve_actual_shape(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> bool {
    if !java_types_have_same_erased_cast_identity(&actual.java_ty, &target.java_ty) {
        return false;
    }
    if !matches!(actual.dispatch, DispatchShape::Concrete)
        || !matches!(actual.view, ViewShape::Native)
    {
        return false;
    }
    matches!(target.dispatch, DispatchShape::ErasedObject)
        || (matches!(target.dispatch, DispatchShape::Concrete)
            && java_type_erases_generic_detail(&actual.java_ty, &target.java_ty))
}

fn java_types_have_same_erased_cast_identity(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (JavaType::Named { name: left, .. }, JavaType::Named { name: right, .. }) => left == right,
        _ => left == right,
    }
}

fn java_type_erases_generic_detail(actual: &JavaType, target: &JavaType) -> bool {
    matches!(
        (actual, target),
        (
            JavaType::Named { args: actual_args, .. },
            JavaType::Named { args: target_args, .. }
        ) if !actual_args.is_empty() && target_args.is_empty()
    )
}

pub fn plan_instanceof(
    expr: &Expr,
    target_ty: &JavaType,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> InstanceOfPlan {
    let actual = infer_expr_shape_with_context(expr, env, ctx);
    let target = project_java_value_shape(ctx.representations, target_ty.clone());
    let default_facts = FactSet::default();
    let facts = ctx.facts.unwrap_or(&default_facts);
    let check = plan_instanceof_check_with_facts(&actual, &target, facts);

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
        Slot::StringConcatSegment | Slot::DisplaySegment
            if actual.optional_element_type().is_some_and(|element_ty| {
                coercion_content_policy(&element_ty, facts).string_content
            }) =>
        {
            SlotConversionPlan::Syntax(SyntaxCoercionPlan {
                actual: actual.java_ty.clone(),
                slot: slot.clone(),
                content: coercion_content_policy(&actual.java_ty, facts),
                policy: SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString,
            })
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if actual
                .optional_element_type()
                .is_some_and(|element_ty| optional_display_value_supported(&element_ty, facts)) =>
        {
            SlotConversionPlan::Syntax(SyntaxCoercionPlan {
                actual: actual.java_ty.clone(),
                slot: slot.clone(),
                content: coercion_content_policy(&actual.java_ty, facts),
                policy: SyntaxSlotCoercionPolicy::OptionalToJavaDisplayValue,
            })
        }
        _ => SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts(
            &actual.java_ty,
            slot,
            facts,
        )),
    }
}

fn optional_display_value_supported(java_ty: &JavaType, facts: &FactSet) -> bool {
    let content = coercion_content_policy(java_ty, facts);
    if content.string_content {
        return false;
    }
    match java_ty {
        JavaType::Primitive(_) => true,
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::is_java_platform_facade_type_name(name)
        }
        JavaType::String | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => false,
    }
}

pub fn actual_shape_for_slot_expr(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    if let Expr::Cast { ty, expr: inner } = expr {
        let inner_shape = infer_expr_shape_with_context(inner, env, ctx);
        if cast_target_only_erases_generic_detail(&inner_shape.java_ty, ty) {
            inner_shape
        } else {
            cast_target_shape_preserving_inner_ownership(ctx, ty.clone(), &inner_shape)
        }
    } else {
        infer_expr_shape_with_context(expr, env, ctx)
    }
}

fn cast_target_shape_preserving_inner_ownership(
    ctx: ExprShapeContext<'_>,
    target_ty: JavaType,
    inner_shape: &RustSlotShape,
) -> RustSlotShape {
    let mut target_shape = project_java_value_shape(ctx.representations, target_ty);
    if !matches!(
        (&inner_shape.java_ty, &target_shape.java_ty),
        (JavaType::Primitive(_), _) | (_, JavaType::Primitive(_))
    ) {
        target_shape.ownership = inner_shape.ownership;
    }
    target_shape
}

fn cast_target_only_erases_generic_detail(actual: &JavaType, target: &JavaType) -> bool {
    match (actual, target) {
        (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: target_name,
                args: target_args,
            },
        ) => {
            actual_name == target_name
                && !actual_args.is_empty()
                && (target_args.is_empty() || actual_args == target_args)
        }
        _ => false,
    }
}

pub fn actual_shape_for_slot_expr_with_slot(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
    slot: &Slot,
) -> RustSlotShape {
    let actual = actual_shape_for_slot_expr(expr, env, ctx);
    if matches!(expr, Expr::Lambda { .. } | Expr::MethodReference { .. })
        && let Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) = slot
    {
        if let Some(element_ty) = target.optional_element_type() {
            return optional_element_slot_shape(target, element_ty);
        }
        return target.clone();
    }
    if matches!(expr, Expr::Call { .. })
        && let Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) = slot
        && target.optional_element_type().is_some()
        && matches!(target.dispatch, DispatchShape::ErasedObject)
        && actual.java_ty == target.java_ty
    {
        let mut actual_option = target.clone();
        actual_option.dispatch = DispatchShape::Concrete;
        actual_option.view = ViewShape::Native;
        return actual_option;
    }
    let Expr::Call { resolved: None, .. } = expr else {
        return actual;
    };
    if !matches!(actual.java_ty, JavaType::Unknown(_)) {
        return actual;
    }
    match slot {
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            if let Some(element_ty) = target.optional_element_type() {
                if matches!(target.dispatch, DispatchShape::ErasedObject)
                    && java_type_is_erased_object_like(&element_ty)
                {
                    RustSlotShape::conservative(actual.java_ty)
                } else {
                    optional_element_slot_shape(target, element_ty)
                }
            } else {
                target.clone()
            }
        }
        _ => actual,
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
            LoweredType::RustScalar {
                java_shape,
                rust_type_name,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) => {
            if let JavaType::Primitive(target_primitive) = target.java_ty {
                SlotConversionPlan::RustScalarCast(RustScalarSlotCastPolicy {
                    rust_type_name: rust_type_name.clone(),
                    java_semantic_shape: (**java_shape).clone(),
                    target: target.clone(),
                    target_primitive,
                })
            } else {
                slot_conversion_plan_with_facts(java_shape, slot, facts)
            }
        }
        (
            LoweredType::RustBorrowed {
                java_shape,
                rust_type_name,
                recovery,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if slot_or_optional_element_targets_erased_object(target) => {
            SlotConversionPlan::RustBorrowedFacadeReturn(RustBorrowedFacadeReturnPolicy {
                rust_type_name: rust_type_name.clone(),
                java_semantic_shape: (**java_shape).clone(),
                target: target.clone(),
                recovery: recovery.unwrap_or(RustBorrowedFacadeReturnRecovery::Clone),
            })
        }
        (
            LoweredType::RustBorrowed { java_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(java_shape, slot, facts),
        (
            LoweredType::RustUnsignedByteArray {
                java_shape,
                optional,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if target_expects_java_byte_array_or_option(target) => {
            SlotConversionPlan::RustUnsignedByteArrayReturn(RustUnsignedByteArrayReturnPolicy {
                java_semantic_shape: (**java_shape).clone(),
                target: target.clone(),
                optional: *optional,
            })
        }
        (
            LoweredType::RustUnsignedByteArray { java_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(java_shape, slot, facts),
        (
            LoweredType::OptionalElementBorrow {
                option_shape,
                element_shape,
                recovery,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) => {
            if slot_or_optional_element_targets_erased_object(target) {
                SlotConversionPlan::OptionalRustBorrowedFacadeReturn(
                    RustBorrowedFacadeReturnPolicy {
                        rust_type_name: rust_type_name_for_shape(element_shape),
                        java_semantic_shape: (**element_shape).clone(),
                        target: target.clone(),
                        recovery: recovery.unwrap_or(RustBorrowedFacadeReturnRecovery::Clone),
                    },
                )
            } else {
                SlotConversionPlan::BorrowedElement(borrowed_element_slot_adapter_policy(
                    BorrowedElementCarrier::Option,
                    target,
                    option_shape,
                    element_shape,
                    facts,
                ))
            }
        }
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
        (
            LoweredType::SetElementBorrow {
                set_shape,
                element_shape,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) => SlotConversionPlan::BorrowedElement(borrowed_element_slot_adapter_policy(
            BorrowedElementCarrier::Set,
            target,
            set_shape,
            element_shape,
            facts,
        )),
        _ => SlotConversionPlan::Identity {
            actual: actual.clone(),
            slot: slot.clone(),
        },
    }
}

fn rust_type_name_for_shape(shape: &RustSlotShape) -> String {
    match &shape.java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

fn target_expects_java_byte_array_or_option(target: &RustSlotShape) -> bool {
    java_type_is_byte_array(&target.java_ty)
        || target
            .optional_element_type()
            .is_some_and(|element_ty| java_type_is_byte_array(&element_ty))
}

fn erased_object_clone_recovery_sequence(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> Option<bool> {
    if !matches!(
        (actual.ownership, target.ownership),
        (OwnershipShape::SharedBorrow, OwnershipShape::Owned)
    ) {
        return None;
    }
    if java_type_is_erased_object_sequence_like(&actual.java_ty) {
        return Some(true);
    }
    if java_type_is_erased_object_like(&actual.java_ty) {
        return Some(false);
    }
    None
}

fn java_type_is_erased_object_display_like(java_ty: &JavaType) -> bool {
    java_type_is_erased_object_like(java_ty) || java_type_is_erased_object_sequence_like(java_ty)
}

fn java_type_needs_java_object_display_fallback(
    java_ty: &JavaType,
    content: &CoercionContentPolicy,
) -> bool {
    if content.string_content {
        return false;
    }
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let receiver_key = crate::stdlib::canonical_external_receiver_type_key(name);
            let receiver_simple = receiver_key.rsplit('.').next().unwrap_or(&receiver_key);
            !matches!(
                receiver_simple,
                "String"
                    | "StringBuilder"
                    | "StringBuffer"
                    | "CharSequence"
                    | "Appendable"
                    | "Class"
                    | "Path"
                    | "File"
                    | "URI"
            ) && !crate::stdlib::is_java_platform_facade_type_name(&receiver_key)
        }
        JavaType::String
        | JavaType::Primitive(_)
        | JavaType::Void
        | JavaType::Array(_)
        | JavaType::Wildcard(_) => false,
    }
}

fn java_type_is_erased_object_sequence_like(java_ty: &JavaType) -> bool {
    crate::types::sequence_element_type(java_ty).is_some_and(|element_ty| {
        java_type_is_erased_object_like(&element_ty)
            || java_type_is_erased_object_sequence_like(&element_ty)
    })
}

fn java_type_is_byte_array(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Array(element)
            if matches!(element.as_ref(), JavaType::Primitive(PrimitiveType::Byte))
    )
}

fn slot_or_optional_element_targets_erased_object(target: &RustSlotShape) -> bool {
    if matches!(target.dispatch, DispatchShape::ErasedObject) {
        return true;
    }
    target.optional_element_type().is_some_and(|element_ty| {
        matches!(
            optional_element_slot_shape(target, element_ty.clone()).dispatch,
            DispatchShape::ErasedObject
        ) || java_type_is_erased_object_like(&element_ty)
    })
}

fn java_type_is_erased_object_like(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Unknown(name) | JavaType::Named { name, .. }
        if name.eq_ignore_ascii_case("Object") || name.eq_ignore_ascii_case("java.lang.Object"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::ReceiverAbi;
    use crate::shape_projection::borrowed_string_content_shape;

    #[test]
    fn receiver_method_autoref_predicates_are_conversion_plan_policy() {
        let object_ty = JavaType::Unknown("FacadeValue".to_string());
        let owned_object =
            LoweredType::Slot(Box::new(RustSlotShape::for_storage(object_ty.clone())));
        let mut borrowed_object = RustSlotShape::for_storage(object_ty);
        borrowed_object.ownership = OwnershipShape::SharedBorrow;
        let facade_plan = plan_lowered_slot_conversion(
            &owned_object,
            argument_slot(borrowed_object.clone()),
            &FactSet::default(),
        );
        assert!(receiver_method_autoref_satisfies_facade_slot(
            ReceiverAbi::BorrowedSelf,
            &facade_plan
        ));
        assert!(!receiver_method_autoref_satisfies_optional_slot(
            &facade_plan
        ));

        let borrowed_string = borrowed_string_content_shape();
        let optional_receiver_plan = LoweredSlotExprConversionPlan {
            actual: LoweredType::Slot(Box::new(borrowed_string.clone())),
            slot: argument_slot(borrowed_string.clone()),
            conversion: SlotConversionPlan::Shape(shape_coercion_plan(
                &borrowed_string,
                &borrowed_string,
            )),
        };
        assert!(receiver_method_autoref_satisfies_optional_slot(
            &optional_receiver_plan
        ));
        assert!(!receiver_method_autoref_satisfies_facade_slot(
            ReceiverAbi::BorrowedSelf,
            &optional_receiver_plan
        ));
    }
}
