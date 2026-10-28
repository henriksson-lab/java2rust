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
use crate::representation::RepresentationPlan;
use crate::shape_projection::{borrowed_sequence_element_shape, project_java_value_shape};
use crate::slot_shape::{
    DispatchShape, OwnershipShape, RustSlotShape, ViewShape, java_types_have_same_erased_identity,
};
use crate::types::{
    is_numeric_primitive_type, list_java_type, optional_element_type, optional_java_type,
    set_java_type,
};

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
    slot: Slot,
}

impl AssignmentValuePlan {
    pub(crate) fn new(slot: Slot) -> Self {
        Self { slot }
    }

    pub fn slot(&self) -> &Slot {
        &self.slot
    }

    pub fn into_slot(self) -> Slot {
        self.slot
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentStatementPlan {
    ClearOptionalTarget,
    AssignValue,
}

pub(crate) fn assignment_target_is_direct_optional_place(target: &Expr) -> bool {
    matches!(target, Expr::Var(_) | Expr::FieldAccess(..))
}

pub fn assignment_statement_plan_for_target(
    target: &Expr,
    value: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> AssignmentStatementPlan {
    if matches!(value, Expr::Literal(crate::java_ir::Literal::Null))
        && assignment_target_is_direct_optional_place(target)
        && infer_assignment_target_shape_with_context(target, env, ctx)
            .is_some_and(|shape| shape.optional_element_type().is_some())
    {
        AssignmentStatementPlan::ClearOptionalTarget
    } else {
        AssignmentStatementPlan::AssignValue
    }
}

pub fn assignment_value_plan_for_shape(shape: &RustSlotShape) -> AssignmentValuePlan {
    AssignmentValuePlan::new(assignment_slot(shape.clone()))
}

pub fn assignment_value_plan_for_target(
    target: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> AssignmentValuePlan {
    AssignmentValuePlan::new(assignment_slot_for_target(target, env, ctx))
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
    /// See `Coercion::ToJavaFloatStringForConcat`.
    ToJavaFloatStringForConcat {
        double: bool,
    },
    Identity,
    IndexToUsize,
    ToStringForConcat,
    CollectionToJavaDisplayString,
    SetToJavaDisplayString,
    MapToJavaDisplayString,
    OptionalCollectionToJavaDisplayString {
        kind: crate::stdlib::JavaCollectionKind,
    },
    ErasedObjectToJavaDisplayString,
    OptionalErasedObjectToJavaDisplayString,
    OptionalToJavaDisplayString,
    OptionalToJavaDisplayValue,
    OptionalToDisplayValue,
    OptionalBorrowedOptionalStringToJavaDisplayString,
    OptionalBorrowedToJavaDisplayValue,
    OptionalBorrowedToDisplayValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewCoercionPolicy {
    NoViewChange,
    BorrowStringView,
    OwnStringFromView,
    /// A view of an identity-preserving String borrows its reference carrier,
    /// never the scalar payload. This contract also admits a UTF-16 payload.
    BorrowStringReferenceView {
        source_borrowed: bool,
    },
    OwnStringReferenceFromView,
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
        optional_sequence: bool,
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
    code: &'static str,
    message: String,
}

impl ConversionDiagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
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
    actual: JavaType,
    target: JavaType,
    kind: JavaTypeConversionKind,
}

impl JavaTypeConversionPlan {
    pub(crate) fn new(actual: JavaType, target: JavaType, kind: JavaTypeConversionKind) -> Self {
        Self {
            actual,
            target,
            kind,
        }
    }

    pub fn actual(&self) -> &JavaType {
        &self.actual
    }

    pub fn target(&self) -> &JavaType {
        &self.target
    }

    pub fn kind(&self) -> &JavaTypeConversionKind {
        &self.kind
    }
}

pub fn java_type_conversion_plan_from_evidence(
    evidence: &JavaCoercionEvidence,
) -> JavaTypeConversionPlan {
    JavaTypeConversionPlan::new(
        evidence.actual.clone(),
        evidence.target.clone(),
        java_type_conversion_kind_from_evidence(evidence),
    )
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
    JavaTypeConversionPlan::new(
        actual.java_ty.clone(),
        target.java_ty.clone(),
        JavaTypeConversionKind::Cast(java_cast_semantics_policy_with_facts(
            actual, target, &coercion, facts,
        )),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfFallbackPolicy {
    CompileError,
    SourceHierarchyRuntimeCarrierRequired { actual: JavaType, target: JavaType },
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
pub enum DispatchCoercionFallbackPolicy {
    CompileError,
    MissingRuntimeTarget { java_ty: JavaType },
    ErasedObjectToDynTrait { trait_name: String },
    ErasedObjectToBoxedDynTrait { trait_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchCoercionPolicy {
    NoDispatchChange,
    NativeIterableListView {
        invalid_cast: InvalidCastPolicy,
    },
    WrapClosedEnum {
        enum_name: String,
        variant_name: String,
    },
    BorrowDynTrait {
        trait_name: String,
        mutable: bool,
    },
    BorrowClosedEnumAsDynTrait {
        enum_name: String,
        trait_name: String,
        variants: Vec<String>,
        mutable: bool,
    },
    RewrapClosedEnum {
        actual_enum_name: String,
        target_enum_name: String,
        variants: Vec<String>,
        clone_payload: bool,
    },
    BorrowBoxedDynTrait {
        trait_name: String,
        mutable: bool,
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
    ReferenceDynTrait {
        trait_name: String,
        sync: bool,
    },
    CloneBoxDynTrait {
        trait_name: String,
    },
    UpcastBoxedDynTrait {
        source_trait: String,
        target_trait: String,
        clone_source: bool,
    },
    EraseObject,
    CloneHierarchyToErasedObject,
    /// Clone the concrete Java reference, not its interface adapter box.
    CloneDynTraitToErasedObject {
        trait_name: String,
    },
    DowncastClosedEnum {
        enum_name: String,
        variant_name: String,
        borrowed_payload: bool,
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
        borrowed: bool,
        invalid_cast: InvalidCastPolicy,
    },
    DowncastErasedObjectToClosedEnum {
        enum_name: String,
        carrier_type: String,
        variants: Vec<String>,
        invalid_cast: InvalidCastPolicy,
    },
    Unsupported {
        actual_dispatch: DispatchShape,
        target_dispatch: DispatchShape,
        policy: DispatchCoercionFallbackPolicy,
    },
}

impl DispatchCoercionPolicy {
    /// Runtime-checked narrowing conversions share one failure classification
    /// across Java cast semantics and callable exception planning.
    fn checked_cast_failure_policy(&self) -> Option<&InvalidCastPolicy> {
        match self {
            Self::NativeIterableListView { invalid_cast } => Some(invalid_cast),
            Self::DowncastClosedEnum { invalid_cast, .. }
            | Self::DowncastDynTrait { invalid_cast, .. }
            | Self::DowncastBoxedDynTrait { invalid_cast, .. }
            | Self::DowncastErasedObject { invalid_cast, .. }
            | Self::DowncastErasedObjectToClosedEnum { invalid_cast, .. } => Some(invalid_cast),
            _ => None,
        }
    }
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
        /// Erasing a retained Java reference copies its handle without moving
        /// the source Option shell, irrespective of local use-count estimates.
        borrow_receiver: bool,
        element_plan: Box<ShapeCoercionPlan>,
    },
    NoChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalSlotConstructionPlan {
    target: RustSlotShape,
    element_shape: RustSlotShape,
    element_plan: ShapeCoercionPlan,
    wrapper_ownership: OwnershipShape,
    /// The handle the **element** is stored behind, if any (0002/0002b). On the
    /// plan because the renderer has no `FactSet`, and because deciding the
    /// `Some` and the handle together prevents `Some(Rc::new(Some(..)))`.
    element_handle: Option<crate::storage_strategy::StoragePlacement>,
}

impl OptionalSlotConstructionPlan {
    pub(crate) fn new(
        target: RustSlotShape,
        element_shape: RustSlotShape,
        element_plan: ShapeCoercionPlan,
        wrapper_ownership: OwnershipShape,
        element_handle: Option<crate::storage_strategy::StoragePlacement>,
    ) -> Self {
        Self {
            target,
            element_shape,
            element_plan,
            wrapper_ownership,
            element_handle,
        }
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn element_shape(&self) -> &RustSlotShape {
        &self.element_shape
    }

    pub fn element_plan(&self) -> &ShapeCoercionPlan {
        &self.element_plan
    }

    pub fn wrapper_ownership(&self) -> OwnershipShape {
        self.wrapper_ownership
    }

    pub fn element_handle(&self) -> Option<crate::storage_strategy::StoragePlacement> {
        self.element_handle
    }
}

fn optional_element_handle_placement(
    target: &RustSlotShape,
    facts: &FactSet,
) -> Option<crate::storage_strategy::StoragePlacement> {
    // A closed-enum carrier owns its runtime payload representation.  The
    // hierarchy's storage-placement fact describes the enum variant payload,
    // not an additional handle around the carrier itself.
    if matches!(target.dispatch, DispatchShape::ClosedEnum { .. }) {
        return None;
    }
    let JavaType::Named { name, .. } = target.optional_element_type()? else {
        return None;
    };
    let placement = facts.storage_placement_for(&name);
    (placement.shared() && placement.is_implemented()).then_some(placement)
}

fn optional_element_handle_placement_with_representations(
    target: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<crate::storage_strategy::StoragePlacement> {
    if matches!(target.dispatch, DispatchShape::ClosedEnum { .. }) {
        return None;
    }
    let JavaType::Named { name, .. } = target.optional_element_type()? else {
        return None;
    };
    optional_element_handle_placement(target, facts).or_else(|| {
        let placement = representations.storage_placement_for(&name);
        (placement.shared() && placement.is_implemented()).then_some(placement)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalPresentConstructionSource {
    Plain,
    IndexedBorrowProjection,
    TernaryBranches { any_indexed_borrow_projection: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalPresentConstructionPlan {
    construction: OptionalSlotConstructionPlan,
    element_lowering_slot: Slot,
}

impl OptionalPresentConstructionPlan {
    pub(crate) fn new(
        construction: OptionalSlotConstructionPlan,
        element_lowering_slot: Slot,
    ) -> Self {
        Self {
            construction,
            element_lowering_slot,
        }
    }

    pub fn construction(&self) -> &OptionalSlotConstructionPlan {
        &self.construction
    }

    pub fn element_lowering_slot(&self) -> &Slot {
        &self.element_lowering_slot
    }

    pub fn into_parts(self) -> (OptionalSlotConstructionPlan, Slot) {
        (self.construction, self.element_lowering_slot)
    }
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
    Some(OptionalSlotConstructionPlan::new(
        target.clone(),
        actual_element_shape.clone(),
        element_plan,
        OwnershipShape::Owned,
        optional_element_handle_placement(target, facts),
    ))
}

pub fn optional_slot_construction_plan_for_actual(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> OptionalSlotConstructionPlan {
    optional_slot_construction_plan_with_actual_element(actual_element_shape, target, facts)
        .expect("target must be an optional slot")
}

pub fn optional_slot_construction_plan_for_actual_with_representations(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> OptionalSlotConstructionPlan {
    let mut plan = optional_slot_construction_plan_for_actual(actual_element_shape, target, facts);
    if plan.element_handle.is_none() {
        plan.element_handle =
            optional_element_handle_placement_with_representations(target, facts, representations);
    }
    plan
}

pub fn optional_present_construction_plan_for_actual(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    element_ty: JavaType,
    source: OptionalPresentConstructionSource,
    facts: &FactSet,
) -> OptionalPresentConstructionPlan {
    let target_element_shape =
        optional_element_slot_shape_with_facts(target, element_ty.clone(), facts);
    let mut actual_element_shape = actual_element_shape.clone();
    let indexed_borrow_projection = match source {
        OptionalPresentConstructionSource::Plain => false,
        OptionalPresentConstructionSource::IndexedBorrowProjection => {
            actual_element_shape.ownership == OwnershipShape::SharedBorrow
        }
        OptionalPresentConstructionSource::TernaryBranches {
            any_indexed_borrow_projection,
        } => {
            if any_indexed_borrow_projection {
                actual_element_shape.ownership = OwnershipShape::SharedBorrow;
            }
            false
        }
    };
    if matches!(actual_element_shape.java_ty, JavaType::Unknown(_))
        || actual_element_shape.optional_element_type().is_some()
    {
        actual_element_shape = target_element_shape.clone();
    }
    if indexed_borrow_projection {
        actual_element_shape = RustSlotShape::conservative(actual_element_shape.java_ty.clone());
    } else if !matches!(
        source,
        OptionalPresentConstructionSource::TernaryBranches { .. }
    ) && matches!(
        target_element_shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) {
        actual_element_shape = target_element_shape;
    }
    let construction =
        optional_slot_construction_plan_for_actual(&actual_element_shape, target, facts);
    let element_lowering_slot = if indexed_borrow_projection {
        Slot::Value
    } else {
        Slot::Assignment(construction.element_shape().clone())
    };
    OptionalPresentConstructionPlan::new(construction, element_lowering_slot)
}

pub fn optional_present_construction_plan_for_actual_with_representations(
    actual_element_shape: &RustSlotShape,
    target: &RustSlotShape,
    element_ty: JavaType,
    source: OptionalPresentConstructionSource,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> OptionalPresentConstructionPlan {
    let mut projected_target_element =
        crate::shape_projection::return_shape_for_java_type(representations, &element_ty);
    if let Some(projected_target_element) = projected_target_element.as_mut() {
        projected_target_element.ownership = OwnershipShape::Owned;
        projected_target_element.view = crate::slot_shape::ViewShape::Native;
    }
    let mut physical_target = target.clone();
    if let Some(projected) = projected_target_element.as_ref()
        && matches!(projected.dispatch, DispatchShape::ClosedEnum { .. })
    {
        // Optionality cannot discard the solved source hierarchy carrier.
        // A new Sub in an Option<BaseValue> slot first becomes BaseValue,
        // then Some; it is not an unsupported plain-class upcast.
        physical_target.dispatch = projected.dispatch.clone();
        physical_target.view = projected.view.clone();
    }
    let mut plan = optional_present_construction_plan_for_actual(
        actual_element_shape,
        &physical_target,
        element_ty.clone(),
        source,
        facts,
    );
    // The portable planner normally replaces an owned actual element with
    // the target's nominal element shape.  When whole-program lowering has
    // already selected a closed hierarchy carrier, that replacement loses
    // the emitted ABI and makes lowering downcast the carrier to one variant
    // immediately before wrapping it in `Some`.  Preserve the actual shape
    // when the representation plan proves both spellings are the same closed
    // carrier; the optional shell is the only conversion required.
    if actual_element_shape.ownership == OwnershipShape::Owned
        && actual_element_shape.optional_element_type().is_none()
        && actual_element_shape.java_ty == element_ty
        && matches!(
            actual_element_shape.dispatch,
            DispatchShape::ClosedEnum { .. }
        )
        && projected_target_element
            .as_ref()
            .is_some_and(|projected| projected.dispatch == actual_element_shape.dispatch)
    {
        let element_plan =
            shape_coercion_plan_with_facts(actual_element_shape, actual_element_shape, facts);
        plan.construction = OptionalSlotConstructionPlan::new(
            target.clone(),
            actual_element_shape.clone(),
            element_plan,
            OwnershipShape::Owned,
            None,
        );
        plan.element_lowering_slot = Slot::Assignment(actual_element_shape.clone());
    }
    if plan.construction.element_handle.is_none() {
        plan.construction.element_handle =
            optional_element_handle_placement_with_representations(target, facts, representations);
    }
    plan
}

pub fn preconverted_optional_slot_construction_plan(
    target: &RustSlotShape,
    element_ty: JavaType,
    facts: &FactSet,
) -> OptionalSlotConstructionPlan {
    let element_shape = optional_element_slot_shape_with_facts(target, element_ty, facts);
    OptionalSlotConstructionPlan::new(
        target.clone(),
        element_shape.clone(),
        shape_coercion_plan_with_facts(&element_shape, &element_shape, facts),
        OwnershipShape::Owned,
        optional_element_handle_placement(target, facts),
    )
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
    java_ty: JavaType,
    recovery: OwnedRecoveryCapability,
}

impl CoercionOwnedRecoveryCapabilityPolicy {
    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn recovery(&self) -> OwnedRecoveryCapability {
        self.recovery.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedElementRecoveryFallbackPolicy {
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
    _element_shape: &RustSlotShape,
) -> ConversionDiagnostic {
    match policy {
        BorrowedElementRecoveryFallbackPolicy::NonCloneableElement { java_ty } => {
            ConversionDiagnostic::new(
                "unsupported_borrowed_element_non_cloneable",
                format!(
                    "unsupported Java borrowed element ownership recovery for non-cloneable type {java_ty:?}"
                ),
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionalElementTargetMatchPolicy {
    matches: bool,
}

impl OptionalElementTargetMatchPolicy {
    pub fn matches(&self) -> bool {
        self.matches
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecElementTargetMatchPolicy {
    matches: bool,
}

impl VecElementTargetMatchPolicy {
    pub fn matches(&self) -> bool {
        self.matches
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetElementTargetMatchPolicy {
    matches: bool,
}

impl SetElementTargetMatchPolicy {
    pub fn matches(&self) -> bool {
        self.matches
    }
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
    RustOwned {
        java_shape: Box<RustSlotShape>,
        rust_type_name: String,
    },
    OptionalRustOwned {
        option_shape: Box<RustSlotShape>,
        rust_type_name: String,
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

    pub fn rust_owned_return(java_ty: JavaType, rust_type_name: String) -> Self {
        Self::RustOwned {
            java_shape: Box::new(RustSlotShape::conservative(java_ty)),
            rust_type_name,
        }
    }

    pub fn optional_rust_owned_return(java_ty: JavaType, rust_type_name: String) -> Self {
        Self::OptionalRustOwned {
            option_shape: Box::new(RustSlotShape::conservative(optional_java_type(java_ty))),
            rust_type_name,
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
        let mut option_shape =
            RustSlotShape::conservative(optional_java_type(element_shape.java_ty.clone()));
        // The Option records lookup presence, not a different allocation.
        // Preserve the stored child's physical carrier through borrow and
        // missing/stored-null normalization (including nested containers).
        option_shape.native_allocation = element_shape.native_allocation;
        option_shape.nested_native_allocations = element_shape.nested_native_allocations.clone();
        option_shape.nested_collection_views = element_shape.nested_collection_views.clone();
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
        Self::collected_set_kind_from_element_shape(
            element_shape,
            crate::stdlib::JavaCollectionKind::Set,
        )
    }

    pub fn collected_set_kind_from_element_shape(
        element_shape: RustSlotShape,
        kind: crate::stdlib::JavaCollectionKind,
    ) -> Self {
        let name = match kind {
            crate::stdlib::JavaCollectionKind::Set => "Set",
            crate::stdlib::JavaCollectionKind::SortedSet => "SortedSet",
            crate::stdlib::JavaCollectionKind::InsertionOrderedSet => "LinkedHashSet",
            _ => unreachable!("a collected set must have a set-like kind"),
        };
        let java_ty = JavaType::Named {
            name: name.to_string(),
            args: vec![element_shape.java_ty.clone()],
        };
        if element_shape.ownership == OwnershipShape::SharedBorrow {
            Self::SetElementBorrow {
                set_shape: Box::new(RustSlotShape::conservative(java_ty)),
                element_shape: Box::new(element_shape),
            }
        } else {
            Self::from_java(java_ty)
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
            LoweredType::RustOwned { java_shape, .. } => Some(&java_shape.java_ty),
            LoweredType::OptionalRustOwned { option_shape, .. } => Some(&option_shape.java_ty),
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
            | LoweredType::RustOwned {
                java_shape: shape, ..
            }
            | LoweredType::OptionalRustOwned {
                option_shape: shape,
                ..
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
    rust_type_name: String,
    target_primitive: PrimitiveType,
}

impl RustScalarSlotCastPolicy {
    pub(crate) fn new(rust_type_name: String, target_primitive: PrimitiveType) -> Self {
        Self {
            rust_type_name,
            target_primitive,
        }
    }

    pub fn rust_type_name(&self) -> &str {
        &self.rust_type_name
    }

    pub fn target_primitive(&self) -> PrimitiveType {
        self.target_primitive
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustBorrowedFacadeReturnPolicy {
    target: RustSlotShape,
    recovery: RustBorrowedFacadeReturnRecovery,
    source_erased_object: bool,
}

impl RustBorrowedFacadeReturnPolicy {
    #[cfg(test)]
    pub(crate) fn new(target: RustSlotShape, recovery: RustBorrowedFacadeReturnRecovery) -> Self {
        Self {
            target,
            recovery,
            source_erased_object: false,
        }
    }

    pub(crate) fn with_source_erased_object(
        target: RustSlotShape,
        recovery: RustBorrowedFacadeReturnRecovery,
        source_erased_object: bool,
    ) -> Self {
        Self {
            target,
            recovery,
            source_erased_object,
        }
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn recovery(&self) -> RustBorrowedFacadeReturnRecovery {
        self.recovery
    }

    pub fn source_erased_object(&self) -> bool {
        self.source_erased_object
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustOwnedFacadeReturnPolicy {
    target: RustSlotShape,
}

impl RustOwnedFacadeReturnPolicy {
    pub(crate) fn new(target: RustSlotShape) -> Self {
        Self { target }
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }
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
    optional: bool,
}

impl RustUnsignedByteArrayReturnPolicy {
    pub(crate) fn new(optional: bool) -> Self {
        Self { optional }
    }

    pub fn optional(&self) -> bool {
        self.optional
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedElementCarrier {
    Option,
    Vec,
    Set {
        kind: crate::stdlib::JavaCollectionKind,
        semantic_equality: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustNominalTypeNamePolicy {
    java_ty: JavaType,
    target_type: Option<String>,
}

impl RustNominalTypeNamePolicy {
    pub(crate) fn new(java_ty: JavaType, target_type: Option<String>) -> Self {
        Self {
            java_ty,
            target_type,
        }
    }

    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn target_type(&self) -> Option<&str> {
        self.target_type.as_deref()
    }

    pub fn into_target_type(self) -> Option<String> {
        self.target_type
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTypeTargetPolicy {
    Nominal { target_type: String },
    MissingNominalTarget { java_ty: JavaType },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimitiveWideningPolicy {
    from: PrimitiveType,
    to: PrimitiveType,
    widens: bool,
}

impl PrimitiveWideningPolicy {
    pub(crate) fn new(from: PrimitiveType, to: PrimitiveType, widens: bool) -> Self {
        Self { from, to, widens }
    }

    pub fn from(&self) -> PrimitiveType {
        self.from
    }

    pub fn to(&self) -> PrimitiveType {
        self.to
    }

    pub fn widens(&self) -> bool {
        self.widens
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionContentPolicy {
    java_ty: JavaType,
    string_content: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionShapeContentPolicy {
    java_ty: JavaType,
    string_content: bool,
}

impl CoercionContentPolicy {
    pub(crate) fn new(java_ty: JavaType, string_content: bool) -> Self {
        Self {
            java_ty,
            string_content,
        }
    }

    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn string_content(&self) -> bool {
        self.string_content
    }
}

impl CoercionShapeContentPolicy {
    pub(crate) fn new(java_ty: JavaType, string_content: bool) -> Self {
        Self {
            java_ty,
            string_content,
        }
    }

    pub fn java_ty(&self) -> &JavaType {
        &self.java_ty
    }

    pub fn string_content(&self) -> bool {
        self.string_content
    }
}

pub fn rust_nominal_type_name_policy(java_ty: &JavaType) -> RustNominalTypeNamePolicy {
    let target_type = match java_ty {
        java_ty if reifiable_raw_collection_runtime_target(java_ty).is_some() => {
            reifiable_raw_collection_runtime_target(java_ty)
        }
        java_ty if reifiable_object_array_runtime_target(java_ty).is_some() => {
            reifiable_object_array_runtime_target(java_ty)
        }
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
            if java_exception_like_runtime_type_name(&canonical) {
                Some("JavaException".to_string())
            } else {
                crate::stdlib::boxed_primitive_type(&canonical)
                    .map(boxed_primitive_runtime_rust_type)
                    .or_else(|| Some(crate::java_naming::rust_name_for_java_named_type(name)))
            }
        }
        JavaType::String => Some("String".to_string()),
        _ => None,
    };
    RustNominalTypeNamePolicy::new(java_ty.clone(), target_type)
}

fn java_exception_like_runtime_type_name(name: &str) -> bool {
    let simple = name.rsplit('.').next().unwrap_or(name);
    matches!(simple, "Throwable" | "Exception" | "Error")
        || simple.ends_with("Exception")
        || simple.ends_with("Error")
}

fn reifiable_object_array_runtime_target(java_ty: &JavaType) -> Option<String> {
    let JavaType::Array(element) = java_ty else {
        return None;
    };
    let JavaType::Named { name, args } = element.as_ref() else {
        return None;
    };
    if !args.is_empty() || crate::stdlib::canonical_external_receiver_type_key(name) != "Object" {
        return None;
    }
    Some("Vec<Option<Box<dyn std::any::Any>>>".to_string())
}

fn reifiable_raw_collection_runtime_target(java_ty: &JavaType) -> Option<String> {
    let JavaType::Named { args, .. } = java_ty else {
        return None;
    };
    let reifiable_erasure = args.is_empty()
        || args
            .iter()
            .all(|arg| matches!(arg, JavaType::Wildcard(None)));
    if !reifiable_erasure {
        return None;
    }
    match crate::stdlib::java_collection_kind_for_type(java_ty)? {
        crate::stdlib::JavaCollectionKind::List => Some("Vec<Box<dyn std::any::Any>>".to_string()),
        _ => None,
    }
}

fn boxed_primitive_runtime_rust_type(primitive: PrimitiveType) -> String {
    crate::emit::emit_type(&crate::operator_lowering::rust_type_for_java_primitive(
        primitive,
    ))
}

pub fn runtime_type_target_policy(java_ty: &JavaType) -> RuntimeTypeTargetPolicy {
    match rust_nominal_type_name_policy(java_ty).into_target_type() {
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
    Some(PrimitiveWideningPolicy::new(
        *from,
        *to,
        crate::types::primitive_widens_to(*from, *to),
    ))
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
    if capability.optional_or_result() {
        return JavaCastSemanticsPolicy::OptionalResultCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if capability.raw_or_wildcard() {
        return JavaCastSemanticsPolicy::RawOrWildcardCast {
            actual: actual.java_ty.clone(),
            target: target.java_ty.clone(),
        };
    }
    if capability.unchecked_generic() {
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
        if let Some(invalid_cast) = coercion.dispatch.checked_cast_failure_policy() {
            return JavaCastSemanticsPolicy::CheckedReferenceDowncast {
                invalid_cast: invalid_cast.clone(),
            };
        }
        return match &coercion.dispatch {
            DispatchCoercionPolicy::WrapClosedEnum { .. }
            | DispatchCoercionPolicy::BorrowDynTrait { .. }
            | DispatchCoercionPolicy::BoxDynTrait { .. }
            | DispatchCoercionPolicy::UpcastBoxedDynTrait { .. }
            | DispatchCoercionPolicy::EraseObject
            | DispatchCoercionPolicy::CloneHierarchyToErasedObject
            | DispatchCoercionPolicy::CloneDynTraitToErasedObject { .. } => {
                JavaCastSemanticsPolicy::ReferenceUpcast
            }
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
    CoercionContentPolicy::new(java_ty.clone(), facts.is_string_like_type(java_ty))
}

pub fn coercion_shape_content_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> CoercionShapeContentPolicy {
    let content = coercion_content_policy_with_effective_facts(&shape.java_ty, facts);
    CoercionShapeContentPolicy::new(content.java_ty().clone(), content.string_content())
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
    if !coercion_shape_content_policy_with_effective_facts(actual, facts).string_content()
        || !coercion_shape_content_policy_with_effective_facts(target, facts).string_content()
    {
        return ViewCoercionPolicy::NoViewChange;
    }
    let identity_string_view = actual.java_ty == JavaType::String
        && target.java_ty == JavaType::String
        && facts.uses_java_string_identity_representation();
    if actual.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::SharedBorrow
        && target.view == ViewShape::Str
    {
        return if identity_string_view {
            ViewCoercionPolicy::BorrowStringReferenceView {
                source_borrowed: false,
            }
        } else {
            ViewCoercionPolicy::BorrowStringView
        };
    }
    if actual.ownership == OwnershipShape::SharedBorrow
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::SharedBorrow
        && target.view == ViewShape::Str
    {
        return if identity_string_view {
            ViewCoercionPolicy::BorrowStringReferenceView {
                source_borrowed: true,
            }
        } else {
            ViewCoercionPolicy::BorrowStringView
        };
    }
    // The source ABI occasionally retains `Owned` ownership metadata for a
    // string slot whose rendered view is still `&str`.  Representation is the
    // deciding factor here: either ownership annotation needs recovery when
    // the target is an owned native `String`.
    if actual.view == ViewShape::Str
        && target.ownership == OwnershipShape::Owned
        && target.view == ViewShape::Native
    {
        return if identity_string_view {
            ViewCoercionPolicy::OwnStringReferenceFromView
        } else {
            ViewCoercionPolicy::OwnStringFromView
        };
    }
    if identity_string_view
        && actual.view == ViewShape::Str
        && target.view == ViewShape::Str
        && target.ownership == OwnershipShape::SharedBorrow
    {
        return ViewCoercionPolicy::BorrowStringReferenceView {
            source_borrowed: actual.ownership == OwnershipShape::SharedBorrow,
        };
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
    if actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::Owned
        && matches!(actual.dispatch, DispatchShape::Concrete)
        && (matches!(target.dispatch, DispatchShape::ErasedObject)
            || (crate::shape_projection::java_type_is_erased_object_storage(&target.java_ty)
                && !facts.parsed_source_controls_native_carrier(&target.java_ty)))
        && let JavaType::Named { name, .. } = &actual.java_ty
        && facts.parsed_source_controls_native_carrier(&actual.java_ty)
        && {
            let placement = facts.storage_placement_for(name);
            placement.shared() && placement.is_implemented()
        }
    {
        // Erasure introduces another owning Java reference. Clone the proven
        // Rc/Arc carrier before boxing, not its payload, even though both
        // outer slots are nominally Owned. Moving the handle consumes a Java
        // local which may still observe the same object afterward.
        return OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: actual.java_ty.clone(),
            },
        };
    }
    if matches!(actual.dispatch, DispatchShape::ReferenceDynTrait { .. })
        && actual.dispatch == target.dispatch
        && actual.java_ty == target.java_ty
        && actual.view == target.view
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && target.ownership == OwnershipShape::Owned
    {
        return OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: actual.java_ty.clone(),
            },
        };
    }
    if actual.ownership == target.ownership {
        return OwnershipCoercionPolicy::NoOwnershipChange;
    }
    if actual.native_allocation.shared()
        && !facts.parsed_source_controls_native_carrier(&actual.java_ty)
        && (matches!(actual.java_ty, JavaType::Array(_))
            || crate::stdlib::java_collection_kind_for_type(&actual.java_ty).is_some())
        && actual.native_allocation == target.native_allocation
        && (actual.java_ty == target.java_ty
            || shared_native_collection_abi_matches(actual, target, facts))
        && actual.dispatch == target.dispatch
        && actual.view == target.view
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && target.ownership == OwnershipShape::Owned
    {
        // The clone capability belongs to Rc/Arc, not Vec<T> or its elements.
        return OwnershipCoercionPolicy::RecoverOwnedWithClone {
            policy: CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: actual.java_ty.clone(),
            },
        };
    }
    if matches!(
        actual.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) && target.ownership == OwnershipShape::SharedBorrow
        && (actual.view == target.view
            || owned_native_matrix_matches_borrowed_matrix_view(actual, target))
        && actual.dispatch == target.dispatch
    {
        return OwnershipCoercionPolicy::BorrowOwnedValue;
    }
    if actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::MutableBorrow
        && (actual.view == target.view
            || owned_native_matrix_matches_borrowed_matrix_view(actual, target))
        && actual.dispatch == target.dispatch
    {
        return OwnershipCoercionPolicy::BorrowOwnedMutably;
    }
    if (actual.java_ty == target.java_ty
        || boxed_primitive_element_matches_target(&actual.java_ty, &target.java_ty))
        && actual.dispatch == target.dispatch
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && actual.view == target.view
        && target.ownership == OwnershipShape::CopyValue
    {
        return OwnershipCoercionPolicy::RecoverOwnedCopyValue;
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
        if let Some((sequence, optional_sequence)) =
            erased_object_clone_recovery_sequence(actual, target)
        {
            return OwnershipCoercionPolicy::RecoverErasedObjectClone {
                sequence,
                optional_sequence,
            };
        }
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        let policy = clone_recovery_policy_from_capability(capability.recovery());
        if crate::types::ndarray_array2_element_type(&actual.java_ty).is_some() {
            return OwnershipCoercionPolicy::RecoverOwnedWithClone { policy };
        }
        return OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { policy };
    }
    if actual.java_ty == target.java_ty
        && actual.dispatch == target.dispatch
        && actual.view == target.view
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && target.ownership == OwnershipShape::Owned
    {
        if let Some((sequence, optional_sequence)) =
            erased_object_clone_recovery_sequence(actual, target)
        {
            return OwnershipCoercionPolicy::RecoverErasedObjectClone {
                sequence,
                optional_sequence,
            };
        }
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        if crate::types::sequence_element_type(&actual.java_ty).is_some() {
            let policy = clone_recovery_policy_from_capability(capability.recovery());
            if crate::types::ndarray_array2_element_type(&actual.java_ty).is_some() {
                return OwnershipCoercionPolicy::RecoverOwnedWithClone { policy };
            }
            return OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { policy };
        }
        ownership_recovery_policy_from_capability(capability.recovery(), actual, target)
    } else if actual.dispatch == DispatchShape::Concrete
        && matches!(target.dispatch, DispatchShape::ClosedEnum { .. })
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && target.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.view == ViewShape::Native
    {
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        ownership_recovery_policy_from_capability(capability.recovery(), actual, target)
    } else if matches!(actual.dispatch, DispatchShape::ClosedEnum { .. })
        && target.dispatch == DispatchShape::Concrete
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && target.ownership == OwnershipShape::Owned
        && matches!(actual.view, ViewShape::Native | ViewShape::EnumView)
        && target.view == ViewShape::Native
    {
        // Matching a borrowed hierarchy carrier yields a borrowed variant
        // payload. A Java cast used in an owned destination (for example
        // `list.add((Child) base)` inside enhanced-for) copies the object
        // reference/value into that destination, so recover the owned payload
        // after the discriminant check.
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&target.java_ty, facts);
        ownership_recovery_policy_from_capability(capability.recovery(), actual, target)
    } else if matches!(
        actual.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && target.ownership == OwnershipShape::Owned
        && matches!(target.dispatch, DispatchShape::ErasedObject)
        && !matches!(actual.java_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        if let Some((sequence, optional_sequence)) =
            erased_object_clone_recovery_sequence(actual, target)
        {
            return OwnershipCoercionPolicy::RecoverErasedObjectClone {
                sequence,
                optional_sequence,
            };
        }
        let capability =
            coercion_owned_recovery_capability_policy_with_effective_facts(&actual.java_ty, facts);
        ownership_recovery_policy_from_capability(capability.recovery(), actual, target)
    } else {
        OwnershipCoercionPolicy::NoOwnershipChange
    }
}

fn owned_native_matrix_matches_borrowed_matrix_view(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> bool {
    actual.java_ty == target.java_ty
        && actual.view == ViewShape::Native
        && target.view == ViewShape::Slice
        && crate::types::ndarray_array2_element_type(&actual.java_ty).is_some()
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
        let mut actual_element =
            optional_element_slot_shape_with_facts(actual, actual_element_ty.clone(), facts);
        let target_element =
            optional_element_slot_shape_with_facts(target, target_element_ty.clone(), facts);
        let borrow_receiver = target_element.dispatch == DispatchShape::ErasedObject
            && actual_element.dispatch != DispatchShape::ErasedObject
            && actual.ownership == OwnershipShape::Owned
            && target_element.ownership == OwnershipShape::Owned
            && optional_element_handle_placement(actual, facts).is_some();
        if borrow_receiver {
            actual_element.ownership = OwnershipShape::SharedBorrow;
        }
        if actual_element != target_element {
            return OptionalShapeCoercionPolicy::MapElement {
                actual_element_ty,
                target_element_ty,
                borrow_receiver,
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
    } else if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(&element_ty) {
        // Optionality is a carrier property, not evidence that an executable
        // generic facade payload has become a dynamically erased object.  A
        // nullable `Future<T>` (and analogous facade carriers) must unwrap to
        // the same concrete/native `Future<T>` ABI as a non-null declaration.
        element_shape.java_ty = element_ty;
        element_shape.dispatch = DispatchShape::Concrete;
        element_shape.view = ViewShape::Native;
    } else if matches!(
        element_shape.dispatch,
        DispatchShape::DynTrait { .. }
            | DispatchShape::BoxedDynTrait { .. }
            | DispatchShape::ReferenceDynTrait { .. }
    ) {
        // A facade slot can already describe how the optional payload crosses
        // the boundary.  In particular, `Optional(Borrowed)` for an
        // object-safe interface is `Option<&dyn Trait>`, while storage is
        // `Option<Box<dyn Trait>>`.  Re-inferring both payloads as boxed here
        // erases that distinction and leaves the outer `Option` to be cast as
        // though it were itself a trait object.
        element_shape.java_ty = element_ty;
        if matches!(
            element_shape.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        ) && let DispatchShape::BoxedDynTrait { trait_name } = &element_shape.dispatch
        {
            // Match signature rendering: a borrowed optional interface is
            // Option<&[mut] dyn Trait>, not an optional owned Box. Storage
            // construction must request owned element ownership explicitly.
            element_shape.dispatch = DispatchShape::DynTrait {
                trait_name: trait_name.clone(),
            };
            element_shape.view = ViewShape::DynTraitView;
        }
    } else if let JavaType::Named { name, .. } = &element_ty
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        let trait_name = name.clone();
        element_shape.java_ty = element_ty;
        element_shape.dispatch = DispatchShape::BoxedDynTrait { trait_name };
        element_shape.view = ViewShape::DynTraitView;
    } else if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        EffectiveFacts::new(facts).object_representation_override_strategy_for(&element_ty)
    {
        element_shape.java_ty = element_ty;
        element_shape.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        };
        element_shape.view = ViewShape::DynTraitView;
    } else {
        element_shape.java_ty = element_ty;
    }
    element_shape.normalize_owned_string_payload_view();
    if matches!(element_shape.java_ty, JavaType::String)
        && matches!(element_shape.ownership, OwnershipShape::SharedBorrow)
        && matches!(element_shape.view, ViewShape::Str)
    {
        element_shape.dispatch = DispatchShape::Concrete;
    }
    element_shape
}

pub fn optional_element_target_match_policy(
    target: &RustSlotShape,
    option_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> OptionalElementTargetMatchPolicy {
    OptionalElementTargetMatchPolicy {
        matches: target.java_ty == option_shape.java_ty && target.is_optional_of(element_shape),
    }
}

pub fn vec_element_target_match_policy(
    target: &RustSlotShape,
    vec_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> VecElementTargetMatchPolicy {
    VecElementTargetMatchPolicy {
        matches: target.java_ty == vec_shape.java_ty && target.is_sequence_of(element_shape),
    }
}

pub fn set_element_target_match_policy(
    target: &RustSlotShape,
    set_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> SetElementTargetMatchPolicy {
    SetElementTargetMatchPolicy {
        matches: crate::stdlib::java_collection_kind_for_type(&target.java_ty)
            .is_some_and(|kind| kind.is_set_like())
            && crate::stdlib::java_collection_kind_for_type(&set_shape.java_ty)
                .is_some_and(|kind| kind.is_set_like())
            && target.is_sequence_of(element_shape)
            && set_shape.is_sequence_of(element_shape),
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
    if coercion_shape_content_policy_with_effective_facts(element_shape, facts).string_content()
        && element_shape.ownership == OwnershipShape::SharedBorrow
        && element_shape.view == ViewShape::Str
    {
        if element_shape.java_ty == JavaType::String
            && facts.uses_java_string_identity_representation()
        {
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: JavaType::String,
                },
            }
        } else {
            BorrowedElementRecoveryPolicy::StringViewToOwned
        }
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
        borrowed_element_recovery_policy_from_capability(capability.recovery(), element_shape)
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
    carrier: BorrowedElementCarrier,
    element_shape: RustSlotShape,
    recovery: BorrowedElementRecoveryPolicy,
    unwrap_carrier: bool,
}

impl BorrowedElementAdapterPolicy {
    pub fn carrier(&self) -> BorrowedElementCarrier {
        self.carrier
    }

    pub fn element_shape(&self) -> &RustSlotShape {
        &self.element_shape
    }

    pub fn recovery(&self) -> &BorrowedElementRecoveryPolicy {
        &self.recovery
    }

    pub fn unwrap_carrier(&self) -> bool {
        self.unwrap_carrier
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.recovery.unsupported_diagnostics(&self.element_shape)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedElementSlotAdapterPolicy {
    target: RustSlotShape,
    carrier_shape: RustSlotShape,
    element_shape: RustSlotShape,
    carrier: BorrowedElementCarrier,
    matches: bool,
    adapter: Option<BorrowedElementAdapterPolicy>,
    string_reference_view: bool,
}

impl BorrowedElementSlotAdapterPolicy {
    pub(crate) fn new(
        target: RustSlotShape,
        carrier_shape: RustSlotShape,
        element_shape: RustSlotShape,
        carrier: BorrowedElementCarrier,
        matches: bool,
        adapter: Option<BorrowedElementAdapterPolicy>,
    ) -> Self {
        Self {
            target,
            carrier_shape,
            element_shape,
            carrier,
            matches,
            adapter,
            string_reference_view: false,
        }
    }

    pub fn string_reference_view(&self) -> bool {
        self.string_reference_view
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn carrier_shape(&self) -> &RustSlotShape {
        &self.carrier_shape
    }

    pub fn element_shape(&self) -> &RustSlotShape {
        &self.element_shape
    }

    pub fn carrier(&self) -> BorrowedElementCarrier {
        self.carrier
    }

    pub fn matches(&self) -> bool {
        self.matches
    }

    pub fn adapter(&self) -> Option<&BorrowedElementAdapterPolicy> {
        self.adapter.as_ref()
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.adapter
            .as_ref()
            .map(BorrowedElementAdapterPolicy::unsupported_diagnostics)
            .unwrap_or_default()
    }
}

pub(crate) fn borrowed_element_adapter_policy_with_unwrap(
    carrier: BorrowedElementCarrier,
    element_shape: &RustSlotShape,
    facts: &FactSet,
    unwrap_carrier: bool,
) -> BorrowedElementAdapterPolicy {
    // `ViewShape::Str` is rendered as `&str`, including in a few source
    // boundary shapes whose ownership is still recorded as `Owned`.  The
    // carrier therefore needs string recovery whenever the destination owns
    // that element; cloning the view would keep it as `&str`.
    let recovery = if matches!(element_shape.java_ty, JavaType::String)
        && matches!(element_shape.view, ViewShape::Str)
    {
        if facts.java_string_identity_representation {
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: CloneRecoveryPolicy::ProvenCloneCapability {
                    java_ty: JavaType::String,
                },
            }
        } else {
            BorrowedElementRecoveryPolicy::StringViewToOwned
        }
    } else {
        borrowed_element_recovery_policy(element_shape, facts)
    };
    BorrowedElementAdapterPolicy {
        carrier,
        element_shape: element_shape.clone(),
        recovery,
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
    let target = normalize_borrowed_element_adapter_target(target, facts);
    let carrier_matches = match carrier {
        BorrowedElementCarrier::Option => {
            optional_element_target_match_policy(&target, carrier_shape, element_shape).matches()
        }
        BorrowedElementCarrier::Vec => {
            vec_element_target_match_policy(&target, carrier_shape, element_shape).matches()
        }
        BorrowedElementCarrier::Set { .. } => {
            set_element_target_match_policy(&target, carrier_shape, element_shape).matches()
        }
    };
    let element_matches = matches!(carrier, BorrowedElementCarrier::Option)
        && target.optional_element_type().is_none()
        && borrowed_element_target_matches_element(&target, element_shape, facts);
    let matches = carrier_matches || element_matches;
    let exact_optional_element_match = matches!(carrier, BorrowedElementCarrier::Option)
        && target.optional_element_type().is_some_and(|element_ty| {
            optional_element_slot_shape_matches(&target, element_ty, element_shape)
        });
    let optional_element_view_projection =
        optional_element_borrow_target_projection(carrier, &target, element_shape, facts);
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
    let string_reference_view = facts.java_string_identity_representation
        && (target.java_ty == JavaType::String
            || target.optional_element_type() == Some(JavaType::String));
    let mut plan = BorrowedElementSlotAdapterPolicy::new(
        target,
        carrier_shape.clone(),
        element_shape.clone(),
        carrier,
        matches,
        adapter,
    );
    plan.string_reference_view = string_reference_view;
    plan
}

fn normalize_borrowed_element_adapter_target(
    target: &RustSlotShape,
    facts: &FactSet,
) -> RustSlotShape {
    let mut normalized = target.clone();
    let effective_facts = EffectiveFacts::new(facts);
    if normalized.view == ViewShape::Str
        && coercion_shape_content_policy_with_effective_facts(&normalized, &effective_facts)
            .string_content()
    {
        normalized.ownership = OwnershipShape::SharedBorrow;
    }
    if matches!(
        normalized.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && external_crate_mapped_dyn_trait_java_type(&normalized.java_ty, facts)
    {
        normalized.dispatch = DispatchShape::Concrete;
        normalized.view = ViewShape::Native;
    }
    normalized
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
            .reaches()
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

pub fn dispatch_coercion_policy_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> DispatchCoercionPolicy {
    if target.view == ViewShape::ListInterface
        && EffectiveFacts::new(facts)
            .native_iterable_list_projection_element(&actual.java_ty, &target.java_ty)
            .is_some()
    {
        return DispatchCoercionPolicy::NativeIterableListView {
            invalid_cast: InvalidCastPolicy::Panic,
        };
    }
    if shared_native_collection_abi_matches(actual, target, &EffectiveFacts::new(facts)) {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if target.dispatch == DispatchShape::ErasedObject
        && target.ownership == OwnershipShape::Owned
        && actual.dispatch != DispatchShape::ErasedObject
        && actual.optional_element_type().is_none()
        && object_safe_trait_boundary(&actual.java_ty, facts)
        && let Some(trait_name) = java_type_name_for_dispatch(&actual.java_ty)
        && EffectiveFacts::new(facts).has_parsed_source_type_capability_for_name(trait_name)
        && !external_crate_mapped_dyn_trait_boundary(trait_name, facts)
    {
        return DispatchCoercionPolicy::CloneDynTraitToErasedObject {
            trait_name: dyn_trait_rust_abi_path(trait_name, &actual.java_ty, facts),
        };
    }
    if matches!(
        actual.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && target.ownership == OwnershipShape::Owned
        && let (
            DispatchShape::DynTrait {
                trait_name: actual_trait,
            },
            DispatchShape::DynTrait {
                trait_name: target_trait,
            },
        ) = (&actual.dispatch, &target.dispatch)
        && dyn_trait_rust_abi_matches(
            actual_trait,
            &actual.java_ty,
            target_trait,
            &target.java_ty,
            facts,
        )
    {
        // The dispatch shape describes the trait view while ownership decides
        // whether that view is `&dyn Trait` or the storage `Box<dyn Trait>`.
        // Crossing from the former to the latter must use the generated
        // object-safe clone adapter; Rust's `Clone` on `&dyn Trait` would only
        // copy the reference.
        return DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: dyn_trait_rust_abi_path(target_trait, &target.java_ty, facts),
        };
    }
    if matches!(
        actual.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && target.ownership == OwnershipShape::Owned
        && let (
            DispatchShape::BoxedDynTrait {
                trait_name: actual_trait,
            },
            DispatchShape::BoxedDynTrait {
                trait_name: target_trait,
            },
        ) = (&actual.dispatch, &target.dispatch)
        && dyn_trait_rust_abi_matches(
            actual_trait,
            &actual.java_ty,
            target_trait,
            &target.java_ty,
            facts,
        )
    {
        return DispatchCoercionPolicy::CloneBoxDynTrait {
            trait_name: dyn_trait_rust_abi_path(target_trait, &target.java_ty, facts),
        };
    }
    if let (
        DispatchShape::ReferenceDynTrait {
            trait_name: source,
            sync: left,
        },
        DispatchShape::ReferenceDynTrait {
            trait_name: destination,
            sync: right,
        },
    ) = (&actual.dispatch, &target.dispatch)
        && left == right
        && dyn_trait_rust_abi_matches(source, &actual.java_ty, destination, &target.java_ty, facts)
    {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if actual.dispatch == target.dispatch && actual.java_ty == target.java_ty {
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if let (
        DispatchShape::BoxedDynTrait {
            trait_name: actual_trait,
        },
        DispatchShape::BoxedDynTrait {
            trait_name: target_trait,
        },
    ) = (&actual.dispatch, &target.dispatch)
        && dyn_trait_rust_abi_matches(
            actual_trait,
            &actual.java_ty,
            target_trait,
            &target.java_ty,
            facts,
        )
    {
        // Java wildcard spellings can differ while declaration-derived
        // variance proves that both sides have the same Rust trait-object
        // ABI. Dispatch is already correct; ownership lowering remains
        // responsible for moves or clones.
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if matches!(target.dispatch, DispatchShape::Concrete)
        && object_safe_trait_boundary(&target.java_ty, facts)
        && let Some(target_trait) = java_type_name_for_dispatch(&target.java_ty)
    {
        match &actual.dispatch {
            DispatchShape::BoxedDynTrait { trait_name }
                if dyn_trait_names_match(trait_name, target_trait) =>
            {
                // A stale Concrete tag does not turn an object-safe Java
                // interface into a nominal Rust struct. The owned carrier is
                // already `Box<dyn Trait<...>>`; expose its trait view using
                // the actual generic instantiation rather than attempting an
                // impossible same-interface downcast.
                if matches!(
                    target.ownership,
                    OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                ) {
                    return DispatchCoercionPolicy::BorrowBoxedDynTrait {
                        mutable: target.ownership == OwnershipShape::MutableBorrow,
                        trait_name: dyn_trait_rust_abi_path(trait_name, &actual.java_ty, facts),
                    };
                }
            }
            DispatchShape::DynTrait { trait_name }
                if dyn_trait_names_match(trait_name, target_trait)
                    && matches!(
                        target.ownership,
                        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                    ) =>
            {
                return DispatchCoercionPolicy::NoDispatchChange;
            }
            _ => {}
        }
    }
    if matches!(
        (actual.ownership, target.ownership),
        (OwnershipShape::SharedBorrow, OwnershipShape::SharedBorrow)
            | (OwnershipShape::MutableBorrow, OwnershipShape::MutableBorrow)
    ) && actual.java_ty == target.java_ty
        && external_crate_mapped_dyn_trait_java_type(&target.java_ty, facts)
    {
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
    if let (
        DispatchShape::ClosedEnum {
            enum_name: actual_enum,
        },
        DispatchShape::ClosedEnum {
            enum_name: target_enum,
        },
    ) = (&actual.dispatch, &target.dispatch)
        && actual_enum == target_enum
        && (crate::slot_render::parameterized_closed_enum_type_name(actual_enum, &actual.java_ty)
            == crate::slot_render::parameterized_closed_enum_type_name(
                target_enum,
                &target.java_ty,
            )
            || java_types_have_same_erased_identity(&actual.java_ty, &target.java_ty))
    {
        // A carrier's generic arguments can require a separate value-level
        // adaptation, but they never change its runtime dispatch. In
        // particular, treating `Carrier<A> -> Carrier<B>` as a dispatch
        // conversion manufactures an impossible enum-to-itself operation and
        // prevents Rust from inferring a generic call's result from its
        // arguments and destination.
        return DispatchCoercionPolicy::NoDispatchChange;
    }
    if let Some(policy) = source_boxed_interface_upcast_policy(actual, target, facts) {
        return policy;
    }
    // An explicit external adapter describes the concrete facade ABI, while
    // hierarchy dispatch is a representation choice.  Source-summary
    // inheritance can project a platform superclass slot as a dyn carrier;
    // do not let that incidental projection replace a modeled conversion
    // such as FileInputStream -> InputStream with an invalid trait cast.
    if let Some(adapter) = external_conversion_adapter_policy(actual, target, facts) {
        return adapter;
    }
    let effective_facts = EffectiveFacts::new(facts);
    let nominal_erased_external_upcast = actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::Owned
        && actual.java_ty != target.java_ty
        && facts
            .hierarchy_subtype_policy(&actual.java_ty, &target.java_ty)
            .reaches()
        && matches!(
            effective_facts.object_representation_override_strategy_for(&actual.java_ty),
            Some(HierarchyStrategy::ErasedObject)
        )
        && matches!(
            effective_facts.object_representation_override_strategy_for(&target.java_ty),
            Some(HierarchyStrategy::ErasedObject)
        );
    if nominal_erased_external_upcast {
        // Erased external-library declarations still render as distinct
        // nominal facade structs. Their shared dispatch strategy therefore
        // does not make the Rust values interchangeable. Reuse the
        // hierarchy-backed `From` surface, including inside `Option::map`.
        return DispatchCoercionPolicy::ExternalInto {
            from_type: actual.java_ty.clone(),
            to_type: target.java_ty.clone(),
        };
    }
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::Concrete, DispatchShape::Concrete) => {
            if let Some(adapter) = java_collection_conversion_policy(actual, target) {
                adapter
            } else if concrete_nominal_rust_abi_is_identical(actual, target) {
                // Nested source types can reach this boundary through two
                // equivalent Java spellings (`Owner.Item` and the flattened
                // emitted `Owner_Item`).  Their Rust value type is already
                // identical, so ordinary ownership lowering should borrow
                // the value.  Treating one spelling as an external facade
                // instead evaluates the value and then tries to borrow its
                // type path (`&Owner_Item`).
                DispatchCoercionPolicy::NoDispatchChange
            } else if actual.java_ty != target.java_ty
                && target.ownership == OwnershipShape::SharedBorrow
                && borrowed_element_target_matches_element(target, actual, facts)
                && let Some(target_type) =
                    rust_nominal_type_name_policy(&target.java_ty).into_target_type()
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
                .into_target_type()
                .map(|variant_name| DispatchCoercionPolicy::WrapClosedEnum {
                    enum_name: enum_name.clone(),
                    variant_name,
                })
                .unwrap_or_else(|| unsupported_dispatch_coercion_policy(actual, target))
        }
        (DispatchShape::Concrete, DispatchShape::DynTrait { trait_name }) => {
            if external_crate_mapped_dyn_trait_boundary(trait_name, facts)
                || external_crate_mapped_dyn_trait_java_type(&target.java_ty, facts)
            {
                return DispatchCoercionPolicy::NoDispatchChange;
            }
            if actual.ownership == OwnershipShape::SharedBorrow
                && actual.java_ty == target.java_ty
                && object_safe_trait_boundary(&target.java_ty, facts)
            {
                return DispatchCoercionPolicy::BorrowBoxedDynTrait {
                    mutable: target.ownership == OwnershipShape::MutableBorrow,
                    trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
                };
            }
            DispatchCoercionPolicy::BorrowDynTrait {
                mutable: target.ownership == OwnershipShape::MutableBorrow,
                trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
            }
        }
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::DynTrait { trait_name }) => {
            let variants = closed_enum_variants_reaching_target(actual, target, facts);
            if !variants.is_empty()
                && matches!(
                    actual.ownership,
                    OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                )
            {
                DispatchCoercionPolicy::BorrowClosedEnumAsDynTrait {
                    mutable: target.ownership == OwnershipShape::MutableBorrow,
                    enum_name: enum_name.clone(),
                    trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
                    variants,
                }
            } else if actual.java_ty != target.java_ty
                && facts
                    .hierarchy_subtype_policy(&actual.java_ty, &target.java_ty)
                    .reaches()
            {
                DispatchCoercionPolicy::BorrowDynTrait {
                    mutable: target.ownership == OwnershipShape::MutableBorrow,
                    trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
                }
            } else {
                unsupported_dispatch_coercion_policy(actual, target)
            }
        }
        (
            DispatchShape::ClosedEnum {
                enum_name: actual_enum_name,
            },
            DispatchShape::ClosedEnum {
                enum_name: target_enum_name,
            },
        ) => {
            let variants = closed_enum_variants_reaching_target(actual, target, facts);
            if actual_enum_name != target_enum_name && !variants.is_empty() {
                DispatchCoercionPolicy::RewrapClosedEnum {
                    actual_enum_name: actual_enum_name.clone(),
                    target_enum_name: target_enum_name.clone(),
                    variants,
                    clone_payload: actual.ownership != OwnershipShape::Owned,
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
        ) if dyn_trait_names_match(actual_trait, target_trait) => {
            DispatchCoercionPolicy::BorrowBoxedDynTrait {
                mutable: target.ownership == OwnershipShape::MutableBorrow,
                trait_name: dyn_trait_rust_abi_path(target_trait, &target.java_ty, facts),
            }
        }
        (DispatchShape::Concrete, DispatchShape::ReferenceDynTrait { trait_name, sync }) => {
            DispatchCoercionPolicy::ReferenceDynTrait {
                trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
                sync: *sync,
            }
        }
        (DispatchShape::Concrete, DispatchShape::BoxedDynTrait { trait_name }) => {
            if external_crate_mapped_dyn_trait_boundary(trait_name, facts) {
                return DispatchCoercionPolicy::EraseObject;
            }
            // Parameter/local inference can retain the conservative Concrete
            // dispatch tag even though an object-safe Java interface is
            // rendered as `&dyn Trait`. An owned interface storage slot must
            // clone that trait object through its object-safe adapter; boxing
            // the reference would produce `Box<&dyn Trait>`.
            if matches!(
                actual.ownership,
                OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
            ) && actual.java_ty == target.java_ty
                && object_safe_trait_boundary(&target.java_ty, facts)
            {
                return DispatchCoercionPolicy::CloneBoxDynTrait {
                    trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
                };
            }
            DispatchCoercionPolicy::BoxDynTrait {
                trait_name: dyn_trait_rust_abi_path(trait_name, &target.java_ty, facts),
            }
        }
        (
            DispatchShape::DynTrait {
                trait_name: actual_trait,
            },
            DispatchShape::BoxedDynTrait {
                trait_name: target_trait,
            },
        ) if dyn_trait_names_match(actual_trait, target_trait) => {
            DispatchCoercionPolicy::CloneBoxDynTrait {
                trait_name: dyn_trait_rust_abi_path(target_trait, &target.java_ty, facts),
            }
        }
        (DispatchShape::ReferenceDynTrait { trait_name, .. }, DispatchShape::ErasedObject) => {
            DispatchCoercionPolicy::CloneDynTraitToErasedObject {
                trait_name: dyn_trait_rust_abi_path(trait_name, &actual.java_ty, facts),
            }
        }
        (DispatchShape::Concrete, DispatchShape::ErasedObject) => {
            DispatchCoercionPolicy::EraseObject
        }
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::ErasedObject) => {
            // A closed hierarchy enum is already the concrete runtime carrier
            // for the Java value.  Crossing an Object boundary erases that
            // carrier in exactly the same way as a concrete struct.
            if facts
                .source_hierarchy_carrier_variants
                .contains_key(enum_name)
            {
                DispatchCoercionPolicy::CloneHierarchyToErasedObject
            } else {
                DispatchCoercionPolicy::EraseObject
            }
        }
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastClosedEnum {
                        enum_name: enum_name.clone(),
                        variant_name: target_type,
                        borrowed_payload: matches!(
                            actual.ownership,
                            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                        ),
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
        (DispatchShape::ErasedObject, DispatchShape::ClosedEnum { enum_name }) => {
            let variants = erased_object_closed_enum_variants_for_target(target, facts);
            if variants.is_empty() {
                unsupported_dispatch_coercion_policy(actual, target)
            } else {
                DispatchCoercionPolicy::DowncastErasedObjectToClosedEnum {
                    enum_name: enum_name.clone(),
                    carrier_type:
                        crate::slot_render::parameterized_closed_enum_type_name_with_facts(
                            enum_name,
                            &target.java_ty,
                            facts,
                        ),
                    variants,
                    invalid_cast: InvalidCastPolicy::Panic,
                }
            }
        }
        (DispatchShape::DynTrait { .. }, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    DispatchCoercionPolicy::DowncastDynTrait {
                        target_type: erased_runtime_target_type(target, target_type, facts),
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
        (
            DispatchShape::BoxedDynTrait { .. } | DispatchShape::ReferenceDynTrait { .. },
            DispatchShape::Concrete,
        ) => match runtime_type_target_policy(&target.java_ty) {
            RuntimeTypeTargetPolicy::Nominal { target_type } => {
                DispatchCoercionPolicy::DowncastBoxedDynTrait {
                    target_type: erased_runtime_target_type(target, target_type, facts),
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
        },
        (DispatchShape::ErasedObject, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    // The carrier holds the handle, so the downcast must ask for
                    // it; recovering `T` would hand back a copy.
                    let placement = match &target.java_ty {
                        JavaType::Named { name, .. } => facts.storage_placement_for(name),
                        _ => Default::default(),
                    };
                    let target_type = if placement.shared() && placement.is_implemented() {
                        placement.wrap(target_type)
                    } else {
                        target_type
                    };
                    DispatchCoercionPolicy::DowncastErasedObject {
                        target_type,
                        borrowed: matches!(
                            actual.ownership,
                            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                        ),
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

fn source_boxed_interface_upcast_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> Option<DispatchCoercionPolicy> {
    let source_trait = match &actual.dispatch {
        DispatchShape::DynTrait { trait_name } | DispatchShape::BoxedDynTrait { trait_name } => {
            trait_name.as_str()
        }
        // A conservative tag does not change the object-safe source
        // interface's physical Box carrier. Adapter evidence below is still
        // required; this does not box arbitrary concrete classes twice.
        DispatchShape::Concrete => java_type_name_for_dispatch(&actual.java_ty)?,
        _ => return None,
    };
    let DispatchShape::BoxedDynTrait {
        trait_name: target_trait,
    } = &target.dispatch
    else {
        return None;
    };
    if target.ownership != OwnershipShape::Owned
        || !object_safe_trait_boundary(&actual.java_ty, facts)
        || !object_safe_trait_boundary(&target.java_ty, facts)
    {
        return None;
    }
    let JavaType::Named { args, .. } = &actual.java_ty else {
        return None;
    };
    let supported = facts.source_interface_upcast_facts.iter().any(|adapter| {
        if adapter.type_params.len() != args.len() {
            return false;
        }
        let substitutions = adapter
            .type_params
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let source_type =
            crate::resolve::substitute_method_type_variables(&adapter.source_type, &substitutions);
        let target_type =
            crate::resolve::substitute_method_type_variables(&adapter.target_type, &substitutions);
        let (
            JavaType::Named {
                name: source_name, ..
            },
            JavaType::Named {
                name: target_name, ..
            },
        ) = (&source_type, &target_type)
        else {
            return false;
        };
        dyn_trait_rust_abi_matches(
            source_trait,
            &actual.java_ty,
            source_name,
            &source_type,
            facts,
        ) && dyn_trait_rust_abi_matches(
            target_trait,
            &target.java_ty,
            target_name,
            &target_type,
            facts,
        )
    });
    supported.then(|| DispatchCoercionPolicy::UpcastBoxedDynTrait {
        source_trait: dyn_trait_rust_abi_path(source_trait, &actual.java_ty, facts),
        target_trait: dyn_trait_rust_abi_path(target_trait, &target.java_ty, facts),
        clone_source: actual.ownership != OwnershipShape::Owned,
    })
}

fn dyn_trait_rust_abi_matches(
    actual_trait: &str,
    actual_ty: &JavaType,
    target_trait: &str,
    target_ty: &JavaType,
    facts: &FactSet,
) -> bool {
    dyn_trait_names_match(actual_trait, target_trait)
        && dyn_trait_rust_abi_path(actual_trait, actual_ty, facts)
            == dyn_trait_rust_abi_path(target_trait, target_ty, facts)
}

fn dyn_trait_rust_abi_path(trait_name: &str, java_ty: &JavaType, facts: &FactSet) -> String {
    crate::slot_render::parameterized_dyn_trait_path_with_facts(trait_name, java_ty, facts)
}

fn concrete_nominal_rust_abi_is_identical(actual: &RustSlotShape, target: &RustSlotShape) -> bool {
    let (
        JavaType::Named {
            args: actual_args, ..
        },
        JavaType::Named {
            args: target_args, ..
        },
    ) = (&actual.java_ty, &target.java_ty)
    else {
        return false;
    };
    actual_args == target_args
        && rust_nominal_type_name_policy(&actual.java_ty).into_target_type()
            == rust_nominal_type_name_policy(&target.java_ty).into_target_type()
}

fn erased_object_closed_enum_variants_for_target(
    target: &RustSlotShape,
    facts: &FactSet,
) -> Vec<String> {
    let target_java_ty = crate::types::optional_element_type(&target.java_ty)
        .unwrap_or_else(|| target.java_ty.clone());
    let Some(target_name) = java_type_name_for_dispatch(&target_java_ty) else {
        return Vec::new();
    };
    let target_args = match &target_java_ty {
        JavaType::Named { args, .. } => args.clone(),
        _ => Vec::new(),
    };
    let published = match &target.dispatch {
        DispatchShape::ClosedEnum { enum_name } => {
            facts.source_hierarchy_carrier_variants.get(enum_name)
        }
        _ => None,
    };
    let mut variants = published
        .cloned()
        .unwrap_or_else(|| vec![target_name.to_string()]);
    for fact in facts
        .type_capability_overrides
        .iter()
        .filter(|_| published.is_none())
    {
        if fact.provenance != crate::facts::FactProvenance::ParsedSource {
            continue;
        }
        if fact.extends.as_ref().and_then(java_type_name_for_dispatch) == Some(target_name)
            && !variants.iter().any(|existing| existing == &fact.type_name)
        {
            variants.push(fact.type_name.clone());
        }
    }
    variants
        .into_iter()
        .filter_map(|name| {
            let variant_java_ty = JavaType::Named {
                name: name.clone(),
                args: target_args.clone(),
            };
            let rust_name = rust_nominal_type_name_policy(&variant_java_ty).into_target_type()?;
            let rust_name = crate::slot_render::parameterized_closed_enum_type_name(
                &rust_name,
                &variant_java_ty,
            );
            let placement = facts.storage_placement_for(&name);
            Some(if placement.shared() && placement.is_implemented() {
                placement.wrap(rust_name)
            } else {
                rust_name
            })
        })
        .collect()
}

/// Rust variants in `actual`'s closed hierarchy whose Java classes also
/// inhabit `target`'s hierarchy/interface.  This is the common runtime subset
/// needed by both Java `instanceof` and carrier-to-carrier/trait projections.
fn closed_enum_variants_reaching_target(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> Vec<String> {
    let Some(actual_name) = java_type_name_for_dispatch(&actual.java_ty) else {
        return Vec::new();
    };
    let Some(target_name) = java_type_name_for_dispatch(&target.java_ty) else {
        return Vec::new();
    };
    let actual_ty = JavaType::Named {
        name: actual_name.to_string(),
        args: Vec::new(),
    };
    let target_ty = JavaType::Named {
        name: target_name.to_string(),
        args: Vec::new(),
    };
    let mut variants = std::collections::BTreeSet::new();
    let published = match &actual.dispatch {
        DispatchShape::ClosedEnum { enum_name } => {
            facts.source_hierarchy_carrier_variants.get(enum_name)
        }
        _ => None,
    };
    for fact in &facts.type_capability_overrides {
        if fact.provenance != crate::facts::FactProvenance::ParsedSource {
            continue;
        }
        if published.is_some_and(|owners| !owners.contains(&fact.type_name)) {
            continue;
        }
        let candidate = JavaType::Named {
            name: fact.type_name.clone(),
            args: Vec::new(),
        };
        if facts
            .hierarchy_subtype_policy(&candidate, &actual_ty)
            .reaches()
            && facts
                .hierarchy_subtype_policy(&candidate, &target_ty)
                .reaches()
            && let Some(variant) = rust_nominal_type_name_policy(&candidate).into_target_type()
        {
            variants.insert(variant);
        }
    }
    variants.into_iter().collect()
}

fn java_type_name_for_dispatch(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        _ => None,
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

fn external_crate_mapped_dyn_trait_java_type(java_ty: &JavaType, facts: &FactSet) -> bool {
    let JavaType::Named { name, args } = java_ty else {
        return false;
    };
    if !args.is_empty() {
        return false;
    }
    external_crate_mapped_dyn_trait_boundary(name, facts)
}

fn object_safe_trait_boundary(java_ty: &JavaType, facts: &FactSet) -> bool {
    match java_ty {
        // Object safety is a property of the interface declaration, not of
        // whether this use site supplies Java generic arguments. The latter
        // only parameterize the emitted `dyn Trait<T>` path.
        JavaType::Named { name, .. } => facts.object_safe_trait_fact_for_name(name) == Some(true),
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
        JavaType::String => Some("String"),
        JavaType::Named { name, .. } => Some(name),
        JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn dyn_trait_names_match(actual: &str, target: &str) -> bool {
    actual == target || rust_trait_name_for_match(actual) == rust_trait_name_for_match(target)
}

fn rust_trait_name_for_match(name: &str) -> String {
    if name.contains('.') || name.contains('$') {
        crate::java_naming::rust_name_for_java_named_type(name)
    } else {
        name.to_string()
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

fn java_collection_conversion_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> Option<DispatchCoercionPolicy> {
    let actual_kind = crate::stdlib::java_collection_kind_for_type(&actual.java_ty)?;
    let target_kind = crate::stdlib::java_collection_kind_for_type(&target.java_ty)?;
    let compatible = if actual_kind.is_map_like() && target_kind.is_map_like() {
        let (actual_key, actual_value) = crate::stdlib::java_map_type_args(&actual.java_ty)?;
        let (target_key, target_value) = crate::stdlib::java_map_type_args(&target.java_ty)?;
        actual_key == target_key && actual_value == target_value
    } else if actual_kind.is_set_like() && target_kind.is_set_like() {
        crate::types::sequence_element_type(&actual.java_ty)
            == crate::types::sequence_element_type(&target.java_ty)
    } else {
        false
    };
    (actual_kind != target_kind && compatible).then(|| DispatchCoercionPolicy::ExternalInto {
        from_type: actual.java_ty.clone(),
        to_type: target.java_ty.clone(),
    })
}

/// Native retained containers share one physical carrier within their family
/// even when Java names an interface or its implementation. Equality and
/// encounter order belong to the allocation, not a value conversion at an
/// assignment boundary. Collection accepts either family, while List and Set
/// remain distinct. Preserve source
/// precedence through the same authority used by slot rendering.
fn shared_native_collection_abi_matches(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> bool {
    if !actual.native_allocation.shared()
        || actual.native_allocation != target.native_allocation
        || actual.dispatch != DispatchShape::Concrete
        || target.dispatch != DispatchShape::Concrete
        || actual.optional_element_type().is_some()
        || target.optional_element_type().is_some()
    {
        return false;
    }
    let (Some(actual_kind), Some(target_kind)) = (
        facts.retained_native_container_kind(&actual.java_ty),
        facts.retained_native_container_kind(&target.java_ty),
    ) else {
        return false;
    };
    let (Some(actual_name), Some(target_name)) = (
        java_type_name_for_dispatch(&actual.java_ty),
        java_type_name_for_dispatch(&target.java_ty),
    ) else {
        return false;
    };
    if !crate::stdlib::is_known_java_platform_subtype(actual_name, target_name) {
        return false;
    }
    if actual_kind.is_map_like() || target_kind.is_map_like() {
        // The canonical map owns equality/encounter order. An interface
        // widening copies its handle, never Into<HashMap<..>> of its payload.
        // Both stored children must have the same solved physical contracts.
        return actual_kind.is_map_like()
            && target_kind.is_map_like()
            && crate::stdlib::java_map_type_args(&actual.java_ty)
                == crate::stdlib::java_map_type_args(&target.java_ty)
            && actual.nested_native_allocations == target.nested_native_allocations;
    }
    (actual_kind == target_kind
        || (actual_kind.is_set_like() && target_kind.is_set_like())
        || crate::stdlib::canonical_subtype_name(target_name) == "Collection")
        && crate::types::sequence_element_type(&actual.java_ty)
            == crate::types::sequence_element_type(&target.java_ty)
}

#[cfg(test)]
mod retained_collection_family_tests {
    use super::*;

    #[test]
    fn retained_map_widening_requires_matching_children_and_source_provenance() {
        let facts = FactSet::default();
        let effective = EffectiveFacts::new(&facts);
        for sync in [false, true] {
            let shape = |name: &str| {
                let mut shape = RustSlotShape::for_storage(JavaType::Named {
                    name: name.into(),
                    args: vec![JavaType::String, crate::parser::parse_type("String[]")],
                });
                shape.native_allocation =
                    crate::storage_strategy::StoragePlacement::new(true, true, sync);
                shape
            };
            let source = shape("java.util.LinkedHashMap");
            let target = shape("java.util.Map");
            assert!(shared_native_collection_abi_matches(
                &source, &target, &effective
            ));
            assert!(!shared_native_collection_abi_matches(
                &target, &source, &effective
            ));
            let mut different = target.clone();
            different.require_native_allocation_at(
                &[1],
                crate::storage_strategy::StoragePlacement::new(true, true, sync),
            );
            assert!(!shared_native_collection_abi_matches(
                &source, &different, &effective
            ));
            let mut nullable = target.clone();
            if let JavaType::Named { args, .. } = &mut nullable.java_ty {
                args[1] = crate::types::optional_java_type(args[1].clone());
            }
            assert!(!shared_native_collection_abi_matches(
                &source, &nullable, &effective
            ));
            let mut shadowed = facts.clone();
            shadowed
                .type_capability_overrides
                .push(crate::facts::TypeCapabilityOverride {
                    type_name: "Map".into(),
                    copy: None,
                    clone: None,
                    string_like: None,
                    sequence_like: None,
                    implements: Vec::new(),
                    extends: None,
                    final_type: None,
                    object_safe_trait: None,
                    provenance: crate::facts::FactProvenance::ParsedSource,
                });
            assert!(!shared_native_collection_abi_matches(
                &source,
                &shape("Map"),
                &EffectiveFacts::new(&shadowed)
            ));
        }
    }

    #[test]
    fn retained_collection_upcasts_preserve_runtime_family() {
        for sync in [false, true] {
            let shape = |name: &str| {
                let mut shape = RustSlotShape::for_storage(JavaType::Named {
                    name: name.into(),
                    args: vec![JavaType::String],
                });
                shape.native_allocation =
                    crate::storage_strategy::StoragePlacement::new(true, true, sync);
                shape
            };
            let facts = FactSet::default();
            let effective = EffectiveFacts::new(&facts);
            for source in ["java.util.List", "java.util.HashSet"] {
                assert!(shared_native_collection_abi_matches(
                    &shape(source),
                    &shape("java.util.Collection"),
                    &effective
                ));
            }
            assert!(!shared_native_collection_abi_matches(
                &shape("java.util.Set"),
                &shape("java.util.List"),
                &effective
            ));
            assert!(!shared_native_collection_abi_matches(
                &shape("java.util.List"),
                &shape("java.util.Set"),
                &effective
            ));
        }
    }
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
    actual: RustSlotShape,
    target: RustSlotShape,
    dispatch: DispatchCoercionPolicy,
    view: ViewCoercionPolicy,
    optional: OptionalShapeCoercionPolicy,
    ownership: OwnershipCoercionPolicy,
    primitive_widening: Option<PrimitiveWideningPolicy>,
    native_allocation: NativeAllocationCoercionPolicy,
    erased_array_adapter: ErasedArrayAdapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErasedArrayAdapter {
    IdentityOnly,
    Primitive,
    FinalReference,
    ErasedObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeAllocationCoercionPolicy {
    Preserve,
    UnsupportedPlacementChange {
        actual: crate::storage_strategy::StoragePlacement,
        target: crate::storage_strategy::StoragePlacement,
    },
}

impl NativeAllocationCoercionPolicy {
    pub fn unsupported_diagnostic(&self) -> Option<ConversionDiagnostic> {
        match self {
            Self::Preserve => None,
            Self::UnsupportedPlacementChange { actual, target } => Some(ConversionDiagnostic::new(
                "native_allocation_conversion_requires_adapter",
                format!(
                    "native allocation conversion requires an identity-preserving adapter: {actual:?} -> {target:?}"
                ),
            )),
        }
    }
}

pub fn shape_coercion_plan_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> ShapeCoercionPlan {
    // A legacy slice preference cannot change the payload of a shared
    // allocation. Canonicalize before selecting ownership/view conversions,
    // just as signature solving and type rendering do.
    fn canonical<'a>(
        shape: &'a RustSlotShape,
        counterpart: &RustSlotShape,
        facts: &FactSet,
    ) -> std::borrow::Cow<'a, RustSlotShape> {
        let effective = EffectiveFacts::new(facts);
        let base = shape
            .optional_element_type()
            .unwrap_or_else(|| shape.java_ty.clone());
        let native_sam = effective.native_functional_carrier(&base).is_some();
        let retained_view = matches!(shape.view, ViewShape::Slice | ViewShape::JavaSemanticHash)
            && effective
                .retained_native_collection_kind(&shape.java_ty)
                .is_some();
        let allocation = if matches!(
            shape.view,
            ViewShape::ListInterface | ViewShape::ReferenceArray
        ) {
            // This operation view owns its receiver reference internally.
            // A synchronized receiver does not add an outer Arc/Mutex or
            // change the local view's ABI.
            Default::default()
        } else if native_sam {
            if shape.native_allocation.sync() {
                crate::storage_strategy::StoragePlacement::new(true, false, true)
            } else {
                Default::default()
            }
        } else if crate::slot_render::native_allocation_has_outer_carrier(
            &shape.java_ty,
            &effective,
        ) || crate::slot_render::native_allocation_has_outer_carrier(
            &counterpart.java_ty,
            &effective,
        ) {
            // At a concrete native <-> symbolic generic boundary the
            // abstract side's qualifier describes the instantiated carrier.
            // Keep it for the same physical comparison as the concrete side.

            shape.native_allocation
        } else {
            Default::default()
        };
        let view = if allocation.shared() && (shape.view == ViewShape::Slice || retained_view) {
            ViewShape::Native
        } else {
            shape.view.clone()
        };
        if allocation != shape.native_allocation || view != shape.view {
            let mut canonical = shape.clone();
            canonical.native_allocation = allocation;
            canonical.view = view;
            std::borrow::Cow::Owned(canonical)
        } else {
            std::borrow::Cow::Borrowed(shape)
        }
    }
    let canonical_actual = canonical(actual, target, facts);
    let canonical_target = canonical(target, actual, facts);
    let actual = canonical_actual;
    let target = canonical_target;
    let actual = actual.as_ref();
    let target = target.as_ref();
    let mut plan = ShapeCoercionPlan::new(
        actual.clone(),
        target.clone(),
        dispatch_coercion_policy_with_facts(actual, target, facts),
        view_coercion_policy(actual, target, facts),
        optional_shape_coercion_policy_with_facts(actual, target, facts),
        ownership_coercion_policy(actual, target, facts),
        primitive_widening_policy(&actual.java_ty, &target.java_ty)
            .filter(|policy| policy.widens()),
    );
    if let JavaType::Array(element) = &actual.java_ty {
        let element =
            crate::types::optional_element_type(element).unwrap_or_else(|| (**element).clone());
        if let JavaType::Named { name, args } = &element
            && args.is_empty()
            && matches!(name.as_str(), "Object" | "java.lang.Object")
            && EffectiveFacts::new(facts).allows_platform_hierarchy_fallback(name)
        {
            plan.erased_array_adapter = ErasedArrayAdapter::ErasedObject;
        }
    }
    plan
}

impl ShapeCoercionPlan {
    pub(crate) fn may_throw_class_cast(&self) -> bool {
        self.dispatch.checked_cast_failure_policy().is_some()
            || match &self.optional {
                OptionalShapeCoercionPolicy::MapElement { element_plan, .. }
                | OptionalShapeCoercionPolicy::Wrap { element_plan, .. }
                | OptionalShapeCoercionPolicy::Unwrap { element_plan, .. } => {
                    element_plan.may_throw_class_cast()
                }
                OptionalShapeCoercionPolicy::NoChange => false,
            }
    }

    pub fn new(
        actual: RustSlotShape,
        target: RustSlotShape,
        dispatch: DispatchCoercionPolicy,
        view: ViewCoercionPolicy,
        optional: OptionalShapeCoercionPolicy,
        ownership: OwnershipCoercionPolicy,
        primitive_widening: Option<PrimitiveWideningPolicy>,
    ) -> Self {
        let erased_array_adapter = if let JavaType::Array(element) = &actual.java_ty {
            match crate::types::optional_element_type(element)
                .unwrap_or_else(|| (**element).clone())
            {
                JavaType::Primitive(_) => ErasedArrayAdapter::Primitive,
                JavaType::String => ErasedArrayAdapter::FinalReference,
                _ => ErasedArrayAdapter::IdentityOnly,
            }
        } else {
            ErasedArrayAdapter::IdentityOnly
        };
        let native_allocation = if actual.native_allocation == target.native_allocation
            || (!actual.native_allocation.shared() && !target.native_allocation.shared())
            // Erasure boxes the existing carrier; the Object slot has no
            // collection allocation to convert to. Preserve its identity.
            || matches!(dispatch, DispatchCoercionPolicy::EraseObject
                | DispatchCoercionPolicy::NativeIterableListView { .. })
        {
            NativeAllocationCoercionPolicy::Preserve
        } else {
            NativeAllocationCoercionPolicy::UnsupportedPlacementChange {
                actual: actual.native_allocation,
                target: target.native_allocation,
            }
        };
        Self {
            actual,
            target,
            dispatch,
            view,
            optional,
            ownership,
            primitive_widening,
            native_allocation,
            erased_array_adapter,
        }
    }

    pub fn actual(&self) -> &RustSlotShape {
        &self.actual
    }

    pub fn erased_array_adapter(&self) -> ErasedArrayAdapter {
        self.erased_array_adapter
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn dispatch(&self) -> &DispatchCoercionPolicy {
        &self.dispatch
    }

    pub fn view(&self) -> &ViewCoercionPolicy {
        &self.view
    }

    pub fn optional(&self) -> &OptionalShapeCoercionPolicy {
        &self.optional
    }

    pub fn ownership(&self) -> &OwnershipCoercionPolicy {
        &self.ownership
    }

    pub fn primitive_widening(&self) -> Option<PrimitiveWideningPolicy> {
        self.primitive_widening
    }

    pub fn native_allocation(&self) -> &NativeAllocationCoercionPolicy {
        &self.native_allocation
    }

    pub(crate) fn with_ownership(mut self, ownership: OwnershipCoercionPolicy) -> Self {
        self.ownership = ownership;
        self
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        let mut diagnostics = Vec::new();
        diagnostics.extend(self.native_allocation.unsupported_diagnostic());
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
    _actual_ownership: OwnershipShape,
    _target_ownership: OwnershipShape,
) -> ConversionDiagnostic {
    match policy {
        OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable { java_ty } => {
            ConversionDiagnostic::new(
                "unsupported_borrowed_to_owned_non_cloneable",
                format!(
                    "unsupported Java borrowed-to-owned recovery for non-cloneable type {java_ty:?}"
                ),
            )
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle { java_ty } => {
            ConversionDiagnostic::new(
                "unsupported_borrowed_to_facade_owned_external_handle",
                format!(
                    "unsupported Java borrowed-to-owned recovery for facade-owned external handle {java_ty:?}; use an explicit facade ownership transfer"
                ),
            )
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFfiOwnedHandle { java_ty } => {
            ConversionDiagnostic::new(
                "unsupported_borrowed_to_ffi_owned_handle",
                format!(
                    "unsupported Java borrowed-to-owned recovery for FFI-owned handle {java_ty:?}; use an explicit FFI ownership transfer"
                ),
            )
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
        DispatchCoercionFallbackPolicy::CompileError => ConversionDiagnostic::new(
            "unsupported_dispatch_coercion",
            format!(
                "unsupported Java dispatch coercion: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        ),
        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty } => {
            ConversionDiagnostic::new(
                "unsupported_dispatch_missing_runtime_target",
                format!(
                    "unsupported Java dispatch coercion: target type {java_ty:?} has no concrete runtime type"
                ),
            )
        }
        DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait { trait_name } => {
            ConversionDiagnostic::new(
                "unsupported_erased_object_to_dyn_trait",
                format!(
                    "unsupported Java erased object to borrowed dyn trait recovery for {trait_name}; a concrete downcast target is required"
                ),
            )
        }
        DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            ConversionDiagnostic::new(
                "unsupported_erased_object_to_boxed_dyn_trait",
                format!(
                    "unsupported Java erased object to owned boxed dyn trait recovery for {trait_name}; a concrete downcast target and ownership proof are required"
                ),
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxCoercionPlan {
    actual: JavaType,
    slot: Slot,
    policy: SyntaxSlotCoercionPolicy,
}

impl SyntaxCoercionPlan {
    pub(crate) fn new(actual: JavaType, slot: Slot, policy: SyntaxSlotCoercionPolicy) -> Self {
        Self {
            actual,
            slot,
            policy,
        }
    }

    pub fn actual(&self) -> &JavaType {
        &self.actual
    }

    pub fn slot(&self) -> &Slot {
        &self.slot
    }

    pub fn policy(&self) -> &SyntaxSlotCoercionPolicy {
        &self.policy
    }
}

pub fn syntax_coercion_plan_with_facts(
    actual: &JavaType,
    slot: &Slot,
    facts: &FactSet,
) -> SyntaxCoercionPlan {
    let content = coercion_content_policy(actual, facts);
    let policy = syntax_slot_coercion_policy_from_content(actual, slot, &content);
    SyntaxCoercionPlan::new(actual.clone(), slot.clone(), policy)
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
        Slot::StringConcatSegment | Slot::DisplaySegment
            if crate::stdlib::java_collection_kind_for_type(actual)
                .is_some_and(|kind| kind.is_map_like()) =>
        {
            SyntaxSlotCoercionPolicy::MapToJavaDisplayString
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if crate::stdlib::java_collection_kind_for_type(actual)
                .is_some_and(|kind| kind.is_set_like()) =>
        {
            SyntaxSlotCoercionPolicy::SetToJavaDisplayString
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if crate::stdlib::java_collection_kind_for_type(actual).is_some() =>
        {
            SyntaxSlotCoercionPolicy::CollectionToJavaDisplayString
        }
        // Java renders a concatenated `double`/`float` with a decimal point;
        // Rust's `Display` does not, so this needs its own conversion.
        Slot::StringConcatSegment | Slot::DisplaySegment
            if matches!(
                actual,
                JavaType::Primitive(PrimitiveType::Double | PrimitiveType::Float)
            ) =>
        {
            SyntaxSlotCoercionPolicy::ToJavaFloatStringForConcat {
                double: matches!(actual, JavaType::Primitive(PrimitiveType::Double)),
            }
        }
        Slot::StringConcatSegment if !content.string_content() => {
            SyntaxSlotCoercionPolicy::ToStringForConcat
        }
        _ => SyntaxSlotCoercionPolicy::Identity,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceOfCheck {
    ErasedIterable {
        optional: bool,
    },
    ErasedReferenceArray {
        optional: bool,
    },
    ThrowableType {
        class_name: String,
        optional: bool,
    },
    JavaSerializableMarker,
    /// `java.lang.Number` is an abstract supertype whose translated boxed
    /// values use their native Rust scalar carriers.  A nominal
    /// `Any::is::<Number>()` check can therefore never be correct.
    JavaNumberMarker,
    ExternalCapabilityMethod {
        method: String,
        optional: bool,
    },
    ClosedEnumVariant {
        enum_name: String,
        variant_name: String,
    },
    ClosedEnumVariants {
        enum_name: String,
        variant_names: Vec<String>,
    },
    OptionalClosedEnumVariant {
        enum_name: String,
        variant_name: String,
    },
    OptionalClosedEnumVariants {
        enum_name: String,
        variant_names: Vec<String>,
    },
    DynTraitType {
        target_type: String,
    },
    BoxedDynTraitType {
        target_type: String,
    },
    OptionalPayload {
        check: Box<InstanceOfCheck>,
    },
    ErasedObjectType {
        target_type: String,
    },
    OptionalErasedObjectType {
        target_type: String,
    },
    ErasedObjectClosedEnum {
        enum_name: String,
        carrier_type: String,
        variants: Vec<String>,
        optional: bool,
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
        InstanceOfFallbackPolicy::CompileError => ConversionDiagnostic::new(
            "unsupported_instanceof_shape",
            format!(
                "unsupported Java instanceof shape: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        ),
        InstanceOfFallbackPolicy::SourceHierarchyRuntimeCarrierRequired { actual, target } => {
            ConversionDiagnostic::new(
                "unsupported_instanceof_source_hierarchy_runtime_carrier",
                format!(
                    "unsupported Java instanceof check from source supertype {actual:?} to subtype {target:?}: a member-preserving runtime carrier is required"
                ),
            )
        }
        InstanceOfFallbackPolicy::PrimitiveTarget { java_ty } => ConversionDiagnostic::new(
            "unsupported_instanceof_primitive_target",
            format!("unsupported Java instanceof check: primitive target type {java_ty:?}"),
        ),
        InstanceOfFallbackPolicy::RawOrWildcardTarget { java_ty } => ConversionDiagnostic::new(
            "unsupported_instanceof_raw_or_wildcard_target",
            format!("unsupported Java instanceof check: raw or wildcard target type {java_ty:?}"),
        ),
        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty } => ConversionDiagnostic::new(
            "unsupported_instanceof_missing_runtime_target",
            format!(
                "unsupported Java instanceof check: target type {java_ty:?} has no concrete runtime type"
            ),
        ),
        InstanceOfFallbackPolicy::ErasedObjectToDynTrait { trait_name } => {
            ConversionDiagnostic::new(
                "unsupported_erased_object_instanceof_dyn_trait",
                format!(
                    "unsupported Java erased object instanceof borrowed dyn trait check for {trait_name}; a concrete runtime target is required"
                ),
            )
        }
        InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            ConversionDiagnostic::new(
                "unsupported_erased_object_instanceof_boxed_dyn_trait",
                format!(
                    "unsupported Java erased object instanceof owned boxed dyn trait check for {trait_name}; a concrete runtime target is required"
                ),
            )
        }
    }
}

pub fn instanceof_target_policy(target: &RustSlotShape, facts: &FactSet) -> InstanceOfTargetPolicy {
    let capability = facts.instanceof_target_capability_policy(&target.java_ty);
    if capability.primitive_target() {
        return InstanceOfTargetPolicy::PrimitiveTarget {
            java_ty: target.java_ty.clone(),
        };
    }
    if capability.raw_or_wildcard_target() {
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
        DispatchShape::BoxedDynTrait { trait_name }
        | DispatchShape::ReferenceDynTrait { trait_name, .. } => {
            InstanceOfTargetPolicy::BoxedDynTraitTarget {
                trait_name: trait_name.clone(),
            }
        }
        DispatchShape::ErasedObject => InstanceOfTargetPolicy::ErasedObjectTarget,
    }
}

/// The type an erased carrier actually stores for `target`.
///
/// A shared source type is boxed as its handle, so both the runtime type test
/// and the downcast have to name `Rc<T>`/`Arc<T>` rather than `T`.
fn erased_runtime_target_type(
    target: &RustSlotShape,
    target_type: String,
    facts: &FactSet,
) -> String {
    if target.java_ty == JavaType::String {
        // Erasure stores the canonical owning carrier, never a borrowed view
        // or its nominal Java/Rust spelling. Keep type tests and downcasts on
        // the same String/JavaString representation as producer slot planning.
        return crate::emit::emit_type(&crate::slot_render::rust_type_for_slot_with_facts(
            &RustSlotShape::for_storage(JavaType::String),
            facts,
        ));
    }
    let JavaType::Named { name, .. } = &target.java_ty else {
        return target_type;
    };
    let placement = facts.storage_placement_for(name);
    if placement.shared() && placement.is_implemented() {
        placement.wrap(target_type)
    } else {
        target_type
    }
}

pub fn plan_instanceof_check_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> InstanceOfCheck {
    if actual.dispatch == DispatchShape::ErasedObject {
        let effective = EffectiveFacts::new(facts);
        if let JavaType::Named { name, args } = &target.java_ty
            && matches!(name.as_str(), "Iterable" | "java.lang.Iterable")
            && effective.allows_platform_hierarchy_fallback(name)
            && (args.is_empty()
                || args
                    .iter()
                    .all(|arg| matches!(arg, JavaType::Wildcard(None))))
        {
            return InstanceOfCheck::ErasedIterable {
                optional: actual.optional_element_type().is_some(),
            };
        }
        if let JavaType::Array(element) = &target.java_ty
            && let JavaType::Named { name, args } = element.as_ref()
            && matches!(name.as_str(), "Object" | "java.lang.Object")
            && args.is_empty()
            && effective.allows_platform_hierarchy_fallback(name)
        {
            return InstanceOfCheck::ErasedReferenceArray {
                optional: actual.optional_element_type().is_some(),
            };
        }
    }
    if EffectiveFacts::new(facts)
        .native_iterable_list_projection_element(&actual.java_ty, &target.java_ty)
        .is_some()
    {
        return InstanceOfCheck::ExternalCapabilityMethod {
            method: "list_type_test_known".into(),
            optional: actual.optional_element_type().is_some(),
        };
    }
    if is_java_serializable_marker_type(&target.java_ty) {
        return InstanceOfCheck::JavaSerializableMarker;
    }
    if is_java_number_marker_type(&target.java_ty) {
        return InstanceOfCheck::JavaNumberMarker;
    }
    let actual_ty = actual
        .optional_element_type()
        .unwrap_or_else(|| actual.java_ty.clone());
    if let JavaType::Named { name, .. } | JavaType::Unknown(name) = &actual_ty
        && (name == "JavaException"
            || (crate::stdlib::is_known_throwable_carrier_type(name)
                && !EffectiveFacts::new(facts).has_parsed_source_type_capability_for_name(name)))
        && let JavaType::Named {
            name: class_name, ..
        }
        | JavaType::Unknown(class_name) = &target.java_ty
    {
        return InstanceOfCheck::ThrowableType {
            class_name: facts.exception_binary_name_for(class_name).to_owned(),
            optional: actual.optional_element_type().is_some(),
        };
    }
    if let Some(method) = external_capability_instanceof_method(&actual.java_ty, &target.java_ty) {
        return InstanceOfCheck::ExternalCapabilityMethod {
            method: method.to_string(),
            optional: actual.optional_element_type().is_some(),
        };
    }
    if let Some(target_type) = reifiable_raw_collection_runtime_target(&target.java_ty) {
        if matches!(actual.dispatch, DispatchShape::ErasedObject) {
            return InstanceOfCheck::ErasedObjectType { target_type };
        }
        if let Some(element_ty) = actual.optional_element_type() {
            let element_shape = optional_element_slot_shape(actual, element_ty);
            if matches!(element_shape.dispatch, DispatchShape::ErasedObject) {
                return InstanceOfCheck::OptionalErasedObjectType { target_type };
            }
        }
    }
    match instanceof_target_policy(target, facts) {
        InstanceOfTargetPolicy::PrimitiveTarget { java_ty } => {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::PrimitiveTarget { java_ty },
            );
        }
        InstanceOfTargetPolicy::RawOrWildcardTarget { java_ty }
            if reifiable_raw_collection_runtime_target(&java_ty).is_none() =>
        {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::RawOrWildcardTarget { java_ty },
            );
        }
        InstanceOfTargetPolicy::RawOrWildcardTarget { .. } => {}
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
    if hierarchy_instanceof_policy(actual, target, facts).always_true() {
        return InstanceOfCheck::AlwaysTrue;
    }
    if let Some(element_ty) = actual.optional_element_type() {
        let element_shape = optional_element_slot_shape(actual, element_ty);
        if matches!(
            element_shape.dispatch,
            DispatchShape::DynTrait { .. }
                | DispatchShape::BoxedDynTrait { .. }
                | DispatchShape::ReferenceDynTrait { .. }
        ) && matches!(target.dispatch, DispatchShape::Concrete)
        {
            return InstanceOfCheck::OptionalPayload {
                check: Box::new(plan_instanceof_check_with_facts(
                    &element_shape,
                    target,
                    facts,
                )),
            };
        }
        if matches!(element_shape.dispatch, DispatchShape::ErasedObject)
            && matches!(target.dispatch, DispatchShape::ClosedEnum { .. })
            && let InstanceOfCheck::ErasedObjectClosedEnum {
                enum_name,
                carrier_type,
                variants,
                ..
            } = plan_instanceof_check_with_facts(&element_shape, target, facts)
        {
            return InstanceOfCheck::ErasedObjectClosedEnum {
                enum_name,
                carrier_type,
                variants,
                optional: true,
            };
        }
        match (&element_shape.dispatch, &target.dispatch) {
            (DispatchShape::ClosedEnum { enum_name }, DispatchShape::DynTrait { .. })
            | (DispatchShape::ClosedEnum { enum_name }, DispatchShape::ClosedEnum { .. }) => {
                let variant_names =
                    closed_enum_variants_reaching_target(&element_shape, target, facts);
                if !variant_names.is_empty() {
                    return InstanceOfCheck::OptionalClosedEnumVariants {
                        enum_name: enum_name.clone(),
                        variant_names,
                    };
                }
            }
            (DispatchShape::ClosedEnum { enum_name }, DispatchShape::Concrete) => {
                match runtime_type_target_policy(&target.java_ty) {
                    RuntimeTypeTargetPolicy::Nominal { target_type } => {
                        return InstanceOfCheck::OptionalClosedEnumVariant {
                            enum_name: enum_name.clone(),
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
            _ => {}
        }
        if matches!(element_shape.dispatch, DispatchShape::ErasedObject)
            && matches!(target.dispatch, DispatchShape::Concrete)
        {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    return InstanceOfCheck::OptionalErasedObjectType {
                        target_type: erased_runtime_target_type(target, target_type, facts),
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
    }
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::DynTrait { .. })
        | (DispatchShape::ClosedEnum { enum_name }, DispatchShape::ClosedEnum { .. }) => {
            let variant_names = closed_enum_variants_reaching_target(actual, target, facts);
            if !variant_names.is_empty() {
                return InstanceOfCheck::ClosedEnumVariants {
                    enum_name: enum_name.clone(),
                    variant_names,
                };
            }
        }
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
                    return InstanceOfCheck::DynTraitType {
                        target_type: erased_runtime_target_type(target, target_type, facts),
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
        (
            DispatchShape::BoxedDynTrait { .. } | DispatchShape::ReferenceDynTrait { .. },
            DispatchShape::Concrete,
        ) => match runtime_type_target_policy(&target.java_ty) {
            RuntimeTypeTargetPolicy::Nominal { target_type } => {
                return InstanceOfCheck::BoxedDynTraitType {
                    target_type: erased_runtime_target_type(target, target_type, facts),
                };
            }
            RuntimeTypeTargetPolicy::MissingNominalTarget { java_ty } => {
                return unsupported_instanceof_check(
                    actual,
                    target,
                    InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty },
                );
            }
        },
        (DispatchShape::ErasedObject, DispatchShape::Concrete) => {
            match runtime_type_target_policy(&target.java_ty) {
                RuntimeTypeTargetPolicy::Nominal { target_type } => {
                    // The same handle the *downcast* asks for. An erased carrier
                    // holding a shared type stores `Rc<T>`, so `is::<T>()` is
                    // always false and the guard silently rejects every value --
                    // the two must name one type. See claude_audits/0002b.
                    return InstanceOfCheck::ErasedObjectType {
                        target_type: erased_runtime_target_type(target, target_type, facts),
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
        (DispatchShape::ErasedObject, DispatchShape::ClosedEnum { enum_name }) => {
            let variants = erased_object_closed_enum_variants_for_target(target, facts);
            if !variants.is_empty() {
                return InstanceOfCheck::ErasedObjectClosedEnum {
                    enum_name: enum_name.clone(),
                    carrier_type: erased_runtime_target_type(target, enum_name.clone(), facts),
                    variants,
                    optional: false,
                };
            }
        }
        (DispatchShape::Concrete, DispatchShape::Concrete) if actual.java_ty == target.java_ty => {
            return InstanceOfCheck::AlwaysTrue;
        }
        (DispatchShape::Concrete, DispatchShape::Concrete)
            if facts
                .hierarchy_subtype_policy(&target.java_ty, &actual.java_ty)
                .reaches() =>
        {
            return unsupported_instanceof_check(
                actual,
                target,
                InstanceOfFallbackPolicy::SourceHierarchyRuntimeCarrierRequired {
                    actual: actual.java_ty.clone(),
                    target: target.java_ty.clone(),
                },
            );
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

fn external_capability_instanceof_method(
    actual: &JavaType,
    target: &JavaType,
) -> Option<&'static str> {
    let actual_element = crate::types::optional_element_type(actual);
    let actual_name = match actual_element.as_ref().unwrap_or(actual) {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        JavaType::String => "String",
        _ => return None,
    };
    let target_element = crate::types::optional_element_type(target);
    let target_name = match target_element.as_ref().unwrap_or(target) {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        JavaType::String => "String",
        _ => return None,
    };
    let actual = crate::stdlib::canonical_external_receiver_type_key(actual_name);
    let target = crate::stdlib::canonical_external_receiver_type_key(target_name);
    match (actual.as_str(), target.as_str()) {
        ("Appendable", "Closeable") => Some("is_closeable"),
        ("Appendable", "Flushable") => Some("is_flushable"),
        ("Appendable", "Writer") => Some("is_writer"),
        ("CharSequence", "String") => Some("is_string"),
        ("CharSequence", "StringBuffer") => Some("is_string_buffer"),
        _ => None,
    }
}

fn is_java_serializable_marker_type(ty: &JavaType) -> bool {
    matches!(
        ty,
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Serializable"
    )
}

fn is_java_number_marker_type(ty: &JavaType) -> bool {
    matches!(
        ty,
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Number"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyInstanceOfPolicy {
    actual: JavaType,
    target: JavaType,
    always_true: bool,
}

impl HierarchyInstanceOfPolicy {
    pub(crate) fn new(actual: JavaType, target: JavaType, always_true: bool) -> Self {
        Self {
            actual,
            target,
            always_true,
        }
    }

    pub fn actual(&self) -> &JavaType {
        &self.actual
    }

    pub fn target(&self) -> &JavaType {
        &self.target
    }

    pub fn always_true(&self) -> bool {
        self.always_true
    }
}

pub fn hierarchy_instanceof_policy(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> HierarchyInstanceOfPolicy {
    let subtype = facts.hierarchy_subtype_policy(&actual.java_ty, &target.java_ty);
    HierarchyInstanceOfPolicy::new(
        actual.java_ty.clone(),
        target.java_ty.clone(),
        subtype.reaches(),
    )
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
    RustOwnedFacadeReturn(RustOwnedFacadeReturnPolicy),
    OptionalRustBorrowedFacadeReturn(RustBorrowedFacadeReturnPolicy),
    OptionalRustOwnedFacadeReturn(RustOwnedFacadeReturnPolicy),
    RustUnsignedByteArrayReturn(RustUnsignedByteArrayReturnPolicy),
    Identity { actual: LoweredType, slot: Slot },
}

impl SlotConversionPlan {
    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        match self {
            SlotConversionPlan::Shape(plan) => plan.unsupported_diagnostics(),
            SlotConversionPlan::OptionalConstruction(plan) => {
                plan.element_plan().unsupported_diagnostics()
            }
            SlotConversionPlan::BorrowedElement(policy) => policy.unsupported_diagnostics(),
            SlotConversionPlan::Syntax(_)
            | SlotConversionPlan::RustScalarCast(_)
            | SlotConversionPlan::RustBorrowedFacadeReturn(_)
            | SlotConversionPlan::RustOwnedFacadeReturn(_)
            | SlotConversionPlan::OptionalRustBorrowedFacadeReturn(_)
            | SlotConversionPlan::OptionalRustOwnedFacadeReturn(_)
            | SlotConversionPlan::RustUnsignedByteArrayReturn(_)
            | SlotConversionPlan::Identity { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastConversionPlan {
    actual: RustSlotShape,
    target: RustSlotShape,
    semantics: JavaCastSemanticsPolicy,
    coercion: ShapeCoercionPlan,
}

impl CastConversionPlan {
    pub(crate) fn new(
        actual: RustSlotShape,
        target: RustSlotShape,
        semantics: JavaCastSemanticsPolicy,
        coercion: ShapeCoercionPlan,
    ) -> Self {
        Self {
            actual,
            target,
            semantics,
            coercion,
        }
    }

    pub fn actual(&self) -> &RustSlotShape {
        &self.actual
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn semantics(&self) -> &JavaCastSemanticsPolicy {
        &self.semantics
    }

    pub fn coercion(&self) -> &ShapeCoercionPlan {
        &self.coercion
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.coercion.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceOfPlan {
    actual: RustSlotShape,
    target: RustSlotShape,
    check: InstanceOfCheck,
}

impl InstanceOfPlan {
    pub(crate) fn new(
        actual: RustSlotShape,
        target: RustSlotShape,
        check: InstanceOfCheck,
    ) -> Self {
        Self {
            actual,
            target,
            check,
        }
    }

    pub fn actual(&self) -> &RustSlotShape {
        &self.actual
    }

    pub fn target(&self) -> &RustSlotShape {
        &self.target
    }

    pub fn check(&self) -> &InstanceOfCheck {
        &self.check
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.check.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotExprConversionPlan {
    actual: RustSlotShape,
    slot: Slot,
    conversion: SlotConversionPlan,
}

impl SlotExprConversionPlan {
    pub fn new(actual: RustSlotShape, slot: Slot, conversion: SlotConversionPlan) -> Self {
        Self {
            actual,
            slot,
            conversion,
        }
    }

    pub fn actual(&self) -> &RustSlotShape {
        &self.actual
    }

    pub fn slot(&self) -> &Slot {
        &self.slot
    }

    pub fn conversion(&self) -> &SlotConversionPlan {
        &self.conversion
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.conversion.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSlotExprConversionPlan {
    actual: LoweredType,
    slot: Slot,
    conversion: SlotConversionPlan,
}

impl LoweredSlotExprConversionPlan {
    pub fn new(actual: LoweredType, slot: Slot, conversion: SlotConversionPlan) -> Self {
        Self {
            actual,
            slot,
            conversion,
        }
    }

    pub fn actual(&self) -> &LoweredType {
        &self.actual
    }

    pub fn slot(&self) -> &Slot {
        &self.slot
    }

    pub fn conversion(&self) -> &SlotConversionPlan {
        &self.conversion
    }

    pub fn unsupported_diagnostics(&self) -> Vec<ConversionDiagnostic> {
        self.conversion.unsupported_diagnostics()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReusedOptionalUnwrapProjection {
    KeepCarrier,
    ExpectBorrowedCarrier,
    BorrowElement { ownership: OwnershipShape },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReusedOptionalUnwrapPlan {
    actual: LoweredType,
    projection: ReusedOptionalUnwrapProjection,
}

impl ReusedOptionalUnwrapPlan {
    fn new(actual: LoweredType, projection: ReusedOptionalUnwrapProjection) -> Self {
        Self { actual, projection }
    }

    pub(crate) fn actual(&self) -> &LoweredType {
        &self.actual
    }

    pub(crate) fn projection(&self) -> ReusedOptionalUnwrapProjection {
        self.projection
    }
}

pub(crate) fn reused_optional_unwrap_plan_for_slot(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
    slot: &Slot,
    move_owned_carrier: bool,
    facts: &FactSet,
) -> Option<ReusedOptionalUnwrapPlan> {
    let target = slot_target_shape(slot)?;
    if matches!(
        target.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        if target.ownership == OwnershipShape::SharedBorrow
            && option_shape.ownership == OwnershipShape::SharedBorrow
            && option_shape.view == target.view
        {
            let actual = LoweredType::optional_element_borrow_from_option_shape(
                option_shape.clone(),
                element_ty,
                Some(OwnershipShape::SharedBorrow),
                facts,
            )?;
            return Some(ReusedOptionalUnwrapPlan::new(
                actual,
                ReusedOptionalUnwrapProjection::ExpectBorrowedCarrier,
            ));
        }
        let actual = LoweredType::optional_element_borrow_from_option_shape(
            option_shape.clone(),
            element_ty,
            Some(target.ownership),
            facts,
        )?;
        return Some(ReusedOptionalUnwrapPlan::new(
            actual,
            ReusedOptionalUnwrapProjection::BorrowElement {
                ownership: target.ownership,
            },
        ));
    }
    if option_shape.ownership == OwnershipShape::SharedBorrow {
        let actual = LoweredType::optional_element_borrow_from_option_shape(
            option_shape.clone(),
            element_ty,
            Some(OwnershipShape::SharedBorrow),
            facts,
        )?;
        return Some(ReusedOptionalUnwrapPlan::new(
            actual,
            ReusedOptionalUnwrapProjection::KeepCarrier,
        ));
    }
    if move_owned_carrier && option_shape.ownership == OwnershipShape::Owned {
        let actual = LoweredType::optional_element_borrow_from_option_shape(
            option_shape.clone(),
            element_ty,
            Some(OwnershipShape::Owned),
            facts,
        )?;
        return Some(ReusedOptionalUnwrapPlan::new(
            actual,
            ReusedOptionalUnwrapProjection::KeepCarrier,
        ));
    }
    let actual = LoweredType::optional_element_borrow_from_option_shape(
        option_shape.clone(),
        element_ty,
        Some(OwnershipShape::SharedBorrow),
        facts,
    )?;
    Some(ReusedOptionalUnwrapPlan::new(
        actual,
        ReusedOptionalUnwrapProjection::BorrowElement {
            ownership: OwnershipShape::SharedBorrow,
        },
    ))
}

fn slot_target_shape(slot: &Slot) -> Option<&RustSlotShape> {
    match slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => Some(shape),
        Slot::Value
        | Slot::Condition
        | Slot::Index
        | Slot::StringConcatSegment
        | Slot::DisplaySegment => None,
    }
}

pub(crate) fn receiver_method_autoref_satisfies_facade_slot(
    receiver_abi: crate::facts::ReceiverAbi,
    plan: &LoweredSlotExprConversionPlan,
) -> bool {
    let SlotConversionPlan::Shape(shape_plan) = plan.conversion() else {
        return false;
    };
    matches!(
        (
            shape_plan.dispatch(),
            shape_plan.view(),
            shape_plan.optional()
        ),
        (
            DispatchCoercionPolicy::NoDispatchChange,
            ViewCoercionPolicy::NoViewChange,
            OptionalShapeCoercionPolicy::NoChange
        )
    ) && shape_plan.primitive_widening().is_none()
        && matches!(
            (receiver_abi, shape_plan.ownership()),
            (
                crate::facts::ReceiverAbi::BorrowedSelf,
                OwnershipCoercionPolicy::BorrowOwnedValue
            ) | (
                crate::facts::ReceiverAbi::MutBorrowedSelf,
                OwnershipCoercionPolicy::BorrowOwnedMutably
            )
        )
}

pub(crate) fn receiver_method_autoref_satisfies_optional_slot(
    plan: &LoweredSlotExprConversionPlan,
) -> bool {
    let SlotConversionPlan::Shape(shape_plan) = plan.conversion() else {
        return false;
    };
    matches!(
        shape_plan.ownership(),
        OwnershipCoercionPolicy::NoOwnershipChange
    ) && matches!(shape_plan.optional(), OptionalShapeCoercionPolicy::NoChange)
        && shape_plan.primitive_widening().is_none()
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
    let mut target = project_java_value_shape(ctx.representations(), target_ty.clone());
    crate::shape_projection::preserve_reference_cast_nullability(&actual, &mut target);
    let default_facts = FactSet::default();
    let facts = ctx.facts().unwrap_or(&default_facts);
    crate::shape_projection::preserve_owned_interface_upcast(&actual, &mut target, facts);
    let list_projection =
        crate::shape_projection::project_native_iterable_list_view(&actual, &mut target, facts);
    if !list_projection && reifiable_raw_collection_runtime_target(target_ty).is_some() {
        target.dispatch = DispatchShape::Concrete;
        target.view = ViewShape::Native;
        target.ownership = OwnershipShape::Owned;
    }
    if external_capability_instanceof_method(&actual.java_ty, target_ty).is_some() {
        // The runtime carrier owns the capability dispatch.  A guarded Java
        // interface cast must therefore preserve that carrier instead of
        // erasing it into `Box<dyn Any>` and losing close/flush behavior.
        target = actual
            .optional_element_type()
            .map(|element_ty| {
                let mut element =
                    optional_element_slot_shape_with_facts(&actual, element_ty, facts);
                // A Java reference cast copies the reference.  Borrow the
                // nullable carrier's payload so a cast of a field through
                // `&self` cannot move its `Option<T>` out of the receiver.
                element.ownership = OwnershipShape::SharedBorrow;
                element
            })
            .unwrap_or_else(|| actual.clone());
    }
    if java_type_is_erased_object_like(target_ty)
        && matches!(actual.dispatch, DispatchShape::ErasedObject)
    {
        target = actual.clone();
    }
    if same_java_type_cast_should_preserve_actual_shape(&actual, &target) {
        target = actual.clone();
    }
    let coercion = shape_coercion_plan_with_facts(&actual, &target, facts);
    let semantics = java_cast_semantics_policy_with_facts(&actual, &target, &coercion, facts);

    CastConversionPlan::new(actual, target, semantics, coercion)
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
    let target = project_java_value_shape(ctx.representations(), target_ty.clone());
    let default_facts = FactSet::default();
    let facts = ctx.facts().unwrap_or(&default_facts);
    let mut check = plan_instanceof_check_with_facts(&actual, &target, facts);
    if let InstanceOfCheck::ErasedObjectClosedEnum { carrier_type, .. } = &mut check
        && let Some(representations) = ctx.representations()
        && matches!(
            representations.representation_for(&target.java_ty),
            crate::representation::ObjectRepresentation::SourceHierarchyCarrier { .. }
        )
    {
        // The enum already carries canonical shared payloads. Erasure boxes
        // this value carrier, not an additional Rc<Enum> guessed from the
        // Java root's storage placement. Use the same ABI as value emission.
        *carrier_type = crate::emit::emit_type(
            &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                &target,
                facts,
                representations,
            ),
        );
    }

    InstanceOfPlan::new(actual, target, check)
}

pub fn plan_slot_conversion(
    actual: RustSlotShape,
    slot: Slot,
    facts: &FactSet,
) -> SlotExprConversionPlan {
    let conversion = slot_conversion_plan_with_facts(&actual, &slot, facts);
    SlotExprConversionPlan::new(actual, slot, conversion)
}

pub fn slot_conversion_plan_with_facts(
    actual: &RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> SlotConversionPlan {
    slot_conversion_plan_by_value(actual, slot, facts)
}

fn slot_conversion_plan_by_value(
    actual: &RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> SlotConversionPlan {
    if matches!(slot, Slot::StringConcatSegment | Slot::DisplaySegment)
        && let Some(kind) = nullable_native_collection_display_kind(actual, facts)
    {
        return SlotConversionPlan::Syntax(SyntaxCoercionPlan::new(
            actual.java_ty.clone(),
            slot.clone(),
            SyntaxSlotCoercionPolicy::OptionalCollectionToJavaDisplayString { kind },
        ));
    }
    match slot {
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            SlotConversionPlan::Shape(shape_coercion_plan_with_facts(actual, target, facts))
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if actual.optional_element_type().is_some_and(|element_ty| {
                optional_element_is_erased_object_display_like(actual, element_ty)
            }) =>
        {
            SlotConversionPlan::Syntax(SyntaxCoercionPlan::new(
                actual.java_ty.clone(),
                slot.clone(),
                SyntaxSlotCoercionPolicy::OptionalErasedObjectToJavaDisplayString,
            ))
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if actual.optional_element_type().is_some_and(|element_ty| {
                coercion_content_policy(&element_ty, facts).string_content()
            }) =>
        {
            SlotConversionPlan::Syntax(SyntaxCoercionPlan::new(
                actual.java_ty.clone(),
                slot.clone(),
                SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString,
            ))
        }
        Slot::StringConcatSegment | Slot::DisplaySegment
            if actual
                .optional_element_type()
                .is_some_and(|element_ty| optional_display_value_supported(&element_ty, facts)) =>
        {
            let policy = actual
                .optional_element_type()
                .filter(|element_ty| optional_display_value_uses_java_helper(element_ty, facts))
                .map(|_| SyntaxSlotCoercionPolicy::OptionalToJavaDisplayValue)
                .unwrap_or(SyntaxSlotCoercionPolicy::OptionalToDisplayValue);
            SlotConversionPlan::Syntax(SyntaxCoercionPlan::new(
                actual.java_ty.clone(),
                slot.clone(),
                policy,
            ))
        }
        _ => SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts(
            &actual.java_ty,
            slot,
            facts,
        )),
    }
}

pub(crate) fn nullable_native_collection_display_kind(
    shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<crate::stdlib::JavaCollectionKind> {
    shape.optional_element_type()?;
    native_collection_display_kind(shape, facts)
}

/// Operands whose Java value conversion can invoke Object/collection code.
/// Keep the effect proof aligned with concat planning and platform precedence.
pub(crate) fn implicit_java_display_operands<'a>(
    expr: &'a Expr,
    types: &crate::types::TypeEnv,
    facts: &FactSet,
) -> Vec<&'a Expr> {
    if let Some(plan) = crate::string_concat_plan::build_string_concat_plan(expr, types) {
        return plan.args().to_vec();
    }
    if let Some(crate::call_plan::CallStatementPlan::NativeSystemPrintStream {
        arg: Some(arg),
        ..
    }) = crate::call_plan::plan_call_statement(expr, facts, types)
    {
        let source_owner = matches!(expr, Expr::Call { resolved: Some(call), .. }
            if EffectiveFacts::new(facts).parsed_source_controls_native_carrier(&JavaType::Named { name: call.declaring_type.clone(), args: vec![] }));
        if !source_owner {
            return vec![arg];
        }
    }
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return vec![];
    };
    let owner = if let Some(resolved) = resolved {
        if resolved.kind != crate::java_ir::CallKind::Static {
            return vec![];
        }
        resolved.declaring_type.as_str()
    } else if let Expr::Var(owner) = receiver.as_ref() {
        if types.local_type(owner).is_some() {
            return vec![];
        }
        types.static_type_alias(owner).unwrap_or(owner)
    } else {
        return vec![];
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(owner);
    let platform = matches!(
        (canonical.as_str(), name.as_str(), args.len()),
        ("String", "valueOf" | "value_of", 1)
            | (
                "Objects" | "JavaUtilObjects",
                "toString" | "to_string",
                1 | 2
            )
    );
    let format = canonical == "String" && name == "format" && !args.is_empty();
    if (platform || format)
        && !EffectiveFacts::new(facts).parsed_source_controls_native_carrier(&JavaType::Named {
            name: owner.into(),
            args: vec![],
        })
    {
        if format {
            args.iter().skip(1).collect()
        } else {
            vec![&args[0]]
        }
    } else {
        vec![]
    }
}

/// Source precedence and nominal native kind are independent of nullable and
/// local/sync physical allocation choices. Shared by lowering and effects.
pub(crate) fn native_collection_display_kind(
    shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<crate::stdlib::JavaCollectionKind> {
    let element = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    if EffectiveFacts::new(facts).parsed_source_controls_native_carrier(&element) {
        return None;
    }
    crate::stdlib::java_collection_kind_for_type(&element)
}

pub(crate) fn optional_display_value_uses_java_helper(
    java_ty: &JavaType,
    _facts: &FactSet,
) -> bool {
    match java_ty {
        JavaType::Primitive(_) => true,
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::boxed_primitive_type(
                crate::stdlib::canonical_external_receiver_type_key(name)
                    .rsplit('.')
                    .next()
                    .unwrap_or(name),
            )
            .is_some()
        }
        JavaType::String
        | JavaType::Void
        | JavaType::Array(_)
        | JavaType::Intersection(_)
        | JavaType::Wildcard(_) => false,
    }
}

fn optional_borrowed_element_is_nullable_string(java_ty: &JavaType) -> bool {
    optional_element_type(java_ty).is_some_and(|element_ty| element_ty == JavaType::String)
}

fn optional_display_value_supported(java_ty: &JavaType, facts: &FactSet) -> bool {
    let content = coercion_content_policy(java_ty, facts);
    if content.string_content() {
        return false;
    }
    match java_ty {
        JavaType::Primitive(_) => true,
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::boxed_primitive_type(
                crate::stdlib::canonical_external_receiver_type_key(name)
                    .rsplit('.')
                    .next()
                    .unwrap_or(name),
            )
            .is_some()
                || crate::stdlib::is_java_platform_facade_type_name(name)
        }
        JavaType::String
        | JavaType::Void
        | JavaType::Array(_)
        | JavaType::Intersection(_)
        | JavaType::Wildcard(_) => false,
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
    let mut target_shape = project_java_value_shape(ctx.representations(), target_ty);
    crate::shape_projection::preserve_reference_cast_nullability(inner_shape, &mut target_shape);
    if let Some(facts) = ctx.facts() {
        crate::shape_projection::preserve_owned_interface_upcast(
            inner_shape,
            &mut target_shape,
            facts,
        );
    }
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
    LoweredSlotExprConversionPlan::new(actual.clone(), slot, conversion)
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
                SlotConversionPlan::RustScalarCast(RustScalarSlotCastPolicy::new(
                    rust_type_name.clone(),
                    target_primitive,
                ))
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
            SlotConversionPlan::RustBorrowedFacadeReturn(
                RustBorrowedFacadeReturnPolicy::with_source_erased_object(
                    target.clone(),
                    recovery.unwrap_or(RustBorrowedFacadeReturnRecovery::Clone),
                    slot_or_optional_element_targets_erased_object(java_shape),
                ),
            )
        }
        (
            LoweredType::RustBorrowed { java_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(java_shape, slot, facts),
        (
            LoweredType::RustOwned { java_shape: _, .. },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if slot_or_optional_element_targets_erased_object(target) => {
            SlotConversionPlan::RustOwnedFacadeReturn(RustOwnedFacadeReturnPolicy::new(
                target.clone(),
            ))
        }
        (
            LoweredType::RustOwned { java_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(java_shape, slot, facts),
        (
            LoweredType::OptionalRustOwned { .. },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if slot_or_optional_element_targets_erased_object(target) => {
            SlotConversionPlan::OptionalRustOwnedFacadeReturn(RustOwnedFacadeReturnPolicy::new(
                target.clone(),
            ))
        }
        (
            LoweredType::OptionalRustOwned { option_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(option_shape, slot, facts),
        (
            LoweredType::RustUnsignedByteArray {
                java_shape,
                optional,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if target_expects_java_byte_array_or_option(target) => {
            SlotConversionPlan::RustUnsignedByteArrayReturn(RustUnsignedByteArrayReturnPolicy::new(
                *optional,
            ))
        }
        (
            LoweredType::RustUnsignedByteArray { java_shape, .. },
            Slot::Assignment(_) | Slot::Return(_) | Slot::Argument(_),
        ) => slot_conversion_plan_with_facts(java_shape, slot, facts),
        (
            LoweredType::OptionalElementBorrow {
                option_shape,
                element_shape,
                ..
            },
            Slot::StringConcatSegment | Slot::DisplaySegment,
        ) => {
            let policy =
                if let Some(kind) = nullable_native_collection_display_kind(option_shape, facts) {
                    SyntaxSlotCoercionPolicy::OptionalCollectionToJavaDisplayString { kind }
                } else if optional_borrowed_element_is_nullable_string(&element_shape.java_ty) {
                    SyntaxSlotCoercionPolicy::OptionalBorrowedOptionalStringToJavaDisplayString
                } else if optional_display_value_uses_java_helper(&element_shape.java_ty, facts) {
                    SyntaxSlotCoercionPolicy::OptionalBorrowedToJavaDisplayValue
                } else {
                    SyntaxSlotCoercionPolicy::OptionalBorrowedToDisplayValue
                };
            SlotConversionPlan::Syntax(SyntaxCoercionPlan::new(
                option_shape.java_ty.clone(),
                slot.clone(),
                policy,
            ))
        }
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
                    RustBorrowedFacadeReturnPolicy::with_source_erased_object(
                        target.clone(),
                        recovery.unwrap_or(RustBorrowedFacadeReturnRecovery::Clone),
                        slot_or_optional_element_targets_erased_object(element_shape),
                    ),
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
            BorrowedElementCarrier::Set {
                semantic_equality: target.view == ViewShape::JavaSemanticHash,
                kind: crate::stdlib::java_collection_kind_for_type(&target.java_ty)
                    .filter(|kind| kind.is_set_like())
                    .or_else(|| crate::stdlib::java_collection_kind_for_type(&set_shape.java_ty))
                    .unwrap_or(crate::stdlib::JavaCollectionKind::Set),
            },
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

fn target_expects_java_byte_array_or_option(target: &RustSlotShape) -> bool {
    java_type_is_byte_array(&target.java_ty)
        || target
            .optional_element_type()
            .is_some_and(|element_ty| java_type_is_byte_array(&element_ty))
}

fn erased_object_clone_recovery_sequence(
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> Option<(bool, bool)> {
    if !matches!(
        (actual.ownership, target.ownership),
        (OwnershipShape::SharedBorrow, OwnershipShape::Owned)
    ) {
        return None;
    }
    if java_type_is_erased_object_sequence_like(&actual.java_ty) {
        return Some((
            true,
            target_erased_object_sequence_elements_are_optional(target),
        ));
    }
    if java_type_is_erased_object_like(&actual.java_ty) {
        return Some((false, false));
    }
    None
}

fn target_erased_object_sequence_elements_are_optional(target: &RustSlotShape) -> bool {
    matches!(
        &target.java_ty,
        JavaType::Array(element)
            if !matches!(element.as_ref(), JavaType::Primitive(_) | JavaType::Void)
                && crate::types::optional_element_type(element).is_none()
    )
}

fn java_type_is_erased_object_display_like(java_ty: &JavaType) -> bool {
    java_type_is_erased_object_like(java_ty) || java_type_is_erased_object_sequence_like(java_ty)
}

fn optional_element_is_erased_object_display_like(
    option_shape: &RustSlotShape,
    element_ty: JavaType,
) -> bool {
    java_type_is_erased_object_display_like(&element_ty)
        || matches!(
            optional_element_slot_shape(option_shape, element_ty).dispatch,
            DispatchShape::ErasedObject
        )
}

fn java_type_needs_java_object_display_fallback(
    java_ty: &JavaType,
    content: &CoercionContentPolicy,
) -> bool {
    if content.string_content() {
        return false;
    }
    match java_ty {
        JavaType::Named { .. } => false,
        JavaType::Unknown(name) => {
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
        | JavaType::Intersection(_)
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
    if matches!(target.dispatch, DispatchShape::ErasedObject)
        || java_type_is_erased_object_like(&target.java_ty)
    {
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
    #[test]
    fn selected_trait_dispatch_uses_payload_arguments_through_nullable_carriers() {
        use super::*;
        let pair = crate::parser::parse_type("Pair<String, Integer>");
        let nullable = crate::types::optional_java_type(pair);
        for (source_dispatch, target_dispatch, source_owned, target_owned) in [
            (
                DispatchShape::Concrete,
                DispatchShape::DynTrait {
                    trait_name: "Pair".into(),
                },
                false,
                false,
            ),
            (
                DispatchShape::Concrete,
                DispatchShape::BoxedDynTrait {
                    trait_name: "Pair".into(),
                },
                true,
                true,
            ),
            (
                DispatchShape::BoxedDynTrait {
                    trait_name: "Pair".into(),
                },
                DispatchShape::DynTrait {
                    trait_name: "Pair".into(),
                },
                false,
                false,
            ),
            (
                DispatchShape::DynTrait {
                    trait_name: "Pair".into(),
                },
                DispatchShape::BoxedDynTrait {
                    trait_name: "Pair".into(),
                },
                false,
                true,
            ),
        ] {
            let mut source =
                RustSlotShape::for_storage(if source_dispatch == DispatchShape::Concrete {
                    crate::parser::parse_type("G")
                } else {
                    nullable.clone()
                });
            source.dispatch = source_dispatch;
            source.ownership = if source_owned {
                OwnershipShape::Owned
            } else {
                OwnershipShape::SharedBorrow
            };
            let mut target = RustSlotShape::for_storage(nullable.clone());
            target.dispatch = target_dispatch;
            target.ownership = if target_owned {
                OwnershipShape::Owned
            } else {
                OwnershipShape::SharedBorrow
            };
            let policy = dispatch_coercion_policy_with_facts(&source, &target, &FactSet::default());
            let path = match policy {
                DispatchCoercionPolicy::BorrowDynTrait { trait_name, .. }
                | DispatchCoercionPolicy::BorrowBoxedDynTrait { trait_name, .. }
                | DispatchCoercionPolicy::BoxDynTrait { trait_name }
                | DispatchCoercionPolicy::CloneBoxDynTrait { trait_name } => trait_name,
                other => panic!("unexpected policy: {other:?}"),
            };
            assert_eq!(path, "Pair<String, i32>");
        }
    }

    #[test]
    fn selected_trait_dispatch_does_not_strip_a_real_optional_trait_constructor() {
        use super::*;
        assert_eq!(
            dyn_trait_rust_abi_path(
                "Optional",
                &crate::parser::parse_hierarchy_type("java.util.Optional<String>"),
                &FactSet::default()
            ),
            "Optional<String>"
        );
        let payload = crate::parser::parse_hierarchy_type("left.Optional<String>");
        let nullable = crate::types::optional_java_type(payload.clone());
        assert_eq!(
            dyn_trait_rust_abi_path("left.Optional", &nullable, &FactSet::default()),
            crate::slot_render::parameterized_dyn_trait_path_with_facts(
                "left.Optional",
                &payload,
                &FactSet::default()
            ),
        );
    }

    #[test]
    fn shared_source_erasure_copies_the_reference_even_between_owned_slots() {
        use super::*;
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Payload".into(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(true),
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });
        let actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Payload".into(),
            args: Vec::new(),
        });
        let mut target = RustSlotShape::for_storage(JavaType::Named {
            name: "Object".into(),
            args: Vec::new(),
        });
        assert_eq!(
            ownership_coercion_policy(&actual, &target, &facts),
            OwnershipCoercionPolicy::NoOwnershipChange
        );
        facts.storage_strategies.insert(
            "Payload".into(),
            crate::storage_strategy::TypeStorageFacts::default()
                .with_locally_aliased(true)
                .with_mutated(true),
        );
        for dispatch in [DispatchShape::Concrete, DispatchShape::ErasedObject] {
            target.dispatch = dispatch;
            let plan = shape_coercion_plan_with_facts(&actual, &target, &facts);
            assert!(matches!(
                plan.ownership(),
                OwnershipCoercionPolicy::RecoverOwnedWithClone {
                    policy: CloneRecoveryPolicy::ProvenCloneCapability { .. }
                }
            ));
            if matches!(target.dispatch, DispatchShape::ErasedObject) {
                assert!(matches!(
                    plan.dispatch(),
                    DispatchCoercionPolicy::EraseObject
                ));
                assert!(matches!(
                    plan.coercions().as_slice(),
                    [
                        crate::coercion::Coercion::CloneRecovery { .. },
                        crate::coercion::Coercion::EraseObject
                    ]
                ));
            }
        }
    }

    #[test]
    fn interface_destination_generic_markers_require_a_physical_outer_carrier() {
        let facts = FactSet::default();
        for ty in [
            JavaType::String,
            crate::parser::parse_type("Payload"),
            crate::parser::parse_type("T"),
        ] {
            let actual = RustSlotShape::for_storage(ty);
            let mut target = actual.clone();
            target.native_allocation =
                crate::storage_strategy::StoragePlacement::new(true, true, true);
            let plan = shape_coercion_plan_with_facts(&actual, &target, &facts);
            assert_eq!(
                plan.native_allocation(),
                &NativeAllocationCoercionPolicy::Preserve
            );
        }
        // An instantiated array retains its qualifier when the other side
        // is still spelled as the source generic binder.
        let mut array = RustSlotShape::for_storage(crate::parser::parse_type("int[]"));
        array.native_allocation = crate::storage_strategy::StoragePlacement::new(true, true, true);
        let mut symbolic = RustSlotShape::for_storage(crate::parser::parse_type("T"));
        symbolic.native_allocation = array.native_allocation;
        for (actual, target) in [(&array, &symbolic), (&symbolic, &array)] {
            assert_eq!(
                shape_coercion_plan_with_facts(actual, target, &facts).native_allocation(),
                &NativeAllocationCoercionPolicy::Preserve
            );
        }
        symbolic.native_allocation = Default::default();
        assert!(matches!(
            shape_coercion_plan_with_facts(&array, &symbolic, &facts).native_allocation(),
            NativeAllocationCoercionPolicy::UnsupportedPlacementChange { .. }
        ));
        // A real mutable array allocation cannot silently change its carrier.
        let actual = RustSlotShape::for_storage(crate::parser::parse_type("int[]"));
        let mut target = actual.clone();
        target.native_allocation = crate::storage_strategy::StoragePlacement::new(true, true, true);
        assert!(matches!(
            shape_coercion_plan_with_facts(&actual, &target, &facts).native_allocation(),
            NativeAllocationCoercionPolicy::UnsupportedPlacementChange { .. }
        ));
    }

    #[test]
    fn native_sam_allocation_requirements_do_not_add_an_outer_storage_handle() {
        use super::*;
        let actual = RustSlotShape::for_storage(JavaType::Named {
            name: "java.lang.Iterable".into(),
            args: vec![JavaType::String],
        });
        let mut target = actual.clone();
        target.native_allocation =
            crate::storage_strategy::StoragePlacement::new(true, true, false);
        let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
        assert_eq!(
            plan.native_allocation(),
            &NativeAllocationCoercionPolicy::Preserve
        );
        let mut optional_actual = actual;
        optional_actual.java_ty = crate::types::optional_java_type(optional_actual.java_ty);
        let mut optional_target = target;
        optional_target.java_ty = crate::types::optional_java_type(optional_target.java_ty);
        let optional_plan =
            shape_coercion_plan_with_facts(&optional_actual, &optional_target, &FactSet::default());
        assert_eq!(
            optional_plan.native_allocation(),
            &NativeAllocationCoercionPolicy::Preserve
        );
    }

    #[test]
    fn erased_iterable_type_test_requires_a_platform_reifiable_target() {
        use super::*;
        let mut actual = RustSlotShape::for_storage(crate::parser::parse_type("Object"));
        actual.dispatch = DispatchShape::ErasedObject;
        let target = RustSlotShape::for_storage(crate::parser::parse_type("Iterable<?>"));
        assert!(matches!(
            plan_instanceof_check_with_facts(&actual, &target, &FactSet::default()),
            InstanceOfCheck::ErasedIterable { .. }
        ));
        let parsed = crate::parser::parse_java_source("class Iterable<T> {}");
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(!matches!(
            plan_instanceof_check_with_facts(&actual, &target, &resolved.facts),
            InstanceOfCheck::ErasedIterable { .. }
        ));
        let bounded =
            RustSlotShape::for_storage(crate::parser::parse_type("Iterable<? extends String>"));
        assert!(!matches!(
            plan_instanceof_check_with_facts(&actual, &bounded, &FactSet::default()),
            InstanceOfCheck::ErasedIterable { .. }
        ));
    }

    #[test]
    fn native_array_erasure_preserves_handle_but_array_storage_changes_are_rejected() {
        use super::*;
        let mut actual = RustSlotShape::for_storage(crate::parser::parse_type("int[]"));
        actual.native_allocation =
            crate::storage_strategy::StoragePlacement::new(true, true, false);
        let mut target = RustSlotShape::for_storage(crate::parser::parse_type("Object"));
        target.dispatch = DispatchShape::ErasedObject;
        let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
        assert_eq!(plan.dispatch(), &DispatchCoercionPolicy::EraseObject);
        assert_eq!(
            plan.native_allocation(),
            &NativeAllocationCoercionPolicy::Preserve
        );
        let target = RustSlotShape::for_storage(crate::parser::parse_type("int[]"));
        let plan = shape_coercion_plan_with_facts(&actual, &target, &FactSet::default());
        assert!(matches!(
            plan.native_allocation(),
            NativeAllocationCoercionPolicy::UnsupportedPlacementChange { .. }
        ));
    }

    #[test]
    fn erased_object_array_adapter_requires_native_component_provenance() {
        let mut actual = RustSlotShape::for_storage(crate::parser::parse_type("Object[]"));
        actual.native_allocation =
            crate::storage_strategy::StoragePlacement::new(true, true, false);
        let mut target = RustSlotShape::for_storage(crate::parser::parse_type("java.lang.Object"));
        target.dispatch = DispatchShape::ErasedObject;
        let mut facts = FactSet::default();
        assert_eq!(
            shape_coercion_plan_with_facts(&actual, &target, &facts).erased_array_adapter(),
            ErasedArrayAdapter::ErasedObject
        );
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Object".into(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(true),
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });
        assert_eq!(
            shape_coercion_plan_with_facts(&actual, &target, &facts).erased_array_adapter(),
            ErasedArrayAdapter::IdentityOnly
        );
    }

    use super::*;
    use crate::facts::ReceiverAbi;
    use crate::object_model::ObjectRepresentation;
    use crate::representation::{RepresentationProvenance, TypeRepresentation};
    use crate::shape_projection::borrowed_string_content_shape;

    #[test]
    fn copy_value_argument_can_satisfy_a_shared_borrow_slot() {
        let actual = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int));
        assert_eq!(actual.ownership, OwnershipShape::CopyValue);
        let mut target = actual.clone();
        target.ownership = OwnershipShape::SharedBorrow;
        assert_eq!(
            ownership_coercion_policy(&actual, &target, &FactSet::default()),
            OwnershipCoercionPolicy::BorrowOwnedValue,
        );
    }

    #[test]
    fn same_closed_carrier_with_different_generic_details_keeps_dispatch() {
        let mut actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Handler".to_string(),
            args: vec![JavaType::Named {
                name: "Concrete".to_string(),
                args: Vec::new(),
            }],
        });
        actual.dispatch = DispatchShape::ClosedEnum {
            enum_name: "HandlerValue".to_string(),
        };
        let mut target = RustSlotShape::for_storage(JavaType::Named {
            name: "Handler".to_string(),
            args: vec![JavaType::Wildcard(None)],
        });
        target.dispatch = DispatchShape::ClosedEnum {
            enum_name: "HandlerValue".to_string(),
        };

        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::NoDispatchChange
        ));
    }

    #[test]
    fn optional_present_preserves_exact_closed_carrier_element_shape() {
        let model_ty = JavaType::Named {
            name: "Model".to_string(),
            args: vec![JavaType::Named {
                name: "O".to_string(),
                args: Vec::new(),
            }],
        };
        let mut actual = RustSlotShape::for_storage(model_ty.clone());
        actual.dispatch = DispatchShape::ClosedEnum {
            enum_name: "ModelValue".to_string(),
        };
        let target = RustSlotShape::for_storage(crate::types::optional_java_type(model_ty.clone()));
        let mut representations = RepresentationPlan::default();
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Model",
            ObjectRepresentation::SourceHierarchyCarrier {
                carrier_name: "ModelValue".to_string(),
                variants: vec!["Model".to_string(), "SpecializedModel".to_string()],
            },
            RepresentationProvenance::ParsedSource,
            vec!["SpecializedModel".to_string()],
        ));

        let plan = optional_present_construction_plan_for_actual_with_representations(
            &actual,
            &target,
            model_ty,
            OptionalPresentConstructionSource::Plain,
            &FactSet::default(),
            &representations,
        );

        assert_eq!(plan.construction().element_shape(), &actual);
        assert_eq!(
            plan.element_lowering_slot(),
            &Slot::Assignment(actual.clone())
        );
        assert!(matches!(
            plan.construction().element_plan().dispatch(),
            DispatchCoercionPolicy::NoDispatchChange
        ));
    }

    #[test]
    fn owned_array2_borrows_into_nullable_matrix_argument_element() {
        let matrix_ty = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double,
        )))));
        let actual = RustSlotShape::for_storage(matrix_ty.clone());
        let mut optional_target =
            RustSlotShape::conservative(crate::types::optional_java_type(matrix_ty.clone()));
        optional_target.ownership = OwnershipShape::SharedBorrow;
        optional_target.view = ViewShape::Slice;
        let target_element = optional_element_slot_shape_with_facts(
            &optional_target,
            matrix_ty,
            &FactSet::default(),
        );

        assert_eq!(
            ownership_coercion_policy(&actual, &target_element, &FactSet::default()),
            OwnershipCoercionPolicy::BorrowOwnedValue
        );
        let OptionalShapeCoercionPolicy::Wrap { element_plan, .. } =
            optional_shape_coercion_policy_with_facts(
                &actual,
                &optional_target,
                &FactSet::default(),
            )
        else {
            panic!("owned matrix should wrap into the nullable argument carrier");
        };
        assert_eq!(
            element_plan.ownership(),
            &OwnershipCoercionPolicy::BorrowOwnedValue
        );
    }

    #[test]
    fn optional_interface_payload_preserves_boundary_ownership() {
        let ty = JavaType::Named {
            name: "Readable".to_string(),
            args: Vec::new(),
        };
        for ownership in [
            OwnershipShape::Owned,
            OwnershipShape::SharedBorrow,
            OwnershipShape::MutableBorrow,
        ] {
            let mut option =
                RustSlotShape::for_storage(crate::types::optional_java_type(ty.clone()));
            option.ownership = ownership;
            option.view = ViewShape::DynTraitView;
            option.dispatch = DispatchShape::BoxedDynTrait {
                trait_name: "Readable".to_string(),
            };
            let payload =
                optional_element_slot_shape_with_facts(&option, ty.clone(), &FactSet::default());
            assert_eq!(payload.ownership, ownership);
            assert_eq!(
                payload.dispatch,
                if ownership == OwnershipShape::Owned {
                    DispatchShape::BoxedDynTrait {
                        trait_name: "Readable".to_string(),
                    }
                } else {
                    DispatchShape::DynTrait {
                        trait_name: "Readable".to_string(),
                    }
                }
            );
        }
    }

    #[test]
    fn borrowed_element_adapter_policy_shares_recovery_across_option_and_vec_carriers() {
        let mut string_view = RustSlotShape::conservative(JavaType::String);
        string_view.ownership = OwnershipShape::SharedBorrow;
        string_view.view = ViewShape::Str;
        let int_element = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
        let external_value = JavaType::Named {
            name: "ExternalValue".to_string(),
            args: Vec::new(),
        };
        let external_element = RustSlotShape::conservative(external_value);
        let facts = FactSet::default();

        let option_string_policy = borrowed_element_adapter_policy_with_unwrap(
            BorrowedElementCarrier::Option,
            &string_view,
            &facts,
            false,
        );
        assert_eq!(
            option_string_policy.carrier(),
            BorrowedElementCarrier::Option
        );
        assert_eq!(option_string_policy.element_shape(), &string_view);
        assert_eq!(
            option_string_policy.recovery(),
            &BorrowedElementRecoveryPolicy::StringViewToOwned
        );
        assert!(!option_string_policy.unwrap_carrier());

        let vec_string_policy = borrowed_element_adapter_policy_with_unwrap(
            BorrowedElementCarrier::Vec,
            &string_view,
            &facts,
            false,
        );
        assert_eq!(vec_string_policy.carrier(), BorrowedElementCarrier::Vec);
        assert_eq!(vec_string_policy.element_shape(), &string_view);
        assert_eq!(
            vec_string_policy.recovery(),
            &BorrowedElementRecoveryPolicy::StringViewToOwned
        );
        assert!(!vec_string_policy.unwrap_carrier());
        assert_eq!(
            borrowed_element_adapter_policy_with_unwrap(
                BorrowedElementCarrier::Option,
                &int_element,
                &facts,
                false,
            )
            .recovery(),
            &BorrowedElementRecoveryPolicy::CopyElements
        );
        assert_eq!(
            borrowed_element_adapter_policy_with_unwrap(
                BorrowedElementCarrier::Vec,
                &external_element,
                &facts,
                false,
            )
            .recovery(),
            &BorrowedElementRecoveryPolicy::CloneElements {
                policy: CloneRecoveryPolicy::AuditRequired,
            }
        );
    }

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
        let optional_receiver_plan = LoweredSlotExprConversionPlan::new(
            LoweredType::Slot(Box::new(borrowed_string.clone())),
            argument_slot(borrowed_string.clone()),
            SlotConversionPlan::Shape(shape_coercion_plan_with_facts(
                &borrowed_string,
                &borrowed_string,
                &FactSet::default(),
            )),
        );
        assert!(receiver_method_autoref_satisfies_optional_slot(
            &optional_receiver_plan
        ));
        assert!(!receiver_method_autoref_satisfies_facade_slot(
            ReceiverAbi::BorrowedSelf,
            &optional_receiver_plan
        ));
    }

    #[test]
    fn optional_borrowed_string_element_to_string_view_uses_borrow_projection() {
        let facts = FactSet::default();
        let carrier_shape =
            RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::String));
        let mut element_shape = borrowed_string_content_shape();
        element_shape.ownership = OwnershipShape::SharedBorrow;
        let mut target = borrowed_string_content_shape();
        target.ownership = OwnershipShape::Owned;

        let policy = borrowed_element_slot_adapter_policy(
            BorrowedElementCarrier::Option,
            &target,
            &carrier_shape,
            &element_shape,
            &facts,
        );

        assert_eq!(policy.target().ownership, OwnershipShape::SharedBorrow);
        assert!(matches!(
            policy.adapter,
            Some(BorrowedElementAdapterPolicy {
                recovery: BorrowedElementRecoveryPolicy::BorrowElement,
                unwrap_carrier: true,
                ..
            })
        ));
    }

    #[test]
    fn nullable_collection_display_policy_preserves_native_kind_and_source_precedence() {
        let facts = FactSet::default();
        for (source, kind) in [
            ("List<String>", crate::stdlib::JavaCollectionKind::List),
            (
                "LinkedHashSet<String>",
                crate::stdlib::JavaCollectionKind::InsertionOrderedSet,
            ),
            (
                "LinkedHashMap<String, Integer>",
                crate::stdlib::JavaCollectionKind::InsertionOrderedMap,
            ),
        ] {
            let mut shape = RustSlotShape::for_storage(crate::types::optional_java_type(
                crate::parser::parse_hierarchy_type(source),
            ));
            for ownership in [
                OwnershipShape::Owned,
                OwnershipShape::SharedBorrow,
                OwnershipShape::MutableBorrow,
            ] {
                shape.ownership = ownership;
                for slot in [Slot::StringConcatSegment, Slot::DisplaySegment] {
                    let plan = slot_conversion_plan_with_facts(&shape, &slot, &facts);
                    assert!(
                        matches!(plan, SlotConversionPlan::Syntax(ref syntax) if syntax.policy() == &SyntaxSlotCoercionPolicy::OptionalCollectionToJavaDisplayString { kind }),
                        "{source}"
                    );
                }
            }
        }
        let shape = RustSlotShape::for_storage(crate::types::optional_java_type(
            crate::parser::parse_hierarchy_type("List<String>"),
        ));
        let mut source = facts.clone();
        source
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "List".into(),
                copy: None,
                clone: Some(true),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(true),
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });
        assert!(nullable_native_collection_display_kind(&shape, &source).is_none());
    }

    #[test]
    fn list_string_concat_uses_java_collection_display_policy() {
        let actual = RustSlotShape::for_storage(JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        });
        let plan = plan_slot_conversion(actual, Slot::StringConcatSegment, &FactSet::default());

        let SlotConversionPlan::Syntax(syntax) = plan.conversion() else {
            panic!("expected syntax coercion plan");
        };
        assert_eq!(
            syntax.policy(),
            &SyntaxSlotCoercionPolicy::CollectionToJavaDisplayString
        );
    }

    #[test]
    fn map_string_concat_uses_java_map_display_policy() {
        let actual = RustSlotShape::for_storage(JavaType::Named {
            name: "TreeMap".to_string(),
            args: vec![JavaType::String, JavaType::Primitive(PrimitiveType::Int)],
        });
        let plan = plan_slot_conversion(actual, Slot::StringConcatSegment, &FactSet::default());

        let SlotConversionPlan::Syntax(syntax) = plan.conversion() else {
            panic!("expected syntax coercion plan");
        };
        assert_eq!(
            syntax.policy(),
            &SyntaxSlotCoercionPolicy::MapToJavaDisplayString
        );
    }

    #[test]
    fn set_string_concat_uses_java_set_display_policy() {
        let actual = RustSlotShape::for_storage(JavaType::Named {
            name: "TreeSet".to_string(),
            args: vec![JavaType::String],
        });
        let plan = plan_slot_conversion(actual, Slot::StringConcatSegment, &FactSet::default());

        let SlotConversionPlan::Syntax(syntax) = plan.conversion() else {
            panic!("expected syntax coercion plan");
        };
        assert_eq!(
            syntax.policy(),
            &SyntaxSlotCoercionPolicy::SetToJavaDisplayString
        );
    }

    #[test]
    fn optional_erased_concrete_facade_string_concat_uses_erased_display_policy() {
        let mut actual = RustSlotShape::for_storage(optional_java_type(JavaType::Named {
            name: "VCFHeader".to_string(),
            args: Vec::new(),
        }));
        actual.dispatch = DispatchShape::ErasedObject;

        let plan = plan_slot_conversion(actual, Slot::StringConcatSegment, &FactSet::default());

        let SlotConversionPlan::Syntax(syntax) = plan.conversion() else {
            panic!("expected syntax coercion plan");
        };
        assert_eq!(
            syntax.policy(),
            &SyntaxSlotCoercionPolicy::OptionalErasedObjectToJavaDisplayString
        );
    }

    #[test]
    fn borrowed_ndarray_backed_array2_recovers_with_clone_not_to_vec() {
        let java_ty = JavaType::Array(Box::new(JavaType::Array(Box::new(JavaType::Primitive(
            PrimitiveType::Double,
        )))));
        let mut target = RustSlotShape::for_storage(java_ty);
        target.ownership = OwnershipShape::Owned;
        let mut actual = target.clone();
        actual.ownership = OwnershipShape::SharedBorrow;

        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);
        let policy =
            ownership_coercion_policy_with_effective_facts(&actual, &target, &effective_facts);

        assert!(
            matches!(
                policy,
                OwnershipCoercionPolicy::RecoverOwnedWithClone { .. }
            ),
            "shared borrowed Array2 recovered with {policy:?}"
        );
    }

    #[test]
    fn mutable_borrowed_cloneable_reference_recovers_owned_alias_with_clone() {
        let java_ty = JavaType::Named {
            name: "StatefulFacade".to_string(),
            args: Vec::new(),
        };
        let mut actual = RustSlotShape::for_storage(java_ty.clone());
        actual.ownership = OwnershipShape::MutableBorrow;
        let target = RustSlotShape::for_storage(java_ty);
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "StatefulFacade".to_string(),
                copy: None,
                clone: Some(true),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(true),
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });
        let effective_facts = EffectiveFacts::new(&facts);

        assert!(matches!(
            ownership_coercion_policy_with_effective_facts(&actual, &target, &effective_facts),
            OwnershipCoercionPolicy::RecoverOwnedWithClone { .. }
        ));
    }

    #[test]
    fn borrowed_hierarchy_carrier_downcast_recovers_owned_variant_payload() {
        let mut actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Base".to_string(),
            args: Vec::new(),
        });
        actual.dispatch = DispatchShape::ClosedEnum {
            enum_name: "BaseValue".to_string(),
        };
        actual.ownership = OwnershipShape::SharedBorrow;
        actual.view = ViewShape::EnumView;
        let target = RustSlotShape::for_storage(JavaType::Named {
            name: "Child".to_string(),
            args: Vec::new(),
        });

        let policy = ownership_coercion_policy(&actual, &target, &FactSet::default());

        assert!(matches!(
            policy,
            OwnershipCoercionPolicy::RecoverOwnedWithClone { .. }
        ));
    }

    #[test]
    fn source_boxed_upcasts_require_the_generated_instantiated_ancestor_adapter() {
        let parsed = crate::parser::parse_java_source(
            "interface Parent<T> { T item(); } interface Child<T> extends Parent<T> {} interface Middle<X> extends Child<X> {}",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let named = |name: &str, args: Vec<JavaType>| JavaType::Named {
            name: name.to_string(),
            args,
        };
        let mut actual = RustSlotShape::for_storage(named("Middle", vec![JavaType::String]));
        actual.dispatch = DispatchShape::DynTrait {
            trait_name: "Middle".to_string(),
        };
        actual.ownership = OwnershipShape::SharedBorrow;
        let mut target = RustSlotShape::for_storage(named("Parent", vec![JavaType::String]));
        target.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Parent".to_string(),
        };
        let expected = DispatchCoercionPolicy::UpcastBoxedDynTrait {
            source_trait: "Middle<JavaString>".to_string(),
            target_trait: "Parent<JavaString>".to_string(),
            clone_source: true,
        };
        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &semantic.facts),
            expected
        );
        actual.ownership = OwnershipShape::Owned;
        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Middle".to_string(),
        };
        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &semantic.facts),
            DispatchCoercionPolicy::UpcastBoxedDynTrait {
                clone_source: false,
                ..
            }
        ));
        actual.dispatch = DispatchShape::Concrete;
        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &semantic.facts),
            DispatchCoercionPolicy::UpcastBoxedDynTrait {
                clone_source: false,
                ..
            }
        ));
        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Middle".to_string(),
        };
        target.java_ty = named("Parent", vec![named("Integer", vec![])]);
        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &semantic.facts),
            DispatchCoercionPolicy::Unsupported { .. }
        ));
        target.java_ty = named("Parent", vec![JavaType::String]);
        let mut no_adapter = semantic.facts.clone();
        no_adapter.source_interface_upcast_facts.clear();
        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &no_adapter),
            DispatchCoercionPolicy::Unsupported { .. }
        ));
        actual.java_ty = named("foreign.Middle", vec![JavaType::String]);
        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "ForeignMiddle".to_string(),
        };
        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &semantic.facts),
            DispatchCoercionPolicy::Unsupported { .. }
        ));
    }

    #[test]
    fn borrowed_dyn_trait_to_owned_storage_uses_clone_box_adapter() {
        let java_ty = JavaType::Named {
            name: "Product".to_string(),
            args: Vec::new(),
        };
        let mut actual = RustSlotShape::for_storage(java_ty.clone());
        actual.dispatch = DispatchShape::DynTrait {
            trait_name: "Product".to_string(),
        };
        actual.view = ViewShape::DynTraitView;
        actual.ownership = OwnershipShape::SharedBorrow;
        let mut target = actual.clone();
        target.ownership = OwnershipShape::Owned;

        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::CloneBoxDynTrait {
                trait_name: "Product".to_string(),
            }
        );

        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Product".to_string(),
        };
        target.dispatch = actual.dispatch.clone();
        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::CloneBoxDynTrait {
                trait_name: "Product".to_string(),
            }
        );
    }

    #[test]
    fn conservatively_tagged_borrowed_generic_interface_clones_to_owned_trait_storage() {
        let java_ty = JavaType::Named {
            name: "Product".to_string(),
            args: vec![JavaType::Named {
                name: "T".to_string(),
                args: Vec::new(),
            }],
        };
        let mut actual = RustSlotShape::for_storage(java_ty.clone());
        actual.ownership = OwnershipShape::SharedBorrow;
        actual.view = ViewShape::DynTraitView;
        let mut target = RustSlotShape::for_storage(java_ty);
        target.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Product".to_string(),
        };
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Product".to_string(),
                copy: None,
                clone: Some(true),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(false),
                object_safe_trait: Some(true),
                provenance: crate::facts::FactProvenance::ParsedSource,
            });

        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &facts),
            DispatchCoercionPolicy::CloneBoxDynTrait {
                trait_name: "Product<T>".to_string(),
            }
        );
    }

    #[test]
    fn stale_concrete_same_interface_target_borrows_existing_trait_carrier() {
        let actual_ty = JavaType::Named {
            name: "Opdf".to_string(),
            args: vec![JavaType::Named {
                name: "O".to_string(),
                args: Vec::new(),
            }],
        };
        let mut actual = RustSlotShape::for_storage(actual_ty);
        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Opdf".to_string(),
        };
        let mut target = RustSlotShape::for_storage(JavaType::Named {
            name: "Opdf".to_string(),
            args: Vec::new(),
        });
        target.ownership = OwnershipShape::SharedBorrow;
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Opdf".to_string(),
                copy: None,
                clone: Some(true),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(false),
                object_safe_trait: Some(true),
                provenance: crate::facts::FactProvenance::ParsedSource,
            });

        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &facts),
            DispatchCoercionPolicy::BorrowBoxedDynTrait {
                trait_name: "Opdf<O>".to_string(),
                mutable: false,
            }
        );

        actual.dispatch = DispatchShape::DynTrait {
            trait_name: "Opdf".to_string(),
        };
        actual.ownership = OwnershipShape::SharedBorrow;
        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &facts),
            DispatchCoercionPolicy::NoDispatchChange
        );
    }

    #[test]
    fn borrowed_boxed_trait_wildcard_with_same_rust_abi_uses_clone_adapter() {
        let type_param = JavaType::Named {
            name: "L".to_string(),
            args: Vec::new(),
        };
        let mut actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Centroid".to_string(),
            args: vec![type_param.clone()],
        });
        actual.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "Centroid".to_string(),
        };
        actual.ownership = OwnershipShape::SharedBorrow;
        let target_element_ty = JavaType::Named {
            name: "Centroid".to_string(),
            args: vec![JavaType::Wildcard(Some(
                crate::java_ir::WildcardBound::Super(Box::new(type_param)),
            ))],
        };
        let mut target =
            RustSlotShape::for_storage(crate::types::optional_java_type(target_element_ty));
        target.dispatch = actual.dispatch.clone();
        target.ownership = OwnershipShape::Owned;
        let mut facts = FactSet::default();
        facts
            .generic_parameter_variance_facts
            .push(crate::facts::GenericParameterVarianceFact {
                type_name: "Centroid".to_string(),
                parameter_index: 0,
                consumed: true,
                produced: false,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });

        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &facts),
            DispatchCoercionPolicy::CloneBoxDynTrait {
                trait_name: "Centroid<L>".to_string(),
            }
        );
    }

    #[test]
    fn boxed_trait_with_different_rust_type_arguments_stays_unsupported() {
        let boxed_opdf = |arg: &str| {
            let mut shape = RustSlotShape::for_storage(JavaType::Named {
                name: "Opdf".to_string(),
                args: vec![JavaType::Named {
                    name: arg.to_string(),
                    args: Vec::new(),
                }],
            });
            shape.dispatch = DispatchShape::BoxedDynTrait {
                trait_name: "Opdf".to_string(),
            };
            shape
        };
        let actual = boxed_opdf("T");
        let target = boxed_opdf("O");

        assert!(matches!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::Unsupported { .. }
        ));
    }

    #[test]
    fn closed_enum_erases_at_object_boundary() {
        let mut actual = RustSlotShape::conservative(JavaType::Named {
            name: "BlockOutputStream".to_string(),
            args: Vec::new(),
        });
        actual.dispatch = DispatchShape::ClosedEnum {
            enum_name: "BlockOutputStreamValue".to_string(),
        };
        let mut target = RustSlotShape::conservative(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        });
        target.dispatch = DispatchShape::ErasedObject;

        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::EraseObject
        );
    }

    #[test]
    fn bounded_wildcard_and_bound_share_the_same_generic_carrier_abi() {
        let type_param = JavaType::Named {
            name: "O".to_string(),
            args: Vec::new(),
        };
        let mut actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Hmm".to_string(),
            args: vec![type_param.clone()],
        });
        actual.dispatch = DispatchShape::ClosedEnum {
            enum_name: "HmmValue".to_string(),
        };
        let mut target = RustSlotShape::for_storage(JavaType::Named {
            name: "Hmm".to_string(),
            args: vec![JavaType::Wildcard(Some(
                crate::java_ir::WildcardBound::Super(Box::new(type_param)),
            ))],
        });
        target.dispatch = actual.dispatch.clone();

        assert_eq!(
            crate::slot_render::parameterized_closed_enum_type_name("HmmValue", &actual.java_ty,),
            "HmmValue<O>"
        );
        assert_eq!(
            crate::slot_render::parameterized_closed_enum_type_name("HmmValue", &target.java_ty,),
            "HmmValue<O>"
        );
        assert_eq!(
            dispatch_coercion_policy_with_facts(&actual, &target, &FactSet::default()),
            DispatchCoercionPolicy::NoDispatchChange
        );
    }

    #[test]
    fn erased_generic_closed_hierarchy_downcast_keeps_args_and_shared_payload_handles() {
        let mut target = RustSlotShape::for_storage(JavaType::Named {
            name: "Hmm".to_string(),
            args: vec![JavaType::Wildcard(None)],
        });
        target.dispatch = DispatchShape::ClosedEnum {
            enum_name: "HmmValue".to_string(),
        };
        let mut actual = RustSlotShape::for_storage(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        });
        actual.dispatch = DispatchShape::ErasedObject;

        let mut facts = FactSet::default();
        for name in ["Hmm", "HmmLogSpace"] {
            facts.storage_strategies.insert(
                name.to_string(),
                crate::storage_strategy::TypeStorageFacts::default()
                    .with_locally_aliased(true)
                    .with_mutated(true),
            );
        }
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "HmmLogSpace".to_string(),
                copy: None,
                clone: Some(true),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: Some(JavaType::Named {
                    name: "Hmm".to_string(),
                    args: vec![JavaType::Wildcard(None)],
                }),
                final_type: Some(false),
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::ParsedSource,
            });

        let policy = dispatch_coercion_policy_with_facts(&actual, &target, &facts);
        let coercion = shape_coercion_plan_with_facts(&actual, &target, &facts);
        assert!(coercion.may_throw_class_cast());
        assert_eq!(
            policy.checked_cast_failure_policy(),
            Some(&InvalidCastPolicy::Panic)
        );
        assert!(matches!(
            java_cast_semantics_policy_with_facts(&actual, &target, &coercion, &facts),
            JavaCastSemanticsPolicy::RawOrWildcardCast { .. }
        ));
        let DispatchCoercionPolicy::DowncastErasedObjectToClosedEnum { variants, .. } = policy
        else {
            panic!("expected erased closed-hierarchy downcast");
        };
        assert_eq!(
            variants,
            vec![
                "std::rc::Rc<std::cell::RefCell<Hmm<Box<dyn std::any::Any>>>>",
                "std::rc::Rc<std::cell::RefCell<HmmLogSpace<Box<dyn std::any::Any>>>>",
            ]
        );
    }
}
