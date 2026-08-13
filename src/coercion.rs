use crate::conversion_plan::{
    BorrowedDynToOwnedBoxedDynRecoveryPolicy, BorrowedElementAdapterPolicy, BorrowedElementCarrier,
    BorrowedElementRecoveryFallbackPolicy, BorrowedElementRecoveryPolicy,
    BorrowedElementSlotAdapterPolicy, CloneRecoveryPolicy, DispatchCoercionFallbackPolicy,
    DispatchCoercionPolicy, DynCloneBoxAdapterGenerationPolicy, InstanceOfCheck,
    InstanceOfFallbackPolicy, InvalidCastPolicy, LoweredType, OptionalShapeCoercionPolicy,
    OwnershipCoercionFallbackPolicy, OwnershipCoercionPolicy, RustBorrowedFacadeReturnPolicy,
    RustBorrowedFacadeReturnRecovery, RustScalarSlotCastPolicy, RustUnsignedByteArrayReturnPolicy,
    ShapeCoercionPlan, Slot, SlotConversionPlan, SyntaxCoercionPlan, SyntaxSlotCoercionPolicy,
    ViewCoercionPolicy, borrowed_element_recovery_policy, dyn_clone_box_adapter_deferral_message,
    dyn_clone_box_adapter_generation_policy, dyn_clone_box_adapter_requirement,
};
use crate::emit;
use crate::facts::FactSet;
use crate::java_ir::{JavaType, Literal, PrimitiveType};
use crate::rust_ir::{RustExpr, RustExprShape, RustStmt};
use crate::slot_shape::ViewShape;
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape};

#[derive(Debug, Clone, PartialEq)]
pub struct LoweredExpr {
    pub expr: RustExpr,
    pub actual: LoweredType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowProjectionUse {
    Argument,
    MapKey,
    StringView,
    SliceView,
    MutableSliceView,
    Display,
    MethodReceiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaDisplayPlan {
    ConcatValue,
    /// `double`/`float` concatenation, which Java renders with a decimal point.
    ConcatFloatValue {
        double: bool,
    },
    FormatValue,
    ErasedObject,
    OptionalString,
    OptionalValue,
    OptionalBorrowedValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralSlotPlan {
    BorrowedStringView { value: String },
    NumericCharCode { ch: char },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BorrowedStringViewProjection {
    AlreadyStringView,
    LocalStringValue { rust_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedStringLiteralTemporaryPlan {
    pub replacement_expr: String,
    pub initializer_expr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalSequenceReceiverProjection {
    Shared,
    Mutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceIterationProjection {
    Borrowed,
    OwnedValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLambdaParamProjection {
    FilterBorrowed,
    BorrowedItem,
    OwnedItem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLambdaArgumentSlotPlan {
    BorrowedStringView,
    OwnedValue,
    GenericSlotConversion,
}

pub fn stream_lambda_argument_slot_plan(slot: &RustSlotShape) -> StreamLambdaArgumentSlotPlan {
    if crate::shape_projection::shape_is_borrowed_string_content(slot) {
        StreamLambdaArgumentSlotPlan::BorrowedStringView
    } else if crate::shape_projection::shape_is_owned(slot) {
        StreamLambdaArgumentSlotPlan::OwnedValue
    } else {
        StreamLambdaArgumentSlotPlan::GenericSlotConversion
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalReceiverProjection {
    Shared,
    Mutable,
    Borrowed,
    SharedUnchecked,
    MutableUnchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexToUsizeProjection {
    JavaStringCharAt,
    StreamLambdaArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RustPrimitiveTypeNamePolicy {
    pub primitive: PrimitiveType,
    pub rust_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoercionDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCoercionMessagePolicy {
    pub code: &'static str,
    pub message: String,
}

impl UnsupportedCoercionMessagePolicy {
    pub fn into_diagnostic(self) -> CoercionDiagnostic {
        CoercionDiagnostic {
            code: self.code,
            message: self.message,
        }
    }

    pub fn compile_error_expr(&self) -> RustExpr {
        crate::unsupported::compile_error_expr(&self.message)
    }

    pub fn compile_error_expr_consuming(&self, expr: &RustExpr) -> RustExpr {
        crate::unsupported::compile_error_expr_consuming(expr, &self.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coercion {
    /// Java prints a concatenated `double`/`float` with a decimal point.
    ToJavaFloatStringForConcat {
        double: bool,
    },
    Identity,
    WidenPrimitive {
        from: PrimitiveType,
        to: PrimitiveType,
    },
    IndexToUsize,
    ToStringForConcat,
    ErasedObjectToJavaDisplayString,
    OptionalToJavaDisplayString,
    OptionalToJavaDisplayValue,
    OptionalBorrowedToJavaDisplayValue,
    AsStr,
    ToOwnedString,
    AsSlice,
    AsMutSlice,
    BorrowMut,
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
    UnsupportedDispatch {
        actual_dispatch: DispatchShape,
        target_dispatch: DispatchShape,
        policy: DispatchCoercionFallbackPolicy,
    },
    WrapOption,
    UnwrapOption,
    UnwrapOptionBorrowed,
    UnwrapOptionMutBorrowed,
    Borrow,
    CopyBorrowedValue,
    CloneErasedObject {
        sequence: bool,
    },
    ToOwnedSequence {
        policy: CloneRecoveryPolicy,
    },
    CloneRecovery {
        policy: CloneRecoveryPolicy,
    },
    UnsupportedOwnership {
        actual_ownership: OwnershipShape,
        target_ownership: OwnershipShape,
        policy: OwnershipCoercionFallbackPolicy,
    },
}

pub fn plan_coercion(actual: &JavaType, slot: &Slot) -> Vec<Coercion> {
    plan_coercion_with_facts(actual, slot, &FactSet::default())
}

pub fn plan_coercion_with_facts(actual: &JavaType, slot: &Slot, facts: &FactSet) -> Vec<Coercion> {
    crate::conversion_plan::syntax_coercion_plan_with_facts(actual, slot, facts).coercions()
}

impl SyntaxCoercionPlan {
    pub fn coercions(&self) -> Vec<Coercion> {
        self.policy.coercions()
    }
}

impl SyntaxSlotCoercionPolicy {
    pub fn coercions(&self) -> Vec<Coercion> {
        match self {
            SyntaxSlotCoercionPolicy::Identity => vec![Coercion::Identity],
            SyntaxSlotCoercionPolicy::IndexToUsize => vec![Coercion::IndexToUsize],
            SyntaxSlotCoercionPolicy::ToStringForConcat => vec![Coercion::ToStringForConcat],
            SyntaxSlotCoercionPolicy::ToJavaFloatStringForConcat { double } => {
                vec![Coercion::ToJavaFloatStringForConcat { double: *double }]
            }
            SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString => {
                vec![Coercion::ErasedObjectToJavaDisplayString]
            }
            SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString => {
                vec![Coercion::OptionalToJavaDisplayString]
            }
            SyntaxSlotCoercionPolicy::OptionalToJavaDisplayValue => {
                vec![Coercion::OptionalToJavaDisplayValue]
            }
            SyntaxSlotCoercionPolicy::OptionalBorrowedToJavaDisplayValue => {
                vec![Coercion::OptionalBorrowedToJavaDisplayValue]
            }
        }
    }
}

pub fn plan_shape_coercion(actual: &RustSlotShape, target: &RustSlotShape) -> Vec<Coercion> {
    plan_shape_coercion_with_facts(actual, target, &FactSet::default())
}

pub fn plan_shape_coercion_with_facts(
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> Vec<Coercion> {
    crate::conversion_plan::shape_coercion_plan_with_facts(actual, target, facts).coercions()
}

impl ShapeCoercionPlan {
    pub fn coercions(&self) -> Vec<Coercion> {
        let mut plan = Vec::new();
        let optional_coercions = self.optional.clone().coercions();
        if !optional_coercions.is_empty() {
            return optional_coercions;
        }
        if matches!(self.dispatch, DispatchCoercionPolicy::EraseObject) {
            plan.extend(self.ownership.coercions());
            plan.extend(self.dispatch.coercions());
            return non_identity_or_identity(plan);
        }
        if matches!(self.dispatch, DispatchCoercionPolicy::WrapClosedEnum { .. }) {
            plan.extend(self.ownership.coercions());
            if matches!(self.ownership, OwnershipCoercionPolicy::NoOwnershipChange)
                && self.actual.ownership == OwnershipShape::SharedBorrow
                && self.actual.view == ViewShape::Native
            {
                plan.push(Coercion::CloneRecovery {
                    policy: CloneRecoveryPolicy::AuditRequired,
                });
            }
            plan.extend(self.dispatch.coercions());
            return non_identity_or_identity(plan);
        }
        plan.extend(self.dispatch.coercions());
        let view_coercions = self.view.coercions();
        if !view_coercions.is_empty() {
            plan.extend(view_coercions);
            return plan;
        }
        if !matches!(
            self.dispatch,
            DispatchCoercionPolicy::CloneBoxDynTrait { .. }
        ) {
            plan.extend(self.ownership.coercions());
        }
        if let Some(policy) = self.primitive_widening {
            plan.push(Coercion::WidenPrimitive {
                from: policy.from,
                to: policy.to,
            });
        }
        non_identity_or_identity(plan)
    }
}

fn non_identity_or_identity(mut coercions: Vec<Coercion>) -> Vec<Coercion> {
    if coercions.is_empty() {
        coercions.push(Coercion::Identity);
    }
    coercions
}

pub fn non_identity_coercions(coercions: Vec<Coercion>) -> Vec<Coercion> {
    coercions
        .into_iter()
        .filter(|coercion| *coercion != Coercion::Identity)
        .collect()
}

impl OptionalShapeCoercionPolicy {
    pub fn coercions(self) -> Vec<Coercion> {
        match self {
            OptionalShapeCoercionPolicy::Wrap { element_plan, .. } => {
                let mut coercions = non_identity_coercions(element_plan.coercions());
                coercions.push(Coercion::WrapOption);
                coercions
            }
            OptionalShapeCoercionPolicy::Unwrap { element_plan, .. } => {
                let mut coercions = vec![optional_unwrap_coercion_for_element_plan(&element_plan)];
                coercions.extend(non_identity_coercions(
                    optional_unwrap_followup_plan(element_plan.as_ref()).coercions(),
                ));
                coercions
            }
            OptionalShapeCoercionPolicy::MapElement { .. } => Vec::new(),
            OptionalShapeCoercionPolicy::NoChange => Vec::new(),
        }
    }
}

fn optional_unwrap_coercion_for_element_plan(element_plan: &ShapeCoercionPlan) -> Coercion {
    match element_plan.target.ownership {
        OwnershipShape::SharedBorrow => Coercion::UnwrapOptionBorrowed,
        OwnershipShape::MutableBorrow => Coercion::UnwrapOptionMutBorrowed,
        _ => Coercion::UnwrapOption,
    }
}

impl ViewCoercionPolicy {
    pub fn coercions(&self) -> Vec<Coercion> {
        match self {
            ViewCoercionPolicy::NoViewChange => Vec::new(),
            ViewCoercionPolicy::BorrowStringView => vec![Coercion::AsStr],
            ViewCoercionPolicy::OwnStringFromView => vec![Coercion::ToOwnedString],
            ViewCoercionPolicy::BorrowSequenceSliceView => vec![Coercion::AsSlice],
            ViewCoercionPolicy::BorrowSequenceMutSliceView => vec![Coercion::AsMutSlice],
        }
    }
}

impl OwnershipCoercionPolicy {
    pub fn coercions(&self) -> Vec<Coercion> {
        match self {
            OwnershipCoercionPolicy::NoOwnershipChange => Vec::new(),
            OwnershipCoercionPolicy::BorrowOwnedValue => vec![Coercion::Borrow],
            OwnershipCoercionPolicy::BorrowOwnedMutably => vec![Coercion::BorrowMut],
            OwnershipCoercionPolicy::RecoverOwnedCopyValue => vec![Coercion::CopyBorrowedValue],
            OwnershipCoercionPolicy::RecoverErasedObjectClone { sequence } => {
                vec![Coercion::CloneErasedObject {
                    sequence: *sequence,
                }]
            }
            OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { policy } => {
                vec![Coercion::ToOwnedSequence {
                    policy: policy.clone(),
                }]
            }
            OwnershipCoercionPolicy::RecoverOwnedWithClone { policy } => {
                vec![Coercion::CloneRecovery {
                    policy: policy.clone(),
                }]
            }
            OwnershipCoercionPolicy::UnsupportedRecovery {
                actual_ownership,
                target_ownership,
                policy,
            } => vec![Coercion::UnsupportedOwnership {
                actual_ownership: *actual_ownership,
                target_ownership: *target_ownership,
                policy: policy.clone(),
            }],
        }
    }

    pub fn diagnostics(&self) -> Vec<CoercionDiagnostic> {
        match self {
            OwnershipCoercionPolicy::RecoverOwnedWithClone { policy }
            | OwnershipCoercionPolicy::RecoverOwnedSequenceWithToVec { policy } => {
                clone_recovery_diagnostics(policy)
            }
            OwnershipCoercionPolicy::UnsupportedRecovery {
                actual_ownership,
                target_ownership,
                policy,
            } => ownership_coercion_fallback_diagnostics(
                policy,
                *actual_ownership,
                *target_ownership,
            ),
            _ => Vec::new(),
        }
    }
}

fn clone_recovery_diagnostics(policy: &CloneRecoveryPolicy) -> Vec<CoercionDiagnostic> {
    match policy {
        CloneRecoveryPolicy::AuditRequired => vec![CoercionDiagnostic {
            code: "clone_audit_required",
            message: "borrowed-to-owned Java value recovery inserted a clone that requires ownership audit"
                .to_string(),
        }],
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => Vec::new(),
    }
}

fn ownership_coercion_fallback_diagnostics(
    policy: &OwnershipCoercionFallbackPolicy,
    actual_ownership: OwnershipShape,
    target_ownership: OwnershipShape,
) -> Vec<CoercionDiagnostic> {
    vec![
        ownership_coercion_fallback_message_policy(policy, actual_ownership, target_ownership)
            .into_diagnostic(),
    ]
}

pub fn ownership_coercion_fallback_message_policy(
    policy: &OwnershipCoercionFallbackPolicy,
    actual_ownership: OwnershipShape,
    target_ownership: OwnershipShape,
) -> UnsupportedCoercionMessagePolicy {
    match policy {
        OwnershipCoercionFallbackPolicy::CompileError => UnsupportedCoercionMessagePolicy {
            code: "unsupported_ownership_coercion",
            message: format!(
                "unsupported Java ownership coercion: actual={actual_ownership:?}, target={target_ownership:?}"
            ),
        },
        OwnershipCoercionFallbackPolicy::BorrowedToOwnedNonCloneable { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_borrowed_to_owned_non_cloneable",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for non-cloneable type {java_ty:?}"
                ),
            }
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFacadeOwnedExternalHandle { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_borrowed_to_facade_owned_external_handle",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for facade-owned external handle {java_ty:?}; use an explicit facade ownership transfer"
                ),
            }
        }
        OwnershipCoercionFallbackPolicy::BorrowedToFfiOwnedHandle { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_borrowed_to_ffi_owned_handle",
                message: format!(
                    "unsupported Java borrowed-to-owned recovery for FFI-owned handle {java_ty:?}; use an explicit FFI ownership transfer"
                ),
            }
        }
    }
}

impl DispatchCoercionPolicy {
    pub fn coercions(&self) -> Vec<Coercion> {
        match self {
            DispatchCoercionPolicy::NoDispatchChange => Vec::new(),
            DispatchCoercionPolicy::WrapClosedEnum {
                enum_name,
                variant_name,
            } => vec![Coercion::WrapClosedEnum {
                enum_name: enum_name.clone(),
                variant_name: variant_name.clone(),
            }],
            DispatchCoercionPolicy::BorrowDynTrait { trait_name } => {
                vec![Coercion::BorrowDynTrait {
                    trait_name: trait_name.clone(),
                }]
            }
            DispatchCoercionPolicy::BorrowBoxedDynTrait { trait_name } => {
                vec![Coercion::BorrowBoxedDynTrait {
                    trait_name: trait_name.clone(),
                }]
            }
            DispatchCoercionPolicy::BorrowConcreteFacade { target_type } => {
                vec![Coercion::BorrowConcreteFacade {
                    target_type: target_type.clone(),
                }]
            }
            DispatchCoercionPolicy::ExternalInto { from_type, to_type } => {
                vec![Coercion::ExternalInto {
                    from_type: from_type.clone(),
                    to_type: to_type.clone(),
                }]
            }
            DispatchCoercionPolicy::BoxDynTrait { trait_name } => vec![Coercion::BoxDynTrait {
                trait_name: trait_name.clone(),
            }],
            DispatchCoercionPolicy::CloneBoxDynTrait { trait_name } => {
                vec![Coercion::CloneBoxDynTrait {
                    trait_name: trait_name.clone(),
                }]
            }
            DispatchCoercionPolicy::EraseObject => vec![Coercion::EraseObject],
            DispatchCoercionPolicy::DowncastClosedEnum {
                enum_name,
                variant_name,
                invalid_cast,
            } => vec![Coercion::DowncastClosedEnum {
                enum_name: enum_name.clone(),
                variant_name: variant_name.clone(),
                invalid_cast: invalid_cast.clone(),
            }],
            DispatchCoercionPolicy::DowncastDynTrait {
                target_type,
                invalid_cast,
            } => vec![Coercion::DowncastDynTrait {
                target_type: target_type.clone(),
                invalid_cast: invalid_cast.clone(),
            }],
            DispatchCoercionPolicy::DowncastBoxedDynTrait {
                target_type,
                invalid_cast,
            } => vec![Coercion::DowncastBoxedDynTrait {
                target_type: target_type.clone(),
                invalid_cast: invalid_cast.clone(),
            }],
            DispatchCoercionPolicy::DowncastErasedObject {
                target_type,
                invalid_cast,
            } => vec![Coercion::DowncastErasedObject {
                target_type: target_type.clone(),
                invalid_cast: invalid_cast.clone(),
            }],
            DispatchCoercionPolicy::Unsupported {
                actual_dispatch,
                target_dispatch,
                policy,
            } => vec![Coercion::UnsupportedDispatch {
                actual_dispatch: actual_dispatch.clone(),
                target_dispatch: target_dispatch.clone(),
                policy: policy.clone(),
            }],
        }
    }

    pub fn diagnostics(&self) -> Vec<CoercionDiagnostic> {
        match self {
            DispatchCoercionPolicy::Unsupported {
                actual_dispatch,
                target_dispatch,
                policy,
            } => dispatch_coercion_fallback_diagnostics(policy, actual_dispatch, target_dispatch),
            _ => Vec::new(),
        }
    }
}

fn dispatch_coercion_fallback_diagnostics(
    policy: &DispatchCoercionFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> Vec<CoercionDiagnostic> {
    let message_policy =
        dispatch_coercion_fallback_message_policy(policy, actual_dispatch, target_dispatch);
    vec![message_policy.into_diagnostic()]
}

pub fn dispatch_coercion_fallback_message_policy(
    policy: &DispatchCoercionFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> UnsupportedCoercionMessagePolicy {
    match policy {
        DispatchCoercionFallbackPolicy::CompileError => UnsupportedCoercionMessagePolicy {
            code: "unsupported_dispatch_coercion",
            message: format!(
                "unsupported Java dispatch coercion: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        },
        DispatchCoercionFallbackPolicy::MissingRuntimeTarget { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_dispatch_missing_runtime_target",
                message: format!(
                    "unsupported Java dispatch coercion: target type {java_ty:?} has no concrete runtime type"
                ),
            }
        }
        DispatchCoercionFallbackPolicy::BorrowedDynToOwnedBoxedDyn {
            trait_name,
            recovery,
        } => borrowed_dyn_to_owned_boxed_dyn_message_policy(trait_name, recovery),
        DispatchCoercionFallbackPolicy::ErasedObjectToDynTrait { trait_name } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_erased_object_to_dyn_trait",
                message: format!(
                    "unsupported Java erased object to borrowed dyn trait recovery for {trait_name}; a concrete downcast target is required"
                ),
            }
        }
        DispatchCoercionFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_erased_object_to_boxed_dyn_trait",
                message: format!(
                    "unsupported Java erased object to owned boxed dyn trait recovery for {trait_name}; a concrete downcast target and ownership proof are required"
                ),
            }
        }
    }
}

fn borrowed_dyn_to_owned_boxed_dyn_message_policy(
    trait_name: &str,
    recovery: &BorrowedDynToOwnedBoxedDynRecoveryPolicy,
) -> UnsupportedCoercionMessagePolicy {
    let requirement = dyn_clone_box_adapter_requirement(trait_name, recovery);
    match dyn_clone_box_adapter_generation_policy(&requirement) {
        DynCloneBoxAdapterGenerationPolicy::AlreadyAvailable { trait_name } => {
            UnsupportedCoercionMessagePolicy {
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
            UnsupportedCoercionMessagePolicy {
                code: "borrowed_dyn_to_owned_boxed_dyn_clone_box_adapter_required",
                message: format!(
                    "Java borrowed dyn trait to owned boxed dyn recovery for {trait_name}; {proof}, and dyn clone-box adapter generation is required"
                ),
            }
        }
        DynCloneBoxAdapterGenerationPolicy::DeferredUnsupported { trait_name, reason } => {
            let diagnostic = dyn_clone_box_adapter_deferral_message(&trait_name, reason);
            UnsupportedCoercionMessagePolicy {
                code: diagnostic.code,
                message: diagnostic.message,
            }
        }
    }
}

pub fn apply_lowered_expr_to_slot(lowered: LoweredExpr, slot: &Slot) -> RustExpr {
    apply_lowered_expr_to_slot_with_facts(lowered, slot, &FactSet::default())
}

pub fn apply_lowered_expr_to_slot_with_facts(
    lowered: LoweredExpr,
    slot: &Slot,
    facts: &FactSet,
) -> RustExpr {
    let plan = crate::conversion_plan::lowered_slot_conversion_plan_with_facts(
        &lowered.actual,
        slot,
        facts,
    );
    apply_slot_conversion_plan(lowered.expr, &plan)
}

pub fn apply_coercions(mut expr: RustExpr, coercions: &[Coercion]) -> RustExpr {
    for coercion in coercions {
        expr = apply_one(expr, coercion);
    }
    expr
}

pub fn apply_shape_coercion_plan(expr: RustExpr, plan: &ShapeCoercionPlan) -> RustExpr {
    match &plan.optional {
        OptionalShapeCoercionPolicy::Wrap { element_plan, .. } => {
            let expr = apply_shape_coercion_plan(expr, element_plan);
            return apply_one(expr, &Coercion::WrapOption);
        }
        OptionalShapeCoercionPolicy::Unwrap { element_plan, .. } => {
            let expr = apply_one(
                expr,
                &optional_unwrap_coercion_for_element_plan(element_plan),
            );
            return apply_shape_coercion_plan(expr, &optional_unwrap_followup_plan(element_plan));
        }
        OptionalShapeCoercionPolicy::MapElement { element_plan, .. } => {
            if optional_map_targets_borrowed_string_view(plan) {
                let target_element_ty = plan
                    .target
                    .optional_element_type()
                    .expect("borrowed optional string view target should have an element");
                let target_element = crate::conversion_plan::optional_element_slot_shape(
                    &plan.target,
                    target_element_ty,
                );
                if expr.recorded_slot_shape().is_some_and(|actual| {
                    slot_shapes_match_ignoring_identity(actual, &target_element)
                }) {
                    return expr;
                }
                return RustExpr::shaped_as(
                    RustExprShape::OptionalProjected,
                    Some(target_element),
                    optional_string_view_projection_expr(expr),
                );
            }
            let mapped_value =
                apply_shape_coercion_plan(RustExpr::Var("value".to_string()), element_plan);
            return render_optional_element_projection(expr, mapped_value, &element_plan.target);
        }
        OptionalShapeCoercionPolicy::NoChange => {}
    }
    if let Some(projected) = apply_borrowed_set_slice_view(expr.clone(), plan) {
        return projected;
    }
    let coerced = apply_coercions(expr, &plan.coercions());
    record_view_coercion_target_shape(coerced, plan)
}

fn optional_map_targets_borrowed_string_view(plan: &ShapeCoercionPlan) -> bool {
    plan.actual
        .optional_element_type()
        .is_some_and(|element_ty| matches!(element_ty, JavaType::String))
        && plan
            .target
            .optional_element_type()
            .is_some_and(|element_ty| matches!(element_ty, JavaType::String))
        && matches!(plan.target.ownership, OwnershipShape::SharedBorrow)
        && matches!(plan.target.view, ViewShape::Str)
}

fn record_view_coercion_target_shape(expr: RustExpr, plan: &ShapeCoercionPlan) -> RustExpr {
    match &plan.view {
        ViewCoercionPolicy::BorrowStringView => {
            RustExpr::shaped_as(RustExprShape::StringView, Some(plan.target.clone()), expr)
        }
        ViewCoercionPolicy::BorrowSequenceSliceView => {
            RustExpr::shaped_as(RustExprShape::SliceView, Some(plan.target.clone()), expr)
        }
        ViewCoercionPolicy::BorrowSequenceMutSliceView => RustExpr::shaped_as(
            RustExprShape::MutableSliceView,
            Some(plan.target.clone()),
            expr,
        ),
        ViewCoercionPolicy::OwnStringFromView => RustExpr::shaped_as(
            RustExprShape::OwnedRecovery,
            Some(plan.target.clone()),
            expr,
        ),
        ViewCoercionPolicy::NoViewChange => expr,
    }
}

fn apply_borrowed_set_slice_view(expr: RustExpr, plan: &ShapeCoercionPlan) -> Option<RustExpr> {
    if !matches!(plan.view, ViewCoercionPolicy::BorrowSequenceSliceView)
        || !matches!(plan.target.ownership, OwnershipShape::SharedBorrow)
        || !matches!(plan.target.view, ViewShape::Slice)
        || crate::stdlib::java_collection_kind_for_type(&plan.actual.java_ty)
            != Some(crate::stdlib::JavaCollectionKind::Set)
    {
        return None;
    }
    if expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &plan.target))
    {
        return Some(expr);
    }
    let rendered = emit::emit_expr(&expr);
    Some(RustExpr::shaped_as(
        RustExprShape::SliceView,
        Some(plan.target.clone()),
        RustExpr::Raw(format!("&({rendered}).iter().cloned().collect::<Vec<_>>()")),
    ))
}

fn render_optional_element_projection(
    expr: RustExpr,
    mapped_value: RustExpr,
    target_element: &RustSlotShape,
) -> RustExpr {
    if expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, target_element))
    {
        return expr;
    }
    match mapped_optional_value_projection(&mapped_value) {
        OptionalMappedValueProjection::SharedBorrow => {
            return optional_projected_structured_expr(
                zero_arg_method_projection(expr, "as_ref"),
                target_element,
            );
        }
        OptionalMappedValueProjection::Identity => return expr,
        OptionalMappedValueProjection::StringView => {
            return optional_projected_structured_expr(
                optional_string_view_projection_expr(expr),
                target_element,
            );
        }
        OptionalMappedValueProjection::SliceView => {
            return optional_projected_structured_expr(
                RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "as_deref".to_string(),
                    args: Vec::new(),
                },
                target_element,
            );
        }
        OptionalMappedValueProjection::MutableSliceView => {
            return optional_projected_structured_expr(
                RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "as_deref_mut".to_string(),
                    args: Vec::new(),
                },
                target_element,
            );
        }
        OptionalMappedValueProjection::Other => {}
    }
    let rendered_mapped = emit::emit_expr(&mapped_value);
    match mapped_value.recorded_shape() {
        Some(RustExprShape::Borrowed)
            if target_element.ownership == OwnershipShape::SharedBorrow
                && target_element.view == ViewShape::Native =>
        {
            return optional_projected_structured_expr(
                optional_map_projection_expr(
                    expr,
                    OptionalMapReceiverProjection::AsRef,
                    rendered_mapped,
                ),
                target_element,
            );
        }
        Some(RustExprShape::StringView)
            if target_element.ownership == OwnershipShape::SharedBorrow
                && target_element.view == ViewShape::Str
                && target_element.java_ty == JavaType::String =>
        {
            return optional_projected_structured_expr(
                optional_map_projection_expr(
                    expr,
                    OptionalMapReceiverProjection::AsRef,
                    rendered_mapped,
                ),
                target_element,
            );
        }
        Some(RustExprShape::SliceView)
            if target_element.ownership == OwnershipShape::SharedBorrow
                && target_element.view == ViewShape::Slice
                && matches!(target_element.java_ty, JavaType::Array(_)) =>
        {
            return optional_projected_structured_expr(
                optional_map_projection_expr(
                    expr,
                    OptionalMapReceiverProjection::AsRef,
                    rendered_mapped,
                ),
                target_element,
            );
        }
        Some(RustExprShape::MutableSliceView)
            if target_element.ownership == OwnershipShape::MutableBorrow
                && target_element.view == ViewShape::Slice
                && matches!(target_element.java_ty, JavaType::Array(_)) =>
        {
            return optional_projected_structured_expr(
                optional_map_projection_expr(
                    expr,
                    OptionalMapReceiverProjection::AsMut,
                    rendered_mapped,
                ),
                target_element,
            );
        }
        _ => {}
    }
    optional_projected_structured_expr(
        optional_map_projection_expr(expr, OptionalMapReceiverProjection::Direct, rendered_mapped),
        target_element,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionalMapReceiverProjection {
    Direct,
    AsRef,
    AsMut,
}

fn optional_map_projection_expr(
    expr: RustExpr,
    receiver_projection: OptionalMapReceiverProjection,
    rendered_mapped: String,
) -> RustExpr {
    let receiver = match receiver_projection {
        OptionalMapReceiverProjection::Direct => expr,
        OptionalMapReceiverProjection::AsRef => zero_arg_method_projection(expr, "as_ref"),
        OptionalMapReceiverProjection::AsMut => zero_arg_method_projection(expr, "as_mut"),
    };
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: "map".to_string(),
        args: vec![RustExpr::Raw(format!("|value| {rendered_mapped}"))],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OptionalMappedValueProjection {
    Identity,
    SharedBorrow,
    StringView,
    SliceView,
    MutableSliceView,
    Other,
}

fn mapped_optional_value_projection(expr: &RustExpr) -> OptionalMappedValueProjection {
    match expr.unshaped() {
        RustExpr::Var(name) if name == "value" => OptionalMappedValueProjection::Identity,
        RustExpr::Unary { op, expr }
            if op == "&" && matches!(expr.unshaped(), RustExpr::Var(name) if name == "value") =>
        {
            OptionalMappedValueProjection::SharedBorrow
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty()
            && matches!(receiver.unshaped(), RustExpr::Var(name) if name == "value") =>
        {
            match method.as_str() {
                "as_str" => OptionalMappedValueProjection::StringView,
                "as_slice" => OptionalMappedValueProjection::SliceView,
                "as_mut_slice" => OptionalMappedValueProjection::MutableSliceView,
                _ => OptionalMappedValueProjection::Other,
            }
        }
        _ => OptionalMappedValueProjection::Other,
    }
}

fn optional_projected_structured_expr(expr: RustExpr, target_element: &RustSlotShape) -> RustExpr {
    RustExpr::shaped_as(
        RustExprShape::OptionalProjected,
        Some(target_element.clone()),
        expr,
    )
}

pub fn project_optional_element_borrow_carrier(
    option_expr: RustExpr,
    element_ownership: OwnershipShape,
) -> Option<RustExpr> {
    match element_ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::shaped(
            RustExprShape::OptionalProjected,
            zero_arg_method_projection(option_expr, "as_ref"),
        )),
        OwnershipShape::MutableBorrow => Some(RustExpr::shaped(
            RustExprShape::OptionalProjected,
            zero_arg_method_projection(option_expr, "as_mut"),
        )),
        OwnershipShape::Owned | OwnershipShape::CopyValue => None,
    }
}

pub fn project_optional_element_borrow_carrier_with_shape(
    option_expr: RustExpr,
    element_shape: &RustSlotShape,
) -> Option<RustExpr> {
    match element_shape.ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::shaped_as(
            RustExprShape::OptionalProjected,
            Some(element_shape.clone()),
            zero_arg_method_projection(option_expr, "as_ref"),
        )),
        OwnershipShape::MutableBorrow => Some(RustExpr::shaped_as(
            RustExprShape::OptionalProjected,
            Some(element_shape.clone()),
            zero_arg_method_projection(option_expr, "as_mut"),
        )),
        OwnershipShape::Owned | OwnershipShape::CopyValue => None,
    }
}

fn optional_unwrap_followup_plan(element_plan: &ShapeCoercionPlan) -> ShapeCoercionPlan {
    let mut plan = element_plan.clone();
    if matches!(
        plan.target.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) {
        plan.ownership = OwnershipCoercionPolicy::NoOwnershipChange;
    }
    plan
}

pub fn apply_syntax_coercion_plan(expr: RustExpr, plan: &SyntaxCoercionPlan) -> RustExpr {
    apply_coercions(expr, &plan.coercions())
}

/// Executes a shared-handle crossing.
///
/// Transparent for reads and cloning; opaque where a conversion constructs a
/// fresh value for a slot that stores a shared handle.
fn apply_handle_adaptation(
    expr: RustExpr,
    adaptation: &crate::conversion_plan::HandleAdaptation,
) -> RustExpr {
    use crate::conversion_plan::HandleAdaptation;
    match adaptation {
        HandleAdaptation::None => expr,
        HandleAdaptation::Wrap(placement) => {
            RustExpr::Raw(placement.wrap_construction(emit::emit_expr(&expr)))
        }
    }
}

pub fn apply_slot_conversion_plan(expr: RustExpr, plan: &SlotConversionPlan) -> RustExpr {
    match plan {
        SlotConversionPlan::Shape(plan) => apply_shape_coercion_plan(expr, plan),
        SlotConversionPlan::Syntax(plan) => apply_syntax_coercion_plan(expr, plan),
        SlotConversionPlan::OptionalConstruction(plan) => {
            apply_optional_slot_construction(expr, plan)
        }
        SlotConversionPlan::BorrowedElement(policy) => {
            apply_borrowed_element_slot_adapter(expr, policy)
        }
        SlotConversionPlan::RustScalarCast(policy) => apply_rust_scalar_slot_cast(expr, policy),
        SlotConversionPlan::RustBorrowedFacadeReturn(policy) => {
            apply_rust_borrowed_facade_return(expr, policy)
        }
        SlotConversionPlan::OptionalRustBorrowedFacadeReturn(policy) => {
            apply_optional_rust_borrowed_facade_return(expr, policy)
        }
        SlotConversionPlan::RustUnsignedByteArrayReturn(policy) => {
            apply_rust_unsigned_byte_array_return(expr, policy)
        }
        SlotConversionPlan::Identity { .. } => expr,
        // The handle crossing wraps the inner conversion, so every caller of
        // this function gets it — there are four, and a sibling field on the
        // outer plan reached only one.
        SlotConversionPlan::Handled { handle, inner } => {
            let converted = apply_slot_conversion_plan(expr, inner);
            apply_handle_adaptation(converted, handle)
        }
    }
}

fn apply_optional_slot_construction(
    expr: RustExpr,
    plan: &crate::conversion_plan::OptionalSlotConstructionPlan,
) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(apply_shape_coercion_plan(
        expr,
        &plan.element_plan,
    ));
    render_option_some(expr)
}

fn normalize_optional_constructed_element_expr(expr: RustExpr) -> RustExpr {
    if let RustExpr::If {
        condition,
        then_expr,
        else_expr,
    } = expr
    {
        return RustExpr::If {
            condition,
            then_expr: Box::new(normalize_optional_constructed_element_expr(*then_expr)),
            else_expr: Box::new(normalize_optional_constructed_element_expr(*else_expr)),
        };
    }
    if let RustExpr::Unary { op, expr: inner } = expr.unshaped()
        && op == "&"
        && expr_is_string_view(inner)
    {
        return inner.as_ref().clone();
    }
    let rendered = emit::emit_expr(&expr);
    if let Some(inner) = rendered
        .strip_prefix('&')
        .and_then(|value| value.strip_suffix(".as_str()"))
    {
        return string_view_expr(zero_arg_method_projection(
            RustExpr::Raw(inner.to_string()),
            "as_str",
        ));
    }
    expr
}

pub fn normalize_optional_constructed_string_view_expr(expr: RustExpr) -> RustExpr {
    if let RustExpr::Call { function, args } = expr {
        if rust_expr_is_single_segment_name(function.as_ref(), "Some") && args.len() == 1 {
            let mut args = args;
            let element = normalize_optional_constructed_string_view_element(args.remove(0));
            return RustExpr::Call {
                function,
                args: vec![element],
            };
        }
        return RustExpr::Call { function, args };
    }
    let rendered = emit::emit_expr(&expr);
    if let Some(inner) = rendered
        .strip_prefix("Some(&")
        .and_then(|value| value.strip_suffix(".as_str())"))
    {
        return RustExpr::shaped(
            RustExprShape::OptionalConstructed,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Some"])),
                args: vec![string_view_expr(zero_arg_method_projection(
                    RustExpr::Raw(inner.to_string()),
                    "as_str",
                ))],
            },
        );
    }
    expr
}

fn normalize_optional_constructed_string_view_element(expr: RustExpr) -> RustExpr {
    match expr.unshaped() {
        RustExpr::Unary { op, expr: inner } if op == "&" && expr_is_string_view(inner) => {
            inner.as_ref().clone()
        }
        _ => expr,
    }
}

pub fn unwrap_rendered_some_expr(expr: &RustExpr) -> Option<RustExpr> {
    unwrap_some_expr(expr)
}

pub fn unwrap_some_expr(expr: &RustExpr) -> Option<RustExpr> {
    match expr.unshaped() {
        RustExpr::Call { function, args }
            if rust_expr_is_single_segment_name(function.as_ref(), "Some") && args.len() == 1 =>
        {
            args.first().cloned()
        }
        RustExpr::Raw(raw) => unwrap_rendered_some_raw_expr(raw),
        _ => None,
    }
}

fn rust_expr_is_single_segment_name(expr: &RustExpr, expected: &str) -> bool {
    match expr.unshaped() {
        RustExpr::Var(name) => name == expected,
        RustExpr::Path(segments) => segments.len() == 1 && segments[0] == expected,
        _ => false,
    }
}

fn unwrap_rendered_some_raw_expr(rendered: &str) -> Option<RustExpr> {
    rendered_complete_some_call_inner(rendered).map(|inner| RustExpr::Raw(inner.to_string()))
}

fn rendered_complete_some_call_inner(rendered: &str) -> Option<&str> {
    let trimmed = rendered.trim();
    let inner_start = "Some(".len();
    if !trimmed.starts_with("Some(") {
        return None;
    }

    let mut depth = 1usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut idx = inner_start;
    while idx < trimmed.len() {
        let ch = trimmed[idx..]
            .chars()
            .next()
            .expect("cursor is always on a char boundary");
        if escaped {
            escaped = false;
            idx += ch.len_utf8();
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            idx += ch.len_utf8();
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(trimmed, idx) {
            idx = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (idx + ch.len_utf8() == trimmed.len())
                        .then_some(&trimmed[inner_start..idx]);
                }
            }
            _ => {}
        }
        idx += ch.len_utf8();
    }
    None
}

pub fn rust_expr_is_constructed_optional_value(expr: &RustExpr) -> bool {
    if expr.recorded_shape() == Some(RustExprShape::OptionalConstructed) {
        return true;
    }
    match expr.unshaped() {
        RustExpr::Call { function, .. } => {
            rust_expr_is_single_segment_name(function.as_ref(), "Some")
        }
        RustExpr::If {
            then_expr,
            else_expr,
            ..
        } => {
            rust_expr_is_constructed_optional_value(then_expr)
                || rust_expr_is_constructed_optional_value(else_expr)
        }
        RustExpr::MethodCall {
            receiver, method, ..
        } if matches!(method.as_str(), "cloned" | "copied") => {
            rust_expr_is_get_projection(receiver)
        }
        RustExpr::Raw(raw) => rendered_expr_is_constructed_optional_value(raw),
        _ => false,
    }
}

fn rust_expr_is_get_projection(expr: &RustExpr) -> bool {
    matches!(expr.unshaped(), RustExpr::MethodCall { method, .. } if method == "get")
}

fn rendered_expr_is_constructed_optional_value(rendered: &str) -> bool {
    let trimmed = rendered.trim_start();
    rendered_complete_some_call_inner(trimmed).is_some()
        || (trimmed.starts_with("if ") && raw_contains_outside_literals(trimmed, "{ Some("))
        || rendered_expr_is_get_option_projection(trimmed)
}

fn rendered_expr_is_get_option_projection(rendered: &str) -> bool {
    let Some(receiver) = rendered
        .strip_suffix(".cloned()")
        .or_else(|| rendered.strip_suffix(".copied()"))
    else {
        return false;
    };
    rendered_terminal_method_is_get(receiver.trim_end())
}

fn rendered_terminal_method_is_get(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    if !trimmed.ends_with(')') {
        return false;
    }
    let final_close = trimmed.len() - 1;
    let mut stack = Vec::new();
    let mut matching_open = None;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut idx = 0;
    while idx < trimmed.len() {
        let ch = trimmed[idx..]
            .chars()
            .next()
            .expect("cursor is always on a char boundary");
        if escaped {
            escaped = false;
            idx += ch.len_utf8();
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            idx += ch.len_utf8();
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(trimmed, idx) {
            idx = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => stack.push(idx),
            ')' => {
                let Some(open) = stack.pop() else {
                    return false;
                };
                if idx == final_close {
                    matching_open = Some(open);
                }
            }
            _ => {}
        }
        idx += ch.len_utf8();
    }
    if !stack.is_empty() || in_string || in_char {
        return false;
    }
    let Some(open) = matching_open else {
        return false;
    };
    trimmed[..open].trim_end().ends_with(".get")
}

pub fn parenthesize_borrowed_method_receiver(receiver: RustExpr) -> RustExpr {
    match receiver {
        RustExpr::Unary { op, expr } if op == "&" || op == "&mut " => RustExpr::Raw(format!(
            "({})",
            emit::emit_expr(&RustExpr::Unary { op, expr })
        )),
        RustExpr::Raw(rendered)
            if rendered.starts_with('&')
                && !(rendered.starts_with("(&") && rendered.ends_with(')')) =>
        {
            RustExpr::Raw(format!("({rendered})"))
        }
        other => other,
    }
}

fn parenthesize_cast_method_receiver(rendered: &str) -> String {
    let trimmed = rendered.trim();
    if balanced_outer_parentheses_wrap_all(trimmed) {
        return trimmed.to_string();
    }
    if raw_contains_top_level_as_cast(trimmed) {
        format!("({trimmed})")
    } else {
        trimmed.to_string()
    }
}

fn raw_contains_top_level_as_cast(raw: &str) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            while chars.peek().is_some_and(|(next, _)| *next < end) {
                chars.next();
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth = angle_depth.saturating_sub(1)
            }
            'a' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0
                && raw[idx..].starts_with("as")
                && raw_identifier_boundary(raw[..idx].chars().next_back())
                && raw_identifier_boundary(raw[idx + 2..].chars().next()) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn raw_contains_as_token_outside_literals(raw: &str) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = None;
    for (idx, ch) in raw.char_indices() {
        if let Some(end) = skip_until {
            if idx < end {
                continue;
            }
            skip_until = None;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = Some(end);
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            'a' if raw[idx..].starts_with("as")
                && raw_identifier_boundary(raw[..idx].chars().next_back())
                && raw_identifier_boundary(raw[idx + 2..].chars().next()) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub fn force_shared_borrow_projection(expr: RustExpr) -> RustExpr {
    if matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::shaped(
            RustExprShape::Borrowed,
            RustExpr::Raw(format!("&{}", emit::emit_expr(&expr))),
        );
    }
    RustExpr::shaped(
        RustExprShape::Borrowed,
        RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(expr),
        },
    )
}

pub fn propagate_result_projection(expr: RustExpr) -> RustExpr {
    if expr_is_result_propagated(&expr) {
        return expr;
    }
    RustExpr::shaped(
        RustExprShape::ResultPropagated,
        RustExpr::Raw(format!("{}?", emit::emit_expr(&expr))),
    )
}

/// Whether `?` has already been applied to this expression.
///
/// A raw string ending in `?` is not enough evidence: it cannot distinguish a
/// try operator from punctuation inside a string literal. Producers that apply
/// result propagation must record `ResultPropagated`.
pub fn expr_is_result_propagated(expr: &RustExpr) -> bool {
    expr.recorded_shape() == Some(RustExprShape::ResultPropagated)
}

pub fn self_associated_path_projection(member: &str) -> RustExpr {
    associated_path_projection(["Self", member])
}

pub fn owner_associated_path_projection(owner: &str, member: &str) -> RustExpr {
    associated_path_projection([owner, member])
}

pub fn java_class_static_path_projection(member: &str) -> RustExpr {
    associated_path_projection(["JavaClass::<()>", member])
}

pub fn java_class_literal_path_projection(rust_type: &str) -> RustExpr {
    associated_path_projection([&format!("JavaClass::<{rust_type}>"), "literal"])
}

pub fn java_class_for_name_path_projection(rust_type: &str) -> RustExpr {
    associated_path_projection([&format!("JavaClass::<{rust_type}>"), "for_name"])
}

pub fn external_crate_path_projection(crate_name: &str, member: &str) -> RustExpr {
    associated_path_projection([crate_name, member])
}

pub fn external_crate_associated_path_projection(
    crate_name: &str,
    type_name: &str,
    member: &str,
) -> RustExpr {
    associated_path_projection([crate_name, type_name, member])
}

pub fn static_storage_accessor_projection(static_name: &str) -> RustExpr {
    RustExpr::Call {
        function: Box::new(associated_path_projection([
            "Self",
            &format!("__static_{}", crate::parser::to_snake_case(static_name)),
        ])),
        args: Vec::new(),
    }
}

pub fn static_storage_setter_path_projection(static_name: &str) -> RustExpr {
    associated_path_projection([
        "Self",
        &format!("__set_static_{}", crate::parser::to_snake_case(static_name)),
    ])
}

pub fn qualified_static_storage_accessor_projection(owner: &str, static_name: &str) -> RustExpr {
    RustExpr::Call {
        function: Box::new(associated_path_projection([
            owner,
            &format!("__static_{}", crate::parser::to_snake_case(static_name)),
        ])),
        args: Vec::new(),
    }
}

pub fn qualified_static_storage_setter_path_projection(owner: &str, static_name: &str) -> RustExpr {
    associated_path_projection([
        owner,
        &format!("__set_static_{}", crate::parser::to_snake_case(static_name)),
    ])
}

fn associated_path_projection<const N: usize>(segments: [&str; N]) -> RustExpr {
    RustExpr::path(segments)
}

/// Reads a Java `static` field from process-wide storage.
///
/// A Java `static` is **one variable shared by every thread**. This used to read
/// a `thread_local!`, giving each thread its own copy — so a shared counter,
/// cache or registry silently became per-thread, and a lazily initialised
/// singleton was built once *per thread*
/// ([0030](../claude_audits/done/0030-java-statics-are-thread-local.md)).
///
/// The lock is taken and released within the accessor, so no guard is ever held
/// across a call — the same discipline
/// [0002b](../claude_audits/0002b-shared-representation-design.md) requires of
/// `RefCell`, and for the same reason: Java's monitors are reentrant and Rust's
/// are not.
pub fn static_storage_getter_body_projection(storage_name: &str, initial: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{storage_name}.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone().unwrap_or_else(|| {})",
        emit::emit_expr(&initial)
    ))
}

/// Writes one *element* of a static array back into the shared storage.
///
/// The getter returns the value **by value** — it clones out of the mutex — so
/// `Self::__static_counter()[0] = v` assigns into a temporary and is silently
/// discarded. Rust accepts it, nothing warns, and the array never changes
/// ([0039](../claude_audits/done/0039-static-array-fields-lose-their-value-and-their-writes.md)).
///
/// `index` and `value` must already be bound to temporaries by the caller. That
/// is not tidiness: `counter[0] = counter[0] + 1` re-reads the same static, and
/// a `Mutex` is not reentrant, so evaluating either operand while holding the
/// guard deadlocks. Binding first also gives Java's order — target
/// subexpressions before the right-hand side
/// ([0026](../claude_audits/done/0026-assignment-evaluation-order-differs-from-java.md)).
///
/// `get_or_insert_with` rather than `as_mut().unwrap()` so a write that lands
/// before any read still installs the declared initial value.
pub fn static_storage_element_write_projection(
    storage_name: &str,
    initial: &RustExpr,
    index: &str,
    value: &str,
) -> String {
    format!(
        "{storage_name}.lock().unwrap_or_else(|poisoned| poisoned.into_inner())\n        .get_or_insert_with(|| {})[{index}] = {value};",
        emit::emit_expr(initial)
    )
}

pub fn static_storage_setter_body_projection(storage_name: &str) -> String {
    format!(
        "*{storage_name}.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);"
    )
}

pub fn associated_const_projection(owner: &str, member: &str) -> RustExpr {
    associated_path_projection([owner, member])
}

pub fn unbound_method_reference_projection(method: &str) -> RustExpr {
    RustExpr::Raw(format!("|value| value.{method}()"))
}

pub fn current_receiver_method_reference_projection(method: &str, arity: usize) -> RustExpr {
    let args = (0..arity)
        .map(|index| {
            if arity == 1 {
                "value".to_string()
            } else {
                format!("arg{index}")
            }
        })
        .collect::<Vec<_>>();
    RustExpr::Raw(format!(
        "|{}| self.{method}({})",
        args.join(", "),
        args.join(", ")
    ))
}

pub fn unbound_method_call_projection(receiver: RustExpr, method: &str) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: method.to_string(),
        args: Vec::new(),
    }
}

pub fn function_call_projection(function: RustExpr, args: Vec<RustExpr>) -> RustExpr {
    RustExpr::Call {
        function: Box::new(function),
        args,
    }
}

pub fn receiver_method_call_projection_exact(
    receiver: RustExpr,
    method: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: method.to_string(),
        args,
    }
}

pub fn receiver_method_call_projection(
    receiver: RustExpr,
    method: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: crate::parser::to_snake_case(method),
        args,
    }
}

pub fn current_receiver_method_call_projection(method: &str, args: Vec<RustExpr>) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Raw("self".to_string())),
        method: method.to_string(),
        args,
    }
}

pub fn typed_single_input_callback_projection(param_type: &str, body: &str) -> RustExpr {
    RustExpr::Raw(format!("|value: {param_type}| {body}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociatedFunctionMethodReferenceProjection {
    PrimitiveIdentityValue,
    ReceiverToString,
    AlwaysTrue,
    StaticFunction,
}

pub fn associated_function_method_reference_body_projection(
    projection: AssociatedFunctionMethodReferenceProjection,
    owner: &str,
    method: &str,
    receiver: &str,
    value_expr: &str,
) -> String {
    match projection {
        AssociatedFunctionMethodReferenceProjection::PrimitiveIdentityValue => {
            value_expr.to_string()
        }
        AssociatedFunctionMethodReferenceProjection::ReceiverToString => {
            emit::emit_expr(&receiver_method_call_projection(
                RustExpr::Raw(receiver.to_string()),
                "to_string",
                Vec::new(),
            ))
        }
        AssociatedFunctionMethodReferenceProjection::AlwaysTrue => "true".to_string(),
        AssociatedFunctionMethodReferenceProjection::StaticFunction => {
            emit::emit_expr(&RustExpr::Call {
                function: Box::new(owner_associated_path_projection(owner, method)),
                args: vec![RustExpr::Raw(receiver.to_string())],
            })
        }
    }
}

pub fn outer_this_field_projection(field_name: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "self.__outer.borrow().{}",
        crate::parser::to_snake_case(field_name)
    ))
}

pub fn outer_this_field_mut_projection(field_name: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "self.__outer.borrow_mut().{}",
        crate::parser::to_snake_case(field_name)
    ))
}

pub fn self_field_projection(field_name: &str) -> RustExpr {
    field_projection(
        RustExpr::Var("self".to_string()),
        &crate::parser::to_snake_case(field_name),
    )
}

pub fn field_projection(receiver: RustExpr, field_name: &str) -> RustExpr {
    RustExpr::Field(Box::new(receiver), field_name.to_string())
}

pub fn char_sequence_collect_string_projection(chars: RustExpr) -> RustExpr {
    if !matches!(chars.unshaped(), RustExpr::Raw(_)) {
        return typed_collect_projection(zero_arg_method_projection(chars, "iter"), "String");
    }
    RustExpr::Raw(format!(
        "{}.iter().collect::<String>()",
        emit::emit_expr(&chars)
    ))
}

pub fn rendered_vec_macro_body(rendered: &str) -> Option<String> {
    let trimmed = rendered.trim();
    trimmed
        .strip_prefix("vec![")
        .and_then(|body| body.strip_suffix(']'))
        .map(str::to_string)
}

pub fn borrow_as_dyn_trait(expr: RustExpr, trait_name: &str) -> RustExpr {
    let trait_name = normalize_emitted_java_type_name(trait_name);
    RustExpr::shaped(
        RustExprShape::Borrowed,
        RustExpr::Raw(format!("&{} as &dyn {trait_name}", emit::emit_expr(&expr))),
    )
}

fn borrow_boxed_as_dyn_trait(expr: RustExpr, trait_name: &str) -> RustExpr {
    let trait_name = normalize_emitted_java_type_name(trait_name);
    RustExpr::shaped(
        RustExprShape::Borrowed,
        RustExpr::Raw(format!(
            "{}.as_ref() as &dyn {trait_name}",
            emit::emit_expr(&expr)
        )),
    )
}

pub fn anonymous_dyn_trait_projection(trait_name: &str) -> RustExpr {
    let trait_name = normalize_emitted_java_type_name(trait_name);
    RustExpr::shaped(
        RustExprShape::AnonymousDynTrait,
        RustExpr::Raw(format!("<dyn {trait_name}>::anonymous()")),
    )
}

pub fn unwrap_optional_shared_value(expr: RustExpr) -> RustExpr {
    zero_arg_method_projection(zero_arg_method_projection(expr, "as_ref"), "unwrap")
}

pub fn unwrap_java_null_receiver_projection(expr: RustExpr) -> RustExpr {
    method_expect_projection(expr, "Java null receiver")
}

pub fn typed_none_projection(rust_ty: &str) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OptionalConstructed,
        RustExpr::Raw(format!("None::<{rust_ty}>")),
    )
}

pub fn optional_get_owned_value_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        method_expect_projection(
            optional_owned_clone_map_projection(receiver),
            "Java null receiver",
        ),
    )
}

pub fn optional_or_else_owned_value_projection(receiver: RustExpr, fallback: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        method_call_projection(
            optional_owned_clone_map_projection(receiver),
            "unwrap_or",
            vec![fallback],
        ),
    )
}

fn optional_owned_clone_map_projection(receiver: RustExpr) -> RustExpr {
    method_call_projection(
        zero_arg_method_projection(receiver, "as_ref"),
        "map",
        vec![RustExpr::Raw("|value| (*value).clone()".to_string())],
    )
}

pub fn rust_expr_mentions_constructor_self_temporary(expr: &RustExpr) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => rust_expr_mentions_constructor_self_temporary(expr),
        RustExpr::MarkedBlock { expr, .. } => rust_expr_mentions_constructor_self_temporary(expr),
        RustExpr::Var(name) => name == "__java_self",
        RustExpr::Path(_) => false,
        RustExpr::Raw(rendered) => raw_mentions_identifier(rendered, "__java_self"),
        RustExpr::Field(receiver, _) => rust_expr_mentions_constructor_self_temporary(receiver),
        RustExpr::Index(target, index) => {
            rust_expr_mentions_constructor_self_temporary(target)
                || rust_expr_mentions_constructor_self_temporary(index)
        }
        RustExpr::Binary { left, right, .. } => {
            rust_expr_mentions_constructor_self_temporary(left)
                || rust_expr_mentions_constructor_self_temporary(right)
        }
        RustExpr::Unary { expr, .. } | RustExpr::Cast { expr, .. } => {
            rust_expr_mentions_constructor_self_temporary(expr)
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            rust_expr_mentions_constructor_self_temporary(condition)
                || rust_expr_mentions_constructor_self_temporary(then_expr)
                || rust_expr_mentions_constructor_self_temporary(else_expr)
        }
        RustExpr::Tuple(items) | RustExpr::ArrayLiteral(items) => items
            .iter()
            .any(rust_expr_mentions_constructor_self_temporary),
        RustExpr::VecLiteral(items) => items
            .iter()
            .any(rust_expr_mentions_constructor_self_temporary),
        RustExpr::VecRepeat { value, len } => {
            rust_expr_mentions_constructor_self_temporary(value)
                || rust_expr_mentions_constructor_self_temporary(len)
        }
        RustExpr::Call { function, args } => {
            rust_expr_mentions_constructor_self_temporary(function)
                || args
                    .iter()
                    .any(rust_expr_mentions_constructor_self_temporary)
        }
        RustExpr::MethodCall { receiver, args, .. } => {
            rust_expr_mentions_constructor_self_temporary(receiver)
                || args
                    .iter()
                    .any(rust_expr_mentions_constructor_self_temporary)
        }
        RustExpr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, value)| rust_expr_mentions_constructor_self_temporary(value)),
        RustExpr::Literal(_) => false,
    }
}

fn raw_mentions_identifier(raw: &str, ident: &str) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = 0usize;
    for (idx, ch) in raw.char_indices() {
        if idx < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ if raw[idx..].starts_with(ident)
                && raw_identifier_boundary(raw[..idx].chars().next_back())
                && raw_identifier_boundary(raw[idx + ident.len()..].chars().next()) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn raw_mentions_call(raw: &str, function: &str) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = 0usize;
    for (idx, ch) in raw.char_indices() {
        if idx < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ if raw[idx..].starts_with(function)
                && raw_identifier_boundary(raw[..idx].chars().next_back())
                && raw_identifier_boundary(raw[idx + function.len()..].chars().next())
                && raw[idx + function.len()..].trim_start().starts_with('(') =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn raw_contains_outside_literals(raw: &str, pattern: &str) -> bool {
    raw_find_outside_literals(raw, pattern).is_some()
}

fn raw_find_outside_literals(raw: &str, pattern: &str) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = 0usize;
    for (idx, ch) in raw.char_indices() {
        if idx < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ if raw[idx..].starts_with(pattern) => return Some(idx),
            _ => {}
        }
    }
    None
}

fn raw_replace_once_outside_literals(
    raw: &str,
    pattern: &str,
    replacement: &str,
) -> Option<String> {
    let index = raw_find_outside_literals(raw, pattern)?;
    Some(format!(
        "{}{}{}",
        &raw[..index],
        replacement,
        &raw[index + pattern.len()..]
    ))
}

fn raw_identifier_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
}

fn raw_string_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if first != 'r' {
        return None;
    }
    let mut hashes = 0usize;
    let mut quote_start = None;
    for (offset, ch) in chars {
        match ch {
            '#' if quote_start.is_none() => hashes += 1,
            '"' if quote_start.is_none() => {
                quote_start = Some(offset);
                break;
            }
            _ => return None,
        }
    }
    let quote_start = quote_start?;
    let body_start = idx + quote_start + 1;
    let terminator = format!("\"{}", "#".repeat(hashes));
    raw[body_start..]
        .find(&terminator)
        .map(|offset| body_start + offset + terminator.len())
}

pub fn constructor_self_field_replacement(replacement: &str) -> String {
    replacement
        .strip_suffix(".clone()")
        .unwrap_or(replacement)
        .to_string()
}

pub fn constructor_self_field_projection(field_name: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "__java_self.{}",
        rust_field_projection_ident(field_name)
    ))
}

fn rust_field_projection_ident(name: &str) -> String {
    if name.starts_with("r#") || name.contains("::") || name.is_empty() {
        return name.to_string();
    }
    if rust_projection_ident_needs_raw(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn rust_projection_ident_needs_raw(name: &str) -> bool {
    matches!(
        name,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

pub fn literal_slot_plan(literal: &Literal, slot: &Slot) -> Option<LiteralSlotPlan> {
    let target = slot_target_shape(slot)?;
    if let Literal::String(value) = literal
        && target.java_ty == JavaType::String
        && target.view == ViewShape::Str
        && target.ownership == OwnershipShape::SharedBorrow
    {
        return Some(LiteralSlotPlan::BorrowedStringView {
            value: value.clone(),
        });
    }
    if matches!(target.java_ty, JavaType::Primitive(PrimitiveType::Char))
        && let Some(ch) = java_integral_literal_char(literal)
    {
        return Some(LiteralSlotPlan::NumericCharCode { ch });
    }
    None
}

pub fn apply_literal_slot_plan(plan: &LiteralSlotPlan) -> RustExpr {
    match plan {
        LiteralSlotPlan::BorrowedStringView { value } => {
            string_view_expr(RustExpr::Literal(format!("{value:?}")))
        }
        LiteralSlotPlan::NumericCharCode { ch } => RustExpr::Literal(format!("{ch:?}")),
    }
}

pub fn java_integral_literal_char(literal: &Literal) -> Option<char> {
    let value = match literal {
        Literal::Int(value) => i64::from(*value),
        Literal::Long(value) => *value,
        _ => return None,
    };
    let value = u32::try_from(value).ok()?;
    if (0xd800..=0xdfff).contains(&value) {
        return None;
    }
    char::from_u32(value)
}

fn slot_target_shape(slot: &Slot) -> Option<&RustSlotShape> {
    match slot {
        Slot::Assignment(shape) | Slot::Return(shape) | Slot::Argument(shape) => Some(shape),
        _ => None,
    }
}

pub fn collapse_redundant_shared_borrow_projection(expr: RustExpr, base: &str) -> RustExpr {
    match expr {
        RustExpr::Unary { op, expr }
            if op == "&" && matches!(expr.as_ref(), RustExpr::Var(name) if name == base) =>
        {
            RustExpr::Var(base.to_string())
        }
        RustExpr::Raw(rendered) if rendered == format!("&{base}") => {
            RustExpr::Var(base.to_string())
        }
        other => other,
    }
}

pub fn normalize_map_key_borrow_projection(
    expr: RustExpr,
    local_name: Option<&str>,
    actual: Option<&RustSlotShape>,
) -> RustExpr {
    let expr_was_raw = matches!(expr.unshaped(), RustExpr::Raw(_));
    let expr = if actual.is_some_and(|shape| shape.ownership == OwnershipShape::SharedBorrow) {
        if let Some(name) = local_name {
            collapse_redundant_shared_borrow_projection(expr, name)
        } else {
            expr
        }
    } else {
        expr
    };
    if let (Some(name), Some(shape)) = (local_name, actual)
        && shape.java_ty == JavaType::String
        && shape.ownership == OwnershipShape::SharedBorrow
        && shape.view == ViewShape::Native
        && rust_expr_is_local_name(&expr, name)
    {
        if expr_was_raw {
            return string_view_expr(RustExpr::Raw(format!("{name}.as_str()")));
        }
        return string_view_expr(zero_arg_method_projection(expr, "as_str"));
    }
    let actual_for_projection = if actual.is_some_and(|shape| {
        matches!(
            shape.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
    }) {
        actual
    } else {
        local_name
            .filter(|name| rust_expr_is_local_name(&expr, name))
            .and(actual)
    };
    ensure_borrow_projection(
        expr,
        actual_for_projection,
        None,
        BorrowProjectionUse::MapKey,
    )
}

fn rust_expr_is_local_name(expr: &RustExpr, name: &str) -> bool {
    match expr {
        RustExpr::Var(var) => var == name,
        RustExpr::Raw(raw) => raw == name,
        _ => false,
    }
}

pub fn render_index_to_usize_projection(
    rendered: &str,
    projection: IndexToUsizeProjection,
) -> String {
    if rendered.ends_with(" as usize") {
        return rendered.to_string();
    }
    match projection {
        IndexToUsizeProjection::JavaStringCharAt
            if rendered.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            format!("{rendered} as usize")
        }
        IndexToUsizeProjection::StreamLambdaArgument
            if !rendered.starts_with('*') && !rendered.contains(' ') =>
        {
            format!("{rendered} as usize")
        }
        _ => format!("({rendered}) as usize"),
    }
}

pub fn render_indexed_access_projection(
    array: &str,
    index: &str,
    projection: IndexToUsizeProjection,
) -> String {
    format!(
        "{array}[{}]",
        render_index_to_usize_projection(index, projection)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderedMutableAliasProjection {
    DirectMutableBorrow,
    MutableSlice,
    OptionalMutableReceiver,
    OptionalDerefMut,
    NestedMutableSlice,
}

pub fn rendered_expr_is_mutable_alias(arg: &RustExpr, base: &str) -> bool {
    if rust_expr_mutable_alias_projection(arg, base).is_some() {
        return true;
    }
    let rendered = emit::emit_expr(arg);
    rendered_mutable_alias_projection(&rendered, base).is_some()
}

fn rust_expr_mutable_alias_projection(
    expr: &RustExpr,
    base: &str,
) -> Option<RenderedMutableAliasProjection> {
    match expr.unshaped() {
        RustExpr::Unary { op, expr } if op == "&mut " && rust_expr_is_local_name(expr, base) => {
            Some(RenderedMutableAliasProjection::DirectMutableBorrow)
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty() && rust_expr_receiver_starts_with_base(receiver, base) => {
            match method.as_str() {
                "as_mut_slice" if rust_expr_is_local_name(receiver, base) => {
                    Some(RenderedMutableAliasProjection::MutableSlice)
                }
                "as_mut_slice" => Some(RenderedMutableAliasProjection::NestedMutableSlice),
                "as_deref_mut" => Some(RenderedMutableAliasProjection::OptionalDerefMut),
                _ => None,
            }
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "expect"
            && args == &[RustExpr::Literal("\"Java null receiver\"".to_string())]
            && matches!(
                receiver.unshaped(),
                RustExpr::MethodCall {
                    method,
                    args,
                    ..
                } if method == "as_mut" && args.is_empty()
            )
            && rust_expr_receiver_starts_with_base(receiver, base) =>
        {
            Some(RenderedMutableAliasProjection::OptionalMutableReceiver)
        }
        _ => None,
    }
}

fn rust_expr_receiver_starts_with_base(expr: &RustExpr, base: &str) -> bool {
    match expr.unshaped() {
        RustExpr::Var(name) => name == base,
        RustExpr::Field(receiver, _) | RustExpr::Index(receiver, _) => {
            rust_expr_receiver_starts_with_base(receiver, base)
        }
        RustExpr::MethodCall { receiver, .. } => {
            rust_expr_receiver_starts_with_base(receiver, base)
        }
        RustExpr::Shaped { .. } => unreachable!("unshaped removes shape tags"),
        _ => false,
    }
}

pub fn rendered_mutable_alias_projection(
    rendered: &str,
    base: &str,
) -> Option<RenderedMutableAliasProjection> {
    if rendered == format!("&mut {base}") {
        return Some(RenderedMutableAliasProjection::DirectMutableBorrow);
    }
    if rendered == format!("{base}.as_mut_slice()") {
        return Some(RenderedMutableAliasProjection::MutableSlice);
    }
    if !rendered.starts_with(&format!("{base}.")) {
        return None;
    }
    if rendered.ends_with(".as_mut().expect(\"Java null receiver\")") {
        return Some(RenderedMutableAliasProjection::OptionalMutableReceiver);
    }
    if rendered.ends_with(".as_deref_mut()") {
        return Some(RenderedMutableAliasProjection::OptionalDerefMut);
    }
    if rendered.ends_with(".as_mut_slice()") {
        return Some(RenderedMutableAliasProjection::NestedMutableSlice);
    }
    None
}

pub fn reborrow_optional_mut_projection(
    expr: &RustExpr,
    target_shape: &RustSlotShape,
) -> Option<RustExpr> {
    let target_element_ty = target_shape.optional_element_type()?;
    let target_element =
        crate::conversion_plan::optional_element_slot_shape(target_shape, target_element_ty);
    if expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &target_element))
    {
        return Some(expr.clone());
    }
    let rendered = emit::emit_expr(expr);
    if rendered.ends_with(".as_deref_mut()") {
        return Some(RustExpr::shaped_as(
            RustExprShape::OptionalProjected,
            Some(target_element),
            expr.clone(),
        ));
    }
    Some(RustExpr::shaped_as(
        RustExprShape::OptionalProjected,
        Some(target_element),
        zero_arg_method_projection(expr.clone(), "as_deref_mut"),
    ))
}

pub fn clone_erased_object_ref_once(expr: RustExpr) -> RustExpr {
    clone_erased_object_helper_once(expr, "java_clone_erased_object_ref")
}

fn clone_erased_object_helper_once(expr: RustExpr, helper: &str) -> RustExpr {
    if rust_expr_is_erased_object_clone_helper_call(&expr) {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let rendered = emit::emit_expr(&expr);
    if raw_mentions_call(&rendered, "java_clone_erased_object_ref")
        || raw_mentions_call(&rendered, "java_clone_erased_object_vec_ref")
    {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Call {
                function: Box::new(RustExpr::path([helper])),
                args: vec![borrowed_erased_object_clone_arg(expr)],
            },
        );
    }
    let borrow_operand = rendered.strip_prefix('&').unwrap_or(&rendered);
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!("{helper}(&{borrow_operand})")),
    )
}

fn rust_expr_is_erased_object_clone_helper_call(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Call { function, args } => {
            args.len() == 1 && rust_expr_is_erased_object_clone_helper(function)
        }
        _ => false,
    }
}

fn rust_expr_is_erased_object_clone_helper(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Var(name) => {
            matches!(
                name.as_str(),
                "java_clone_erased_object_ref" | "java_clone_erased_object_vec_ref"
            )
        }
        RustExpr::Path(segments) if segments.len() == 1 => {
            matches!(
                segments[0].as_str(),
                "java_clone_erased_object_ref" | "java_clone_erased_object_vec_ref"
            )
        }
        RustExpr::Raw(raw) => {
            raw == "java_clone_erased_object_ref" || raw == "java_clone_erased_object_vec_ref"
        }
        _ => false,
    }
}

fn borrowed_erased_object_clone_arg(expr: RustExpr) -> RustExpr {
    if matches!(expr.unshaped(), RustExpr::Unary { op, .. } if op == "&") {
        return expr;
    }
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(expr),
    }
}

pub fn optional_clone_recovery_unwrap_replacement(name: &str, erased_object: bool) -> String {
    if erased_object {
        format!(
            "{name}.as_ref().map(|value| java_clone_erased_object_ref(value)){}\
             .expect(\"Java null receiver\")",
            render_clone_recovery_policy(&CloneRecoveryPolicy::AuditRequired)
        )
    } else {
        format!(
            "{name}.as_ref().cloned(){}.expect(\"Java null receiver\")",
            render_clone_recovery_policy(&CloneRecoveryPolicy::AuditRequired)
        )
    }
}

pub fn replace_optional_clone_recovery_unwrap(
    rendered: &str,
    name: &str,
    clone_before_unwrap: bool,
    erased_object: bool,
) -> Option<String> {
    let unwrap_expr = if clone_before_unwrap {
        format!("{name}.clone().unwrap()")
    } else {
        format!("{name}.unwrap()")
    };
    raw_replace_once_outside_literals(
        rendered,
        &unwrap_expr,
        &optional_clone_recovery_unwrap_replacement(name, erased_object),
    )
}

pub fn rendered_expr_has_optional_unwrap_or_java_null_expect(rendered: &str) -> bool {
    raw_contains_outside_literals(rendered, ".unwrap()")
        || raw_contains_outside_literals(rendered, ".expect(\"Java null receiver\")")
}

pub fn optional_clone_recovery_owned_element_projection(
    option_expr: RustExpr,
    erased_object: bool,
) -> RustExpr {
    let rendered = emit::emit_expr(&option_expr);
    if erased_object {
        RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{rendered}.as_ref().map(|value| java_clone_erased_object_ref(value)){}\
                 .expect(\"Java null receiver\")",
                render_clone_recovery_policy(&CloneRecoveryPolicy::AuditRequired)
            )),
        )
    } else {
        RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{rendered}.as_ref().cloned(){}.expect(\"Java null receiver\")",
                render_clone_recovery_policy(&CloneRecoveryPolicy::AuditRequired)
            )),
        )
    }
}

pub fn render_parse_receiver(expr: RustExpr) -> String {
    let rendered = emit::emit_expr(&expr);
    let Some(stripped) = rendered.trim().strip_prefix('&') else {
        return rendered;
    };
    let stripped = stripped.trim();
    if stripped.starts_with('(') && stripped.ends_with(')') {
        stripped.to_string()
    } else if stripped
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || stripped.starts_with("self.")
        || stripped.contains(".as_str()")
        || stripped.contains(".next_token()")
    {
        stripped.to_string()
    } else {
        format!("({stripped})")
    }
}

pub fn primitive_parse_projection(arg: RustExpr, target: PrimitiveType) -> RustExpr {
    let rust_ty = match target {
        PrimitiveType::Int => "i32",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
        PrimitiveType::Byte => "i8",
        PrimitiveType::Short => "i16",
        PrimitiveType::Long => "i64",
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Char => {
            return crate::unsupported::compile_error_expr(
                "Java Character parsing is not a primitive parse projection",
            );
        }
    };
    if matches!(arg.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::Raw(format!(
            "{}.parse::<{rust_ty}>().unwrap()",
            render_parse_receiver(arg)
        ));
    }
    let receiver = match arg {
        RustExpr::Unary { op, expr } if op == "&" => *expr,
        _ => arg,
    };
    zero_arg_method_projection(
        zero_arg_method_projection(receiver, &format!("parse::<{rust_ty}>")),
        "unwrap",
    )
}

pub fn string_literal_source(expr: &RustExpr) -> Option<String> {
    match expr.unshaped() {
        RustExpr::Literal(raw) => rust_string_literal_token_source(raw),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "to_string" && args.is_empty() => match receiver.unshaped() {
            RustExpr::Literal(raw) => rust_string_literal_token_source(raw),
            _ => None,
        },
        _ => None,
    }
}

fn rust_string_literal_token_source(raw: &str) -> Option<String> {
    let raw = raw.strip_suffix(".to_string()").unwrap_or(raw);
    if raw.starts_with('"') && raw.ends_with('"') {
        Some(raw[1..raw.len() - 1].to_string())
    } else {
        None
    }
}

pub fn render_string_join_vararg_value(value: RustExpr) -> String {
    if let Some(literal) = string_literal_source(&value) {
        return format!("{literal:?}.to_string()");
    }
    let rendered = emit::emit_expr(&value);
    if rendered.ends_with(".to_string()") {
        rendered
    } else {
        format!("({rendered}).to_string()")
    }
}

pub fn string_join_delimiter_projection(delimiter: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_source(&delimiter) {
        RustExpr::Literal(format!("{literal:?}"))
    } else {
        delimiter
    }
}

pub fn string_join_varargs_projection(delimiter: RustExpr, values: Vec<RustExpr>) -> RustExpr {
    let values = values
        .into_iter()
        .map(render_string_join_vararg_value)
        .collect::<Vec<_>>()
        .join(", ");
    RustExpr::Raw(format!(
        "vec![{values}].join({})",
        emit::emit_expr(&string_join_delimiter_projection(delimiter))
    ))
}

pub fn render_string_equality_operand_projection(
    expr: RustExpr,
    already_string_view: bool,
) -> String {
    let rendered = emit::emit_expr(&expr);
    if already_string_view {
        rendered
    } else {
        format!("{rendered}.as_str()")
    }
}

pub fn optional_string_view_projection(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OptionalProjected,
        optional_string_view_projection_expr(expr),
    )
}

fn optional_string_view_projection_expr(expr: RustExpr) -> RustExpr {
    if matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::Raw(format!(
            "{}.as_ref().map(|value| value.as_str())",
            emit::emit_expr(&expr)
        ));
    }
    RustExpr::MethodCall {
        receiver: Box::new(zero_arg_method_projection(expr, "as_ref")),
        method: "map".to_string(),
        args: vec![RustExpr::Raw("|value| value.as_str()".to_string())],
    }
}

pub fn string_helper_receiver_projection(receiver: RustExpr) -> RustExpr {
    if expr_is_string_view(&receiver) {
        return receiver;
    }
    if expr_is_as_ref_unwrap_projection(&receiver) {
        return string_view_expr(zero_arg_method_projection(receiver, "as_str"));
    }
    let rendered = emit::emit_expr(&receiver);
    if rendered.ends_with(".as_ref().unwrap()") {
        string_view_expr(RustExpr::Raw(format!("{rendered}.as_str()")))
    } else if matches!(receiver.unshaped(), RustExpr::Raw(_)) {
        string_view_expr(RustExpr::Raw(format!("&{rendered}")))
    } else {
        string_view_expr(force_shared_borrow_projection(receiver))
    }
}

fn expr_is_as_ref_unwrap_projection(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "unwrap" && args.is_empty() => match receiver.unshaped() {
            RustExpr::MethodCall { method, args, .. } => method == "as_ref" && args.is_empty(),
            _ => false,
        },
        _ => false,
    }
}

pub fn string_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_hash_code"])),
        args: vec![string_helper_receiver_projection(receiver)],
    }
}

pub fn java_list_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_list_hash_code"])),
        args: vec![force_shared_borrow_projection(receiver)],
    }
}

pub fn java_set_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_set_hash_code"])),
        args: vec![force_shared_borrow_projection(receiver)],
    }
}

pub fn java_map_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_map_hash_code"])),
        args: vec![force_shared_borrow_projection(receiver)],
    }
}

pub fn java_value_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_value_hash_code"])),
        args: vec![force_shared_borrow_projection(receiver)],
    }
}

pub fn string_replace_first_projection(
    receiver: RustExpr,
    regex: RustExpr,
    replacement: RustExpr,
) -> RustExpr {
    string_regex_replace_projection("java_string_replace_first", receiver, regex, replacement)
}

pub fn string_replace_all_projection(
    receiver: RustExpr,
    regex: RustExpr,
    replacement: RustExpr,
) -> RustExpr {
    string_regex_replace_projection("java_string_replace_all", receiver, regex, replacement)
}

fn string_regex_replace_projection(
    function: &str,
    receiver: RustExpr,
    regex: RustExpr,
    replacement: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path([function])),
        args: vec![
            string_helper_receiver_projection(receiver),
            regex,
            replacement,
        ],
    }
}

pub fn string_matches_projection(receiver: RustExpr, pattern: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_matches"])),
        args: vec![string_helper_receiver_projection(receiver), pattern],
    }
}

pub fn string_split_projection(receiver: RustExpr, regex: RustExpr, limit: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_split"])),
        args: vec![string_helper_receiver_projection(receiver), regex, limit],
    }
}

pub fn string_from_bytes_projection(bytes: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_from_bytes"])),
        args: vec![force_shared_borrow_projection(bytes)],
    }
}

pub fn string_from_byte_range_projection(
    bytes: RustExpr,
    offset: RustExpr,
    len: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_from_byte_range"])),
        args: vec![force_shared_borrow_projection(bytes), offset, len],
    }
}

pub fn string_compare_to_projection(receiver: RustExpr, other: RustExpr) -> RustExpr {
    string_one_arg_runtime_helper_projection("java_string_compare_to", receiver, other)
}

pub fn string_compare_to_ignore_case_projection(receiver: RustExpr, other: RustExpr) -> RustExpr {
    string_one_arg_runtime_helper_projection("java_string_compare_to_ignore_case", receiver, other)
}

pub fn string_equals_ignore_case_projection(receiver: RustExpr, other: RustExpr) -> RustExpr {
    string_one_arg_runtime_helper_projection("java_string_equals_ignore_case", receiver, other)
}

fn string_one_arg_runtime_helper_projection(
    function: &str,
    receiver: RustExpr,
    other: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path([function])),
        args: vec![string_helper_receiver_projection(receiver), other],
    }
}

pub fn string_get_bytes_projection(receiver: RustExpr) -> RustExpr {
    typed_collect_projection(
        RustExpr::MethodCall {
            receiver: Box::new(zero_arg_method_projection(
                zero_arg_method_projection(receiver, "as_bytes"),
                "iter",
            )),
            method: "map".to_string(),
            args: vec![RustExpr::Raw("|byte| *byte as i8".to_string())],
        },
        "Vec<i8>",
    )
}

pub fn string_to_char_array_projection(receiver: RustExpr) -> RustExpr {
    typed_collect_projection(zero_arg_method_projection(receiver, "chars"), "Vec<char>")
}

pub fn sequence_index_of_projection(receiver: RustExpr, needle: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(zero_arg_method_projection(
                    parenthesize_raw_method_receiver(receiver),
                    "iter",
                )),
                method: "position".to_string(),
                args: vec![RustExpr::Raw(format!(
                    "|item| item == {}",
                    emit::emit_expr(&needle)
                ))],
            }),
            method: "map".to_string(),
            args: vec![RustExpr::Raw("|index| index as i32".to_string())],
        }),
        method: "unwrap_or".to_string(),
        args: vec![RustExpr::Literal("-1".to_string())],
    }
}

pub fn string_char_at_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    let index = RustExpr::Raw(render_index_to_usize_projection(
        &emit::emit_expr(&index),
        IndexToUsizeProjection::JavaStringCharAt,
    ));
    method_expect_projection(
        RustExpr::MethodCall {
            receiver: Box::new(zero_arg_method_projection(
                parenthesize_raw_method_receiver(receiver),
                "chars",
            )),
            method: "nth".to_string(),
            args: vec![index],
        },
        "Java string index out of bounds",
    )
}

fn parenthesize_raw_method_receiver(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if balanced_outer_parentheses_wrap_all(trimmed)
        || (!raw_contains_top_level_binary_operator(trimmed)
            && !raw_contains_top_level_as_cast(trimmed)
            && !trimmed.starts_with('*'))
    {
        RustExpr::Raw(raw)
    } else {
        RustExpr::Raw(format!("({trimmed})"))
    }
}

pub fn string_index_of_projection(
    receiver: RustExpr,
    needle: RustExpr,
    from_index: Option<RustExpr>,
) -> RustExpr {
    let receiver = string_view_argument_projection(receiver);
    let needle = string_view_argument_projection(string_search_needle_projection(needle));
    if let Some(from_index) = from_index {
        return RustExpr::Call {
            function: Box::new(RustExpr::path(["java_string_index_of_from"])),
            args: vec![receiver, needle, from_index],
        };
    }
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_index_of"])),
        args: vec![receiver, needle],
    }
}

pub fn string_last_index_of_projection(
    receiver: RustExpr,
    needle: RustExpr,
    from_index: Option<RustExpr>,
) -> RustExpr {
    let receiver = string_view_argument_projection(receiver);
    let needle = string_view_argument_projection(string_search_needle_projection(needle));
    if let Some(from_index) = from_index {
        return RustExpr::Call {
            function: Box::new(RustExpr::path(["java_string_last_index_of_from"])),
            args: vec![receiver, needle, from_index],
        };
    }
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_last_index_of"])),
        args: vec![receiver, needle],
    }
}

fn string_search_needle_projection(needle: RustExpr) -> RustExpr {
    let rendered = emit::emit_expr(&needle);
    if let Some(literal) = string_literal_from_rust_char_literal(&rendered) {
        RustExpr::Literal(literal)
    } else {
        needle
    }
}

fn string_literal_from_rust_char_literal(rendered: &str) -> Option<String> {
    let trimmed = rendered.trim();
    let literal = trimmed.strip_prefix('&').unwrap_or(trimmed);
    let inner = literal.strip_prefix('\'')?.strip_suffix('\'')?;
    if inner.is_empty() {
        return None;
    }
    let string_inner = if inner == "\\'" { "'" } else { inner };
    Some(format!("\"{string_inner}\""))
}

pub fn string_substring_projection(receiver: RustExpr, begin: RustExpr, end: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_string_substring"])),
        args: vec![string_view_argument_projection(receiver), begin, end],
    }
}

pub fn primitive_number_value_projection(receiver: RustExpr, target: PrimitiveType) -> RustExpr {
    let rust_ty = match target {
        PrimitiveType::Byte => "i8",
        PrimitiveType::Short => "i16",
        PrimitiveType::Int => "i32",
        PrimitiveType::Long => "i64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
        PrimitiveType::Boolean | PrimitiveType::Char => return receiver,
    };
    RustExpr::Cast {
        expr: Box::new(parenthesize_raw_primitive_number_cast_operand(receiver)),
        ty: rust_ty.to_string(),
    }
}

pub fn java_char_from_u32_mask_projection(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Call {
            function: Box::new(RustExpr::path(["char", "from_u32"])),
            args: vec![RustExpr::Binary {
                op: "&".to_string(),
                left: Box::new(RustExpr::Cast {
                    expr: Box::new(parenthesize_raw_primitive_number_cast_operand(expr)),
                    ty: "u32".to_string(),
                }),
                right: Box::new(RustExpr::Literal("0xffff".to_string())),
            }],
        }),
        method: "unwrap_or".to_string(),
        args: vec![RustExpr::Literal("'\\0'".to_string())],
    }
}

pub fn java_char_to_numeric_projection(expr: RustExpr, target: PrimitiveType) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(RustExpr::Cast {
            expr: Box::new(expr),
            ty: "u32".to_string(),
        }),
        ty: rust_primitive_type_name_policy(target)
            .rust_name
            .to_string(),
    }
}

fn parenthesize_raw_primitive_number_cast_operand(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if balanced_outer_parentheses_wrap_all(trimmed)
        || (!raw_contains_top_level_binary_operator(trimmed)
            && !raw_contains_top_level_as_cast(trimmed))
    {
        RustExpr::Raw(raw)
    } else {
        RustExpr::Raw(format!("({trimmed})"))
    }
}

pub fn facade_receiver_method_call_projection(
    receiver: RustExpr,
    rust_path: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    receiver_method_call_projection(receiver, &facade_method_name(rust_path), args)
}

pub fn facade_method_name(rust_path: &str) -> String {
    rust_path
        .rsplit("::")
        .next()
        .unwrap_or(rust_path)
        .to_string()
}

pub fn primitive_hash_code_projection(receiver: RustExpr, source: PrimitiveType) -> RustExpr {
    if !matches!(receiver.unshaped(), RustExpr::Raw(_)) {
        match source {
            PrimitiveType::Boolean => {
                return RustExpr::If {
                    condition: Box::new(receiver),
                    then_expr: Box::new(RustExpr::Literal("1231".to_string())),
                    else_expr: Box::new(RustExpr::Literal("1237".to_string())),
                };
            }
            PrimitiveType::Byte
            | PrimitiveType::Short
            | PrimitiveType::Char
            | PrimitiveType::Int => {
                return RustExpr::Cast {
                    expr: Box::new(receiver),
                    ty: "i32".to_string(),
                };
            }
            PrimitiveType::Float => {
                return RustExpr::Cast {
                    expr: Box::new(zero_arg_method_projection(receiver, "to_bits")),
                    ty: "i32".to_string(),
                };
            }
            PrimitiveType::Long => {
                let wide = RustExpr::Cast {
                    expr: Box::new(receiver.clone()),
                    ty: "i64".to_string(),
                };
                return RustExpr::Cast {
                    expr: Box::new(RustExpr::Binary {
                        op: "^".to_string(),
                        left: Box::new(wide.clone()),
                        right: Box::new(RustExpr::Binary {
                            op: ">>".to_string(),
                            left: Box::new(wide),
                            right: Box::new(RustExpr::Literal("32".to_string())),
                        }),
                    }),
                    ty: "i32".to_string(),
                };
            }
            PrimitiveType::Double => {
                let bits = zero_arg_method_projection(receiver.clone(), "to_bits");
                return RustExpr::Cast {
                    expr: Box::new(RustExpr::Binary {
                        op: "^".to_string(),
                        left: Box::new(bits.clone()),
                        right: Box::new(RustExpr::Binary {
                            op: ">>".to_string(),
                            left: Box::new(bits),
                            right: Box::new(RustExpr::Literal("32".to_string())),
                        }),
                    }),
                    ty: "i32".to_string(),
                };
            }
        }
    }
    let rendered = emit::emit_expr(&receiver);
    match source {
        PrimitiveType::Boolean => {
            RustExpr::Raw(format!("if {rendered} {{ 1231 }} else {{ 1237 }}"))
        }
        PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Char | PrimitiveType::Int => {
            RustExpr::Raw(format!("{rendered} as i32"))
        }
        PrimitiveType::Long => RustExpr::Raw(format!(
            "(({rendered} as i64) ^ (({rendered} as i64) >> 32)) as i32"
        )),
        PrimitiveType::Float => RustExpr::Raw(format!("({rendered}).to_bits() as i32")),
        PrimitiveType::Double => RustExpr::Raw(format!(
            "(({rendered}).to_bits() ^ (({rendered}).to_bits() >> 32)) as i32"
        )),
    }
}

pub fn primitive_compare_to_projection(receiver: RustExpr, other: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_primitive_compare_to"])),
        args: vec![receiver, other],
    }
}

pub fn primitive_f64_method_projection(arg: RustExpr, method: &str) -> RustExpr {
    zero_arg_method_projection(f64_cast_projection(arg), method)
}

pub fn primitive_f64_binary_method_projection(
    left: RustExpr,
    right: RustExpr,
    method: &str,
) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(f64_cast_projection(left)),
        method: method.to_string(),
        args: vec![f64_cast_projection(right)],
    }
}

pub fn math_pow_projection(base: RustExpr, exponent: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(f64_cast_projection(base)),
        method: "powf".to_string(),
        args: vec![f64_cast_projection(exponent)],
    }
}

fn f64_cast_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(expr),
        ty: "f64".to_string(),
    }
}

pub fn math_round_projection(arg: RustExpr, source: PrimitiveType) -> RustExpr {
    let (helper, arg) = match source {
        PrimitiveType::Float => ("java_math_round_f32", arg),
        PrimitiveType::Double => ("java_math_round_f64", arg),
        _ => (
            "java_math_round_f64",
            RustExpr::Cast {
                expr: Box::new(arg),
                ty: "f64".to_string(),
            },
        ),
    };
    RustExpr::Call {
        function: Box::new(RustExpr::path([helper])),
        args: vec![arg],
    }
}

pub fn math_abs_projection(arg: RustExpr, source: PrimitiveType) -> RustExpr {
    let method = match source {
        PrimitiveType::Float | PrimitiveType::Double => "abs",
        _ => "wrapping_abs",
    };
    if matches!(arg.unshaped(), RustExpr::Raw(_)) {
        let rendered = emit::emit_expr(&arg);
        return receiver_method_call_projection_exact(
            RustExpr::Raw(format!("({rendered})")),
            method,
            Vec::new(),
        );
    }
    zero_arg_method_projection(arg, method)
}

pub fn math_floor_div_projection(
    left: RustExpr,
    right: RustExpr,
    primitive: PrimitiveType,
) -> RustExpr {
    math_floor_helper_call_projection("div", left, right, primitive)
}

pub fn math_floor_mod_projection(
    left: RustExpr,
    right: RustExpr,
    primitive: PrimitiveType,
    return_primitive: PrimitiveType,
) -> RustExpr {
    let call = math_floor_helper_call_projection("mod", left, right, primitive);
    if primitive == PrimitiveType::Long && return_primitive == PrimitiveType::Int {
        if matches!(call.unshaped(), RustExpr::Raw(_)) {
            RustExpr::Raw(format!("({}) as i32", emit::emit_expr(&call)))
        } else {
            RustExpr::Cast {
                expr: Box::new(call),
                ty: "i32".to_string(),
            }
        }
    } else {
        call
    }
}

fn math_floor_helper_call_projection(
    operation: &str,
    left: RustExpr,
    right: RustExpr,
    primitive: PrimitiveType,
) -> RustExpr {
    let (helper, cast_ty, suffix) = match primitive {
        PrimitiveType::Long => (format!("java_math_floor_{operation}_i64"), "i64", " as i64"),
        _ => (format!("java_math_floor_{operation}_i32"), "i32", " as i32"),
    };
    if !matches!(left.unshaped(), RustExpr::Raw(_)) && !matches!(right.unshaped(), RustExpr::Raw(_))
    {
        return RustExpr::Call {
            function: Box::new(RustExpr::path([helper])),
            args: vec![
                RustExpr::Cast {
                    expr: Box::new(left),
                    ty: cast_ty.to_string(),
                },
                RustExpr::Cast {
                    expr: Box::new(right),
                    ty: cast_ty.to_string(),
                },
            ],
        };
    }
    RustExpr::Raw(format!(
        "{helper}({}{suffix}, {}{suffix})",
        emit::emit_expr(&left),
        emit::emit_expr(&right)
    ))
}

pub fn len_i32_projection(expr: RustExpr) -> RustExpr {
    method_call_as_i32_projection(expr, "len")
}

pub fn array2_rows_i32_projection(expr: RustExpr) -> RustExpr {
    method_call_as_i32_projection(expr, "nrows")
}

pub fn array2_cols_i32_projection(expr: RustExpr) -> RustExpr {
    len_i32_projection(expr)
}

pub fn owned_map_keys_vec_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(
            zero_arg_method_projection(zero_arg_method_projection(receiver, "keys"), "cloned"),
            "Vec<_>",
        ),
    )
}

pub fn owned_map_keys_set_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(
            zero_arg_method_projection(zero_arg_method_projection(receiver, "keys"), "cloned"),
            "std::collections::HashSet<_>",
        ),
    )
}

pub fn owned_map_values_vec_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(
            zero_arg_method_projection(zero_arg_method_projection(receiver, "values"), "cloned"),
            "Vec<_>",
        ),
    )
}

pub fn owned_map_values_set_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(
            zero_arg_method_projection(zero_arg_method_projection(receiver, "values"), "cloned"),
            "std::collections::HashSet<_>",
        ),
    )
}

pub fn owned_collection_to_vec_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(sequence_iter_cloned_projection(receiver), "Vec<_>"),
    )
}

pub fn owned_collection_to_vec_projection_for_receiver_type(
    receiver: RustExpr,
    receiver_ty: Option<&JavaType>,
) -> RustExpr {
    if receiver_ty.is_some_and(raw_erased_object_list_carrier_type) {
        return erased_object_vec_clone_projection(receiver);
    }
    owned_collection_to_vec_projection(receiver)
}

fn raw_erased_object_list_carrier_type(java_ty: &JavaType) -> bool {
    let JavaType::Named { args, .. } = java_ty else {
        return false;
    };
    let reifiable_erasure = args.is_empty()
        || args
            .iter()
            .all(|arg| matches!(arg, JavaType::Wildcard(None)));
    reifiable_erasure
        && crate::stdlib::java_collection_kind_for_type(java_ty)
            == Some(crate::stdlib::JavaCollectionKind::List)
}

fn erased_object_vec_clone_projection(receiver: RustExpr) -> RustExpr {
    let receiver = collection_iteration_receiver(receiver);
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!(
            "{receiver}.iter().map(|value| java_clone_erased_object_ref(value)).collect::<Vec<_>>()",
        )),
    )
}

fn collection_iteration_receiver(receiver: RustExpr) -> String {
    let rendered = emit::emit_expr(&receiver);
    if rust_receiver_expr_needs_parentheses(&receiver, &rendered) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn typed_collect_projection(receiver: RustExpr, ty: &str) -> RustExpr {
    zero_arg_method_projection(receiver, &format!("collect::<{ty}>"))
}

pub fn java_iterator_from_collection_projection(receiver: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["JavaIterator", "new"]),
        vec![sequence_iter_cloned_projection(receiver)],
    )
}

pub fn sequence_iter_cloned_projection(receiver: RustExpr) -> RustExpr {
    zero_arg_method_projection(zero_arg_method_projection(receiver, "iter"), "cloned")
}

pub fn erased_object_sequence_iter_cloned_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.iter().map(|value| java_clone_erased_object_ref(value))",
        emit::emit_expr(&receiver)
    ))
}

pub fn sequence_iter_copied_projection(receiver: RustExpr) -> RustExpr {
    zero_arg_method_projection(zero_arg_method_projection(receiver, "iter"), "copied")
}

pub fn iterator_next_expect_projection(iterator: RustExpr) -> RustExpr {
    let next = zero_arg_method_projection(iterator, "next");
    if matches!(next.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::Raw(format!(
            "{}.expect(\"Java iterator exhausted\")",
            emit::emit_expr(&next)
        ));
    }
    RustExpr::MethodCall {
        receiver: Box::new(next),
        method: "expect".to_string(),
        args: vec![RustExpr::Literal("\"Java iterator exhausted\"".to_string())],
    }
}

pub fn tuple_field_projection(tuple: RustExpr, field_index: usize) -> RustExpr {
    RustExpr::Field(Box::new(tuple), field_index.to_string())
}

pub fn subtype_owned_constructor_from_borrowed_projection(
    target_ty: &str,
    borrowed: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(associated_rust_path_expr(target_ty, "new_1")),
        args: vec![zero_arg_method_projection(borrowed, "clone")],
    }
}

fn associated_rust_path_expr(owner: &str, member: &str) -> RustExpr {
    if owner
        .split("::")
        .chain(std::iter::once(member))
        .all(rust_path_segment_is_plain_ident)
    {
        let mut segments = owner.split("::").collect::<Vec<_>>();
        segments.push(member);
        RustExpr::path(segments)
    } else {
        RustExpr::Raw(format!("{owner}::{member}"))
    }
}

fn rust_path_segment_is_plain_ident(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub fn primitive_is_nan_projection(expr: RustExpr) -> RustExpr {
    zero_arg_method_projection(expr, "is_nan")
}

fn method_call_as_i32_projection(expr: RustExpr, method: &str) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(zero_arg_method_projection(expr, method)),
        ty: "i32".to_string(),
    }
}

fn zero_arg_method_projection(receiver: RustExpr, method: &str) -> RustExpr {
    method_call_projection(receiver, method, Vec::new())
}

fn method_call_projection(receiver: RustExpr, method: &str, args: Vec<RustExpr>) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: method.to_string(),
        args,
    }
}

fn method_expect_projection(expr: RustExpr, message: &str) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "expect".to_string(),
        args: vec![RustExpr::Literal(format!("{message:?}"))],
    }
}

fn strip_terminal_unwrap_projection(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Shaped { expr, .. } => strip_terminal_unwrap_projection(*expr),
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "unwrap" && args.is_empty() => *receiver,
        other => other,
    }
}

pub fn file_exists_projection(receiver: RustExpr) -> RustExpr {
    zero_arg_method_projection(
        RustExpr::Call {
            function: Box::new(RustExpr::path(["std", "path", "Path", "new"])),
            args: vec![zero_arg_method_projection(
                zero_arg_method_projection(receiver, "get_path"),
                "as_str",
            )],
        },
        "exists",
    )
}

pub fn system_exit_projection(status: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "process", "exit"])),
        args: vec![primitive_number_value_projection(
            status,
            PrimitiveType::Int,
        )],
    }
}

pub fn thread_sleep_millis_projection(millis: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "thread", "sleep"])),
        args: vec![RustExpr::Call {
            function: Box::new(RustExpr::path(["std", "time", "Duration", "from_millis"])),
            args: vec![RustExpr::Cast {
                expr: Box::new(millis),
                ty: "u64".to_string(),
            }],
        }],
    }
}

pub fn print_macro_projection(macro_name: &str, arg: Option<RustExpr>) -> RustExpr {
    let mut args = Vec::new();
    if let Some(arg) = arg {
        args.push(RustExpr::Literal("\"{}\"".to_string()));
        args.push(arg);
    }
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!("{macro_name}!"))),
        args,
    }
}

pub fn logical_not_projection(expr: RustExpr) -> RustExpr {
    let expr = match expr {
        RustExpr::Raw(raw) => RustExpr::Raw(format!("({raw})")),
        other => other,
    };
    RustExpr::Unary {
        op: "!".to_string(),
        expr: Box::new(expr),
    }
}

pub fn math_max_projection(left: RustExpr, right: RustExpr, primitive: PrimitiveType) -> RustExpr {
    match primitive {
        PrimitiveType::Float => RustExpr::Call {
            function: Box::new(RustExpr::path(["java_math_max_f32"])),
            args: vec![left, right],
        },
        PrimitiveType::Double => RustExpr::Call {
            function: Box::new(RustExpr::path(["java_math_max_f64"])),
            args: vec![left, right],
        },
        _ => RustExpr::Call {
            function: Box::new(RustExpr::path(["std", "cmp", "max"])),
            args: vec![left, right],
        },
    }
}

pub fn math_min_projection(left: RustExpr, right: RustExpr, primitive: PrimitiveType) -> RustExpr {
    match primitive {
        PrimitiveType::Float => RustExpr::Call {
            function: Box::new(RustExpr::path(["java_math_min_f32"])),
            args: vec![left, right],
        },
        PrimitiveType::Double => RustExpr::Call {
            function: Box::new(RustExpr::path(["java_math_min_f64"])),
            args: vec![left, right],
        },
        _ => RustExpr::Call {
            function: Box::new(RustExpr::path(["std", "cmp", "min"])),
            args: vec![left, right],
        },
    }
}

pub fn objects_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Binary {
        op: "==".to_string(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn objects_non_null_projection(arg: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(arg),
        method: "is_some".to_string(),
        args: Vec::new(),
    }
}

pub fn objects_is_null_projection(arg: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(arg),
        method: "is_none".to_string(),
        args: Vec::new(),
    }
}

pub fn arrays_as_list_projection(args: Vec<RustExpr>) -> RustExpr {
    RustExpr::VecLiteral(args)
}

pub fn native_collection_from_source_projection(
    kind: crate::stdlib::JavaCollectionKind,
    source: RustExpr,
) -> RustExpr {
    match kind {
        crate::stdlib::JavaCollectionKind::List => source,
        crate::stdlib::JavaCollectionKind::Set => typed_collect_projection(
            RustExpr::MethodCall {
                receiver: Box::new(source),
                method: "into_iter".to_string(),
                args: Vec::new(),
            },
            "std::collections::HashSet<_>",
        ),
        crate::stdlib::JavaCollectionKind::Map => source,
    }
}

pub fn arrays_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_equals"])),
        args: vec![left, right],
    }
}

pub fn arrays_deep_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_deep_equals"])),
        args: vec![left, right],
    }
}

pub fn arrays_deep_hash_code_projection(source: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_deep_hash_code"])),
        args: vec![source],
    }
}

pub fn arrays_equals_range_projection(
    left: RustExpr,
    left_from: RustExpr,
    left_to: RustExpr,
    right: RustExpr,
    right_from: RustExpr,
    right_to: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_left = &{}; let __java_arrays_left_from = {}; let __java_arrays_left_to = {}; let __java_arrays_right = &{}; let __java_arrays_right_from = {}; let __java_arrays_right_to = {}; if __java_arrays_left_from > __java_arrays_left_to || __java_arrays_right_from > __java_arrays_right_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_left_from < 0 || __java_arrays_left_to as usize > __java_arrays_left.len() || __java_arrays_right_from < 0 || __java_arrays_right_to as usize > __java_arrays_right.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_left_from = __java_arrays_left_from as usize; let __java_arrays_left_to = __java_arrays_left_to as usize; let __java_arrays_right_from = __java_arrays_right_from as usize; let __java_arrays_right_to = __java_arrays_right_to as usize; java_arrays_equals(&__java_arrays_left[__java_arrays_left_from..__java_arrays_left_to], &__java_arrays_right[__java_arrays_right_from..__java_arrays_right_to]) }}",
        emit::emit_expr(&left),
        emit::emit_expr(&left_from),
        emit::emit_expr(&left_to),
        emit::emit_expr(&right),
        emit::emit_expr(&right_from),
        emit::emit_expr(&right_to),
    ))
}

pub fn arrays_binary_search_projection(source: RustExpr, key: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_source = &{}; let __java_arrays_key = {}; match __java_arrays_source.binary_search(&__java_arrays_key) {{ Ok(__java_arrays_index) => __java_arrays_index as i32, Err(__java_arrays_insert) => -((__java_arrays_insert as i32) + 1) }} }}",
        emit::emit_expr(&source),
        emit::emit_expr(&key),
    ))
}

pub fn arrays_binary_search_range_projection(
    source: RustExpr,
    from: RustExpr,
    to: RustExpr,
    key: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_source = &{}; let __java_arrays_from = {}; let __java_arrays_to = {}; if __java_arrays_from > __java_arrays_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_from < 0 || __java_arrays_to as usize > __java_arrays_source.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; let __java_arrays_key = {}; match __java_arrays_source[__java_arrays_from..__java_arrays_to].binary_search(&__java_arrays_key) {{ Ok(__java_arrays_index) => (__java_arrays_from + __java_arrays_index) as i32, Err(__java_arrays_insert) => -((__java_arrays_from + __java_arrays_insert) as i32 + 1) }} }}",
        emit::emit_expr(&source),
        emit::emit_expr(&from),
        emit::emit_expr(&to),
        emit::emit_expr(&key),
    ))
}

pub fn arrays_compare_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_compare"])),
        args: vec![left, right],
    }
}

pub fn arrays_compare_range_projection(
    left: RustExpr,
    left_from: RustExpr,
    left_to: RustExpr,
    right: RustExpr,
    right_from: RustExpr,
    right_to: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_left = &{}; let __java_arrays_left_from = {}; let __java_arrays_left_to = {}; let __java_arrays_right = &{}; let __java_arrays_right_from = {}; let __java_arrays_right_to = {}; if __java_arrays_left_from > __java_arrays_left_to || __java_arrays_right_from > __java_arrays_right_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_left_from < 0 || __java_arrays_left_to as usize > __java_arrays_left.len() || __java_arrays_right_from < 0 || __java_arrays_right_to as usize > __java_arrays_right.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_left_from = __java_arrays_left_from as usize; let __java_arrays_left_to = __java_arrays_left_to as usize; let __java_arrays_right_from = __java_arrays_right_from as usize; let __java_arrays_right_to = __java_arrays_right_to as usize; java_arrays_compare(&__java_arrays_left[__java_arrays_left_from..__java_arrays_left_to], &__java_arrays_right[__java_arrays_right_from..__java_arrays_right_to]) }}",
        emit::emit_expr(&left),
        emit::emit_expr(&left_from),
        emit::emit_expr(&left_to),
        emit::emit_expr(&right),
        emit::emit_expr(&right_from),
        emit::emit_expr(&right_to),
    ))
}

pub fn arrays_mismatch_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_mismatch"])),
        args: vec![left, right],
    }
}

pub fn arrays_mismatch_range_projection(
    left: RustExpr,
    left_from: RustExpr,
    left_to: RustExpr,
    right: RustExpr,
    right_from: RustExpr,
    right_to: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_left = &{}; let __java_arrays_left_from = {}; let __java_arrays_left_to = {}; let __java_arrays_right = &{}; let __java_arrays_right_from = {}; let __java_arrays_right_to = {}; if __java_arrays_left_from > __java_arrays_left_to || __java_arrays_right_from > __java_arrays_right_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_left_from < 0 || __java_arrays_left_to as usize > __java_arrays_left.len() || __java_arrays_right_from < 0 || __java_arrays_right_to as usize > __java_arrays_right.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_left_from = __java_arrays_left_from as usize; let __java_arrays_left_to = __java_arrays_left_to as usize; let __java_arrays_right_from = __java_arrays_right_from as usize; let __java_arrays_right_to = __java_arrays_right_to as usize; java_arrays_mismatch(&__java_arrays_left[__java_arrays_left_from..__java_arrays_left_to], &__java_arrays_right[__java_arrays_right_from..__java_arrays_right_to]) }}",
        emit::emit_expr(&left),
        emit::emit_expr(&left_from),
        emit::emit_expr(&left_to),
        emit::emit_expr(&right),
        emit::emit_expr(&right_from),
        emit::emit_expr(&right_to),
    ))
}

pub fn arrays_copy_of_projection(source: RustExpr, len: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_source = &{}; let __java_arrays_len = {}; if __java_arrays_len < 0 {{ panic!(\"Java NegativeArraySizeException\"); }} let __java_arrays_len = __java_arrays_len as usize; let mut __java_arrays_out = __java_arrays_source.iter().cloned().take(__java_arrays_len).collect::<Vec<_>>(); __java_arrays_out.resize(__java_arrays_len, Default::default()); __java_arrays_out }}",
        emit::emit_expr(&source),
        emit::emit_expr(&len),
    ))
}

pub fn arrays_copy_of_range_projection(source: RustExpr, from: RustExpr, to: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_source = &{}; let __java_arrays_from = {}; let __java_arrays_to = {}; if __java_arrays_from > __java_arrays_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_from < 0 || __java_arrays_from as usize > __java_arrays_source.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; let __java_arrays_len = __java_arrays_to - __java_arrays_from; let mut __java_arrays_out = __java_arrays_source[__java_arrays_from..__java_arrays_source.len().min(__java_arrays_to)].to_vec(); __java_arrays_out.resize(__java_arrays_len, Default::default()); __java_arrays_out }}",
        emit::emit_expr(&source),
        emit::emit_expr(&from),
        emit::emit_expr(&to),
    ))
}

pub fn arrays_to_string_projection(source: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_to_string"])),
        args: vec![source],
    }
}

pub fn arrays_deep_to_string_projection(source: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_deep_to_string"])),
        args: vec![source],
    }
}

pub fn arrays_hash_code_projection(source: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["java_arrays_hash_code"])),
        args: vec![source],
    }
}

pub fn collections_empty_list_projection() -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["Vec", "new"])),
        args: Vec::new(),
    }
}

pub fn collections_empty_set_projection() -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "collections", "HashSet", "new"])),
        args: Vec::new(),
    }
}

pub fn collections_empty_map_projection() -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "collections", "HashMap", "new"])),
        args: Vec::new(),
    }
}

pub fn collections_empty_sorted_set_projection() -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "collections", "BTreeSet", "new"])),
        args: Vec::new(),
    }
}

pub fn collections_unmodifiable_projection(value: RustExpr) -> RustExpr {
    value
}

pub fn collections_singleton_projection(value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "collections", "HashSet", "from"])),
        args: vec![RustExpr::ArrayLiteral(vec![value])],
    }
}

pub fn collections_singleton_list_projection(value: RustExpr) -> RustExpr {
    RustExpr::VecLiteral(vec![value])
}

pub fn collections_singleton_map_projection(key: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["std", "collections", "HashMap", "from"])),
        args: vec![RustExpr::ArrayLiteral(vec![RustExpr::Tuple(vec![
            key, value,
        ])])],
    }
}

pub fn collections_n_copies_projection(count: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::VecRepeat {
        value: Box::new(value),
        len: Box::new(RustExpr::Cast {
            expr: Box::new(count),
            ty: "usize".to_string(),
        }),
    }
}

pub fn native_collection_empty_new_projection(kind: crate::stdlib::JavaCollectionKind) -> RustExpr {
    RustExpr::Call {
        function: Box::new(native_collection_constructor_path(kind, "new")),
        args: Vec::new(),
    }
}

pub fn native_collection_with_capacity_projection(
    kind: crate::stdlib::JavaCollectionKind,
    capacity: RustExpr,
) -> RustExpr {
    let capacity = RustExpr::Cast {
        expr: Box::new(capacity),
        ty: "usize".to_string(),
    };
    RustExpr::Call {
        function: Box::new(native_collection_constructor_path(kind, "with_capacity")),
        args: vec![capacity],
    }
}

pub fn native_collection_default_constructor_projection(
    kind: crate::stdlib::JavaCollectionKind,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(native_collection_constructor_path(kind, "new")),
        args: Vec::new(),
    }
}

fn native_collection_constructor_path(
    kind: crate::stdlib::JavaCollectionKind,
    member: &str,
) -> RustExpr {
    let mut segments = native_collection_type_name(kind)
        .split("::")
        .map(str::to_string)
        .collect::<Vec<_>>();
    segments.push(member.to_string());
    RustExpr::path(segments)
}

fn native_collection_type_name(kind: crate::stdlib::JavaCollectionKind) -> &'static str {
    match kind {
        crate::stdlib::JavaCollectionKind::List => "Vec",
        crate::stdlib::JavaCollectionKind::Set => "std::collections::HashSet",
        crate::stdlib::JavaCollectionKind::Map => "std::collections::HashMap",
    }
}

pub fn discard_value_projection(value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{{ let _ = {}; }}", emit::emit_expr(&value)))
}

pub fn varargs_packed_vec_projection(values: Vec<RustExpr>) -> RustExpr {
    if values.is_empty() {
        RustExpr::Call {
            function: Box::new(RustExpr::path(["Vec", "new"])),
            args: Vec::new(),
        }
    } else {
        RustExpr::VecLiteral(values)
    }
}

pub fn varargs_spread_extend_projection(packed_vec: RustExpr, tail: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let mut __java_varargs = {}; __java_varargs.extend({}); __java_varargs }}",
        emit::emit_expr(&packed_vec),
        emit::emit_expr(&tail)
    ))
}

pub fn collection_remove_if_projection(
    receiver: RustExpr,
    param_name: &str,
    predicate: &str,
) -> RustExpr {
    receiver_method_call_projection_exact(
        receiver,
        "retain",
        vec![RustExpr::Raw(format!("|{param_name}| !({predicate})"))],
    )
}

pub fn collection_sort_projection(target: RustExpr) -> RustExpr {
    zero_arg_method_projection(target, "sort")
}

pub fn collection_sort_range_projection(
    target: RustExpr,
    from: RustExpr,
    to: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_target = &mut {}; let __java_arrays_from = {}; let __java_arrays_to = {}; if __java_arrays_from > __java_arrays_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_from < 0 || __java_arrays_to as usize > __java_arrays_target.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; __java_arrays_target[__java_arrays_from..__java_arrays_to].sort(); }}",
        emit::emit_expr(&target),
        emit::emit_expr(&from),
        emit::emit_expr(&to),
    ))
}

pub fn collection_sort_by_comparator_projection(
    target: RustExpr,
    comparator: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_comparator = {}; {}.sort_by(|a, b| __java_comparator.compare(a.clone(), b.clone()).cmp(&0)); }}",
        emit::emit_expr(&comparator),
        emit::emit_expr(&target)
    ))
}

pub fn collection_sort_by_callback_projection(target: RustExpr, comparator: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let mut __java_comparator = {}; {}.sort_by(|a, b| __java_comparator(a.clone(), b.clone()).cmp(&0)); }}",
        emit::emit_expr(&comparator),
        emit::emit_expr(&target)
    ))
}

pub fn array_fill_projection(target: RustExpr, value: RustExpr) -> RustExpr {
    method_call_projection(target, "fill", vec![value])
}

pub fn array_fill_range_projection(
    target: RustExpr,
    from: RustExpr,
    to: RustExpr,
    value: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let __java_arrays_target = &mut {}; let __java_arrays_from = {}; let __java_arrays_to = {}; if __java_arrays_from > __java_arrays_to {{ panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); }} if __java_arrays_from < 0 || __java_arrays_to as usize > __java_arrays_target.len() {{ panic!(\"Java ArrayIndexOutOfBoundsException\"); }} let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; __java_arrays_target[__java_arrays_from..__java_arrays_to].fill({}); }}",
        emit::emit_expr(&target),
        emit::emit_expr(&from),
        emit::emit_expr(&to),
        emit::emit_expr(&value)
    ))
}

pub fn sequence_get_expect_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    method_expect_projection(
        method_call_projection(receiver, "get", vec![index]),
        "Java index out of bounds",
    )
}

pub fn map_get_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    method_call_projection(receiver, "get", vec![key])
}

pub fn map_get_expect_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    method_expect_projection(map_get_projection(receiver, key), "Java null receiver")
}

pub fn map_get_mut_expect_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    method_expect_projection(
        method_call_projection(receiver, "get_mut", vec![key]),
        "Java null receiver",
    )
}

pub fn optional_copied_expect_projection(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        method_expect_projection(
            zero_arg_method_projection(expr, "copied"),
            "Java null receiver",
        ),
    )
}

pub fn render_string_format_arg_value(arg: RustExpr) -> String {
    emit::emit_expr(&string_format_arg_value_projection(arg))
}

pub fn string_format_arg_value_projection(arg: RustExpr) -> RustExpr {
    zero_arg_method_projection(parenthesize_raw_method_receiver(arg), "to_string")
}

pub fn string_format_projection(mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() {
        return RustExpr::Call {
            function: Box::new(RustExpr::path(["String", "new"])),
            args: Vec::new(),
        };
    }
    if args.len() >= 2 && is_java_locale_format_arg(&args[0]) {
        args.remove(0);
    }
    let format_arg = args.remove(0);
    let Some(java_format) = string_literal_source(&format_arg) else {
        return RustExpr::Call {
            function: Box::new(RustExpr::path(["java_string_format"])),
            args: vec![
                format_arg,
                RustExpr::VecLiteral(
                    args.into_iter()
                        .map(string_format_arg_value_projection)
                        .collect(),
                ),
            ],
        };
    };
    let rust_format = java_format_to_rust_format(&java_format);
    let mut rendered_args = vec![RustExpr::Literal(format!("{rust_format:?}"))];
    rendered_args.extend(args);
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("format!".to_string())),
        args: rendered_args,
    }
}

fn is_java_locale_format_arg(arg: &RustExpr) -> bool {
    match arg.unshaped() {
        RustExpr::Var(name) => return matches!(name.as_str(), "Locale" | "JavaUtilLocale"),
        RustExpr::Path(segments) => {
            return matches!(
                segments.first().map(String::as_str),
                Some("Locale" | "JavaUtilLocale")
            );
        }
        RustExpr::Raw(_) => {}
        _ => return false,
    }
    let raw = emit::emit_expr(arg);
    raw == "Locale"
        || raw.starts_with("Locale::")
        || raw == "JavaUtilLocale"
        || raw.starts_with("JavaUtilLocale::")
}

fn java_format_to_rust_format(format: &str) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if matches!(chars.peek(), Some('%')) {
            chars.next();
            out.push('%');
            continue;
        }

        let mut spec = String::new();
        while let Some(next) = chars.peek().copied() {
            if next.is_ascii_alphabetic() {
                chars.next();
                match next {
                    's' | 'd' | 'b' | 'c' => out.push_str("{}"),
                    'f' => out.push_str(&rust_float_format(&spec)),
                    _ => out.push_str("{}"),
                }
                break;
            }
            spec.push(next);
            chars.next();
        }
    }
    out
}

fn rust_float_format(spec: &str) -> String {
    if let Some(dot) = spec.find('.') {
        let width = spec[..dot]
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let precision = spec[dot + 1..]
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        return match (width.is_empty(), precision.is_empty()) {
            (true, true) => "{}".to_string(),
            (true, false) => format!("{{:.{precision}}}"),
            (false, true) => format!("{{:{width}}}"),
            (false, false) => format!("{{:{width}.{precision}}}"),
        };
    }
    let width = spec
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if width.is_empty() {
        "{}".to_string()
    } else {
        format!("{{:{width}}}")
    }
}

pub fn borrowed_sequence_extend_arg(
    arg: RustExpr,
    element_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let arg = match arg {
        RustExpr::Raw(raw) => RustExpr::Raw(parenthesize_borrow_operand(&raw)),
        other => other,
    };
    let iter = RustExpr::MethodCall {
        receiver: Box::new(arg),
        method: "iter".to_string(),
        args: Vec::new(),
    };
    match borrowed_element_recovery_policy(element_shape, facts) {
        BorrowedElementRecoveryPolicy::BorrowElement => iter,
        BorrowedElementRecoveryPolicy::StringViewToOwned => RustExpr::MethodCall {
            receiver: Box::new(iter),
            method: "map".to_string(),
            args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
        },
        BorrowedElementRecoveryPolicy::CopyElements => RustExpr::MethodCall {
            receiver: Box::new(iter),
            method: "copied".to_string(),
            args: Vec::new(),
        },
        BorrowedElementRecoveryPolicy::CloneElements { .. } => RustExpr::MethodCall {
            receiver: Box::new(iter),
            method: "cloned".to_string(),
            args: Vec::new(),
        },
        BorrowedElementRecoveryPolicy::Unsupported { .. } => RustExpr::Raw(format!(
            "{}.cloned() /* java2rust: unsupported borrowed element recovery for extend */",
            emit::emit_expr(&iter)
        )),
    }
}

pub fn project_optional_sequence_receiver(
    expr: RustExpr,
    projection: OptionalSequenceReceiverProjection,
) -> RustExpr {
    let projection = match projection {
        OptionalSequenceReceiverProjection::Shared => OptionalReceiverProjection::Shared,
        OptionalSequenceReceiverProjection::Mutable => OptionalReceiverProjection::Mutable,
    };
    project_optional_receiver(expr, projection)
}

pub fn render_sequence_iteration_source(
    source: &str,
    optional_sequence: bool,
    projection: SequenceIterationProjection,
) -> String {
    match (optional_sequence, projection) {
        (true, SequenceIterationProjection::Borrowed) => {
            format!("{source}.as_ref().unwrap().iter()")
        }
        (true, SequenceIterationProjection::OwnedValue) => format!("{source}.unwrap().into_iter()"),
        (false, SequenceIterationProjection::Borrowed) => format!("{source}.iter()"),
        (false, SequenceIterationProjection::OwnedValue) => format!("{source}.into_iter()"),
    }
}

pub fn render_optional_non_null_filter_map(item_is_borrowed: bool) -> &'static str {
    if item_is_borrowed {
        ".filter_map(|item| item.as_ref())"
    } else {
        ".filter_map(|item| item)"
    }
}

pub fn render_nullable_stream_map_body(
    before_guard: &str,
    condition: &str,
    else_body: &str,
    final_expr: &str,
) -> String {
    format!(
        "{{ {before_guard}if {condition} {{ None }} else {{ {else_body}Some({final_expr}) }} }}"
    )
}

pub fn render_borrowed_collection_flat_map(param_name: &str) -> String {
    format!(".flat_map(|{param_name}| {param_name}.iter())")
}

pub fn render_stream_lambda_param_projection(
    projection: StreamLambdaParamProjection,
    param: &str,
    parenthesized: bool,
) -> String {
    match (projection, parenthesized) {
        (StreamLambdaParamProjection::FilterBorrowed, false) => format!("**{param}"),
        (StreamLambdaParamProjection::BorrowedItem, false) => format!("*{param}"),
        (StreamLambdaParamProjection::OwnedItem, false) => param.to_string(),
        (StreamLambdaParamProjection::FilterBorrowed, true) => format!("(**{param})"),
        (StreamLambdaParamProjection::BorrowedItem, true) => format!("(*{param})"),
        (StreamLambdaParamProjection::OwnedItem, true) => param.to_string(),
    }
}

pub fn project_optional_receiver(
    expr: RustExpr,
    projection: OptionalReceiverProjection,
) -> RustExpr {
    if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
        let expr = strip_terminal_unwrap_projection(expr);
        return match projection {
            OptionalReceiverProjection::Shared => method_expect_projection(
                zero_arg_method_projection(expr, "as_ref"),
                "Java null receiver",
            ),
            OptionalReceiverProjection::Mutable => method_expect_projection(
                zero_arg_method_projection(expr, "as_mut"),
                "Java null receiver",
            ),
            OptionalReceiverProjection::Borrowed => {
                method_expect_projection(expr, "Java null receiver")
            }
            OptionalReceiverProjection::SharedUnchecked => {
                zero_arg_method_projection(zero_arg_method_projection(expr, "as_ref"), "unwrap")
            }
            OptionalReceiverProjection::MutableUnchecked => {
                zero_arg_method_projection(zero_arg_method_projection(expr, "as_mut"), "unwrap")
            }
        };
    }
    let rendered = emit::emit_expr(&expr);
    let receiver = rendered.strip_suffix(".unwrap()").unwrap_or(&rendered);
    let receiver = RustExpr::Raw(parenthesize_borrow_operand(receiver));
    match projection {
        OptionalReceiverProjection::Shared => method_expect_projection(
            RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: "as_ref".to_string(),
                args: Vec::new(),
            },
            "Java null receiver",
        ),
        OptionalReceiverProjection::Mutable => method_expect_projection(
            RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: "as_mut".to_string(),
                args: Vec::new(),
            },
            "Java null receiver",
        ),
        OptionalReceiverProjection::Borrowed => RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        },
        OptionalReceiverProjection::SharedUnchecked => RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
            method: "unwrap".to_string(),
            args: Vec::new(),
        },
        OptionalReceiverProjection::MutableUnchecked => RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(receiver),
                method: "as_mut".to_string(),
                args: Vec::new(),
            }),
            method: "unwrap".to_string(),
            args: Vec::new(),
        },
    }
}

pub fn project_optional_receiver_with_shape(
    expr: RustExpr,
    projection: OptionalReceiverProjection,
    element_shape: &RustSlotShape,
) -> RustExpr {
    let ownership = match projection {
        OptionalReceiverProjection::Shared | OptionalReceiverProjection::SharedUnchecked => {
            OwnershipShape::SharedBorrow
        }
        OptionalReceiverProjection::Mutable | OptionalReceiverProjection::MutableUnchecked => {
            OwnershipShape::MutableBorrow
        }
        OptionalReceiverProjection::Borrowed => element_shape.ownership,
    };
    optional_borrowed_element_projected_expr(
        project_optional_receiver(expr, projection),
        element_shape,
        ownership,
    )
}

pub fn rewrite_downcast_expect_message(
    expr: RustExpr,
    replacement_message: &'static str,
) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    let cast_expect = crate::output_markers::invalid_java_cast_expect();
    if !raw_contains_outside_literals(rendered.as_str(), crate::output_markers::DOWNCAST_TURBOFISH)
    {
        return expr;
    }
    raw_replace_once_outside_literals(
        &rendered,
        &cast_expect,
        &format!(".expect(\"{replacement_message}\")"),
    )
    .map(RustExpr::Raw)
    .unwrap_or(expr)
}

pub fn rewrite_optional_erased_borrowed_string_view_call_args(
    lowered: RustExpr,
    optional_string_locals: &[String],
    erased_string_cast_locals: &[String],
) -> RustExpr {
    let mut rendered = emit::emit_expr(&lowered);
    let mut changed = false;
    for name in optional_string_locals {
        let erased_string_box =
            format!("{name}.map(|value| Box::new(value.clone()) as Box<dyn std::any::Any>)");
        if let Some(rewritten) = raw_replace_all_outside_literals(
            &rendered,
            &erased_string_box,
            &format!("{name}.as_ref().map(|value| value.as_str())"),
        ) {
            rendered = rewritten;
            changed = true;
        }
    }
    for name in erased_string_cast_locals {
        let consuming_view = format!(
            "{name}.unwrap().downcast::<String>(){}.as_str()",
            crate::output_markers::invalid_java_cast_expect()
        );
        if let Some(rewritten) = raw_replace_all_outside_literals(
            &rendered,
            &consuming_view,
            &format!(
                "{name}.as_ref().and_then(|value| value.downcast_ref::<String>()){}.as_str()",
                crate::output_markers::invalid_java_cast_expect()
            ),
        ) {
            rendered = rewritten;
            changed = true;
        }
    }
    if let Some(rewritten) = rewrite_rendered_optional_erased_string_views(&rendered) {
        rendered = rewritten;
        changed = true;
    }
    if changed {
        RustExpr::Raw(rendered)
    } else {
        lowered
    }
}

fn raw_replace_all_outside_literals(raw: &str, pattern: &str, replacement: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut changed = false;
    while let Some(index) = raw_find_outside_literals(rest, pattern) {
        out.push_str(&rest[..index]);
        out.push_str(replacement);
        rest = &rest[index + pattern.len()..];
        changed = true;
    }
    if changed {
        out.push_str(rest);
        Some(out)
    } else {
        None
    }
}

fn rewrite_rendered_optional_erased_string_views(rendered: &str) -> Option<String> {
    let suffix = format!(
        ".unwrap().downcast::<String>(){}.as_str()",
        crate::output_markers::invalid_java_cast_expect()
    );
    let suffix = suffix.as_str();
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered;
    let mut changed = false;
    while let Some(pos) = raw_find_outside_literals(rest, suffix) {
        let prefix = &rest[..pos];
        let local_start = prefix
            .char_indices()
            .rev()
            .find_map(|(index, ch)| (!rust_ident_char(ch)).then_some(index + ch.len_utf8()))
            .unwrap_or(0);
        let local_name = &prefix[local_start..];
        if local_name.is_empty()
            || !local_name
                .chars()
                .next()
                .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        {
            out.push_str(prefix);
            out.push_str(suffix);
            rest = &rest[pos + suffix.len()..];
            continue;
        }
        out.push_str(&prefix[..local_start]);
        out.push_str(&format!(
            "{local_name}.as_ref().and_then(|value| value.downcast_ref::<String>()){}.as_str()",
            crate::output_markers::invalid_java_cast_expect()
        ));
        rest = &rest[pos + suffix.len()..];
        changed = true;
    }
    if changed {
        out.push_str(rest);
        Some(out)
    } else {
        None
    }
}

fn rust_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

pub fn deref_borrowed_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Unary {
        op: "*".to_string(),
        expr: Box::new(parenthesize_raw_deref_operand(expr)),
    }
}

fn parenthesize_raw_deref_operand(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if balanced_outer_parentheses_wrap_all(trimmed)
        || !raw_contains_top_level_binary_operator(trimmed)
    {
        RustExpr::Raw(raw)
    } else {
        RustExpr::Raw(format!("({trimmed})"))
    }
}

pub fn unwrap_result_or_panic_uncaught_java_exception(expr: RustExpr) -> RustExpr {
    let panic_recovery_closure = "|ex| panic!(\"uncaught Java exception: {:?}\", ex)";
    let panic_recovery = ".unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))";
    if let Some(expr) = recover_nested_result_try_operator(expr.clone(), panic_recovery) {
        return expr;
    }
    if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return method_call_projection(
            expr,
            "unwrap_or_else",
            vec![RustExpr::Raw(panic_recovery_closure.to_string())],
        );
    }
    let rendered = emit::emit_expr(&expr);
    // Already recovered: re-applying would splice a second copy into the `{:?}`
    // of the recovery text this function previously appended.
    if rendered.ends_with(panic_recovery) {
        return RustExpr::Raw(rendered);
    }
    if let Some(rendered) = rendered.strip_suffix('?') {
        return RustExpr::Raw(format!("{rendered}{panic_recovery}"));
    }
    // Only a genuine `?` operator counts. `first_rust_try_operator_index` skips
    // string and char literals; a plain `find('?')` fallback would defeat that
    // and corrupt format specifiers such as `{:?}`.
    if let Some(index) = first_rust_try_operator_index(&rendered) {
        let mut recovered = String::with_capacity(rendered.len() + panic_recovery.len());
        recovered.push_str(&rendered[..index]);
        recovered.push_str(panic_recovery);
        recovered.push_str(&rendered[index + 1..]);
        return RustExpr::Raw(recovered);
    }
    RustExpr::Raw(format!("{rendered}{panic_recovery}"))
}

pub fn if_err_panic_uncaught_java_exception_stmt(expr: RustExpr) -> RustStmt {
    RustStmt::Raw(format!(
        "if let Err(ex) = {} {{ panic!(\"uncaught Java exception: {{:?}}\", ex); }}",
        emit::emit_expr(&expr)
    ))
}

fn recover_nested_result_try_operator(expr: RustExpr, recovery: &str) -> Option<RustExpr> {
    match expr {
        // Rewriting the inside cannot preserve what the tag asserted, so the
        // rewritten expression comes back untagged rather than mislabelled.
        RustExpr::Shaped { expr, .. } => recover_nested_result_try_operator(*expr, recovery),
        RustExpr::Raw(raw) => raw.trim_end().strip_suffix('?').map(|without_try| {
            first_rust_try_operator_index(without_try)
                .map(|index| {
                    let mut recovered = String::with_capacity(without_try.len() + recovery.len());
                    recovered.push_str(&without_try[..index]);
                    recovered.push_str(recovery);
                    recovered.push_str(&without_try[index + 1..]);
                    RustExpr::Raw(recovered)
                })
                .unwrap_or_else(|| RustExpr::Raw(format!("{without_try}{recovery}")))
        }),
        RustExpr::Field(receiver, field) => recover_nested_result_try_operator(*receiver, recovery)
            .map(|receiver| RustExpr::Field(Box::new(receiver), field)),
        RustExpr::Index(array, index) => {
            if let Some(array) = recover_nested_result_try_operator((*array).clone(), recovery) {
                return Some(RustExpr::Index(Box::new(array), index));
            }
            recover_nested_result_try_operator(*index, recovery)
                .map(|index| RustExpr::Index(array, Box::new(index)))
        }
        RustExpr::Binary { op, left, right } => {
            if let Some(left) = recover_nested_result_try_operator((*left).clone(), recovery) {
                return Some(RustExpr::Binary {
                    op,
                    left: Box::new(left),
                    right,
                });
            }
            recover_nested_result_try_operator(*right, recovery).map(|right| RustExpr::Binary {
                op,
                left,
                right: Box::new(right),
            })
        }
        RustExpr::Unary { op, expr } => {
            recover_nested_result_try_operator(*expr, recovery).map(|expr| RustExpr::Unary {
                op,
                expr: Box::new(expr),
            })
        }
        RustExpr::Cast { expr, ty } => {
            recover_nested_result_try_operator(*expr, recovery).map(|expr| RustExpr::Cast {
                expr: Box::new(expr),
                ty,
            })
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            if let Some(condition) =
                recover_nested_result_try_operator((*condition).clone(), recovery)
            {
                return Some(RustExpr::If {
                    condition: Box::new(condition),
                    then_expr,
                    else_expr,
                });
            }
            if let Some(then_expr) =
                recover_nested_result_try_operator((*then_expr).clone(), recovery)
            {
                return Some(RustExpr::If {
                    condition,
                    then_expr: Box::new(then_expr),
                    else_expr,
                });
            }
            recover_nested_result_try_operator(*else_expr, recovery).map(|else_expr| RustExpr::If {
                condition,
                then_expr,
                else_expr: Box::new(else_expr),
            })
        }
        RustExpr::Tuple(items) => {
            recover_first_arg_try_operator(items, recovery).map(RustExpr::Tuple)
        }
        RustExpr::ArrayLiteral(items) => {
            recover_first_arg_try_operator(items, recovery).map(RustExpr::ArrayLiteral)
        }
        RustExpr::VecLiteral(items) => {
            recover_first_arg_try_operator(items, recovery).map(RustExpr::VecLiteral)
        }
        RustExpr::VecRepeat { value, len } => {
            if let Some(value) = recover_nested_result_try_operator((*value).clone(), recovery) {
                return Some(RustExpr::VecRepeat {
                    value: Box::new(value),
                    len,
                });
            }
            recover_nested_result_try_operator(*len, recovery).map(|len| RustExpr::VecRepeat {
                value,
                len: Box::new(len),
            })
        }
        RustExpr::Call { function, args } => {
            if let Some(function) =
                recover_nested_result_try_operator((*function).clone(), recovery)
            {
                return Some(RustExpr::Call {
                    function: Box::new(function),
                    args,
                });
            }
            recover_first_arg_try_operator(args, recovery)
                .map(|args| RustExpr::Call { function, args })
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            if let Some(receiver) =
                recover_nested_result_try_operator((*receiver).clone(), recovery)
            {
                return Some(RustExpr::MethodCall {
                    receiver: Box::new(receiver),
                    method,
                    args,
                });
            }
            recover_first_arg_try_operator(args, recovery).map(|args| RustExpr::MethodCall {
                receiver,
                method,
                args,
            })
        }
        RustExpr::StructLit { name, fields } => {
            let mut updated = Vec::with_capacity(fields.len());
            let mut recovered = false;
            for (field, value) in fields {
                if !recovered
                    && let Some(value) = recover_nested_result_try_operator(value.clone(), recovery)
                {
                    updated.push((field, value));
                    recovered = true;
                } else {
                    updated.push((field, value));
                }
            }
            recovered.then_some(RustExpr::StructLit {
                name,
                fields: updated,
            })
        }
        RustExpr::MarkedBlock { marker, expr } => {
            recover_nested_result_try_operator(*expr, recovery).map(|expr| RustExpr::MarkedBlock {
                marker,
                expr: Box::new(expr),
            })
        }
        RustExpr::Literal(_) | RustExpr::Var(_) | RustExpr::Path(_) => None,
    }
}

fn recover_first_arg_try_operator(args: Vec<RustExpr>, recovery: &str) -> Option<Vec<RustExpr>> {
    let mut updated = Vec::with_capacity(args.len());
    let mut recovered = false;
    for arg in args {
        if !recovered && let Some(arg) = recover_nested_result_try_operator(arg.clone(), recovery) {
            updated.push(arg);
            recovered = true;
        } else {
            updated.push(arg);
        }
    }
    recovered.then_some(updated)
}

fn first_rust_try_operator_index(rendered: &str) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in rendered.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string || in_char => {
                escaped = true;
            }
            '"' if !in_char => {
                in_string = !in_string;
            }
            '\'' if !in_string => {
                in_char = !in_char;
            }
            '?' if !in_string && !in_char => return Some(index),
            _ => {}
        }
    }
    None
}

pub fn normalize_reused_clone_for_borrowed_argument(
    lowered: &RustExpr,
    name: &str,
    target_shape: &RustSlotShape,
) -> Option<RustExpr> {
    if !rust_expr_has_clone_recovery(lowered) {
        return None;
    }
    match (
        &target_shape.java_ty,
        &target_shape.view,
        target_shape.ownership,
    ) {
        (JavaType::String, ViewShape::Str, OwnershipShape::SharedBorrow) => Some(string_view_expr(
            zero_arg_method_projection(RustExpr::Var(name.to_string()), "as_str"),
        )),
        (JavaType::Array(_), ViewShape::Slice, OwnershipShape::MutableBorrow) => {
            Some(mutable_slice_view_expr(zero_arg_method_projection(
                RustExpr::Var(name.to_string()),
                "as_mut_slice",
            )))
        }
        (JavaType::Array(_), ViewShape::Slice, OwnershipShape::SharedBorrow) => {
            Some(slice_view_expr(zero_arg_method_projection(
                RustExpr::Var(name.to_string()),
                "as_slice",
            )))
        }
        (_, _, OwnershipShape::SharedBorrow) => Some(force_shared_borrow_projection(
            RustExpr::Var(name.to_string()),
        )),
        _ => None,
    }
}

pub fn borrowed_string_literal_temporary_plan(
    raw: &str,
    temporary_name: &str,
) -> Option<BorrowedStringLiteralTemporaryPlan> {
    if let Some(inner) = raw
        .strip_prefix("Some(Box::new(")
        .and_then(|rest| rest.strip_suffix(") as Box<dyn std::any::Any>)"))
        && string_literal_to_string_expr(inner)
    {
        return Some(BorrowedStringLiteralTemporaryPlan {
            replacement_expr: format!("Some(&{temporary_name})"),
            initializer_expr: inner.to_string(),
        });
    }
    if let Some(inner) = raw
        .strip_prefix("Some(&")
        .and_then(|rest| rest.strip_suffix(')'))
        && string_literal_to_string_expr(inner)
    {
        return Some(BorrowedStringLiteralTemporaryPlan {
            replacement_expr: format!("Some(&{temporary_name})"),
            initializer_expr: inner.to_string(),
        });
    }
    if let Some(inner) = raw.strip_prefix('&')
        && string_literal_to_string_expr(inner)
    {
        return Some(BorrowedStringLiteralTemporaryPlan {
            replacement_expr: format!("&{temporary_name}"),
            initializer_expr: inner.to_string(),
        });
    }
    None
}

fn string_literal_to_string_expr(raw: &str) -> bool {
    raw.starts_with('"') && raw.ends_with(".to_string()")
}

pub fn erased_object_box_projection(value: RustExpr) -> RustExpr {
    if !matches!(value.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::Cast {
            expr: Box::new(function_call_projection(
                RustExpr::path(["Box", "new"]),
                vec![value],
            )),
            ty: "Box<dyn std::any::Any>".to_string(),
        };
    }
    let value = emit::emit_expr(&value);
    let value = if raw_contains_top_level_comma(&value) {
        format!("({value})")
    } else {
        value
    };
    RustExpr::Raw(format!("Box::new({value}) as Box<dyn std::any::Any>"))
}

pub fn erased_object_constructor_projection(args: Vec<RustExpr>) -> RustExpr {
    let value = match args.as_slice() {
        [] => RustExpr::Tuple(Vec::new()),
        [single] => single.clone(),
        _ => RustExpr::Tuple(args),
    };
    erased_object_box_projection(value)
}

pub fn boxed_dyn_trait_projection(value: RustExpr, trait_name: &str) -> RustExpr {
    if !matches!(value.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::Cast {
            expr: Box::new(function_call_projection(
                RustExpr::path(["Box", "new"]),
                vec![value],
            )),
            ty: format!("Box<dyn {trait_name}>"),
        };
    }
    let value = emit::emit_expr(&value);
    let value = if raw_contains_top_level_comma(&value) {
        format!("({value})")
    } else {
        value
    };
    RustExpr::Raw(format!("Box::new({value}) as Box<dyn {trait_name}>"))
}

pub fn unsupported_deserialization_cast_handoff_projection(
    value: RustExpr,
    java_name: &str,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let _java_deserialized = {}; panic!(\"unsupported Java deserialization cast to {java_name}; provide a typed serialization facade\") }}",
        emit::emit_expr(&value)
    ))
}

pub fn abstract_method_stub_projection(owner: &str, method: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "panic!(\"abstract method {owner}.{method} has no implementation\")"
    ))
}

pub fn abstract_interface_method_stub_projection(owner: &str, method: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "panic!(\"abstract interface method {owner}.{method} has no implementation\")"
    ))
}

pub fn unsupported_outer_capture_constructor_projection(owner: &str, outer_type: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "panic!(\"{} for {owner}; pass an outer {outer_type} handle\")",
        crate::output_markers::UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE
    ))
}

pub fn unsupported_constructor_call_handoff_projection(message: &str) -> RustExpr {
    RustExpr::Raw(format!(
        "panic!({message:?})/* TODO(translation): constructor call targets unsupported Java object graph mutation */"
    ))
}

fn raw_contains_top_level_comma(raw: &str) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth = angle_depth.saturating_sub(1)
            }
            ',' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0 =>
            {
                return true;
            }
            '"' | '\'' => {
                let quote = ch;
                let mut escaped = false;
                for (_, inner) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == quote {
                        break;
                    }
                }
            }
            'r' if raw[idx..].starts_with("r#\"") => {
                for (_, inner) in chars.by_ref() {
                    if inner == '"' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn apply_rust_unsigned_byte_array_return(
    expr: RustExpr,
    policy: &RustUnsignedByteArrayReturnPolicy,
) -> RustExpr {
    let expr = emit::emit_expr(&expr);
    if policy.optional {
        RustExpr::Raw(format!(
            "{expr}.map(|bytes| bytes.into_iter().map(|byte| byte as i8).collect::<Vec<_>>())"
        ))
    } else {
        RustExpr::Raw(format!(
            "{expr}.into_iter().map(|byte| byte as i8).collect::<Vec<_>>()"
        ))
    }
}

fn apply_rust_borrowed_facade_return(
    expr: RustExpr,
    policy: &RustBorrowedFacadeReturnPolicy,
) -> RustExpr {
    let recovered = match policy.recovery {
        RustBorrowedFacadeReturnRecovery::ToPathBuf => {
            zero_arg_method_projection(expr, "to_path_buf")
        }
        RustBorrowedFacadeReturnRecovery::Clone => zero_arg_method_projection(expr, "clone"),
    };
    let erased = erased_object_box_projection(recovered);
    if policy.target.optional_element_type().is_some() {
        render_option_some(erased)
    } else {
        erased
    }
}

fn apply_optional_rust_borrowed_facade_return(
    expr: RustExpr,
    policy: &RustBorrowedFacadeReturnPolicy,
) -> RustExpr {
    let recovered = match policy.recovery {
        RustBorrowedFacadeReturnRecovery::ToPathBuf => "value.to_path_buf()",
        RustBorrowedFacadeReturnRecovery::Clone => "value.clone()",
    };
    let mapped = RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "map".to_string(),
        args: vec![RustExpr::Raw(format!(
            "|value| Box::new({recovered}) as Box<dyn std::any::Any>"
        ))],
    };
    if policy.target.optional_element_type().is_some() {
        mapped
    } else {
        method_expect_projection(mapped, "Java null receiver")
    }
}

fn apply_rust_scalar_slot_cast(expr: RustExpr, policy: &RustScalarSlotCastPolicy) -> RustExpr {
    if rust_scalar_type_name_for_primitive(policy.target_primitive)
        == Some(policy.rust_type_name.as_str())
    {
        return expr;
    }
    if policy.target_primitive == PrimitiveType::Char {
        return java_char_from_u32_mask_projection(expr);
    }
    if policy.rust_type_name == "char" {
        return java_char_to_numeric_projection(expr, policy.target_primitive);
    }
    if rust_scalar_is_integral(&policy.rust_type_name)
        && java_primitive_is_integral(policy.target_primitive)
    {
        if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
            return zero_arg_method_projection(
                zero_arg_method_projection(expr, "try_into"),
                "unwrap",
            );
        }
        let rendered = parenthesize_borrow_operand(&emit::emit_expr(&expr));
        return RustExpr::Raw(format!("{rendered}.try_into().unwrap()"));
    }
    RustExpr::Cast {
        expr: Box::new(expr),
        ty: rust_primitive_type_name_policy(policy.target_primitive)
            .rust_name
            .to_string(),
    }
}

fn rust_scalar_type_name_for_primitive(primitive: PrimitiveType) -> Option<&'static str> {
    Some(rust_primitive_type_name_policy(primitive).rust_name)
}

fn rust_scalar_is_integral(type_name: &str) -> bool {
    matches!(
        type_name,
        "usize" | "isize" | "u64" | "u32" | "u16" | "u8" | "i64" | "i32" | "i16" | "i8"
    )
}

fn java_primitive_is_integral(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        PrimitiveType::Byte
            | PrimitiveType::Short
            | PrimitiveType::Char
            | PrimitiveType::Int
            | PrimitiveType::Long
    )
}

pub fn apply_expr_to_slot(expr: RustExpr, actual: RustSlotShape, slot: &Slot) -> RustExpr {
    apply_expr_to_slot_with_facts(expr, actual, slot, &FactSet::default())
}

pub fn apply_expr_to_slot_with_facts(
    expr: RustExpr,
    actual: RustSlotShape,
    slot: &Slot,
    facts: &FactSet,
) -> RustExpr {
    let plan = crate::conversion_plan::slot_conversion_plan_with_facts(&actual, slot, facts);
    apply_slot_conversion_plan(expr, &plan)
}

pub fn apply_expr_to_shape(
    expr: RustExpr,
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> RustExpr {
    apply_expr_to_shape_with_facts(expr, actual, target, &FactSet::default())
}

pub fn apply_expr_to_shape_with_facts(
    expr: RustExpr,
    actual: &RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let plan = crate::conversion_plan::shape_coercion_plan_with_facts(actual, target, facts);
    apply_shape_coercion_plan(expr, &plan)
}

pub fn ensure_borrow_projection(
    expr: RustExpr,
    actual: Option<&RustSlotShape>,
    target: Option<&RustSlotShape>,
    use_case: BorrowProjectionUse,
) -> RustExpr {
    if borrow_projection_already_satisfied(&expr, actual, target, use_case) {
        return expr;
    }
    match use_case {
        BorrowProjectionUse::MutableSliceView => {
            RustExpr::Raw(format!("&mut {}", borrow_projection_operand(expr)))
        }
        _ => borrow_expr(expr),
    }
}

pub fn ensure_borrow_projection_rendered(
    expr: RustExpr,
    actual: Option<&RustSlotShape>,
    target: Option<&RustSlotShape>,
    use_case: BorrowProjectionUse,
) -> String {
    emit::emit_expr(&ensure_borrow_projection(expr, actual, target, use_case))
}

pub fn render_string_view_argument(expr: RustExpr) -> String {
    ensure_borrow_projection_rendered(expr, None, None, BorrowProjectionUse::StringView)
}

fn string_view_argument_projection(expr: RustExpr) -> RustExpr {
    ensure_borrow_projection(expr, None, None, BorrowProjectionUse::StringView)
}

pub fn borrowed_string_view_projection(
    rendered: &str,
    local_rust_name: Option<&str>,
) -> Option<BorrowedStringViewProjection> {
    if rendered_expr_is_already_string_view(rendered) {
        return Some(BorrowedStringViewProjection::AlreadyStringView);
    }
    local_rust_name.map(|rust_name| BorrowedStringViewProjection::LocalStringValue {
        rust_name: rust_name.to_string(),
    })
}

pub fn apply_borrowed_string_view_projection(
    rendered: &str,
    projection: &BorrowedStringViewProjection,
) -> String {
    match projection {
        BorrowedStringViewProjection::AlreadyStringView => {
            rendered.trim_start_matches('*').to_string()
        }
        BorrowedStringViewProjection::LocalStringValue { rust_name } => {
            format!("{rust_name}.as_str()")
        }
    }
}

/// The concat plan for a value of this Java type.
pub fn java_concat_display_plan(java_ty: &crate::java_ir::JavaType) -> JavaDisplayPlan {
    match java_ty {
        crate::java_ir::JavaType::Primitive(crate::java_ir::PrimitiveType::Double) => {
            JavaDisplayPlan::ConcatFloatValue { double: true }
        }
        crate::java_ir::JavaType::Primitive(crate::java_ir::PrimitiveType::Float) => {
            JavaDisplayPlan::ConcatFloatValue { double: false }
        }
        _ => JavaDisplayPlan::ConcatValue,
    }
}

pub fn apply_java_display_plan(expr: RustExpr, plan: JavaDisplayPlan) -> RustExpr {
    match plan {
        JavaDisplayPlan::ConcatValue => {
            RustExpr::Raw(format!("({}).to_string()", emit::emit_expr(&expr)))
        }
        JavaDisplayPlan::ConcatFloatValue { double } => {
            let helper = if double {
                "java_double_to_string"
            } else {
                "java_float_to_string"
            };
            function_call_projection(RustExpr::Var(helper.to_string()), vec![expr])
        }
        JavaDisplayPlan::FormatValue => {
            let display_operand = RustExpr::Raw(render_java_display_operand(&expr));
            zero_arg_method_projection(
                parenthesize_raw_method_receiver(display_operand),
                "to_string",
            )
        }
        JavaDisplayPlan::ErasedObject => function_call_projection(
            RustExpr::Var("java_erased_object_to_string".to_string()),
            vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(expr),
            }],
        ),
        JavaDisplayPlan::OptionalString => {
            if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
                return RustExpr::MethodCall {
                    receiver: Box::new(zero_arg_method_projection(expr, "as_deref")),
                    method: "unwrap_or".to_string(),
                    args: vec![RustExpr::Literal("\"null\"".to_string())],
                };
            }
            RustExpr::Raw(format!(
                "{}.as_deref().unwrap_or(\"null\")",
                emit::emit_expr(&expr)
            ))
        }
        JavaDisplayPlan::OptionalValue => {
            if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
                return RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(zero_arg_method_projection(expr, "as_ref")),
                        method: "map".to_string(),
                        args: vec![RustExpr::Raw("|value| value.to_string()".to_string())],
                    }),
                    method: "unwrap_or_else".to_string(),
                    args: vec![RustExpr::Raw("|| \"null\".to_string()".to_string())],
                };
            }
            RustExpr::Raw(format!(
                "{}.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
                emit::emit_expr(&expr)
            ))
        }
        JavaDisplayPlan::OptionalBorrowedValue => {
            if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
                return RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(expr),
                        method: "map".to_string(),
                        args: vec![RustExpr::Raw("|value| value.to_string()".to_string())],
                    }),
                    method: "unwrap_or_else".to_string(),
                    args: vec![RustExpr::Raw("|| \"null\".to_string()".to_string())],
                };
            }
            RustExpr::Raw(format!(
                "{}.map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
                emit::emit_expr(&expr)
            ))
        }
    }
}

pub fn render_java_display_operand(expr: &RustExpr) -> String {
    strip_arithmetic_display_conversion(expr)
}

/// Textual fallback for "has an owned value already been recovered here".
///
/// This used to have three further arms matching the emitted
/// `/* TODO(translation): validate added clone */` **comment** verbatim, one of
/// them with `contains` rather than `ends_with`. A comment whose *wording*
/// changes behaviour is not a comment, it is syntax.
///
/// The comment is still tolerated — it just is not spelled here. Trailing block
/// comments are stripped first, so an annotated clone is recognised whatever the
/// annotation says. That keeps idempotence (a clone is never applied twice)
/// without making any particular text load-bearing.
pub fn rendered_expr_has_clone_recovery(rendered: &str) -> bool {
    let rendered = strip_trailing_block_comments(rendered);
    rendered.ends_with(".clone()")
        || rendered.ends_with(".clone_box()")
        || rendered.ends_with(".cloned()")
        || rendered.ends_with(".cloned().expect(\"Java null receiver\")")
}

pub fn rendered_expr_has_direct_clone_recovery(rendered: &str) -> bool {
    let rendered = strip_trailing_block_comments(rendered);
    rendered.ends_with(".clone()") || rendered.ends_with(".clone_box()")
}

pub fn rust_expr_has_direct_clone_recovery(expr: &RustExpr) -> bool {
    rust_expr_has_direct_clone_recovery_projection(expr)
        || matches!(
            expr.unshaped(),
            RustExpr::Raw(_) if rendered_expr_has_direct_clone_recovery(&emit::emit_expr(expr))
        )
}

fn rust_expr_has_direct_clone_recovery_projection(expr: &RustExpr) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::MethodCall { method, args, .. }
            if args.is_empty() && matches!(method.as_str(), "clone" | "clone_box")
    )
}

/// Drops block comments from the end of a rendered expression, including ones
/// followed by further method calls (`x.clone()/* … */.expect(…)`).
///
/// Structural, not textual: it recognises `/* … */` as a comment rather than
/// matching what any particular comment says.
fn strip_trailing_block_comments(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered;
    while let Some(open) = raw_find_outside_literals(rest, "/*") {
        let Some(close) = rest[open..].find("*/") else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &rest[open + close + 2..];
    }
    out.push_str(rest);
    out
}

/// Whether an owned value has already been recovered from a borrow here.
///
/// Producers that record `OwnedRecovery` answer structurally. The rendered
/// fallback remains for legacy untagged producers, but it delegates to the
/// comment-insensitive clone recovery predicate above rather than matching any
/// particular translation marker text.
pub fn rust_expr_has_clone_recovery(expr: &RustExpr) -> bool {
    expr.recorded_shape() == Some(RustExprShape::OwnedRecovery)
        || rust_expr_has_clone_recovery_projection(expr)
        || matches!(
            expr.unshaped(),
            RustExpr::Raw(rendered) if rendered_expr_has_clone_recovery(rendered)
        )
}

fn rust_expr_has_clone_recovery_projection(expr: &RustExpr) -> bool {
    if rust_expr_has_direct_clone_recovery_projection(expr) {
        return true;
    }

    match expr.unshaped() {
        RustExpr::MethodCall { method, args, .. } if args.is_empty() && method == "cloned" => true,
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "expect"
            && args.as_slice() == [RustExpr::Literal("\"Java null receiver\"".to_string())] =>
        {
            matches!(
                receiver.unshaped(),
                RustExpr::MethodCall { method, args, .. } if args.is_empty() && method == "cloned"
            )
        }
        _ => false,
    }
}

pub fn snapshot_rendered_shared_borrow_argument(lowered: RustExpr, name: &str) -> RustExpr {
    let rendered = emit::emit_expr(&lowered);
    let replacements = [
        (
            format!("Some(&{name})"),
            format!(
                "Some(&{name}.clone()/* TODO(translation): validate same-call alias snapshot */)"
            ),
        ),
        (
            format!("&{name}"),
            format!("&{name}.clone()/* TODO(translation): validate same-call alias snapshot */"),
        ),
        (
            format!("{name}.as_slice()"),
            format!(
                "{name}.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice()"
            ),
        ),
        (
            format!("Some({name}.as_slice())"),
            format!(
                "Some({name}.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice())"
            ),
        ),
        (
            format!("{name}.as_str()"),
            format!(
                "{name}.clone()/* TODO(translation): validate same-call alias snapshot */.as_str()"
            ),
        ),
        (
            format!("Some({name}.as_str())"),
            format!(
                "Some({name}.clone()/* TODO(translation): validate same-call alias snapshot */.as_str())"
            ),
        ),
    ];
    for (from, to) in replacements {
        if rendered == from {
            return RustExpr::Raw(to);
        }
    }
    lowered
}

pub fn apply_audited_clone_recovery(expr: RustExpr) -> RustExpr {
    apply_clone_recovery(expr, &CloneRecoveryPolicy::AuditRequired)
}

pub fn apply_clone_projection(expr: RustExpr) -> RustExpr {
    if matches!(expr.unshaped(), RustExpr::Raw(_)) {
        let rendered = emit::emit_expr(&expr);
        RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(render_clone_projection_expr(&rendered, "")),
        )
    } else {
        RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "clone".to_string(),
                args: Vec::new(),
            },
        )
    }
}

pub fn into_projection(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "into".to_string(),
        args: Vec::new(),
    }
}

pub fn java_update_expression_projection(
    target: RustExpr,
    update_op: &str,
    prefix: bool,
) -> RustExpr {
    let target = emit::emit_expr(&target);
    if prefix {
        RustExpr::Raw(format!("{{ {target} {update_op} 1; {target} }}"))
    } else {
        RustExpr::Raw(format!(
            "{{ let __java_old_value = {target}; {target} {update_op} 1; __java_old_value }}"
        ))
    }
}

pub fn java_primitive_update_expression_projection(
    target: RustExpr,
    increment: bool,
    primitive: PrimitiveType,
    prefix: bool,
) -> Option<RustExpr> {
    let target = emit::emit_expr(&target);
    let value = java_primitive_update_value_projection(&target, increment, primitive)?;
    if prefix {
        Some(RustExpr::Raw(format!("{{ {target} = {value}; {target} }}")))
    } else {
        Some(RustExpr::Raw(format!(
            "{{ let __java_old_value = {target}; {target} = {value}; __java_old_value }}"
        )))
    }
}

pub fn java_primitive_updated_value_projection(
    target: RustExpr,
    increment: bool,
    primitive: PrimitiveType,
) -> Option<RustExpr> {
    let target = emit::emit_expr(&target);
    java_primitive_update_value_projection(&target, increment, primitive).map(RustExpr::Raw)
}

fn java_primitive_update_value_projection(
    target: &str,
    increment: bool,
    primitive: PrimitiveType,
) -> Option<String> {
    let method = if increment {
        "wrapping_add"
    } else {
        "wrapping_sub"
    };
    match primitive {
        PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Int | PrimitiveType::Long => {
            Some(format!("({target}).{method}(1)"))
        }
        PrimitiveType::Char => Some(format!(
            "char::from_u32(((((({target}) as u32) as i32).{method}(1)) as u32) & 0xffff).unwrap_or('\\0')"
        )),
        PrimitiveType::Float | PrimitiveType::Double | PrimitiveType::Boolean => None,
    }
}

pub fn exception_self_as_supertype_facade_boundary_projection() -> RustExpr {
    RustExpr::Raw(
        "Default::default()/* TODO(translation): Java exception self-as-supertype facade boundary */"
            .to_string(),
    )
}

pub fn explicit_java_facade_cast_placeholder_projection(
    source: RustExpr,
    rust_type: &str,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{{ let _java_facade_cast_source = &({}); {rust_type}::default() }}/* TODO(translation): validate explicit Java facade cast */",
        emit::emit_expr(&source)
    ))
}

pub fn render_audited_clone_recovery(rendered: &str) -> String {
    render_clone_recovery_expr(rendered, &CloneRecoveryPolicy::AuditRequired)
}

fn strip_arithmetic_display_conversion(expr: &RustExpr) -> String {
    if let Some(receiver) = arithmetic_display_conversion_receiver(expr) {
        return receiver;
    }
    emit::emit_expr(expr)
}

fn arithmetic_display_conversion_receiver(expr: &RustExpr) -> Option<String> {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "to_string" && args.is_empty() && rust_expr_is_arithmetic_expr(receiver) => {
            Some(emit::emit_expr(receiver))
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "to_string" && args.is_empty() => {
            let rendered_receiver = emit::emit_expr(receiver);
            if raw_contains_top_level_arithmetic_operator(&rendered_receiver) {
                Some(rendered_receiver)
            } else {
                None
            }
        }
        RustExpr::Raw(rendered) => raw_arithmetic_display_conversion_receiver(rendered),
        _ => None,
    }
}

fn rust_expr_is_arithmetic_expr(expr: &RustExpr) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::Binary { op, .. } if matches!(op.as_str(), "+" | "-" | "*" | "/" | "%")
    )
}

fn raw_arithmetic_display_conversion_receiver(rendered: &str) -> Option<String> {
    let receiver = rendered.trim().strip_suffix(".to_string()")?.trim_end();
    if balanced_outer_parentheses_wrap_all(receiver)
        && raw_contains_top_level_arithmetic_operator(&receiver[1..receiver.len() - 1])
    {
        Some(receiver.to_string())
    } else if receiver.ends_with(')') && raw_contains_top_level_arithmetic_operator(receiver) {
        Some(receiver.to_string())
    } else {
        None
    }
}

fn borrow_projection_already_satisfied(
    expr: &RustExpr,
    actual: Option<&RustSlotShape>,
    target: Option<&RustSlotShape>,
    use_case: BorrowProjectionUse,
) -> bool {
    if matches!(
        expr,
        RustExpr::Unary { op, .. } if op == "&" || op == "&mut "
    ) {
        return true;
    }
    let rendered = emit::emit_expr(expr);
    let trimmed = rendered.trim_start();
    if trimmed.starts_with('&') {
        return true;
    }
    match use_case {
        BorrowProjectionUse::MapKey => {
            rust_expr_is_already_borrowed_map_key(expr)
                || actual.is_some_and(map_key_shape_is_borrowed_string_view)
                || (actual.is_some_and(|shape| shape.ownership == OwnershipShape::SharedBorrow)
                    && rendered_expr_is_simple_local_path(&rendered))
        }
        BorrowProjectionUse::StringView => expr_is_string_view(expr),
        BorrowProjectionUse::SliceView => expr_is_slice_view(expr),
        BorrowProjectionUse::MutableSliceView => {
            matches!(
                expr.recorded_shape(),
                Some(RustExprShape::MutableSliceView | RustExprShape::MutBorrowed)
            ) || target.is_some_and(|target| {
                target.ownership == OwnershipShape::MutableBorrow
                    && actual
                        .is_some_and(|actual| actual.ownership == OwnershipShape::MutableBorrow)
            })
        }
        BorrowProjectionUse::Argument
        | BorrowProjectionUse::Display
        | BorrowProjectionUse::MethodReceiver => {
            matches!(
                expr.recorded_shape(),
                Some(RustExprShape::Borrowed | RustExprShape::MutBorrowed)
            ) || target.is_some_and(|target| {
                actual.is_some_and(|actual| actual.ownership == target.ownership)
                    && matches!(
                        target.ownership,
                        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                    )
            })
        }
    }
}

fn borrow_expr(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(raw) => RustExpr::Raw(format!("&{}", parenthesize_borrow_operand(&raw))),
        other => RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(other),
        },
    }
}

fn borrow_projection_operand(expr: RustExpr) -> String {
    match expr {
        RustExpr::Raw(raw) => parenthesize_borrow_operand(&raw),
        other => emit::emit_expr(&other),
    }
}

fn parenthesize_borrow_operand(rendered: &str) -> String {
    let trimmed = rendered.trim();
    if rendered_borrow_operand_is_prefix_safe(trimmed) {
        rendered.to_string()
    } else {
        format!("({rendered})")
    }
}

fn rendered_borrow_operand_is_prefix_safe(rendered: &str) -> bool {
    if rendered.starts_with('"') || raw_string_literal_end_at(rendered, 0).is_some() {
        return rendered_expr_is_complete_string_literal(rendered);
    }
    rendered
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.'))
        || rendered.starts_with("self.")
        || rendered_expr_is_complete_string_literal(rendered)
        || rendered_expr_is_simple_index_chain(rendered)
        || rendered.ends_with(')')
}

fn rendered_expr_is_complete_string_literal(rendered: &str) -> bool {
    normal_string_literal_end_at(rendered, 0) == Some(rendered.len())
        || raw_string_literal_end_at(rendered, 0) == Some(rendered.len())
}

fn normal_string_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    if !rest.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    for (offset, ch) in rest[1..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some(idx + 1 + offset + ch.len_utf8()),
            _ => {}
        }
    }
    None
}

fn rendered_expr_is_simple_index_chain(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    let Some(first_open) = trimmed.find('[') else {
        return false;
    };
    let base = trimmed[..first_open].trim();
    if base.is_empty()
        || !base
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.' | '#'))
    {
        return false;
    }

    let mut depth = 0usize;
    let mut saw_index = false;
    for (index, ch) in trimmed[first_open..].char_indices() {
        match ch {
            '[' => {
                if depth == 0 && index != 0 {
                    let between = trimmed[first_open..first_open + index].trim();
                    if !between.is_empty() {
                        return false;
                    }
                }
                depth += 1;
                saw_index = true;
            }
            ']' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ if depth == 0 && !ch.is_whitespace() => return false,
            _ => {}
        }
    }
    saw_index && depth == 0
}

fn rendered_expr_is_already_borrowed_map_key(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()")
        || rendered_expr_is_complete_string_literal(trimmed)
        || trimmed.starts_with("java_string_")
}

fn rust_expr_is_already_borrowed_map_key(expr: &RustExpr) -> bool {
    if expr.recorded_shape() == Some(RustExprShape::StringView) {
        return true;
    }
    match expr.unshaped() {
        RustExpr::Literal(literal) => rendered_expr_is_complete_string_literal(literal),
        RustExpr::MethodCall { method, args, .. } if method == "as_str" && args.is_empty() => true,
        RustExpr::Raw(rendered) => rendered_expr_is_already_borrowed_map_key(rendered),
        _ => false,
    }
}

fn map_key_shape_is_borrowed_string_view(shape: &RustSlotShape) -> bool {
    shape.java_ty == JavaType::String
        && shape.ownership == OwnershipShape::SharedBorrow
        && shape.view == ViewShape::Str
}

fn rendered_expr_is_simple_local_path(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#'))
}

fn rendered_expr_is_associated_path_without_call(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.contains("::")
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '#'))
}

/// Whether an expression is already a `&str`.
///
/// Asks the shape recorded when the expression was built, and only falls back
/// to matching the rendered text when nothing was recorded. Every producer that
/// gains a tag is one shape this question no longer has to guess at; until they
/// all have one, the textual answer is still needed and is still correct for
/// what it can see.
pub fn expr_is_string_view(expr: &RustExpr) -> bool {
    expr.recorded_shape() == Some(RustExprShape::StringView)
        || rust_expr_is_string_view_projection(expr)
        || matches!(
            expr.unshaped(),
            RustExpr::Raw(rendered) if rendered_expr_is_already_string_view(rendered)
        )
}

fn rust_expr_is_string_view_projection(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall { method, args, .. } if method == "as_str" && args.is_empty() => true,
        RustExpr::Literal(literal) => rendered_expr_is_complete_string_literal(literal),
        _ => false,
    }
}

/// The text-only form of [`expr_is_string_view`], for consumers that never had
/// the expression — only its rendering.
pub fn rendered_expr_is_already_string_view(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()") || rendered_expr_is_complete_string_literal(trimmed)
}

/// A `&str` produced by an adapter, tagged so the next consumer does not have
/// to recognise the spelling.
pub fn string_view_expr(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(RustExprShape::StringView, expr)
}

/// Whether an expression is already a slice view.
///
/// Producers that apply shared slice-view adaptation record `SliceView`; raw
/// text ending in `.as_slice()` is no longer semantic evidence here.
pub fn expr_is_slice_view(expr: &RustExpr) -> bool {
    expr.recorded_shape() == Some(RustExprShape::SliceView)
}

/// A slice view produced by an adapter.
pub fn slice_view_expr(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(RustExprShape::SliceView, expr)
}

/// A mutable slice view produced by an adapter.
pub fn mutable_slice_view_expr(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(RustExprShape::MutableSliceView, expr)
}

fn simple_deref_projection_source(rendered: &str) -> Option<&str> {
    let source = rendered.trim().strip_prefix('*')?;
    (!source.is_empty()
        && source
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
    .then_some(source)
}

pub fn apply_instanceof_check(
    expr: RustExpr,
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> RustExpr {
    render_instanceof_check(
        expr,
        &crate::conversion_plan::plan_instanceof_check(actual, target),
    )
}

pub fn render_instanceof_check(expr: RustExpr, check: &InstanceOfCheck) -> RustExpr {
    match check {
        InstanceOfCheck::JavaSerializableMarker => function_call_projection(
            RustExpr::path(["java_is_serializable"]),
            vec![force_shared_borrow_projection(expr)],
        ),
        InstanceOfCheck::ClosedEnumVariant {
            enum_name,
            variant_name,
        } => RustExpr::Raw(format!(
            "matches!({}, {enum_name}::{variant_name}(_))",
            emit::emit_expr(&expr)
        )),
        InstanceOfCheck::DynTraitType { target_type } => RustExpr::Raw(format!(
            "{}.as_any().is::<{target_type}>()",
            emit::emit_expr(&expr)
        )),
        InstanceOfCheck::BoxedDynTraitType { target_type } => RustExpr::Raw(format!(
            "{}.as_ref().as_any().is::<{target_type}>()",
            emit::emit_expr(&expr)
        )),
        InstanceOfCheck::ErasedObjectType { target_type } => {
            RustExpr::Raw(format!("{}.is::<{target_type}>()", emit::emit_expr(&expr)))
        }
        InstanceOfCheck::OptionalErasedObjectType { target_type } => RustExpr::Raw(format!(
            "{}.as_ref().is_some_and(|value| value.is::<{target_type}>())",
            emit::emit_expr(&expr)
        )),
        InstanceOfCheck::AlwaysTrue => RustExpr::Literal("true".to_string()),
        InstanceOfCheck::Unsupported {
            actual_dispatch,
            target_dispatch,
            policy,
        } => render_instanceof_fallback_policy(policy, actual_dispatch, target_dispatch),
    }
}

pub fn instanceof_pattern_downcast_ref_projection(
    expr: RustExpr,
    check: &InstanceOfCheck,
) -> Option<RustExpr> {
    let rendered = parenthesize_cast_method_receiver(&emit::emit_expr(&expr));
    match check {
        InstanceOfCheck::ClosedEnumVariant {
            enum_name,
            variant_name,
        } => Some(RustExpr::Raw(format!(
            "match &{rendered} {{ {enum_name}::{variant_name}(value) => Some(value), _ => None }}"
        ))),
        InstanceOfCheck::DynTraitType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(RustExpr::Raw(format!(
                "{rendered}.as_any().downcast_ref::<{target_type}>()"
            )))
        }
        InstanceOfCheck::BoxedDynTraitType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(RustExpr::Raw(format!(
                "{rendered}.as_ref().as_any().downcast_ref::<{target_type}>()"
            )))
        }
        InstanceOfCheck::ErasedObjectType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(RustExpr::Raw(format!(
                "{rendered}.downcast_ref::<{target_type}>()"
            )))
        }
        InstanceOfCheck::OptionalErasedObjectType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(RustExpr::Raw(format!(
                "{rendered}.as_ref().and_then(|value| value.downcast_ref::<{target_type}>())"
            )))
        }
        _ => None,
    }
}

impl InstanceOfCheck {
    pub fn diagnostics(&self) -> Vec<CoercionDiagnostic> {
        match self {
            InstanceOfCheck::Unsupported {
                actual_dispatch,
                target_dispatch,
                policy,
            } => instanceof_fallback_diagnostics(policy, actual_dispatch, target_dispatch),
            _ => Vec::new(),
        }
    }
}

fn instanceof_fallback_diagnostics(
    policy: &InstanceOfFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> Vec<CoercionDiagnostic> {
    let message_policy =
        instanceof_fallback_message_policy(policy, actual_dispatch, target_dispatch);
    vec![message_policy.into_diagnostic()]
}

pub fn instanceof_fallback_message_policy(
    policy: &InstanceOfFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> UnsupportedCoercionMessagePolicy {
    match policy {
        InstanceOfFallbackPolicy::CompileError => UnsupportedCoercionMessagePolicy {
            code: "unsupported_instanceof_shape",
            message: format!(
                "unsupported Java instanceof shape: actual={actual_dispatch:?}, target={target_dispatch:?}"
            ),
        },
        InstanceOfFallbackPolicy::SourceHierarchyRuntimeCarrierRequired { actual, target } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_instanceof_source_hierarchy_runtime_carrier",
                message: format!(
                    "unsupported Java instanceof check from source supertype {actual:?} to subtype {target:?}: a member-preserving runtime carrier is required"
                ),
            }
        }
        InstanceOfFallbackPolicy::PrimitiveTarget { java_ty } => UnsupportedCoercionMessagePolicy {
            code: "unsupported_instanceof_primitive_target",
            message: format!(
                "unsupported Java instanceof check: primitive target type {java_ty:?}"
            ),
        },
        InstanceOfFallbackPolicy::RawOrWildcardTarget { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_instanceof_raw_or_wildcard_target",
                message: format!(
                    "unsupported Java instanceof check: raw or wildcard target type {java_ty:?}"
                ),
            }
        }
        InstanceOfFallbackPolicy::MissingRuntimeTarget { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_instanceof_missing_runtime_target",
                message: format!(
                    "unsupported Java instanceof check: target type {java_ty:?} has no concrete runtime type"
                ),
            }
        }
        InstanceOfFallbackPolicy::ErasedObjectToDynTrait { trait_name } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_erased_object_instanceof_dyn_trait",
                message: format!(
                    "unsupported Java erased object instanceof borrowed dyn trait check for {trait_name}; a concrete runtime target is required"
                ),
            }
        }
        InstanceOfFallbackPolicy::ErasedObjectToBoxedDynTrait { trait_name } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_erased_object_instanceof_boxed_dyn_trait",
                message: format!(
                    "unsupported Java erased object instanceof owned boxed dyn trait check for {trait_name}; a concrete runtime target is required"
                ),
            }
        }
    }
}

fn render_instanceof_fallback_policy(
    policy: &InstanceOfFallbackPolicy,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
) -> RustExpr {
    instanceof_fallback_message_policy(policy, actual_dispatch, target_dispatch)
        .compile_error_expr()
}

fn apply_borrowed_element_slot_adapter(
    expr: RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> RustExpr {
    if let Some(adapter) = &policy.adapter {
        if borrowed_option_adapter_should_unwrap_by_move(adapter, policy) {
            return method_expect_projection(expr, "Java null receiver");
        }
        if matches!(
            adapter.recovery,
            BorrowedElementRecoveryPolicy::BorrowElement
        ) {
            let already_projected_to_target = expr
                .recorded_slot_shape()
                .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &policy.target));
            if already_projected_to_target {
                return expr;
            }
            let adapted = apply_borrowed_element_borrow_adapter(expr, adapter, policy);
            return apply_borrowed_element_target_view(adapted, policy);
        }
        apply_borrowed_element_adapter(expr, adapter)
    } else {
        expr
    }
}

fn borrowed_option_adapter_should_unwrap_by_move(
    adapter: &BorrowedElementAdapterPolicy,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> bool {
    matches!(adapter.carrier, BorrowedElementCarrier::Option)
        && adapter.unwrap_carrier
        && policy.target.optional_element_type().is_none()
        && policy.carrier_shape.ownership == OwnershipShape::Owned
        && policy.target.ownership == OwnershipShape::Owned
        && adapter.element_shape.ownership == OwnershipShape::Owned
        && policy.target.java_ty == adapter.element_shape.java_ty
}

fn apply_borrowed_element_borrow_adapter(
    expr: RustExpr,
    adapter: &BorrowedElementAdapterPolicy,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> RustExpr {
    if !adapter.unwrap_carrier {
        return expr;
    }
    let rendered = emit::emit_expr(&expr);
    match (adapter.carrier, policy.target.ownership) {
        (BorrowedElementCarrier::Option, OwnershipShape::SharedBorrow)
            if adapter.element_shape.ownership == OwnershipShape::SharedBorrow =>
        {
            optional_borrowed_element_projection_expr(
                format!("{rendered}.expect(\"Java null receiver\")"),
                &adapter.element_shape,
                adapter.element_shape.ownership,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow)
            if adapter.element_shape.ownership == OwnershipShape::MutableBorrow =>
        {
            optional_borrowed_element_projection_expr(
                format!("{rendered}.expect(\"Java null receiver\")"),
                &adapter.element_shape,
                adapter.element_shape.ownership,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::SharedBorrow) => {
            optional_borrowed_element_projection_expr(
                format!("{rendered}.as_ref().expect(\"Java null receiver\")"),
                &adapter.element_shape,
                OwnershipShape::SharedBorrow,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow) => {
            optional_borrowed_element_projection_expr(
                format!("{rendered}.as_mut().expect(\"Java null receiver\")"),
                &adapter.element_shape,
                OwnershipShape::MutableBorrow,
            )
        }
        _ => method_expect_projection(expr, "Java null receiver"),
    }
}

fn optional_borrowed_element_projection_expr(
    rendered: String,
    element_shape: &RustSlotShape,
    ownership: OwnershipShape,
) -> RustExpr {
    optional_borrowed_element_projected_expr(RustExpr::Raw(rendered), element_shape, ownership)
}

fn optional_borrowed_element_projected_expr(
    expr: RustExpr,
    element_shape: &RustSlotShape,
    ownership: OwnershipShape,
) -> RustExpr {
    let mut produced = element_shape.clone();
    produced.ownership = ownership;
    RustExpr::shaped_as(RustExprShape::OptionalProjected, Some(produced), expr)
}

fn apply_borrowed_element_adapter(
    expr: RustExpr,
    policy: &BorrowedElementAdapterPolicy,
) -> RustExpr {
    let adapted = match (&policy.carrier, &policy.recovery) {
        (_, BorrowedElementRecoveryPolicy::BorrowElement) => expr,
        (
            BorrowedElementCarrier::Option,
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: clone_policy,
            },
        ) if policy.element_shape.ownership == OwnershipShape::Owned => {
            apply_owned_option_clone_adapter(expr, clone_policy)
        }
        (
            BorrowedElementCarrier::Option,
            BorrowedElementRecoveryPolicy::CloneElements {
                policy: clone_policy,
            },
        ) if borrowed_element_is_sequence_slice(&policy.element_shape) => {
            apply_borrowed_option_slice_clone_adapter(expr, clone_policy)
        }
        (BorrowedElementCarrier::Option, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
                },
            )
        }
        (BorrowedElementCarrier::Option, BorrowedElementRecoveryPolicy::CopyElements) => {
            RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "copied".to_string(),
                    args: Vec::new(),
                },
            )
        }
        (
            BorrowedElementCarrier::Option,
            BorrowedElementRecoveryPolicy::CloneElements { policy },
        ) => apply_borrowed_option_clone_adapter(expr, policy),
        (BorrowedElementCarrier::Vec, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            borrowed_collection_string_view_to_owned_projection(expr, "collect::<Vec<_>>")
        }
        (BorrowedElementCarrier::Set, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            borrowed_collection_string_view_to_owned_projection(
                expr,
                "collect::<std::collections::HashSet<_>>",
            )
        }
        (BorrowedElementCarrier::Vec, BorrowedElementRecoveryPolicy::CopyElements) => {
            borrowed_collection_copy_to_owned_projection(expr, "collect::<Vec<_>>")
        }
        (BorrowedElementCarrier::Set, BorrowedElementRecoveryPolicy::CopyElements) => {
            borrowed_collection_copy_to_owned_projection(
                expr,
                "collect::<std::collections::HashSet<_>>",
            )
        }
        (BorrowedElementCarrier::Vec, BorrowedElementRecoveryPolicy::CloneElements { policy }) => {
            apply_borrowed_vec_clone_adapter(expr, policy)
        }
        (BorrowedElementCarrier::Set, BorrowedElementRecoveryPolicy::CloneElements { policy }) => {
            apply_borrowed_set_clone_adapter(expr, policy)
        }
        (_, BorrowedElementRecoveryPolicy::Unsupported { policy: fallback }) => {
            render_borrowed_element_recovery_fallback_policy(expr, &policy.element_shape, fallback)
        }
    };
    if policy.unwrap_carrier {
        let unwrapped = method_expect_projection(adapted.clone(), "Java null receiver");
        if adapted.recorded_shape() == Some(RustExprShape::OwnedRecovery) {
            RustExpr::shaped(RustExprShape::OwnedRecovery, unwrapped)
        } else {
            unwrapped
        }
    } else {
        adapted
    }
}

fn borrowed_collection_string_view_to_owned_projection(
    expr: RustExpr,
    collect_ty: &str,
) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(borrowed_collection_into_iter_projection(expr)),
                method: "map".to_string(),
                args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
            }),
            method: collect_ty.to_string(),
            args: Vec::new(),
        },
    )
}

fn borrowed_collection_copy_to_owned_projection(expr: RustExpr, collect_ty: &str) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(borrowed_collection_into_iter_projection(expr)),
                method: "copied".to_string(),
                args: Vec::new(),
            }),
            method: collect_ty.to_string(),
            args: Vec::new(),
        },
    )
}

fn borrowed_collection_into_iter_projection(expr: RustExpr) -> RustExpr {
    let expr = match expr {
        RustExpr::Raw(raw) => RustExpr::Raw(parenthesize_borrow_operand(&raw)),
        other => other,
    };
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "into_iter".to_string(),
        args: Vec::new(),
    }
}

fn apply_borrowed_element_target_view(
    expr: RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> RustExpr {
    if expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &policy.target))
    {
        return expr;
    }
    if let Some(projected) = apply_optional_borrowed_element_target_view(&expr, policy) {
        return projected;
    }
    if let Some(projected) = apply_required_borrowed_element_target_view(&expr, policy) {
        return projected;
    }
    match &policy.target.dispatch {
        DispatchShape::DynTrait { trait_name } => {
            let trait_name = normalize_emitted_java_type_name(trait_name);
            RustExpr::shaped_as(
                RustExprShape::Borrowed,
                Some(policy.target.clone()),
                RustExpr::Raw(format!("{} as &dyn {trait_name}", emit::emit_expr(&expr))),
            )
        }
        DispatchShape::Concrete if policy.target.java_ty != policy.element_shape.java_ty => {
            match &policy.target.java_ty {
                crate::java_ir::JavaType::Named { name, .. }
                | crate::java_ir::JavaType::Unknown(name) => {
                    let target_type = crate::slot_render::rust_name_for_java_named_type(name);
                    if is_native_collection_facade_target(&target_type) {
                        expr
                    } else {
                        RustExpr::shaped_as(
                            RustExprShape::Borrowed,
                            Some(policy.target.clone()),
                            RustExpr::Raw(format!(
                                "{{ let _java_value = {}; &{} }}",
                                emit::emit_expr(&expr),
                                target_type,
                            )),
                        )
                    }
                }
                _ => expr,
            }
        }
        _ => expr,
    }
}

fn apply_optional_borrowed_element_target_view(
    expr: &RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> Option<RustExpr> {
    if !matches!(policy.carrier, BorrowedElementCarrier::Option) {
        return None;
    }
    let target_element_ty = policy.target.optional_element_type()?;
    let target_element =
        crate::conversion_plan::optional_element_slot_shape(&policy.target, target_element_ty);
    // The question every arm below asks is "has *this* adapter already been
    // applied", and `RustExprShape::OptionalProjected` cannot answer it: all
    // five arms are in that one family. The recorded **slot shape** can, because
    // it is what each arm produces.
    //
    // Note the direction of the danger here, which is the opposite of the other
    // migrations in 0007: `true` means *skip the adapter*, so a tag that is
    // right about the family and wrong about the member emits a **missing**
    // adapter. The text check stays as the fallback for producers that have not
    // recorded an `actual` yet, so an untagged expression is still handled
    // exactly as before.
    let already_applied = expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &target_element));
    if already_applied {
        return Some(expr.clone());
    }
    let adapted = match (target_element.ownership, &target_element.view) {
        (OwnershipShape::SharedBorrow, ViewShape::Slice)
            if matches!(target_element.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            zero_arg_method_projection(expr.clone(), "as_deref")
        }
        (OwnershipShape::MutableBorrow, ViewShape::Slice)
            if matches!(target_element.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            zero_arg_method_projection(expr.clone(), "as_deref_mut")
        }
        (OwnershipShape::SharedBorrow, ViewShape::Str)
            if target_element.java_ty == crate::java_ir::JavaType::String =>
        {
            optional_string_view_projection_expr(expr.clone())
        }
        (OwnershipShape::SharedBorrow, ViewShape::Native) => {
            zero_arg_method_projection(expr.clone(), "as_ref")
        }
        (OwnershipShape::MutableBorrow, ViewShape::Native) => {
            zero_arg_method_projection(expr.clone(), "as_mut")
        }
        _ => return None,
    };
    // Record what this arm produced, so the next consumer asks the tag rather
    // than the text.
    Some(RustExpr::shaped_as(
        RustExprShape::OptionalProjected,
        Some(target_element),
        adapted,
    ))
}

fn slot_shapes_match_ignoring_identity(actual: &RustSlotShape, target: &RustSlotShape) -> bool {
    actual.java_ty == target.java_ty
        && actual.ownership == target.ownership
        && actual.view == target.view
        && actual.dispatch == target.dispatch
        && actual.mutability == target.mutability
}

fn apply_required_borrowed_element_target_view(
    expr: &RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> Option<RustExpr> {
    if policy.target.optional_element_type().is_some() {
        return None;
    }
    let already_projected = expr
        .recorded_slot_shape()
        .is_some_and(|actual| slot_shapes_match_ignoring_identity(actual, &policy.target));
    if already_projected {
        return Some(expr.clone());
    }
    if required_borrowed_element_already_matches_target(policy) {
        return None;
    }
    let rendered = emit::emit_expr(expr);
    match (policy.target.ownership, &policy.target.view) {
        (OwnershipShape::SharedBorrow, ViewShape::Slice)
            if matches!(policy.target.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            Some(RustExpr::shaped_as(
                RustExprShape::SliceView,
                Some(policy.target.clone()),
                zero_arg_method_projection(expr.clone(), "as_slice"),
            ))
        }
        (OwnershipShape::MutableBorrow, ViewShape::Slice)
            if matches!(policy.target.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            Some(RustExpr::shaped_as(
                RustExprShape::MutableSliceView,
                Some(policy.target.clone()),
                zero_arg_method_projection(expr.clone(), "as_mut_slice"),
            ))
        }
        (OwnershipShape::SharedBorrow, ViewShape::Str)
            if policy.target.java_ty == crate::java_ir::JavaType::String =>
        {
            if let Some(borrowed_erased_view) =
                optional_erased_string_downcast_ref_view_projection(&rendered)
            {
                return Some(RustExpr::shaped_as(
                    RustExprShape::StringView,
                    Some(policy.target.clone()),
                    RustExpr::Raw(borrowed_erased_view),
                ));
            }
            Some(RustExpr::shaped_as(
                RustExprShape::StringView,
                Some(policy.target.clone()),
                zero_arg_method_projection(expr.clone(), "as_str"),
            ))
        }
        _ => None,
    }
}

fn optional_erased_string_downcast_ref_view_projection(rendered: &str) -> Option<String> {
    let local_name = rendered.strip_suffix(&format!(
        ".unwrap().downcast::<String>(){}",
        crate::output_markers::invalid_java_cast_expect()
    ))?;
    if local_name.is_empty()
        || !local_name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(format!(
        "{local_name}.as_ref().and_then(|value| value.downcast_ref::<String>()){}.as_str()",
        crate::output_markers::invalid_java_cast_expect()
    ))
}

fn required_borrowed_element_already_matches_target(
    policy: &BorrowedElementSlotAdapterPolicy,
) -> bool {
    policy.element_shape.java_ty == policy.target.java_ty
        && policy.element_shape.ownership == policy.target.ownership
        && policy.element_shape.view == policy.target.view
        && policy.element_shape.dispatch == policy.target.dispatch
}

fn apply_borrowed_option_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    match policy {
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "cloned".to_string(),
                args: Vec::new(),
            },
        ),
        CloneRecoveryPolicy::AuditRequired => RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{}.cloned(){}",
                emit::emit_expr(&expr),
                render_clone_recovery_policy(policy)
            )),
        ),
    }
}

fn apply_owned_option_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    match policy {
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "clone".to_string(),
                args: Vec::new(),
            },
        ),
        CloneRecoveryPolicy::AuditRequired => RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{}.clone(){}",
                emit::emit_expr(&expr),
                render_clone_recovery_policy(policy)
            )),
        ),
    }
}

fn borrowed_element_is_sequence_slice(element_shape: &RustSlotShape) -> bool {
    matches!(element_shape.java_ty, crate::java_ir::JavaType::Array(_))
        && element_shape.ownership == OwnershipShape::SharedBorrow
        && element_shape.view == ViewShape::Slice
}

fn apply_borrowed_option_slice_clone_adapter(
    expr: RustExpr,
    policy: &CloneRecoveryPolicy,
) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!(
            "{rendered}.map(|value| value.to_vec()){}",
            render_clone_recovery_policy(policy)
        )),
    )
}

fn apply_borrowed_vec_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery) {
        return expr;
    }
    if rust_expr_is_cloned_collect_call(&expr, "collect::<Vec<_>>") {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let rendered = emit::emit_expr(&expr);
    if rendered.ends_with(".cloned().collect::<Vec<_>>()") {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    if let Some(iterator) = rendered.strip_suffix(".collect::<Vec<_>>()") {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{iterator}.cloned(){}.collect::<Vec<_>>()",
                render_clone_recovery_policy(policy)
            )),
        );
    }
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!(
            "{rendered}.into_iter().cloned(){}.collect::<Vec<_>>()",
            render_clone_recovery_policy(policy)
        )),
    )
}

fn apply_borrowed_set_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery) {
        return expr;
    }
    if rust_expr_is_cloned_collect_call(&expr, "collect::<std::collections::HashSet<_>>") {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let rendered = emit::emit_expr(&expr);
    if rendered.ends_with(".cloned().collect::<std::collections::HashSet<_>>()") {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    if let Some(iterator) = rendered.strip_suffix(".collect::<std::collections::HashSet<_>>()") {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw(format!(
                "{iterator}.cloned(){}.collect::<std::collections::HashSet<_>>()",
                render_clone_recovery_policy(policy)
            )),
        );
    }
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!(
            "{rendered}.into_iter().cloned(){}.collect::<std::collections::HashSet<_>>()",
            render_clone_recovery_policy(policy)
        )),
    )
}

fn rust_expr_is_cloned_collect_call(expr: &RustExpr, collect_method: &str) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == collect_method && args.is_empty() => {
            matches!(
                receiver.unshaped(),
                RustExpr::MethodCall { method, args, .. }
                    if method == "cloned" && args.is_empty()
            )
        }
        _ => false,
    }
}

impl BorrowedElementRecoveryPolicy {
    pub fn diagnostics(&self, element_shape: &RustSlotShape) -> Vec<CoercionDiagnostic> {
        match self {
            BorrowedElementRecoveryPolicy::CloneElements { policy } => {
                borrowed_element_clone_recovery_diagnostics(policy)
            }
            BorrowedElementRecoveryPolicy::Unsupported { policy } => {
                borrowed_element_recovery_fallback_diagnostics(policy, element_shape)
            }
            _ => Vec::new(),
        }
    }
}

fn borrowed_element_clone_recovery_diagnostics(
    policy: &CloneRecoveryPolicy,
) -> Vec<CoercionDiagnostic> {
    match policy {
        CloneRecoveryPolicy::AuditRequired => vec![CoercionDiagnostic {
            code: "borrowed_element_clone_audit_required",
            message:
                "borrowed Java element recovery inserted cloned() and requires ownership audit"
                    .to_string(),
        }],
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => Vec::new(),
    }
}

fn borrowed_element_recovery_fallback_diagnostics(
    policy: &BorrowedElementRecoveryFallbackPolicy,
    element_shape: &RustSlotShape,
) -> Vec<CoercionDiagnostic> {
    vec![borrowed_element_recovery_fallback_message_policy(policy, element_shape).into_diagnostic()]
}

pub fn borrowed_element_recovery_fallback_message_policy(
    policy: &BorrowedElementRecoveryFallbackPolicy,
    element_shape: &RustSlotShape,
) -> UnsupportedCoercionMessagePolicy {
    match policy {
        BorrowedElementRecoveryFallbackPolicy::CompileError => UnsupportedCoercionMessagePolicy {
            code: "unsupported_borrowed_element_recovery",
            message: format!(
                "unsupported Java borrowed element ownership recovery: type={:?}",
                element_shape.java_ty
            ),
        },
        BorrowedElementRecoveryFallbackPolicy::NonCloneableElement { java_ty } => {
            UnsupportedCoercionMessagePolicy {
                code: "unsupported_borrowed_element_non_cloneable",
                message: format!(
                    "unsupported Java borrowed element ownership recovery for non-cloneable type {java_ty:?}"
                ),
            }
        }
    }
}

fn render_borrowed_element_recovery_fallback_policy(
    expr: RustExpr,
    element_shape: &RustSlotShape,
    policy: &BorrowedElementRecoveryFallbackPolicy,
) -> RustExpr {
    borrowed_element_recovery_fallback_message_policy(policy, element_shape)
        .compile_error_expr_consuming(&expr)
}

fn apply_one(expr: RustExpr, coercion: &Coercion) -> RustExpr {
    match coercion {
        Coercion::Identity => expr,
        Coercion::IndexToUsize if matches!(expr.unshaped(), RustExpr::Raw(_)) => {
            RustExpr::Raw(format!(
                "{} as usize",
                render_cast_operand_with_structural_parentheses(&expr)
            ))
        }
        Coercion::IndexToUsize => RustExpr::Cast {
            expr: Box::new(expr),
            ty: "usize".to_string(),
        },
        Coercion::ToStringForConcat => apply_java_display_plan(expr, JavaDisplayPlan::ConcatValue),
        Coercion::ToJavaFloatStringForConcat { double } => {
            apply_java_display_plan(expr, JavaDisplayPlan::ConcatFloatValue { double: *double })
        }
        Coercion::ErasedObjectToJavaDisplayString => {
            apply_java_display_plan(expr, JavaDisplayPlan::ErasedObject)
        }
        Coercion::OptionalToJavaDisplayString => {
            apply_java_display_plan(expr, JavaDisplayPlan::OptionalString)
        }
        Coercion::OptionalToJavaDisplayValue => {
            apply_java_display_plan(expr, JavaDisplayPlan::OptionalValue)
        }
        Coercion::OptionalBorrowedToJavaDisplayValue => {
            apply_java_display_plan(expr, JavaDisplayPlan::OptionalBorrowedValue)
        }
        Coercion::AsStr => {
            let rendered = emit::emit_expr(&expr);
            if let Some(borrowed_item) = simple_deref_projection_source(&rendered) {
                string_view_expr(RustExpr::Raw(format!("{borrowed_item}.as_str()")))
            } else if let Some(borrowed_item) = rendered.trim().strip_prefix('*') {
                string_view_expr(RustExpr::Raw(format!("{borrowed_item}.as_str()")))
            } else if matches!(expr.unshaped(), RustExpr::Raw(_)) {
                string_view_expr(RustExpr::Raw(format!("{rendered}.as_str()")))
            } else {
                string_view_expr(RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "as_str".to_string(),
                    args: Vec::new(),
                })
            }
        }
        Coercion::ToOwnedString => {
            let rendered = emit::emit_expr(&expr);
            if rendered.ends_with(".to_string()") {
                RustExpr::shaped(RustExprShape::OwnedRecovery, expr)
            } else {
                RustExpr::shaped(
                    RustExprShape::OwnedRecovery,
                    RustExpr::Raw(format!("{rendered}.to_string()")),
                )
            }
        }
        Coercion::AsSlice => {
            let rendered = emit::emit_expr(&expr);
            if rendered_expr_is_associated_path_without_call(&rendered) {
                slice_view_expr(RustExpr::Raw(rendered))
            } else if matches!(expr.unshaped(), RustExpr::Raw(_)) {
                slice_view_expr(RustExpr::Raw(format!("{rendered}.as_slice()")))
            } else {
                slice_view_expr(RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "as_slice".to_string(),
                    args: Vec::new(),
                })
            }
        }
        Coercion::AsMutSlice => {
            if matches!(expr.unshaped(), RustExpr::Raw(_)) {
                mutable_slice_view_expr(RustExpr::Raw(format!(
                    "{}.as_mut_slice()",
                    emit::emit_expr(&expr)
                )))
            } else {
                mutable_slice_view_expr(RustExpr::MethodCall {
                    receiver: Box::new(expr),
                    method: "as_mut_slice".to_string(),
                    args: Vec::new(),
                })
            }
        }
        Coercion::WrapClosedEnum {
            enum_name,
            variant_name,
        } => {
            let rendered = emit::emit_expr(&expr);
            if java_type_param_variant_name(variant_name) {
                let root_variant = enum_name.strip_suffix("Value").unwrap_or(enum_name);
                RustExpr::Raw(format!("{enum_name}::{root_variant}({rendered}.into())"))
            } else {
                RustExpr::Raw(format!("{enum_name}::{variant_name}({rendered})"))
            }
        }
        Coercion::BorrowDynTrait { trait_name } => borrow_as_dyn_trait(expr, trait_name),
        Coercion::BorrowBoxedDynTrait { trait_name } => borrow_boxed_as_dyn_trait(expr, trait_name),
        Coercion::BorrowConcreteFacade { target_type } => {
            if is_native_collection_facade_target(target_type) {
                ensure_borrow_projection(expr, None, None, BorrowProjectionUse::Argument)
            } else {
                RustExpr::shaped(
                    RustExprShape::Borrowed,
                    RustExpr::Raw(format!(
                        "{{ let _java_value = {}; &{target_type} }}",
                        emit::emit_expr(&expr)
                    )),
                )
            }
        }
        Coercion::ExternalInto { .. } => into_projection(expr),
        Coercion::BoxDynTrait { trait_name } => RustExpr::Raw(format!(
            "Box::new({}) as Box<dyn {trait_name}>",
            emit::emit_expr(&expr)
        )),
        Coercion::CloneBoxDynTrait { .. } => {
            let rendered = emit::emit_expr(&expr);
            if expr.recorded_shape() == Some(RustExprShape::AnonymousDynTrait) {
                expr
            } else if rendered_expr_is_anonymous_dyn_trait_constructor(&rendered) {
                RustExpr::Raw(rendered)
            } else {
                let rendered = parenthesize_cast_method_receiver(&rendered);
                RustExpr::shaped(
                    RustExprShape::OwnedRecovery,
                    RustExpr::Raw(format!("{rendered}.clone_box()")),
                )
            }
        }
        Coercion::EraseObject => RustExpr::Raw(format!(
            "Box::new({}) as Box<dyn std::any::Any>",
            emit::emit_expr(&expr)
        )),
        Coercion::DowncastClosedEnum {
            enum_name,
            variant_name,
            invalid_cast,
        } => RustExpr::Raw(format!(
            "match {} {{ {enum_name}::{variant_name}(value) => value, _ => {} }}",
            emit::emit_expr(&expr),
            render_invalid_cast_policy(invalid_cast)
        )),
        Coercion::DowncastDynTrait {
            target_type,
            invalid_cast,
        } => apply_dyn_trait_downcast(expr, target_type, invalid_cast),
        Coercion::DowncastBoxedDynTrait {
            target_type,
            invalid_cast,
        } => apply_boxed_dyn_trait_downcast(expr, target_type, invalid_cast),
        Coercion::DowncastErasedObject {
            target_type,
            invalid_cast,
        } => apply_erased_object_downcast(expr, target_type, invalid_cast),
        Coercion::UnsupportedDispatch {
            actual_dispatch,
            target_dispatch,
            policy,
        } => apply_unsupported_dispatch_coercion(expr, actual_dispatch, target_dispatch, policy),
        Coercion::WidenPrimitive { from, to } => {
            if *to == PrimitiveType::Char {
                return java_char_from_u32_mask_projection(expr);
            }
            if *from == PrimitiveType::Char {
                return java_char_to_numeric_projection(expr, *to);
            }
            RustExpr::Cast {
                expr: Box::new(expr),
                ty: rust_primitive_type_name_policy(*to).rust_name.to_string(),
            }
        }
        Coercion::WrapOption => render_raw_option_some(expr),
        Coercion::UnwrapOption => zero_arg_method_projection(expr, "unwrap"),
        Coercion::UnwrapOptionBorrowed => RustExpr::shaped(
            RustExprShape::Borrowed,
            method_expect_projection(
                zero_arg_method_projection(expr, "as_ref"),
                "Java null receiver",
            ),
        ),
        Coercion::UnwrapOptionMutBorrowed => RustExpr::shaped(
            RustExprShape::MutBorrowed,
            method_expect_projection(
                zero_arg_method_projection(expr, "as_mut"),
                "Java null receiver",
            ),
        ),
        Coercion::Borrow => RustExpr::shaped(
            RustExprShape::Borrowed,
            ensure_borrow_projection(expr, None, None, BorrowProjectionUse::Argument),
        ),
        Coercion::BorrowMut => RustExpr::shaped(
            RustExprShape::MutBorrowed,
            ensure_borrow_projection(expr, None, None, BorrowProjectionUse::MutableSliceView),
        ),
        Coercion::CopyBorrowedValue => {
            if expr_is_string_view(&expr) {
                return expr;
            }
            deref_borrowed_projection(expr)
        }
        Coercion::CloneErasedObject { sequence } => {
            let helper = if *sequence {
                "java_clone_erased_object_vec_ref"
            } else {
                "java_clone_erased_object_ref"
            };
            clone_erased_object_helper_once(expr, helper)
        }
        Coercion::ToOwnedSequence { policy } => apply_to_owned_sequence(expr, policy),
        Coercion::CloneRecovery { policy } => apply_clone_recovery(expr, policy),
        Coercion::UnsupportedOwnership {
            actual_ownership,
            target_ownership,
            policy,
        } => {
            apply_unsupported_ownership_coercion(expr, *actual_ownership, *target_ownership, policy)
        }
    }
}

fn java_type_param_variant_name(name: &str) -> bool {
    name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase())
}

fn render_option_some(expr: RustExpr) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(expr);
    RustExpr::shaped(
        RustExprShape::OptionalConstructed,
        normalize_optional_constructed_string_view_expr(RustExpr::Call {
            function: Box::new(RustExpr::path(["Some"])),
            args: vec![expr],
        }),
    )
}

fn render_raw_option_some(expr: RustExpr) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(expr);
    RustExpr::shaped(
        RustExprShape::OptionalConstructed,
        normalize_optional_constructed_string_view_expr(RustExpr::Raw(format!(
            "Some({})",
            emit::emit_expr(&expr)
        ))),
    )
}

fn render_cast_operand_with_structural_parentheses(expr: &RustExpr) -> String {
    match expr.unshaped() {
        RustExpr::Binary { .. } => format!("({})", emit::emit_expr(expr)),
        RustExpr::Raw(rendered) => parenthesize_compound_cast_operand(rendered),
        _ => emit::emit_expr(expr).trim().to_string(),
    }
}

fn parenthesize_compound_cast_operand(rendered: &str) -> String {
    let trimmed = rendered.trim();
    if balanced_outer_parentheses_wrap_all(trimmed) {
        return trimmed.to_string();
    }
    if raw_contains_top_level_binary_operator(trimmed) {
        format!("({trimmed})")
    } else {
        trimmed.to_string()
    }
}

fn raw_contains_top_level_binary_operator(raw: &str) -> bool {
    raw_contains_top_level_operator_matching(raw, top_level_binary_operator_at)
}

fn raw_contains_top_level_arithmetic_operator(raw: &str) -> bool {
    raw_contains_top_level_operator_matching(raw, top_level_arithmetic_operator_at)
}

fn raw_contains_top_level_operator_matching(
    raw: &str,
    operator_at: fn(&str, usize, char, Option<char>) -> bool,
) -> bool {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = 0usize;
    let mut prev_significant: Option<char> = None;
    for (idx, ch) in raw.char_indices() {
        if idx < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            skip_until = end;
            prev_significant = Some(')');
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {
                if paren_depth == 0
                    && bracket_depth == 0
                    && brace_depth == 0
                    && operator_at(raw, idx, ch, prev_significant)
                {
                    return true;
                }
            }
        }
        if !ch.is_whitespace() {
            prev_significant = Some(ch);
        }
    }
    false
}

fn top_level_arithmetic_operator_at(
    raw: &str,
    idx: usize,
    ch: char,
    prev_significant: Option<char>,
) -> bool {
    if !top_level_operator_has_left_operand(prev_significant) {
        return false;
    }
    match ch {
        '+' | '*' | '/' | '%' => true,
        '-' => raw[idx + ch.len_utf8()..].trim_start().len() > 0,
        _ => false,
    }
}

fn top_level_binary_operator_at(
    raw: &str,
    idx: usize,
    ch: char,
    prev_significant: Option<char>,
) -> bool {
    if !top_level_operator_has_left_operand(prev_significant) {
        return false;
    }
    match ch {
        '+' | '*' | '/' | '%' | '^' => true,
        '-' => true,
        '&' => raw[idx..].starts_with("&&") || raw[idx + ch.len_utf8()..].trim_start().len() > 0,
        '|' => raw[idx..].starts_with("||") || raw[idx + ch.len_utf8()..].trim_start().len() > 0,
        '<' => raw[idx..].starts_with("<<"),
        '>' => raw[idx..].starts_with(">>"),
        _ => false,
    }
}

fn top_level_operator_has_left_operand(prev_significant: Option<char>) -> bool {
    prev_significant.is_some_and(|prev| {
        !matches!(
            prev,
            '(' | '['
                | '{'
                | ','
                | ':'
                | '?'
                | '='
                | '!'
                | '<'
                | '>'
                | '&'
                | '|'
                | '+'
                | '-'
                | '*'
                | '/'
                | '%'
        )
    })
}

fn balanced_outer_parentheses_wrap_all(raw: &str) -> bool {
    if !raw.starts_with('(') || !raw.ends_with(')') {
        return false;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            '(' => depth += 1,
            ')' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
                if depth == 0 && index != raw.len() - 1 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn is_native_collection_facade_target(target_type: &str) -> bool {
    matches!(
        target_type,
        "Collection"
            | "List"
            | "ArrayList"
            | "Set"
            | "HashSet"
            | "Map"
            | "HashMap"
            | "LinkedHashMap"
    )
}

fn apply_unsupported_dispatch_coercion(
    expr: RustExpr,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
    policy: &DispatchCoercionFallbackPolicy,
) -> RustExpr {
    dispatch_coercion_fallback_message_policy(policy, actual_dispatch, target_dispatch)
        .compile_error_expr_consuming(&expr)
}

fn render_invalid_cast_policy(policy: &InvalidCastPolicy) -> String {
    match policy {
        InvalidCastPolicy::Panic => crate::output_markers::invalid_java_cast_panic(),
    }
}

fn apply_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    let rendered = parenthesize_cast_method_receiver(&emit::emit_expr(&expr));
    let target_type = normalize_emitted_java_type_name(target_type);
    match invalid_cast {
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "{rendered}.as_any().downcast_ref::<{target_type}>(){}",
            crate::output_markers::invalid_java_cast_expect()
        )),
    }
}

fn apply_boxed_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    let rendered = parenthesize_cast_method_receiver(&emit::emit_expr(&expr));
    let target_type = normalize_emitted_java_type_name(target_type);
    match invalid_cast {
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "{rendered}.as_ref().as_any().downcast_ref::<{target_type}>(){}",
            crate::output_markers::invalid_java_cast_expect()
        )),
    }
}

pub fn erased_object_borrowed_downcast_ref_projection(
    expr: RustExpr,
    target_type: &str,
    optional_carrier: bool,
) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    let target_type = normalize_emitted_java_type_name(target_type);
    let handled_inner = shared_handle_inner_type(&target_type);
    if optional_carrier {
        if handled_inner.is_some() {
            return RustExpr::Raw(format!(
                "{rendered}.as_ref().and_then(|value| value.downcast_ref::<{target_type}>().map(|value| value.as_ref())){}",
                crate::output_markers::invalid_java_cast_expect()
            ));
        }
        RustExpr::Raw(format!(
            "{rendered}.as_ref().and_then(|value| value.downcast_ref::<{target_type}>()){}",
            crate::output_markers::invalid_java_cast_expect()
        ))
    } else {
        let rendered = parenthesize_cast_method_receiver(&rendered);
        if handled_inner.is_some() {
            return RustExpr::Raw(format!(
                "{rendered}.downcast_ref::<{target_type}>().map(|value| value.as_ref()){}",
                crate::output_markers::invalid_java_cast_expect()
            ));
        }
        RustExpr::Raw(format!(
            "{rendered}.downcast_ref::<{target_type}>(){}",
            crate::output_markers::invalid_java_cast_expect()
        ))
    }
}

fn shared_handle_inner_type(target_type: &str) -> Option<&str> {
    target_type
        .strip_prefix("std::rc::Rc<")
        .or_else(|| target_type.strip_prefix("std::sync::Arc<"))
        .and_then(|inner| inner.strip_suffix('>'))
}

fn apply_erased_object_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    let target_type = normalize_emitted_java_type_name(target_type);
    if !plausible_rust_nominal_type_name(&target_type) {
        return expr;
    }
    let rendered = parenthesize_cast_method_receiver(&emit::emit_expr(&expr));
    match invalid_cast {
        // Parenthesised: `.` binds tighter than unary `*`, so an appended field
        // access turned `*x.downcast::<T>().expect(..).v` into a deref of the
        // *field* -- `error[E0614]: type i32 cannot be dereferenced`.
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "(*{rendered}.downcast::<{target_type}>(){})",
            crate::output_markers::invalid_java_cast_expect()
        )),
    }
}

/// Mangles a Java type name into the Rust name actually emitted for it.
///
/// A nested Java type is written `Outer.Inner`, which is not a Rust path.
/// [0013](../claude_audits/done/0013-nested-type-names-unmangled-in-downcast.md)
/// fixed this for `downcast::<T>` targets only, so the same name reached `as
/// &dyn Outer.Inner` unmangled and failed to parse — a fix applied at one
/// symptom rather than at the name. Every position that emits a Java type name
/// as Rust goes through this.
fn normalize_emitted_java_type_name(target_type: &str) -> String {
    if target_type.contains('.') || target_type.contains('$') {
        crate::slot_render::rust_name_for_java_named_type(target_type)
    } else {
        target_type.to_string()
    }
}

/// Whether a rendered type name is a plausible downcast target.
///
/// The question is about the *nominal* name, so a qualified or wrapped name is
/// judged by its innermost identifier: `std::rc::Rc<FormatLine>` is as nominal
/// as `FormatLine`. Judging it by its first character instead rejected every
/// handle-wrapped target (0002b), and rejection here is silent -- the downcast
/// is dropped and the value flows on with the wrong type.
fn plausible_rust_nominal_type_name(target_type: &str) -> bool {
    let innermost = target_type
        .rsplit_once('<')
        .map(|(_, inner)| inner.trim_end_matches('>'))
        .unwrap_or(target_type);
    let innermost = innermost.rsplit("::").next().unwrap_or(innermost);
    matches!(
        innermost,
        "bool" | "i8" | "i16" | "char" | "i32" | "i64" | "f32" | "f64"
    ) || innermost
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
}

fn apply_to_owned_sequence(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    to_owned_sequence_projection(expr, policy)
}

pub fn to_owned_sequence_projection(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if matches!(
        expr.unshaped(),
        RustExpr::MethodCall { method, args, .. } if method == "to_vec" && args.is_empty()
    ) {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    if !matches!(expr.unshaped(), RustExpr::Raw(_))
        && matches!(policy, CloneRecoveryPolicy::ProvenCloneCapability { .. })
    {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            zero_arg_method_projection(expr, "to_vec"),
        );
    }
    let rendered = emit::emit_expr(&expr);
    if rendered.ends_with(".to_vec()") {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let receiver = if rust_owned_sequence_receiver_needs_parentheses(&expr, &rendered) {
        format!("({rendered})")
    } else {
        rendered.to_string()
    };
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(format!(
            "{receiver}.to_vec(){}",
            render_clone_recovery_policy(policy)
        )),
    )
}

fn rust_owned_sequence_receiver_needs_parentheses(expr: &RustExpr, rendered: &str) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => {
            rust_owned_sequence_receiver_needs_parentheses(expr, rendered)
        }
        RustExpr::Raw(_) => raw_contains_as_token_outside_literals(rendered.trim()),
        _ => rust_receiver_expr_needs_parentheses(expr, rendered),
    }
}

fn apply_clone_recovery(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if rust_expr_has_clone_recovery(&expr) {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let expr = match expr {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "into" && args.is_empty() => {
            return RustExpr::shaped(
                RustExprShape::OwnedRecovery,
                RustExpr::MethodCall {
                    receiver: Box::new(apply_clone_recovery(*receiver, policy)),
                    method,
                    args,
                },
            );
        }
        other => other,
    };
    let rendered = emit::emit_expr(&expr);
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Raw(render_clone_recovery_expr(&rendered, policy)),
    )
}

fn render_clone_recovery_expr(rendered: &str, policy: &CloneRecoveryPolicy) -> String {
    render_clone_projection_expr_with_receiver_policy(
        rendered,
        render_clone_recovery_policy(policy),
        rust_clone_recovery_receiver_needs_parentheses,
    )
}

fn render_clone_projection_expr(rendered: &str, suffix: &str) -> String {
    render_clone_projection_expr_with_receiver_policy(
        rendered,
        suffix,
        rust_method_receiver_needs_parentheses,
    )
}

fn render_clone_projection_expr_with_receiver_policy(
    rendered: &str,
    suffix: &str,
    receiver_needs_parentheses: fn(&str) -> bool,
) -> String {
    if let Some(inner) = rendered
        .strip_prefix("Some(")
        .and_then(|inner| inner.strip_suffix(')'))
    {
        return format!(
            "Some({})",
            render_clone_projection_expr_with_receiver_policy(
                inner,
                suffix,
                receiver_needs_parentheses
            )
        );
    }
    if let Some(receiver) = rendered.strip_suffix(".into()") {
        return format!("{receiver}.clone(){suffix}.into()");
    }
    let receiver = if receiver_needs_parentheses(rendered) {
        format!("({rendered})")
    } else {
        rendered.to_string()
    };
    format!("{receiver}.clone(){suffix}")
}

fn rendered_expr_is_anonymous_dyn_trait_constructor(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.starts_with("<dyn ") && trimmed.ends_with(">::anonymous()")
}

fn rust_receiver_expr_needs_parentheses(expr: &RustExpr, rendered: &str) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => rust_receiver_expr_needs_parentheses(expr, rendered),
        RustExpr::Field(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. }
        | RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_) => false,
        RustExpr::Index(_, _) => true,
        RustExpr::Tuple(_) => true,
        RustExpr::ArrayLiteral(_) | RustExpr::VecLiteral(_) | RustExpr::VecRepeat { .. } => true,
        RustExpr::Raw(_) => rust_method_receiver_needs_parentheses(rendered),
        RustExpr::Binary { .. }
        | RustExpr::Unary { .. }
        | RustExpr::Cast { .. }
        | RustExpr::If { .. } => true,
        RustExpr::MarkedBlock { .. } => false,
        RustExpr::StructLit { .. } => true,
    }
}

fn rust_method_receiver_needs_parentheses(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    raw_contains_top_level_as_cast(trimmed) || trimmed.starts_with('*')
}

fn rust_clone_recovery_receiver_needs_parentheses(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    raw_contains_as_token_outside_literals(trimmed) || trimmed.starts_with('*')
}

fn render_clone_recovery_policy(policy: &CloneRecoveryPolicy) -> &'static str {
    match policy {
        CloneRecoveryPolicy::AuditRequired => "/* TODO(translation): validate added clone */",
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => "",
    }
}

fn apply_unsupported_ownership_coercion(
    expr: RustExpr,
    actual_ownership: OwnershipShape,
    target_ownership: OwnershipShape,
    policy: &OwnershipCoercionFallbackPolicy,
) -> RustExpr {
    ownership_coercion_fallback_message_policy(policy, actual_ownership, target_ownership)
        .compile_error_expr_consuming(&expr)
}

pub fn rust_primitive_type_name_policy(primitive: PrimitiveType) -> RustPrimitiveTypeNamePolicy {
    let rust_name = match primitive {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Byte => "i8",
        PrimitiveType::Short => "i16",
        PrimitiveType::Char => "char",
        PrimitiveType::Int => "i32",
        PrimitiveType::Long => "i64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
    };
    RustPrimitiveTypeNamePolicy {
        primitive,
        rust_name,
    }
}

#[cfg(test)]
mod nominal_downcast_target_tests {
    use super::plausible_rust_nominal_type_name;

    /// A downcast target is judged by its innermost identifier, not by the
    /// first character of the rendered string.
    ///
    /// Rejection here is silent: `apply_erased_object_downcast` returns the
    /// expression unchanged, so the value keeps flowing at the wrong type with
    /// no diagnostic. Judging `std::rc::Rc<FormatLine>` by its leading `s` --
    /// which is what this did -- dropped the cast on every qualified or
    /// wrapped target. See claude_audits/0002b.
    #[test]
    fn qualified_and_wrapped_targets_are_nominal() {
        for target in [
            "FormatLine",
            "std::rc::Rc<FormatLine>",
            "std::sync::Arc<Standards>",
            "crate::Foo",
            "_Anon",
            "i32",
        ] {
            assert!(
                plausible_rust_nominal_type_name(target),
                "expected a nominal target: {target}"
            );
        }
    }

    #[test]
    fn non_nominal_targets_are_still_rejected() {
        for target in ["", "dyn", "&str", "(i32, i32)", "std::rc::Rc<dyn>"] {
            assert!(
                !plausible_rust_nominal_type_name(target),
                "expected a non-nominal target: {target}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn required_borrowed_array_policy(
        target_ownership: OwnershipShape,
    ) -> BorrowedElementSlotAdapterPolicy {
        let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
        let carrier_shape = RustSlotShape::conservative(array_ty.clone());
        let element_shape = RustSlotShape::conservative(array_ty.clone());
        let mut target = RustSlotShape::conservative(array_ty);
        target.ownership = target_ownership;
        target.view = ViewShape::Slice;
        BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape,
            element_shape,
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: None,
        }
    }

    fn optional_borrowed_array_policy(
        target_ownership: OwnershipShape,
    ) -> BorrowedElementSlotAdapterPolicy {
        let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int)));
        let carrier_shape =
            RustSlotShape::conservative(crate::types::optional_java_type(array_ty.clone()));
        let element_shape = RustSlotShape::conservative(array_ty.clone());
        let mut target = RustSlotShape::conservative(crate::types::optional_java_type(array_ty));
        target.ownership = target_ownership;
        target.view = ViewShape::Slice;
        BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape,
            element_shape,
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: None,
        }
    }

    fn optional_borrowed_native_policy(
        target_ownership: OwnershipShape,
    ) -> BorrowedElementSlotAdapterPolicy {
        let element_ty = JavaType::Named {
            name: "ExternalValue".to_string(),
            args: Vec::new(),
        };
        let carrier_shape =
            RustSlotShape::conservative(crate::types::optional_java_type(element_ty.clone()));
        let element_shape = RustSlotShape::conservative(element_ty.clone());
        let mut target = RustSlotShape::conservative(crate::types::optional_java_type(element_ty));
        target.ownership = target_ownership;
        BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape,
            element_shape,
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: None,
        }
    }

    #[test]
    fn optional_borrowed_slice_view_keeps_structured_method_tail() {
        let policy = optional_borrowed_array_policy(OwnershipShape::SharedBorrow);
        let expr = RustExpr::Var("values".to_string());

        let projected =
            apply_optional_borrowed_element_target_view(&expr, &policy).expect("slice view");

        assert_eq!(emit::emit_expr(&projected), "values.as_deref()");
        assert_eq!(
            projected.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "as_deref".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn optional_borrowed_mut_slice_view_keeps_structured_method_tail() {
        let policy = optional_borrowed_array_policy(OwnershipShape::MutableBorrow);
        let expr = RustExpr::Var("values".to_string());

        let projected = apply_optional_borrowed_element_target_view(&expr, &policy)
            .expect("mutable slice view");

        assert_eq!(emit::emit_expr(&projected), "values.as_deref_mut()");
        assert_eq!(
            projected.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "as_deref_mut".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn optional_borrowed_native_views_keep_structured_method_tails() {
        let shared_policy = optional_borrowed_native_policy(OwnershipShape::SharedBorrow);
        let shared_expr = RustExpr::Var("value".to_string());
        let shared = apply_optional_borrowed_element_target_view(&shared_expr, &shared_policy)
            .expect("shared native view");

        assert_eq!(emit::emit_expr(&shared), "value.as_ref()");
        assert_eq!(
            shared.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(shared_expr),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }
        );

        let mutable_policy = optional_borrowed_native_policy(OwnershipShape::MutableBorrow);
        let mutable_expr = RustExpr::Var("value".to_string());
        let mutable = apply_optional_borrowed_element_target_view(&mutable_expr, &mutable_policy)
            .expect("mutable native view");

        assert_eq!(emit::emit_expr(&mutable), "value.as_mut()");
        assert_eq!(
            mutable.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(mutable_expr),
                method: "as_mut".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn required_borrowed_slice_view_keeps_structured_method_tail() {
        let policy = required_borrowed_array_policy(OwnershipShape::SharedBorrow);
        let expr = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        };

        let projected =
            apply_required_borrowed_element_target_view(&expr, &policy).expect("slice view");

        assert_eq!(
            emit::emit_expr(&projected),
            "values.expect(\"Java null receiver\").as_slice()"
        );
        assert_eq!(
            projected.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "as_slice".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn required_borrowed_mut_slice_view_keeps_structured_method_tail() {
        let policy = required_borrowed_array_policy(OwnershipShape::MutableBorrow);
        let expr = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("values".to_string())),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        };

        let projected = apply_required_borrowed_element_target_view(&expr, &policy)
            .expect("mutable slice view");

        assert_eq!(
            emit::emit_expr(&projected),
            "values.expect(\"Java null receiver\").as_mut_slice()"
        );
        assert_eq!(
            projected.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "as_mut_slice".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn required_borrowed_string_view_keeps_structured_method_tail() {
        let string_shape = RustSlotShape::conservative(JavaType::String);
        let mut target = string_shape.clone();
        target.ownership = OwnershipShape::SharedBorrow;
        target.view = ViewShape::Str;
        let policy = BorrowedElementSlotAdapterPolicy {
            target,
            carrier_shape: string_shape.clone(),
            element_shape: string_shape,
            carrier: BorrowedElementCarrier::Option,
            matches: true,
            adapter: None,
        };
        let expr = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        };

        let projected =
            apply_required_borrowed_element_target_view(&expr, &policy).expect("string view");

        assert_eq!(
            emit::emit_expr(&projected),
            "value.expect(\"Java null receiver\").as_str()"
        );
        assert_eq!(
            projected.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "as_str".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn borrow_projection_uses_structured_borrow_for_named_expr() {
        let expr = ensure_borrow_projection(
            RustExpr::Var("value".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&expr), "&value");
    }

    #[test]
    fn borrow_projection_parenthesizes_raw_binary_expr() {
        let expr = ensure_borrow_projection(
            RustExpr::Raw("left + right".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&expr), "&(left + right)");
    }

    #[test]
    fn borrow_projection_parenthesizes_compound_indexed_raw_expr() {
        let simple_index = ensure_borrow_projection(
            RustExpr::Raw("values[i]".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&simple_index), "&values[i]");

        let compound_index = ensure_borrow_projection(
            RustExpr::Raw("left + values[i]".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&compound_index), "&(left + values[i])");

        let mutable_compound = apply_one(
            RustExpr::Raw("left + values[i]".to_string()),
            &Coercion::BorrowMut,
        );
        assert_eq!(
            emit::emit_expr(&mutable_compound),
            "&mut (left + values[i])"
        );
    }

    #[test]
    fn borrow_projection_only_treats_complete_string_literals_as_prefix_safe() {
        let string_literal = ensure_borrow_projection(
            RustExpr::Raw("\"literal\"".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&string_literal), "&\"literal\"");

        let raw_string_literal = ensure_borrow_projection(
            RustExpr::Raw("r#\"literal\"#".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&raw_string_literal), "&r#\"literal\"#");

        let string_suffix = ensure_borrow_projection(
            RustExpr::Raw("\"literal\".to_string()".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(
            emit::emit_expr(&string_suffix),
            "&(\"literal\".to_string())"
        );

        let raw_string_suffix = ensure_borrow_projection(
            RustExpr::Raw("r#\"literal\"#.to_string()".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(
            emit::emit_expr(&raw_string_suffix),
            "&(r#\"literal\"#.to_string())"
        );
    }

    #[test]
    fn borrow_projection_keeps_string_view_idempotent() {
        let expr = ensure_borrow_projection(
            RustExpr::Raw("name.as_str()".to_string()),
            None,
            None,
            BorrowProjectionUse::StringView,
        );
        assert_eq!(emit::emit_expr(&expr), "name.as_str()");
    }

    #[test]
    fn borrow_projection_keeps_map_key_helper_idempotent() {
        let expr = ensure_borrow_projection(
            RustExpr::Raw("java_string_substring(name.as_str(), 0, 1)".to_string()),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(
            emit::emit_expr(&expr),
            "java_string_substring(name.as_str(), 0, 1)"
        );
    }

    #[test]
    fn borrow_projection_keeps_tagged_map_key_string_view_idempotent() {
        let tagged = string_view_expr(RustExpr::Raw("custom_key_view(value)".to_string()));
        assert_eq!(
            ensure_borrow_projection(tagged.clone(), None, None, BorrowProjectionUse::MapKey),
            tagged
        );

        let raw = RustExpr::Raw("custom_key_view(value)".to_string());
        assert_eq!(
            emit::emit_expr(&ensure_borrow_projection(
                raw,
                None,
                None,
                BorrowProjectionUse::MapKey
            )),
            "&custom_key_view(value)"
        );
    }

    #[test]
    fn borrow_projection_keeps_structured_map_key_string_views_idempotent() {
        let method_view = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            ensure_borrow_projection(method_view.clone(), None, None, BorrowProjectionUse::MapKey),
            method_view
        );

        let literal = RustExpr::Literal("\"literal\"".to_string());
        assert_eq!(
            ensure_borrow_projection(literal.clone(), None, None, BorrowProjectionUse::MapKey),
            literal
        );

        let not_string_view = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            emit::emit_expr(&ensure_borrow_projection(
                not_string_view,
                None,
                None,
                BorrowProjectionUse::MapKey,
            )),
            "&name.to_string()"
        );
    }

    #[test]
    fn borrow_producers_record_borrow_shapes_for_idempotence() {
        let borrowed = apply_one(RustExpr::Var("value".to_string()), &Coercion::Borrow);
        assert_eq!(emit::emit_expr(&borrowed), "&value");
        assert_eq!(borrowed.recorded_shape(), Some(RustExprShape::Borrowed));
        assert_eq!(
            ensure_borrow_projection(borrowed.clone(), None, None, BorrowProjectionUse::Argument),
            borrowed
        );

        let forced_structured = force_shared_borrow_projection(RustExpr::Var("value".to_string()));
        assert_eq!(emit::emit_expr(&forced_structured), "&value");
        assert_eq!(
            forced_structured.clone().into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            }
        );
        assert_eq!(
            forced_structured.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );
        assert_eq!(
            ensure_borrow_projection(
                forced_structured.clone(),
                None,
                None,
                BorrowProjectionUse::Argument
            ),
            forced_structured
        );

        let forced =
            force_shared_borrow_projection(RustExpr::Raw("custom_view(value)".to_string()));
        assert_eq!(emit::emit_expr(&forced), "&custom_view(value)");
        assert_eq!(forced.recorded_shape(), Some(RustExprShape::Borrowed));
        assert_eq!(
            ensure_borrow_projection(forced.clone(), None, None, BorrowProjectionUse::Argument),
            forced
        );

        let dyn_trait = borrow_as_dyn_trait(RustExpr::Var("source".to_string()), "Readable");
        assert_eq!(emit::emit_expr(&dyn_trait), "&source as &dyn Readable");
        assert_eq!(dyn_trait.recorded_shape(), Some(RustExprShape::Borrowed));
        assert_eq!(
            ensure_borrow_projection(dyn_trait.clone(), None, None, BorrowProjectionUse::Argument),
            dyn_trait
        );

        let boxed_dyn_trait =
            borrow_boxed_as_dyn_trait(RustExpr::Var("boxed".to_string()), "Readable");
        assert_eq!(
            emit::emit_expr(&boxed_dyn_trait),
            "boxed.as_ref() as &dyn Readable"
        );
        assert_eq!(
            boxed_dyn_trait.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );
        assert_eq!(
            ensure_borrow_projection(
                boxed_dyn_trait.clone(),
                None,
                None,
                BorrowProjectionUse::Argument
            ),
            boxed_dyn_trait
        );

        let concrete_facade = apply_one(
            RustExpr::Var("source".to_string()),
            &Coercion::BorrowConcreteFacade {
                target_type: "ReadableFacade".to_string(),
            },
        );
        assert_eq!(
            emit::emit_expr(&concrete_facade),
            "{ let _java_value = source; &ReadableFacade }"
        );
        assert_eq!(
            concrete_facade.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );
        assert_eq!(
            ensure_borrow_projection(
                concrete_facade.clone(),
                None,
                None,
                BorrowProjectionUse::Argument
            ),
            concrete_facade
        );

        let opaque_borrow = RustExpr::shaped(
            RustExprShape::Borrowed,
            RustExpr::Raw("custom_borrow(value)".to_string()),
        );
        assert_eq!(
            ensure_borrow_projection(
                opaque_borrow.clone(),
                None,
                None,
                BorrowProjectionUse::MethodReceiver
            ),
            opaque_borrow
        );

        let optional_borrow = apply_one(
            RustExpr::Var("value".to_string()),
            &Coercion::UnwrapOptionBorrowed,
        );
        assert_eq!(
            emit::emit_expr(&optional_borrow),
            "value.as_ref().expect(\"Java null receiver\")"
        );
        assert_eq!(
            optional_borrow.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );
        assert_eq!(
            optional_borrow.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            ensure_borrow_projection(
                optional_borrow.clone(),
                None,
                None,
                BorrowProjectionUse::Argument
            ),
            optional_borrow
        );

        let mutable = apply_one(
            RustExpr::Var("value".to_string()),
            &Coercion::UnwrapOptionMutBorrowed,
        );
        assert_eq!(
            emit::emit_expr(&mutable),
            "value.as_mut().expect(\"Java null receiver\")"
        );
        assert_eq!(mutable.recorded_shape(), Some(RustExprShape::MutBorrowed));
        assert_eq!(
            mutable.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "as_mut".to_string(),
                    args: Vec::new(),
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            ensure_borrow_projection(
                mutable.clone(),
                None,
                None,
                BorrowProjectionUse::MutableSliceView
            ),
            mutable
        );
    }

    #[test]
    fn owned_optional_unwrap_keeps_method_structure_for_structured_receivers() {
        let unwrapped = apply_one(RustExpr::Var("value".to_string()), &Coercion::UnwrapOption);
        assert_eq!(emit::emit_expr(&unwrapped), "value.unwrap()");
        assert_eq!(
            unwrapped,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
        );

        let raw = apply_one(
            RustExpr::Raw("factory()".to_string()),
            &Coercion::UnwrapOption,
        );
        assert_eq!(emit::emit_expr(&raw), "factory().unwrap()");
        assert_eq!(
            raw,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("factory()".to_string())),
                method: "unwrap".to_string(),
                args: Vec::new(),
            },
            "legacy raw producers keep the raw receiver leaf, not the old opaque method tail"
        );
    }

    #[test]
    fn optional_element_projection_uses_recorded_mapped_value_shape_for_opaque_adapters() {
        let mut string_target = RustSlotShape::conservative(JavaType::String);
        string_target.ownership = OwnershipShape::SharedBorrow;
        string_target.view = ViewShape::Str;
        let string_projected = render_optional_element_projection(
            RustExpr::Var("name".to_string()),
            string_view_expr(RustExpr::Raw("custom_string_view(value)".to_string())),
            &string_target,
        );
        assert_eq!(
            emit::emit_expr(&string_projected),
            "name.as_ref().map(|value| custom_string_view(value))"
        );
        assert_eq!(
            string_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("name".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_string_view(value)".to_string()
                )],
            }
        );
        assert_eq!(
            string_projected.recorded_shape(),
            Some(RustExprShape::OptionalProjected)
        );
        assert_eq!(string_projected.recorded_slot_shape(), Some(&string_target));
        let raw_string_projected = render_optional_element_projection(
            RustExpr::Raw("make_name()".to_string()),
            string_view_expr(RustExpr::Raw("custom_string_view(value)".to_string())),
            &string_target,
        );
        assert_eq!(
            emit::emit_expr(&raw_string_projected),
            "make_name().as_ref().map(|value| custom_string_view(value))"
        );
        assert_eq!(
            raw_string_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("make_name()".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_string_view(value)".to_string()
                )],
            }
        );

        let builtin_string_projected = render_optional_element_projection(
            RustExpr::Var("name".to_string()),
            apply_one(RustExpr::Var("value".to_string()), &Coercion::AsStr),
            &string_target,
        );
        assert_eq!(
            emit::emit_expr(&builtin_string_projected),
            "name.as_ref().map(|value| value.as_str())"
        );
        assert_eq!(
            builtin_string_projected.recorded_slot_shape(),
            Some(&string_target)
        );

        let mut slice_target = RustSlotShape::conservative(JavaType::Array(Box::new(
            JavaType::Primitive(PrimitiveType::Int),
        )));
        slice_target.ownership = OwnershipShape::SharedBorrow;
        slice_target.view = ViewShape::Slice;
        let slice_projected = render_optional_element_projection(
            RustExpr::Var("values".to_string()),
            slice_view_expr(RustExpr::Raw("custom_slice_view(value)".to_string())),
            &slice_target,
        );
        assert_eq!(
            emit::emit_expr(&slice_projected),
            "values.as_ref().map(|value| custom_slice_view(value))"
        );
        assert_eq!(
            slice_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_slice_view(value)".to_string()
                )],
            }
        );
        assert_eq!(slice_projected.recorded_slot_shape(), Some(&slice_target));

        let builtin_slice_projected = render_optional_element_projection(
            RustExpr::Var("values".to_string()),
            apply_one(RustExpr::Var("value".to_string()), &Coercion::AsSlice),
            &slice_target,
        );
        assert_eq!(
            emit::emit_expr(&builtin_slice_projected),
            "values.as_deref()"
        );
        assert_eq!(
            builtin_slice_projected.recorded_slot_shape(),
            Some(&slice_target)
        );

        let mut mut_slice_target = slice_target.clone();
        mut_slice_target.ownership = OwnershipShape::MutableBorrow;
        let builtin_mut_slice_projected = render_optional_element_projection(
            RustExpr::Var("values".to_string()),
            apply_one(RustExpr::Var("value".to_string()), &Coercion::AsMutSlice),
            &mut_slice_target,
        );
        assert_eq!(
            emit::emit_expr(&builtin_mut_slice_projected),
            "values.as_deref_mut()"
        );
        assert_eq!(
            builtin_mut_slice_projected.recorded_slot_shape(),
            Some(&mut_slice_target)
        );
        let custom_mut_slice_projected = render_optional_element_projection(
            RustExpr::Var("values".to_string()),
            mutable_slice_view_expr(RustExpr::Raw("custom_mut_slice_view(value)".to_string())),
            &mut_slice_target,
        );
        assert_eq!(
            emit::emit_expr(&custom_mut_slice_projected),
            "values.as_mut().map(|value| custom_mut_slice_view(value))"
        );
        assert_eq!(
            custom_mut_slice_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "as_mut".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_mut_slice_view(value)".to_string()
                )],
            }
        );
        assert_eq!(
            custom_mut_slice_projected.recorded_slot_shape(),
            Some(&mut_slice_target)
        );

        let owned_target = RustSlotShape::conservative(JavaType::String);
        let owned_projected = render_optional_element_projection(
            RustExpr::Var("name".to_string()),
            RustExpr::Raw("custom_owned_string(value)".to_string()),
            &owned_target,
        );
        assert_eq!(
            emit::emit_expr(&owned_projected),
            "name.map(|value| custom_owned_string(value))"
        );
        assert_eq!(
            owned_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("name".to_string())),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_owned_string(value)".to_string()
                )],
            }
        );
        assert_eq!(
            owned_projected.recorded_shape(),
            Some(RustExprShape::OptionalProjected)
        );
        assert_eq!(owned_projected.recorded_slot_shape(), Some(&owned_target));
        let raw_owned_projected = render_optional_element_projection(
            RustExpr::Raw("make_name()".to_string()),
            RustExpr::Raw("custom_owned_string(value)".to_string()),
            &owned_target,
        );
        assert_eq!(
            emit::emit_expr(&raw_owned_projected),
            "make_name().map(|value| custom_owned_string(value))"
        );
        assert_eq!(
            raw_owned_projected.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("make_name()".to_string())),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| custom_owned_string(value)".to_string()
                )],
            }
        );
    }

    #[test]
    fn cast_operand_parenthesization_prefers_expression_structure() {
        let binary = RustExpr::Binary {
            op: "+".to_string(),
            left: Box::new(RustExpr::Var("left".to_string())),
            right: Box::new(RustExpr::Var("right".to_string())),
        };
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&binary),
            "(left + right)"
        );

        let tagged_binary = RustExpr::shaped(RustExprShape::OwnedRecovery, binary);
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&tagged_binary),
            "(left + right)"
        );

        let literal = RustExpr::Literal("\"left + right\"".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&literal),
            "\"left + right\""
        );

        let raw_compat = RustExpr::Raw("left + right".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&raw_compat),
            "(left + right)"
        );

        let raw_without_spaces = RustExpr::Raw("left+right".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&raw_without_spaces),
            "(left+right)"
        );

        let nested_operator = RustExpr::Raw("call(left + right)".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&nested_operator),
            "call(left + right)"
        );

        let string_operator = RustExpr::Raw("call(\"left + right\")".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&string_operator),
            "call(\"left + right\")"
        );

        let unary = RustExpr::Raw("-value".to_string());
        assert_eq!(
            render_cast_operand_with_structural_parentheses(&unary),
            "-value"
        );
    }

    #[test]
    fn option_wrap_records_constructed_optional_shape() {
        let expr = apply_one(
            RustExpr::Raw("make_value()".to_string()),
            &Coercion::WrapOption,
        );
        assert_eq!(emit::emit_expr(&expr), "Some(make_value())");
        assert_eq!(
            expr.recorded_shape(),
            Some(RustExprShape::OptionalConstructed)
        );
        assert!(rust_expr_is_constructed_optional_value(&expr));

        let opaque = RustExpr::shaped(
            RustExprShape::OptionalConstructed,
            RustExpr::Raw("custom_present_value()".to_string()),
        );
        assert!(rust_expr_is_constructed_optional_value(&opaque));
    }

    #[test]
    fn optional_string_view_construction_normalizes_structured_borrow_by_tag() {
        let expr = normalize_optional_constructed_string_view_expr(RustExpr::Call {
            function: Box::new(RustExpr::Var("Some".to_string())),
            args: vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(string_view_expr(RustExpr::Raw(
                    "custom_string_view(value)".to_string(),
                ))),
            }],
        });
        assert_eq!(emit::emit_expr(&expr), "Some(custom_string_view(value))");

        let raw = normalize_optional_constructed_string_view_expr(RustExpr::Raw(
            "Some(&value.as_str())".to_string(),
        ));
        assert_eq!(emit::emit_expr(&raw), "Some(value.as_str())");
        assert_eq!(
            raw.recorded_shape(),
            Some(RustExprShape::OptionalConstructed)
        );
        assert_eq!(
            raw.clone().into_unshaped(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Some"])),
                args: vec![string_view_expr(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("value".to_string())),
                    method: "as_str".to_string(),
                    args: Vec::new(),
                })],
            }
        );
        assert!(rust_expr_is_constructed_optional_value(&raw));

        let wrapped = apply_one(
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(string_view_expr(RustExpr::Raw(
                    "custom_string_view(value)".to_string(),
                ))),
            },
            &Coercion::WrapOption,
        );
        assert_eq!(emit::emit_expr(&wrapped), "Some(custom_string_view(value))");
    }

    #[test]
    fn constructor_self_temporary_raw_fallback_requires_identifier_token() {
        assert!(rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Field(
                Box::new(RustExpr::Var("__java_self".to_string())),
                "context".to_string(),
            )
        ));
        assert!(rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("__java_self.context".to_string(),)
        ));
        assert!(rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("helper(__java_self.clone())".to_string(),)
        ));

        assert!(!rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("\"__java_self\".to_string()".to_string(),)
        ));
        assert!(!rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("r#\"__java_self\"#".to_string(),)
        ));
        assert!(!rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("not__java_self.context".to_string(),)
        ));
        assert!(!rust_expr_mentions_constructor_self_temporary(
            &RustExpr::Raw("__java_self_extra.context".to_string(),)
        ));
    }

    #[test]
    fn string_view_argument_renderer_owns_borrow_projection_policy() {
        assert_eq!(
            render_string_view_argument(RustExpr::Var("value".to_string())),
            "&value"
        );
        assert_eq!(
            render_string_view_argument(RustExpr::Raw("value.as_str()".to_string())),
            "value.as_str()"
        );
    }

    #[test]
    fn as_str_coercion_removes_simple_borrowed_item_deref() {
        let expr = apply_coercions(RustExpr::Raw("*partition".to_string()), &[Coercion::AsStr]);
        assert_eq!(emit::emit_expr(&expr), "partition.as_str()");
    }

    #[test]
    fn parse_receiver_rendering_removes_redundant_shared_borrow() {
        assert_eq!(
            render_parse_receiver(RustExpr::Raw("&value".to_string())),
            "value"
        );
        assert_eq!(
            render_parse_receiver(RustExpr::Raw("&next.value()".to_string())),
            "(next.value())"
        );
        assert_eq!(
            render_parse_receiver(RustExpr::Raw("&tokenizer.next_token()".to_string())),
            "tokenizer.next_token()"
        );
    }

    #[test]
    fn primitive_parse_projection_owns_parse_call_spelling() {
        let structured =
            primitive_parse_projection(RustExpr::Var("value".to_string()), PrimitiveType::Int);
        assert_eq!(
            emit::emit_expr(&structured),
            "value.parse::<i32>().unwrap()"
        );
        assert_eq!(
            structured,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "parse::<i32>".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
        );

        let structured_borrow = primitive_parse_projection(
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            },
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&structured_borrow),
            "value.parse::<i32>().unwrap()"
        );

        assert_eq!(
            emit::emit_expr(&primitive_parse_projection(
                RustExpr::Raw("&value".to_string()),
                PrimitiveType::Int,
            )),
            "value.parse::<i32>().unwrap()"
        );
        assert_eq!(
            emit::emit_expr(&primitive_parse_projection(
                RustExpr::Raw("&tokenizer.next_token()".to_string()),
                PrimitiveType::Float,
            )),
            "tokenizer.next_token().parse::<f32>().unwrap()"
        );
        assert_eq!(
            emit::emit_expr(&primitive_parse_projection(
                RustExpr::Raw("value.as_str()".to_string()),
                PrimitiveType::Double,
            )),
            "value.as_str().parse::<f64>().unwrap()"
        );
    }

    #[test]
    fn primitive_hash_and_compare_projections_own_boxed_primitive_spellings() {
        let boolean_hash = primitive_hash_code_projection(
            RustExpr::Var("flag".to_string()),
            PrimitiveType::Boolean,
        );
        assert_eq!(
            emit::emit_expr(&boolean_hash),
            "if flag { 1231 } else { 1237 }"
        );
        assert_eq!(
            boolean_hash,
            RustExpr::If {
                condition: Box::new(RustExpr::Var("flag".to_string())),
                then_expr: Box::new(RustExpr::Literal("1231".to_string())),
                else_expr: Box::new(RustExpr::Literal("1237".to_string())),
            }
        );

        let int_hash =
            primitive_hash_code_projection(RustExpr::Var("value".to_string()), PrimitiveType::Int);
        assert_eq!(emit::emit_expr(&int_hash), "value as i32");
        assert_eq!(
            int_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("value".to_string())),
                ty: "i32".to_string(),
            }
        );

        let float_hash = primitive_hash_code_projection(
            RustExpr::Var("score".to_string()),
            PrimitiveType::Float,
        );
        assert_eq!(emit::emit_expr(&float_hash), "score.to_bits() as i32");
        assert_eq!(
            float_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("score".to_string())),
                    method: "to_bits".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&primitive_hash_code_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int,
            )),
            "left + right as i32"
        );
        let long_hash =
            primitive_hash_code_projection(RustExpr::Var("wide".to_string()), PrimitiveType::Long);
        assert_eq!(
            emit::emit_expr(&long_hash),
            "(wide as i64 ^ (wide as i64) >> 32) as i32"
        );
        assert_eq!(
            long_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Binary {
                    op: "^".to_string(),
                    left: Box::new(RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("wide".to_string())),
                        ty: "i64".to_string(),
                    }),
                    right: Box::new(RustExpr::Binary {
                        op: ">>".to_string(),
                        left: Box::new(RustExpr::Cast {
                            expr: Box::new(RustExpr::Var("wide".to_string())),
                            ty: "i64".to_string(),
                        }),
                        right: Box::new(RustExpr::Literal("32".to_string())),
                    }),
                }),
                ty: "i32".to_string(),
            }
        );

        let double_hash = primitive_hash_code_projection(
            RustExpr::Var("score".to_string()),
            PrimitiveType::Double,
        );
        assert_eq!(
            emit::emit_expr(&double_hash),
            "(score.to_bits() ^ score.to_bits() >> 32) as i32"
        );
        assert_eq!(
            double_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Binary {
                    op: "^".to_string(),
                    left: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("score".to_string())),
                        method: "to_bits".to_string(),
                        args: Vec::new(),
                    }),
                    right: Box::new(RustExpr::Binary {
                        op: ">>".to_string(),
                        left: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("score".to_string())),
                            method: "to_bits".to_string(),
                            args: Vec::new(),
                        }),
                        right: Box::new(RustExpr::Literal("32".to_string())),
                    }),
                }),
                ty: "i32".to_string(),
            }
        );
        let compare = primitive_compare_to_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&compare),
            "java_primitive_compare_to(left, right)"
        );
        assert_eq!(
            compare,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_primitive_compare_to"])),
                args: vec![
                    RustExpr::Var("left".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
        let raw_compare = primitive_compare_to_projection(
            RustExpr::Raw("left + middle".to_string()),
            RustExpr::Var("right".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&raw_compare),
            "java_primitive_compare_to(left + middle, right)"
        );
        assert_eq!(
            raw_compare,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_primitive_compare_to"])),
                args: vec![
                    RustExpr::Raw("left + middle".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
    }

    #[test]
    fn string_char_at_projection_owns_index_and_bounds_spelling() {
        let char_at = string_char_at_projection(
            RustExpr::Var("value".to_string()),
            RustExpr::Var("index".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&char_at),
            "value.chars().nth((index) as usize).expect(\"Java string index out of bounds\")"
        );
        assert_eq!(
            char_at,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("value".to_string())),
                        method: "chars".to_string(),
                        args: Vec::new(),
                    }),
                    method: "nth".to_string(),
                    args: vec![RustExpr::Raw("(index) as usize".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal(
                    "\"Java string index out of bounds\"".to_string()
                )],
            }
        );
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Raw("left + right".to_string()),
            )),
            "value.chars().nth((left + right) as usize).expect(\"Java string index out of bounds\")"
        );
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Raw("make_value()".to_string()),
                RustExpr::Var("index".to_string()),
            )),
            "make_value().chars().nth((index) as usize).expect(\"Java string index out of bounds\")"
        );
        assert_eq!(
            string_char_at_projection(
                RustExpr::Raw("make_value()".to_string()),
                RustExpr::Var("index".to_string()),
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Raw("make_value()".to_string())),
                        method: "chars".to_string(),
                        args: Vec::new(),
                    }),
                    method: "nth".to_string(),
                    args: vec![RustExpr::Raw("(index) as usize".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal(
                    "\"Java string index out of bounds\"".to_string()
                )],
            }
        );
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Raw("left + right".to_string()),
                RustExpr::Var("index".to_string()),
            )),
            "(left + right).chars().nth((index) as usize).expect(\"Java string index out of bounds\")"
        );
    }

    #[test]
    fn string_index_and_substring_projections_own_runtime_helper_shapes() {
        assert_eq!(
            emit::emit_expr(&string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Raw("\"&\"".to_string()),
                None,
            )),
            "java_string_index_of(&value, \"&\")"
        );
        assert_eq!(
            string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Raw("\"&\"".to_string()),
                None,
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_string_index_of"])),
                args: vec![
                    RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var("value".to_string())),
                    },
                    RustExpr::Raw("\"&\"".to_string()),
                ],
            }
        );
        assert_eq!(
            emit::emit_expr(&string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Var("needle".to_string()),
                Some(RustExpr::Var("start".to_string())),
            )),
            "java_string_index_of_from(&value, &needle, start)"
        );
        assert_eq!(
            string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Var("needle".to_string()),
                Some(RustExpr::Var("start".to_string())),
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_string_index_of_from"])),
                args: vec![
                    RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var("value".to_string())),
                    },
                    RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var("needle".to_string())),
                    },
                    RustExpr::Var("start".to_string()),
                ],
            }
        );
        assert_eq!(
            emit::emit_expr(&string_substring_projection(
                RustExpr::Raw("value.as_str()".to_string()),
                RustExpr::Literal("0".to_string()),
                RustExpr::Literal("2".to_string()),
            )),
            "java_string_substring(value.as_str(), 0, 2)"
        );
        assert_eq!(
            string_substring_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("0".to_string()),
                RustExpr::Literal("2".to_string()),
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_string_substring"])),
                args: vec![
                    RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var("value".to_string())),
                    },
                    RustExpr::Literal("0".to_string()),
                    RustExpr::Literal("2".to_string()),
                ],
            }
        );
    }

    #[test]
    fn string_runtime_helper_projections_own_helper_call_shapes() {
        assert_eq!(
            emit::emit_expr(&string_hash_code_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "java_string_hash_code(&value)"
        );
        assert_eq!(
            emit::emit_expr(&string_replace_first_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"-\"".to_string()),
                RustExpr::Literal("\"\"".to_string()),
            )),
            "java_string_replace_first(&value, \"-\", \"\")"
        );
        assert_eq!(
            emit::emit_expr(&string_matches_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"[0-9]\"".to_string()),
            )),
            "java_string_matches(&value, \"[0-9]\")"
        );
        assert_eq!(
            emit::emit_expr(&string_split_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"_\"".to_string()),
                RustExpr::Literal("0".to_string()),
            )),
            "java_string_split(&value, \"_\", 0)"
        );
        let string_compare = string_compare_to_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Raw("right.as_str()".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&string_compare),
            "java_string_compare_to(&left, right.as_str())"
        );
        assert_eq!(
            string_compare,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_string_compare_to"])),
                args: vec![
                    RustExpr::Shaped {
                        shape: RustExprShape::StringView,
                        actual: None,
                        expr: Box::new(RustExpr::Unary {
                            op: "&".to_string(),
                            expr: Box::new(RustExpr::Var("left".to_string())),
                        }),
                    },
                    RustExpr::Raw("right.as_str()".to_string()),
                ],
            }
        );
        assert_eq!(
            emit::emit_expr(&string_compare_to_ignore_case_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Raw("right.as_str()".to_string()),
            )),
            "java_string_compare_to_ignore_case(&left, right.as_str())"
        );
        assert_eq!(
            emit::emit_expr(&string_equals_ignore_case_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Raw("right.as_str()".to_string()),
            )),
            "java_string_equals_ignore_case(&left, right.as_str())"
        );
    }

    #[test]
    fn string_byte_constructor_projections_own_runtime_helper_shapes() {
        assert_eq!(
            emit::emit_expr(&string_from_bytes_projection(RustExpr::Var(
                "bytes".to_string()
            ))),
            "java_string_from_bytes(&bytes)"
        );
        assert_eq!(
            emit::emit_expr(&string_from_byte_range_projection(
                RustExpr::Var("bytes".to_string()),
                RustExpr::Literal("0".to_string()),
                RustExpr::Var("len".to_string()),
            )),
            "java_string_from_byte_range(&bytes, 0, len)"
        );
    }

    #[test]
    fn jdk_platform_projections_own_runtime_call_spellings() {
        let exists = file_exists_projection(RustExpr::Var("file".to_string()));
        assert_eq!(
            emit::emit_expr(&exists),
            "std::path::Path::new(file.get_path().as_str()).exists()"
        );
        assert_eq!(
            exists,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["std", "path", "Path", "new"])),
                    args: vec![RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("file".to_string())),
                            method: "get_path".to_string(),
                            args: Vec::new(),
                        }),
                        method: "as_str".to_string(),
                        args: Vec::new(),
                    }],
                }),
                method: "exists".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&file_exists_projection(RustExpr::Raw(
                "make_file()".to_string()
            ))),
            "std::path::Path::new(make_file().get_path().as_str()).exists()"
        );
        assert_eq!(
            file_exists_projection(RustExpr::Raw("make_file()".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["std", "path", "Path", "new"])),
                    args: vec![RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Raw("make_file()".to_string())),
                            method: "get_path".to_string(),
                            args: Vec::new(),
                        }),
                        method: "as_str".to_string(),
                        args: Vec::new(),
                    }],
                }),
                method: "exists".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&system_exit_projection(RustExpr::Var("status".to_string()))),
            "std::process::exit(status as i32)"
        );
        assert_eq!(
            system_exit_projection(RustExpr::Var("status".to_string())),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "process", "exit"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("status".to_string())),
                    ty: "i32".to_string(),
                }],
            }
        );
        let recovered =
            unwrap_result_or_panic_uncaught_java_exception(RustExpr::Var("result".to_string()));
        assert_eq!(
            emit::emit_expr(&recovered),
            "result.unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
        assert_eq!(
            recovered,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("result".to_string())),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Raw(
                    "|ex| panic!(\"uncaught Java exception: {:?}\", ex)".to_string()
                )],
            }
        );
        assert_eq!(
            unwrap_result_or_panic_uncaught_java_exception(RustExpr::Raw(
                "fallible()?".to_string()
            )),
            RustExpr::Raw(
                "fallible().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
                    .to_string()
            )
        );
        assert_eq!(
            emit::emit_expr(&thread_sleep_millis_projection(RustExpr::Var(
                "millis".to_string()
            ))),
            "std::thread::sleep(std::time::Duration::from_millis(millis as u64))"
        );
        assert_eq!(
            thread_sleep_millis_projection(RustExpr::Var("millis".to_string())),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "thread", "sleep"])),
                args: vec![RustExpr::Call {
                    function: Box::new(RustExpr::path(["std", "time", "Duration", "from_millis",])),
                    args: vec![RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("millis".to_string())),
                        ty: "u64".to_string(),
                    }],
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&thread_sleep_millis_projection(RustExpr::Raw(
                "left + right".to_string()
            ))),
            "std::thread::sleep(std::time::Duration::from_millis(left + right as u64))"
        );
        assert_eq!(
            emit::emit_expr(&print_macro_projection(
                "println",
                Some(RustExpr::Var("value".to_string()))
            )),
            "println!(\"{}\", value)"
        );
        assert!(matches!(
            print_macro_projection("println", Some(RustExpr::Var("value".to_string()))),
            RustExpr::Call { args, .. } if args == vec![
                RustExpr::Literal("\"{}\"".to_string()),
                RustExpr::Var("value".to_string()),
            ]
        ));
        assert_eq!(
            emit::emit_expr(&print_macro_projection("println", None)),
            "println!()"
        );
        assert!(matches!(
            print_macro_projection("println", None),
            RustExpr::Call { args, .. } if args.is_empty()
        ));
        let logical_not = logical_not_projection(RustExpr::Var("ready".to_string()));
        assert_eq!(emit::emit_expr(&logical_not), "!ready");
        assert_eq!(
            logical_not,
            RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(RustExpr::Var("ready".to_string())),
            }
        );
        assert_eq!(
            emit::emit_expr(&logical_not_projection(RustExpr::Binary {
                op: "&&".to_string(),
                left: Box::new(RustExpr::Var("left".to_string())),
                right: Box::new(RustExpr::Var("right".to_string())),
            })),
            "!(left && right)"
        );
        assert_eq!(
            emit::emit_expr(&logical_not_projection(RustExpr::Raw(
                "left && right".to_string()
            ))),
            "!(left && right)"
        );
        assert_eq!(
            logical_not_projection(RustExpr::Raw("left && right".to_string())),
            RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(RustExpr::Raw("(left && right)".to_string())),
            }
        );
        let optional_owned =
            optional_get_owned_value_projection(RustExpr::Var("cached".to_string()));
        assert_eq!(
            emit::emit_expr(&optional_owned),
            "cached.as_ref().map(|value| (*value).clone()).expect(\"Java null receiver\")"
        );
        assert_eq!(
            optional_owned.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert_eq!(
            optional_owned.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("cached".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| (*value).clone()".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        let optional_or_else = optional_or_else_owned_value_projection(
            RustExpr::Var("cached".to_string()),
            RustExpr::Var("fallback".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&optional_or_else),
            "cached.as_ref().map(|value| (*value).clone()).unwrap_or(fallback)"
        );
        assert_eq!(
            optional_or_else.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert_eq!(
            optional_or_else.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("cached".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| (*value).clone()".to_string())],
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Var("fallback".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&optional_get_owned_value_projection(RustExpr::Raw(
                "make_cached()".to_string()
            ))),
            "make_cached().as_ref().map(|value| (*value).clone()).expect(\"Java null receiver\")"
        );
        assert_eq!(
            optional_get_owned_value_projection(RustExpr::Raw("make_cached()".to_string()))
                .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Raw("make_cached()".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| (*value).clone()".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&optional_or_else_owned_value_projection(
                RustExpr::Raw("make_cached()".to_string()),
                RustExpr::Var("fallback".to_string()),
            )),
            "make_cached().as_ref().map(|value| (*value).clone()).unwrap_or(fallback)"
        );
        assert_eq!(
            optional_or_else_owned_value_projection(
                RustExpr::Raw("make_cached()".to_string()),
                RustExpr::Var("fallback".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Raw("make_cached()".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| (*value).clone()".to_string())],
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Var("fallback".to_string())],
            }
        );
    }

    #[test]
    fn jdk_math_and_objects_projections_own_expression_shapes() {
        let int_max = math_max_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(emit::emit_expr(&int_max), "std::cmp::max(left, right)");
        assert_eq!(
            int_max,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "cmp", "max"])),
                args: vec![
                    RustExpr::Var("left".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
        let int_min = math_min_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(emit::emit_expr(&int_min), "std::cmp::min(left, right)");
        assert_eq!(
            int_min,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "cmp", "min"])),
                args: vec![
                    RustExpr::Var("left".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
        let float_max = math_max_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Float,
        );
        assert_eq!(
            emit::emit_expr(&float_max),
            "java_math_max_f32(left, right)"
        );
        assert_eq!(
            float_max,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_max_f32"])),
                args: vec![
                    RustExpr::Var("left".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
        let double_min = math_min_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Double,
        );
        assert_eq!(
            emit::emit_expr(&double_min),
            "java_math_min_f64(left, right)"
        );
        assert_eq!(
            double_min,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_min_f64"])),
                args: vec![
                    RustExpr::Var("left".to_string()),
                    RustExpr::Var("right".to_string()),
                ],
            }
        );
        assert_eq!(
            emit::emit_expr(&objects_equals_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "left == right"
        );
        assert_eq!(
            emit::emit_expr(&objects_non_null_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "value.is_some()"
        );
        assert_eq!(
            emit::emit_expr(&objects_is_null_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "value.is_none()"
        );
        let anonymous = anonymous_dyn_trait_projection("Callback");
        assert_eq!(emit::emit_expr(&anonymous), "<dyn Callback>::anonymous()");
        assert_eq!(
            anonymous.recorded_shape(),
            Some(RustExprShape::AnonymousDynTrait)
        );
        assert_eq!(
            emit::emit_expr(&unwrap_optional_shared_value(RustExpr::Var(
                "stream".to_string()
            ))),
            "stream.as_ref().unwrap()"
        );
        assert_eq!(
            unwrap_optional_shared_value(RustExpr::Var("stream".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("stream".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&unwrap_java_null_receiver_projection(RustExpr::Var(
                "stream".to_string()
            ))),
            "stream.expect(\"Java null receiver\")"
        );
        assert_eq!(
            unwrap_java_null_receiver_projection(RustExpr::Var("stream".to_string()))
                .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("stream".to_string())),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        let raw_unwrap =
            unwrap_java_null_receiver_projection(RustExpr::Raw("make_stream()".to_string()));
        assert_eq!(
            emit::emit_expr(&raw_unwrap),
            "make_stream().expect(\"Java null receiver\")"
        );
        assert_eq!(
            raw_unwrap.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("make_stream()".to_string())),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        let typed_none = typed_none_projection("Box<dyn std::any::Any>");
        assert_eq!(
            emit::emit_expr(&typed_none),
            "None::<Box<dyn std::any::Any>>"
        );
        assert_eq!(
            typed_none.recorded_shape(),
            Some(RustExprShape::OptionalConstructed)
        );
        assert!(rust_expr_is_constructed_optional_value(&typed_none));
        assert_eq!(
            emit::emit_expr(&self_associated_path_projection("build")),
            "Self::build"
        );
        assert_eq!(
            self_associated_path_projection("build"),
            RustExpr::Path(vec!["Self".to_string(), "build".to_string()])
        );
        assert_eq!(
            emit::emit_expr(&owner_associated_path_projection("Owner", "build")),
            "Owner::build"
        );
        assert_eq!(
            owner_associated_path_projection("Owner", "build"),
            RustExpr::Path(vec!["Owner".to_string(), "build".to_string()])
        );
        assert_eq!(
            emit::emit_expr(&java_class_static_path_projection("for_name")),
            "JavaClass::<()>::for_name"
        );
        assert!(matches!(
            java_class_static_path_projection("for_name"),
            RustExpr::Path(_)
        ));
        assert_eq!(
            emit::emit_expr(&java_class_for_name_path_projection("Owner")),
            "JavaClass::<Owner>::for_name"
        );
        assert_eq!(
            java_class_for_name_path_projection("Owner"),
            RustExpr::Path(vec![
                "JavaClass::<Owner>".to_string(),
                "for_name".to_string(),
            ])
        );
        assert_eq!(
            emit::emit_expr(&external_crate_path_projection("hdf5", "flush")),
            "hdf5::flush"
        );
        assert!(matches!(
            external_crate_path_projection("hdf5", "flush"),
            RustExpr::Path(_)
        ));
        assert_eq!(
            emit::emit_expr(&external_crate_associated_path_projection(
                "sqlite",
                "SQLiteConfig",
                "new",
            )),
            "sqlite::SQLiteConfig::new"
        );
        assert_eq!(
            emit::emit_expr(&static_storage_accessor_projection("VALUES")),
            "Self::__static_v_a_l_u_e_s()"
        );
        assert!(matches!(
            static_storage_accessor_projection("VALUES"),
            RustExpr::Call { function, args }
                if matches!(function.unshaped(), RustExpr::Path(_)) && args.is_empty()
        ));
        assert_eq!(
            emit::emit_expr(&associated_const_projection("Owner", "VALUE")),
            "Owner::VALUE"
        );
        assert!(matches!(
            associated_const_projection("Owner", "VALUE"),
            RustExpr::Path(_)
        ));
        assert_eq!(
            emit::emit_expr(&unbound_method_reference_projection("name")),
            "|value| value.name()"
        );
        assert_eq!(
            emit::emit_expr(&current_receiver_method_reference_projection("emit", 3)),
            "|arg0, arg1, arg2| self.emit(arg0, arg1, arg2)"
        );
        assert_eq!(
            emit::emit_expr(&unbound_method_call_projection(
                RustExpr::Raw("value".to_string()),
                "name",
            )),
            "value.name()"
        );
        assert_eq!(
            emit::emit_expr(&current_receiver_method_call_projection(
                "emit",
                vec![RustExpr::Raw("value".to_string())],
            )),
            "self.emit(value)"
        );
        assert_eq!(
            emit::emit_expr(&typed_single_input_callback_projection(
                "String",
                "value.trim()"
            )),
            "|value: String| value.trim()"
        );
        assert_eq!(
            rendered_vec_macro_body(" vec![1, f(2, 3), vec![4]] "),
            Some("1, f(2, 3), vec![4]".to_string())
        );
        assert_eq!(rendered_vec_macro_body("[1, 2]"), None);
        let logical_not = logical_not_projection(RustExpr::Var("ready".to_string()));
        assert_eq!(emit::emit_expr(&logical_not), "!ready");
        assert_eq!(
            logical_not,
            RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(RustExpr::Var("ready".to_string())),
            }
        );
        assert_eq!(
            emit::emit_expr(&into_projection(RustExpr::Var("value".to_string()))),
            "value.into()"
        );
        assert_eq!(
            emit::emit_expr(&into_projection(RustExpr::Raw(
                "Box::new(value) as Box<dyn std::any::Any>".to_string()
            ))),
            "(Box::new(value) as Box<dyn std::any::Any>).into()"
        );
        assert_eq!(
            emit::emit_expr(&java_update_expression_projection(
                RustExpr::Var("i".to_string()),
                "+=",
                true,
            )),
            "{ i += 1; i }"
        );
        assert_eq!(
            emit::emit_expr(&java_update_expression_projection(
                RustExpr::Var("i".to_string()),
                "+=",
                false,
            )),
            "{ let __java_old_value = i; i += 1; __java_old_value }"
        );
    }

    #[test]
    fn jdk_collection_static_projections_own_expression_shapes() {
        assert_eq!(
            emit::emit_expr(&arrays_as_list_projection(vec![
                RustExpr::Var("first".to_string()),
                RustExpr::Var("second".to_string()),
            ])),
            "vec![first, second]"
        );
        assert!(matches!(
            arrays_as_list_projection(vec![RustExpr::Var("first".to_string())]),
            RustExpr::VecLiteral(_)
        ));
        assert_eq!(
            emit::emit_expr(&arrays_equals_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "java_arrays_equals(left, right)"
        );
        let arrays_to_string = arrays_to_string_projection(RustExpr::Var("values".to_string()));
        assert_eq!(
            emit::emit_expr(&arrays_to_string),
            "java_arrays_to_string(values)"
        );
        assert_eq!(
            arrays_to_string,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_arrays_to_string"])),
                args: vec![RustExpr::Var("values".to_string())],
            }
        );
        let arrays_deep_to_string =
            arrays_deep_to_string_projection(RustExpr::Var("values".to_string()));
        assert_eq!(
            emit::emit_expr(&arrays_deep_to_string),
            "java_arrays_deep_to_string(values)"
        );
        assert_eq!(
            arrays_deep_to_string,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_arrays_deep_to_string"])),
                args: vec![RustExpr::Var("values".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&collections_empty_list_projection()),
            "Vec::new()"
        );
        assert_eq!(
            collections_empty_list_projection(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            collections_empty_set_projection(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "collections", "HashSet", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            collections_empty_map_projection(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "collections", "HashMap", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&collections_empty_sorted_set_projection()),
            "std::collections::BTreeSet::new()"
        );
        assert_eq!(
            collections_empty_sorted_set_projection(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "collections", "BTreeSet", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&collections_singleton_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "std::collections::HashSet::from([value])"
        );
        assert!(matches!(
            collections_singleton_projection(RustExpr::Var("value".to_string())),
            RustExpr::Call { args, .. } if matches!(&args[0], RustExpr::ArrayLiteral(_))
        ));
        assert_eq!(
            emit::emit_expr(&collections_singleton_list_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "vec![value]"
        );
        assert!(matches!(
            collections_singleton_list_projection(RustExpr::Var("value".to_string())),
            RustExpr::VecLiteral(_)
        ));
        assert_eq!(
            emit::emit_expr(&collections_singleton_map_projection(
                RustExpr::Var("key".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "std::collections::HashMap::from([(key, value)])"
        );
        assert!(matches!(
            collections_singleton_map_projection(
                RustExpr::Var("key".to_string()),
                RustExpr::Var("value".to_string()),
            ),
            RustExpr::Call { args, .. } if matches!(&args[0], RustExpr::ArrayLiteral(_))
        ));
        assert_eq!(
            emit::emit_expr(&collections_n_copies_projection(
                RustExpr::Var("count".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "vec![value; count as usize]"
        );
        assert!(matches!(
            collections_n_copies_projection(
                RustExpr::Var("count".to_string()),
                RustExpr::Var("value".to_string()),
            ),
            RustExpr::VecRepeat { .. }
        ));
        assert_eq!(
            emit::emit_expr(&native_collection_empty_new_projection(
                crate::stdlib::JavaCollectionKind::List,
            )),
            "Vec::new()"
        );
        assert_eq!(
            emit::emit_expr(&native_collection_empty_new_projection(
                crate::stdlib::JavaCollectionKind::Map,
            )),
            "std::collections::HashMap::new()"
        );
        assert_eq!(
            emit::emit_expr(&native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Var("capacity".to_string()),
            )),
            "Vec::with_capacity(capacity as usize)"
        );
        assert_eq!(
            native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Var("capacity".to_string()),
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "with_capacity"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("capacity".to_string())),
                    ty: "usize".to_string(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Raw("left + right".to_string()),
            )),
            "Vec::with_capacity(left + right as usize)"
        );
        assert_eq!(
            native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("capacity".to_string())),
                    ty: "i32".to_string(),
                },
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "with_capacity"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("capacity".to_string())),
                        ty: "i32".to_string(),
                    }),
                    ty: "usize".to_string(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("capacity".to_string())),
                    ty: "i32".to_string(),
                },
            )),
            "Vec::with_capacity((capacity as i32) as usize)"
        );
        assert_eq!(
            emit::emit_expr(&native_collection_default_constructor_projection(
                crate::stdlib::JavaCollectionKind::Set,
            )),
            "std::collections::HashSet::new()"
        );
        assert_eq!(
            native_collection_default_constructor_projection(
                crate::stdlib::JavaCollectionKind::Set,
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "collections", "HashSet", "new"])),
                args: Vec::new(),
            }
        );
        let set_from_source = native_collection_from_source_projection(
            crate::stdlib::JavaCollectionKind::Set,
            RustExpr::Var("values".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&set_from_source),
            "values.into_iter().collect::<std::collections::HashSet<_>>()"
        );
        assert_eq!(
            set_from_source.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "into_iter".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<std::collections::HashSet<_>>".to_string(),
                args: Vec::new(),
            }
        );
        let raw_set_from_source = native_collection_from_source_projection(
            crate::stdlib::JavaCollectionKind::Set,
            RustExpr::Raw("factory()".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&raw_set_from_source),
            "factory().into_iter().collect::<std::collections::HashSet<_>>()"
        );
        assert!(matches!(
            raw_set_from_source,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.unshaped(), RustExpr::MethodCall { .. })
        ));
        assert_eq!(
            emit::emit_expr(&java_iterator_from_collection_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "JavaIterator::new(values.iter().cloned())"
        );
        assert_eq!(
            java_iterator_from_collection_projection(RustExpr::Var("values".to_string()))
                .into_unshaped(),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["JavaIterator", "new"])),
                args: vec![RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    }),
                    method: "cloned".to_string(),
                    args: Vec::new(),
                }],
            }
        );
        let collected_string =
            char_sequence_collect_string_projection(RustExpr::Var("chars".to_string()));
        assert_eq!(
            emit::emit_expr(&collected_string),
            "chars.iter().collect::<String>()"
        );
        assert_eq!(
            collected_string,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("chars".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<String>".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&char_sequence_collect_string_projection(RustExpr::Raw(
                "make_chars()".to_string()
            ))),
            "make_chars().iter().collect::<String>()"
        );
        assert_eq!(
            emit::emit_expr(&sequence_iter_cloned_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.iter().cloned()"
        );
        assert_eq!(
            sequence_iter_cloned_projection(RustExpr::Var("values".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "cloned".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&sequence_iter_copied_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.iter().copied()"
        );
        assert_eq!(
            sequence_iter_copied_projection(RustExpr::Var("values".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "copied".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&iterator_next_expect_projection(RustExpr::Var(
                "entries".to_string()
            ))),
            "entries.next().expect(\"Java iterator exhausted\")"
        );
        assert_eq!(
            iterator_next_expect_projection(RustExpr::Var("entries".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("entries".to_string())),
                    method: "next".to_string(),
                    args: Vec::new(),
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java iterator exhausted\"".to_string())],
            }
        );
        let tuple_field = tuple_field_projection(RustExpr::Var("entry".to_string()), 1);
        assert_eq!(emit::emit_expr(&tuple_field), "entry.1");
        assert_eq!(
            tuple_field,
            RustExpr::Field(
                Box::new(RustExpr::Var("entry".to_string())),
                "1".to_string()
            )
        );
        assert_eq!(
            emit::emit_expr(&discard_value_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "{ let _ = value; }"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_box_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "Box::new(value) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            erased_object_box_projection(RustExpr::Var("value".to_string())).into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Var("value".to_string())],
                }),
                ty: "Box<dyn std::any::Any>".to_string(),
            }
        );
        let mut erased_target = RustSlotShape::conservative(JavaType::Named {
            name: "Object".to_string(),
            args: Vec::new(),
        });
        erased_target.dispatch = DispatchShape::ErasedObject;
        let borrowed_facade_policy = RustBorrowedFacadeReturnPolicy {
            rust_type_name: "&std::path::Path".to_string(),
            java_semantic_shape: RustSlotShape::conservative(JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }),
            target: erased_target.clone(),
            recovery: RustBorrowedFacadeReturnRecovery::ToPathBuf,
        };
        assert_eq!(
            emit::emit_expr(&apply_rust_borrowed_facade_return(
                RustExpr::Var("path".to_string()),
                &borrowed_facade_policy,
            )),
            "Box::new(path.to_path_buf()) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            apply_rust_borrowed_facade_return(
                RustExpr::Var("path".to_string()),
                &borrowed_facade_policy,
            )
            .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("path".to_string())),
                        method: "to_path_buf".to_string(),
                        args: Vec::new(),
                    }],
                }),
                ty: "Box<dyn std::any::Any>".to_string(),
            }
        );
        let mut optional_erased_target = RustSlotShape::conservative(
            crate::types::optional_java_type(erased_target.java_ty.clone()),
        );
        optional_erased_target.dispatch = DispatchShape::ErasedObject;
        let optional_borrowed_facade_policy = RustBorrowedFacadeReturnPolicy {
            rust_type_name: "&std::path::Path".to_string(),
            java_semantic_shape: RustSlotShape::conservative(JavaType::Named {
                name: "File".to_string(),
                args: Vec::new(),
            }),
            target: optional_erased_target,
            recovery: RustBorrowedFacadeReturnRecovery::ToPathBuf,
        };
        let optional_borrowed_facade_return = apply_rust_borrowed_facade_return(
            RustExpr::Var("path".to_string()),
            &optional_borrowed_facade_policy,
        );
        assert_eq!(
            emit::emit_expr(&optional_borrowed_facade_return),
            "Some(Box::new(path.to_path_buf()) as Box<dyn std::any::Any>)"
        );
        assert_eq!(
            optional_borrowed_facade_return.recorded_shape(),
            Some(RustExprShape::OptionalConstructed)
        );
        let optional_source_borrowed_facade_return = apply_optional_rust_borrowed_facade_return(
            RustExpr::Var("maybe_path".to_string()),
            &borrowed_facade_policy,
        );
        assert_eq!(
            emit::emit_expr(&optional_source_borrowed_facade_return),
            "maybe_path.map(|value| Box::new(value.to_path_buf()) as Box<dyn std::any::Any>).expect(\"Java null receiver\")"
        );
        assert_eq!(
            optional_source_borrowed_facade_return.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("maybe_path".to_string())),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw(
                        "|value| Box::new(value.to_path_buf()) as Box<dyn std::any::Any>"
                            .to_string()
                    )],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        let optional_source_optional_target = apply_optional_rust_borrowed_facade_return(
            RustExpr::Var("maybe_path".to_string()),
            &optional_borrowed_facade_policy,
        );
        assert_eq!(
            emit::emit_expr(&optional_source_optional_target),
            "maybe_path.map(|value| Box::new(value.to_path_buf()) as Box<dyn std::any::Any>)"
        );
        assert_eq!(
            optional_source_optional_target,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("maybe_path".to_string())),
                method: "map".to_string(),
                args: vec![RustExpr::Raw(
                    "|value| Box::new(value.to_path_buf()) as Box<dyn std::any::Any>".to_string()
                )],
            }
        );
        assert_eq!(
            emit::emit_expr(&erased_object_box_projection(RustExpr::Raw(
                "(variant, None::<Void>)".to_string()
            ))),
            "Box::new((variant, None::<Void>)) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_constructor_projection(Vec::new())),
            "Box::new(()) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            erased_object_constructor_projection(Vec::new()).into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Tuple(Vec::new())],
                }),
                ty: "Box<dyn std::any::Any>".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&erased_object_constructor_projection(vec![RustExpr::Var(
                "value".to_string()
            )])),
            "Box::new(value) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_constructor_projection(vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string())
            ])),
            "Box::new((left, right)) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            erased_object_constructor_projection(vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string())
            ])
            .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Tuple(vec![
                        RustExpr::Var("left".to_string()),
                        RustExpr::Var("right".to_string())
                    ])],
                }),
                ty: "Box<dyn std::any::Any>".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&boxed_dyn_trait_projection(
                RustExpr::Var("value".to_string()),
                "Writer",
            )),
            "Box::new(value) as Box<dyn Writer>"
        );
        assert_eq!(
            boxed_dyn_trait_projection(RustExpr::Var("value".to_string()), "Writer")
                .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Var("value".to_string())],
                }),
                ty: "Box<dyn Writer>".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&boxed_dyn_trait_projection(
                RustExpr::Raw("left, right".to_string()),
                "Writer",
            )),
            "Box::new((left, right)) as Box<dyn Writer>"
        );
        assert_eq!(
            emit::emit_expr(&unsupported_deserialization_cast_handoff_projection(
                RustExpr::Raw("r#in.read_object()?".to_string()),
                "Localizable",
            )),
            "{ let _java_deserialized = r#in.read_object()?; panic!(\"unsupported Java deserialization cast to Localizable; provide a typed serialization facade\") }"
        );
        assert_eq!(
            emit::emit_expr(&varargs_packed_vec_projection(Vec::new())),
            "Vec::new()"
        );
        assert_eq!(
            varargs_packed_vec_projection(Vec::new()),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&varargs_packed_vec_projection(vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            ])),
            "vec![left, right]"
        );
        assert!(matches!(
            varargs_packed_vec_projection(vec![RustExpr::Var("left".to_string())]),
            RustExpr::VecLiteral(_)
        ));
        assert_eq!(
            emit::emit_expr(&varargs_spread_extend_projection(
                RustExpr::Raw("vec![left]".to_string()),
                RustExpr::Var("tail".to_string()),
            )),
            "{ let mut __java_varargs = vec![left]; __java_varargs.extend(tail); __java_varargs }"
        );
        assert_eq!(
            emit::emit_expr(&collection_remove_if_projection(
                RustExpr::Var("lines".to_string()),
                "line",
                "line.is_empty()",
            )),
            "lines.retain(|line| !(line.is_empty()))"
        );
        assert_eq!(
            collection_remove_if_projection(
                RustExpr::Var("lines".to_string()),
                "line",
                "line.is_empty()",
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("lines".to_string())),
                method: "retain".to_string(),
                args: vec![RustExpr::Raw("|line| !(line.is_empty())".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&collection_sort_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.sort()"
        );
        assert_eq!(
            collection_sort_projection(RustExpr::Var("values".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "sort".to_string(),
                args: Vec::new(),
            }
        );
        let index_of = sequence_index_of_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("needle".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&index_of),
            "values.iter().position(|item| item == needle).map(|index| index as i32).unwrap_or(-1)"
        );
        assert_eq!(
            index_of,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("values".to_string())),
                            method: "iter".to_string(),
                            args: Vec::new(),
                        }),
                        method: "position".to_string(),
                        args: vec![RustExpr::Raw("|item| item == needle".to_string())],
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|index| index as i32".to_string())],
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Literal("-1".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&sequence_index_of_projection(
                RustExpr::Raw("make_values()".to_string()),
                RustExpr::Var("needle".to_string()),
            )),
            "make_values().iter().position(|item| item == needle).map(|index| index as i32).unwrap_or(-1)"
        );
        assert_eq!(
            sequence_index_of_projection(
                RustExpr::Raw("make_values()".to_string()),
                RustExpr::Var("needle".to_string()),
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Raw("make_values()".to_string())),
                            method: "iter".to_string(),
                            args: Vec::new(),
                        }),
                        method: "position".to_string(),
                        args: vec![RustExpr::Raw("|item| item == needle".to_string())],
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|index| index as i32".to_string())],
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Literal("-1".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&sequence_index_of_projection(
                RustExpr::Raw("left + right".to_string()),
                RustExpr::Var("needle".to_string()),
            )),
            "(left + right).iter().position(|item| item == needle).map(|index| index as i32).unwrap_or(-1)"
        );
        assert_eq!(
            emit::emit_expr(&collection_sort_by_comparator_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("comparator".to_string()),
            )),
            "{ let __java_comparator = comparator; values.sort_by(|a, b| __java_comparator.compare(a.clone(), b.clone()).cmp(&0)); }"
        );
        assert_eq!(
            emit::emit_expr(&array_fill_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "values.fill(value)"
        );
        assert_eq!(
            array_fill_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("value".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "fill".to_string(),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&sequence_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Raw("index as usize".to_string()),
            )),
            "values.get(index as usize).expect(\"Java index out of bounds\")"
        );
        assert_eq!(
            sequence_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Raw("index as usize".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "get".to_string(),
                    args: vec![RustExpr::Raw("index as usize".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal(
                    "\"Java index out of bounds\"".to_string()
                )],
            }
        );
        assert_eq!(
            emit::emit_expr(&map_get_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get(key)"
        );
        assert_eq!(
            map_get_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "get".to_string(),
                args: vec![RustExpr::Var("key".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&map_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get(key).expect(\"Java null receiver\")"
        );
        assert_eq!(
            map_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "get".to_string(),
                    args: vec![RustExpr::Var("key".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&map_get_mut_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get_mut(key).expect(\"Java null receiver\")"
        );
        assert_eq!(
            map_get_mut_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "get_mut".to_string(),
                    args: vec![RustExpr::Var("key".to_string())],
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        let copied_expect =
            optional_copied_expect_projection(RustExpr::Raw("values.get(key)".to_string()));
        assert_eq!(
            emit::emit_expr(&copied_expect),
            "values.get(key).copied().expect(\"Java null receiver\")"
        );
        assert_eq!(
            copied_expect.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        let structured_copied_expect =
            optional_copied_expect_projection(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&structured_copied_expect),
            "value.copied().expect(\"Java null receiver\")"
        );
        assert_eq!(
            structured_copied_expect.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "copied".to_string(),
                    args: Vec::new(),
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            structured_copied_expect.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        let subtype_constructor = subtype_owned_constructor_from_borrowed_projection(
            "Locatable",
            RustExpr::Var("value".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&subtype_constructor),
            "Locatable::new_1(value.clone())"
        );
        assert_eq!(
            subtype_constructor,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Locatable", "new_1"])),
                args: vec![RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "clone".to_string(),
                    args: Vec::new(),
                }],
            }
        );
        let raw_subtype_constructor = subtype_owned_constructor_from_borrowed_projection(
            "Locatable",
            RustExpr::Raw("left + right".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&raw_subtype_constructor),
            "Locatable::new_1(left + right.clone())"
        );
        assert_eq!(
            raw_subtype_constructor,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Locatable", "new_1"])),
                args: vec![RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("left + right".to_string())),
                    method: "clone".to_string(),
                    args: Vec::new(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&exception_self_as_supertype_facade_boundary_projection()),
            "Default::default()/* TODO(translation): Java exception self-as-supertype facade boundary */"
        );
        assert_eq!(
            emit::emit_expr(&explicit_java_facade_cast_placeholder_projection(
                RustExpr::Var("graphics".to_string()),
                "Graphics2D",
            )),
            "{ let _java_facade_cast_source = &(graphics); Graphics2D::default() }/* TODO(translation): validate explicit Java facade cast */"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                false,
            )),
            "obj.downcast_ref::<Target>().expect(\"invalid Java cast\")"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                true,
            )),
            "obj.as_ref().and_then(|value| value.downcast_ref::<Target>()).expect(\"invalid Java cast\")"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "GuideTree.Node",
                false,
            )),
            "obj.downcast_ref::<GuideTreeNode>().expect(\"invalid Java cast\")"
        );
    }

    #[test]
    fn string_join_projection_helpers_own_literal_and_to_string_idempotence() {
        assert_eq!(
            render_string_join_vararg_value(RustExpr::Literal("\"bin\"".to_string())),
            "\"bin\".to_string()"
        );
        let structured_literal_to_string = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Literal("\"bin\"".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            string_literal_source(&structured_literal_to_string),
            Some("bin".to_string())
        );
        assert_eq!(
            string_literal_source(&RustExpr::Literal("\"bin\".to_string()".to_string())),
            Some("bin".to_string())
        );
        assert_eq!(
            string_literal_source(&RustExpr::Raw("\"bin\"".to_string())),
            None
        );
        assert_eq!(
            string_literal_source(&RustExpr::Raw("\"bin\".to_string()".to_string())),
            None
        );
        assert_eq!(
            render_string_join_vararg_value(RustExpr::Raw("\"bin\"".to_string())),
            "(\"bin\").to_string()"
        );
        assert_eq!(
            render_string_join_vararg_value(RustExpr::Raw("extra.to_string()".to_string())),
            "extra.to_string()"
        );
        assert_eq!(
            render_string_join_vararg_value(RustExpr::Raw("format!(\"{}\", name)".to_string())),
            "(format!(\"{}\", name)).to_string()"
        );
        assert_eq!(
            emit::emit_expr(&string_join_delimiter_projection(RustExpr::Literal(
                "\"|\"".to_string()
            ))),
            "\"|\""
        );
        assert_eq!(
            emit::emit_expr(&string_join_delimiter_projection(RustExpr::Var(
                "sep".to_string()
            ))),
            "sep"
        );
        assert_eq!(
            emit::emit_expr(&string_join_varargs_projection(
                RustExpr::Var("sep".to_string()),
                vec![
                    RustExpr::Literal("\"bin\"".to_string()),
                    RustExpr::Var("extra".to_string()),
                    RustExpr::Raw("suffix.to_string()".to_string()),
                ],
            )),
            "vec![\"bin\".to_string(), (extra).to_string(), suffix.to_string()].join(sep)"
        );
    }

    #[test]
    fn string_format_arg_projection_owns_display_to_string_spelling() {
        assert_eq!(
            render_string_format_arg_value(RustExpr::Var("count".to_string())),
            "count.to_string()"
        );
        assert_eq!(
            string_format_arg_value_projection(RustExpr::Var("count".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("count".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            render_string_format_arg_value(RustExpr::Raw("value + 1".to_string())),
            "(value + 1).to_string()"
        );
        assert_eq!(
            string_format_arg_value_projection(RustExpr::Raw("value + 1".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("(value + 1)".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&string_format_projection(Vec::new())),
            "String::new()"
        );
        assert_eq!(
            string_format_projection(Vec::new()),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["String", "new"])),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::Literal("\"%s:%04.2f\"".to_string()),
                RustExpr::Var("name".to_string()),
                RustExpr::Var("score".to_string()),
            ])),
            "format!(\"{}:{:04.2}\", name, score)"
        );
        assert!(matches!(
            string_format_projection(vec![
                RustExpr::Literal("\"%s:%04.2f\"".to_string()),
                RustExpr::Var("name".to_string()),
                RustExpr::Var("score".to_string()),
            ]),
            RustExpr::Call { function, args }
                if matches!(function.unshaped(), RustExpr::Raw(raw) if raw == "format!")
                    && args.len() == 3
        ));
        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::Var("template".to_string()),
                RustExpr::Var("count".to_string()),
            ])),
            "java_string_format(template, vec![count.to_string()])"
        );
        let runtime_format = string_format_projection(vec![
            RustExpr::Var("template".to_string()),
            RustExpr::Var("count".to_string()),
        ]);
        assert!(matches!(
            runtime_format,
            RustExpr::Call { args, .. } if matches!(&args[1], RustExpr::VecLiteral(_))
        ));
        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::Raw("\"%s\"".to_string()),
                RustExpr::Var("name".to_string()),
            ])),
            "java_string_format(\"%s\", vec![name.to_string()])"
        );
        assert!(is_java_locale_format_arg(&RustExpr::Var(
            "Locale".to_string()
        )));
        assert!(is_java_locale_format_arg(&RustExpr::path(["Locale", "US"])));
        assert!(is_java_locale_format_arg(&RustExpr::path([
            "JavaUtilLocale",
            "US"
        ])));
        assert!(!is_java_locale_format_arg(&RustExpr::path([
            "LocaleSuffix",
            "US"
        ])));
        assert!(is_java_locale_format_arg(&RustExpr::Raw(
            "Locale::US".to_string()
        )));

        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::path(["Locale", "US"]),
                RustExpr::Literal("\"%s\"".to_string()),
                RustExpr::Var("name".to_string()),
            ])),
            "format!(\"{}\", name)"
        );
    }

    #[test]
    fn string_helper_receiver_projection_owns_runtime_helper_receiver_view() {
        let structured = string_helper_receiver_projection(RustExpr::Var("value".to_string()));
        assert_eq!(emit::emit_expr(&structured), "&value");
        assert_eq!(
            structured.clone().into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            }
        );
        assert_eq!(structured.recorded_shape(), Some(RustExprShape::StringView));

        let optional_unwrap = string_helper_receiver_projection(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("left".to_string())),
                method: "as_ref".to_string(),
                args: Vec::new(),
            }),
            method: "unwrap".to_string(),
            args: Vec::new(),
        });
        assert_eq!(
            emit::emit_expr(&optional_unwrap),
            "left.as_ref().unwrap().as_str()"
        );
        assert_eq!(
            optional_unwrap.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("left".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "unwrap".to_string(),
                    args: Vec::new(),
                }),
                method: "as_str".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            optional_unwrap.recorded_shape(),
            Some(RustExprShape::StringView)
        );

        assert_eq!(
            emit::emit_expr(&string_helper_receiver_projection(RustExpr::Raw(
                "left.as_ref().unwrap()".to_string()
            ))),
            "left.as_ref().unwrap().as_str()"
        );
        assert_eq!(
            string_helper_receiver_projection(RustExpr::Raw("opaque(value)".to_string()))
                .into_unshaped(),
            RustExpr::Raw("&opaque(value)".to_string())
        );
    }

    #[test]
    fn string_get_bytes_projection_owns_byte_view_spelling() {
        let bytes = string_get_bytes_projection(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&bytes),
            "value.as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()"
        );
        assert_eq!(
            bytes,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("value".to_string())),
                            method: "as_bytes".to_string(),
                            args: Vec::new(),
                        }),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|byte| *byte as i8".to_string())],
                }),
                method: "collect::<Vec<i8>>".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&string_get_bytes_projection(RustExpr::Raw(
                "make_value()".to_string()
            ))),
            "make_value().as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()"
        );
        assert_eq!(
            string_get_bytes_projection(RustExpr::Raw("make_value()".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Raw("make_value()".to_string())),
                            method: "as_bytes".to_string(),
                            args: Vec::new(),
                        }),
                        method: "iter".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|byte| *byte as i8".to_string())],
                }),
                method: "collect::<Vec<i8>>".to_string(),
                args: Vec::new(),
            }
        );
        let chars = string_to_char_array_projection(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&chars),
            "value.chars().collect::<Vec<char>>()"
        );
        assert_eq!(
            chars,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "chars".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<Vec<char>>".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&string_to_char_array_projection(RustExpr::Raw(
                "make_value()".to_string()
            ))),
            "make_value().chars().collect::<Vec<char>>()"
        );
        assert_eq!(
            string_to_char_array_projection(RustExpr::Raw("make_value()".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("make_value()".to_string())),
                    method: "chars".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<Vec<char>>".to_string(),
                args: Vec::new(),
            }
        );
    }

    #[test]
    fn primitive_number_value_projection_owns_boxed_value_cast_spelling() {
        let cast = primitive_number_value_projection(
            RustExpr::Var("value".to_string()),
            PrimitiveType::Double,
        );
        assert_eq!(emit::emit_expr(&cast), "value as f64");
        assert_eq!(
            cast,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("value".to_string())),
                ty: "f64".to_string(),
            }
        );

        assert_eq!(
            primitive_number_value_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Double
            ),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Raw("(left + right)".to_string())),
                ty: "f64".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&primitive_number_value_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Double
            )),
            "(left + right) as f64"
        );
        assert_eq!(
            emit::emit_expr(&primitive_number_value_projection(
                RustExpr::Var("flag".to_string()),
                PrimitiveType::Boolean
            )),
            "flag"
        );

        let scalar_policy = RustScalarSlotCastPolicy {
            rust_type_name: "usize".to_string(),
            java_semantic_shape: RustSlotShape::conservative(JavaType::Primitive(
                PrimitiveType::Long,
            )),
            target: RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int)),
            target_primitive: PrimitiveType::Int,
        };
        let narrowing =
            apply_rust_scalar_slot_cast(RustExpr::Var("length".to_string()), &scalar_policy);
        assert_eq!(emit::emit_expr(&narrowing), "length.try_into().unwrap()");
        assert_eq!(
            narrowing,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("length".to_string())),
                    method: "try_into".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&apply_rust_scalar_slot_cast(
                RustExpr::Raw("left + right".to_string()),
                &scalar_policy
            )),
            "(left + right).try_into().unwrap()"
        );
    }

    #[test]
    fn facade_receiver_method_call_projection_owns_method_path_spelling() {
        assert_eq!(
            emit::emit_expr(&facade_receiver_method_call_projection(
                RustExpr::Var("value".to_string()),
                "StringFacade::display_name",
                Vec::new()
            )),
            "value.display_name()"
        );
    }

    #[test]
    fn primitive_f64_method_projection_owns_math_cast_spelling() {
        let projected = primitive_f64_method_projection(RustExpr::Var("value".to_string()), "ln");
        assert_eq!(emit::emit_expr(&projected), "(value as f64).ln()");
        assert_eq!(
            projected,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("value".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "ln".to_string(),
                args: Vec::new(),
            }
        );

        assert_eq!(
            emit::emit_expr(&primitive_f64_method_projection(
                RustExpr::Raw("left + right".to_string()),
                "ln"
            )),
            "(left + right as f64).ln()"
        );
        assert_eq!(
            primitive_f64_method_projection(RustExpr::Raw("left + right".to_string()), "ln"),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Raw("left + right".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "ln".to_string(),
                args: Vec::new(),
            }
        );

        let binary = primitive_f64_binary_method_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            "atan2",
        );
        assert_eq!(
            emit::emit_expr(&binary),
            "(left as f64).atan2(right as f64)"
        );
        assert_eq!(
            binary,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("left".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "atan2".to_string(),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("right".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&primitive_f64_binary_method_projection(
                RustExpr::Raw("left + middle".to_string()),
                RustExpr::Var("right".to_string()),
                "atan2",
            )),
            "(left + middle as f64).atan2(right as f64)"
        );
        assert_eq!(
            primitive_f64_binary_method_projection(
                RustExpr::Raw("left + middle".to_string()),
                RustExpr::Var("right".to_string()),
                "atan2",
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Raw("left + middle".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "atan2".to_string(),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("right".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );

        let pow = math_pow_projection(
            RustExpr::Var("base".to_string()),
            RustExpr::Var("exponent".to_string()),
        );
        assert_eq!(emit::emit_expr(&pow), "(base as f64).powf(exponent as f64)");
        assert_eq!(
            pow,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("base".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "powf".to_string(),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("exponent".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&math_pow_projection(
                RustExpr::Var("base".to_string()),
                RustExpr::Raw("exponent + delta".to_string()),
            )),
            "(base as f64).powf(exponent + delta as f64)"
        );
        assert_eq!(
            math_pow_projection(
                RustExpr::Var("base".to_string()),
                RustExpr::Raw("exponent + delta".to_string()),
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("base".to_string())),
                    ty: "f64".to_string(),
                }),
                method: "powf".to_string(),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Raw("exponent + delta".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );

        let float_round =
            math_round_projection(RustExpr::Var("value".to_string()), PrimitiveType::Float);
        assert_eq!(emit::emit_expr(&float_round), "java_math_round_f32(value)");
        assert_eq!(
            float_round,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_round_f32"])),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let int_round =
            math_round_projection(RustExpr::Var("value".to_string()), PrimitiveType::Int);
        assert_eq!(
            emit::emit_expr(&int_round),
            "java_math_round_f64(value as f64)"
        );
        assert_eq!(
            int_round,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_round_f64"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("value".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&math_round_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int,
            )),
            "java_math_round_f64(left + right as f64)"
        );
        assert_eq!(
            math_round_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_round_f64"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::Raw("left + right".to_string())),
                    ty: "f64".to_string(),
                }],
            }
        );

        let int_abs = math_abs_projection(RustExpr::Var("value".to_string()), PrimitiveType::Int);
        assert_eq!(emit::emit_expr(&int_abs), "value.wrapping_abs()");
        assert_eq!(
            int_abs,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "wrapping_abs".to_string(),
                args: Vec::new(),
            }
        );

        let double_abs =
            math_abs_projection(RustExpr::Var("value".to_string()), PrimitiveType::Double);
        assert_eq!(emit::emit_expr(&double_abs), "value.abs()");
        assert_eq!(
            double_abs,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "abs".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&math_abs_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int,
            )),
            "(left + right).wrapping_abs()"
        );
        assert_eq!(
            math_abs_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("(left + right)".to_string())),
                method: "wrapping_abs".to_string(),
                args: Vec::new(),
            }
        );

        let floor_div = math_floor_div_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&floor_div),
            "java_math_floor_div_i32(left as i32, right as i32)"
        );
        assert_eq!(
            floor_div,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_floor_div_i32"])),
                args: vec![
                    RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("left".to_string())),
                        ty: "i32".to_string(),
                    },
                    RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("right".to_string())),
                        ty: "i32".to_string(),
                    },
                ],
            }
        );

        let floor_mod = math_floor_mod_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Long,
            PrimitiveType::Long,
        );
        assert_eq!(
            emit::emit_expr(&floor_mod),
            "java_math_floor_mod_i64(left as i64, right as i64)"
        );
        assert_eq!(
            floor_mod,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_floor_mod_i64"])),
                args: vec![
                    RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("left".to_string())),
                        ty: "i64".to_string(),
                    },
                    RustExpr::Cast {
                        expr: Box::new(RustExpr::Var("right".to_string())),
                        ty: "i64".to_string(),
                    },
                ],
            }
        );
        let narrowed_floor_mod = math_floor_mod_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Long,
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&narrowed_floor_mod),
            "java_math_floor_mod_i64(left as i64, right as i64) as i32"
        );
        assert_eq!(
            narrowed_floor_mod,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["java_math_floor_mod_i64"])),
                    args: vec![
                        RustExpr::Cast {
                            expr: Box::new(RustExpr::Var("left".to_string())),
                            ty: "i64".to_string(),
                        },
                        RustExpr::Cast {
                            expr: Box::new(RustExpr::Var("right".to_string())),
                            ty: "i64".to_string(),
                        },
                    ],
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&math_floor_mod_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Raw("right + delta".to_string()),
                PrimitiveType::Long,
                PrimitiveType::Int,
            )),
            "(java_math_floor_mod_i64(left as i64, right + delta as i64)) as i32"
        );
        assert_eq!(
            emit::emit_expr(&math_floor_div_projection(
                RustExpr::Raw("left + middle".to_string()),
                RustExpr::Var("right".to_string()),
                PrimitiveType::Int,
            )),
            "java_math_floor_div_i32(left + middle as i32, right as i32)"
        );
    }

    #[test]
    fn len_i32_projection_owns_java_length_spelling() {
        assert_eq!(
            len_i32_projection(RustExpr::Var("value".to_string())).into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "len".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&len_i32_projection(RustExpr::Var("value".to_string()))),
            "value.len() as i32"
        );
        assert_eq!(
            array2_rows_i32_projection(RustExpr::Var("matrix".to_string())).into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("matrix".to_string())),
                    method: "nrows".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&array2_rows_i32_projection(RustExpr::Var(
                "matrix".to_string()
            ))),
            "matrix.nrows() as i32"
        );
        assert_eq!(
            array2_cols_i32_projection(RustExpr::Raw(
                "matrix.row(row as usize).to_vec()".to_string()
            ))
            .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw(
                        "matrix.row(row as usize).to_vec()".to_string()
                    )),
                    method: "len".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&array2_cols_i32_projection(RustExpr::Raw(
                "matrix.row(row as usize).to_vec()".to_string()
            ))),
            "matrix.row(row as usize).to_vec().len() as i32"
        );
    }

    #[test]
    fn primitive_is_nan_projection_owns_float_nan_spelling() {
        assert_eq!(
            primitive_is_nan_projection(RustExpr::Var("value".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "is_nan".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&primitive_is_nan_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "value.is_nan()"
        );
    }

    #[test]
    fn borrowed_sequence_extend_arg_owns_string_view_recovery_spelling() {
        let arg = borrowed_sequence_extend_arg(
            RustExpr::Var("other".to_string()),
            &crate::shape_projection::borrowed_string_content_shape(),
            &FactSet::default(),
        );
        assert_eq!(emit::emit_expr(&arg), "other.iter().map(|v| v.to_string())");
        assert_eq!(
            arg,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("other".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
            }
        );
        assert_eq!(
            emit::emit_expr(&borrowed_sequence_extend_arg(
                RustExpr::Raw("left + right".to_string()),
                &crate::shape_projection::borrowed_string_content_shape(),
                &FactSet::default(),
            )),
            "(left + right).iter().map(|v| v.to_string())"
        );
    }

    #[test]
    fn java_display_plan_renders_optional_null_policy() {
        let format_value = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::FormatValue,
        );
        assert_eq!(emit::emit_expr(&format_value), "(left + right).to_string()");
        assert_eq!(
            format_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Raw("(left + right)".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );

        let float_value = apply_java_display_plan(
            RustExpr::Var("value".to_string()),
            JavaDisplayPlan::ConcatFloatValue { double: true },
        );
        assert_eq!(
            emit::emit_expr(&float_value),
            "java_double_to_string(value)"
        );
        assert_eq!(
            float_value,
            RustExpr::Call {
                function: Box::new(RustExpr::Var("java_double_to_string".to_string())),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let erased_object = apply_java_display_plan(
            RustExpr::Var("item".to_string()),
            JavaDisplayPlan::ErasedObject,
        );
        assert_eq!(
            emit::emit_expr(&erased_object),
            "java_erased_object_to_string(&item)"
        );
        assert_eq!(
            erased_object,
            RustExpr::Call {
                function: Box::new(RustExpr::Var("java_erased_object_to_string".to_string())),
                args: vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("item".to_string())),
                }],
            }
        );

        let expr = apply_java_display_plan(
            RustExpr::Var("name".to_string()),
            JavaDisplayPlan::OptionalString,
        );
        assert_eq!(
            emit::emit_expr(&expr),
            "name.as_deref().unwrap_or(\"null\")"
        );
        assert_eq!(
            expr,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("name".to_string())),
                    method: "as_deref".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap_or".to_string(),
                args: vec![RustExpr::Literal("\"null\"".to_string())],
            }
        );

        let optional_value = apply_java_display_plan(
            RustExpr::Var("item".to_string()),
            JavaDisplayPlan::OptionalValue,
        );
        assert_eq!(
            emit::emit_expr(&optional_value),
            "item.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        assert_eq!(
            optional_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("item".to_string())),
                        method: "as_ref".to_string(),
                        args: Vec::new(),
                    }),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| value.to_string()".to_string())],
                }),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Raw("|| \"null\".to_string()".to_string())],
            }
        );

        let optional_borrowed_value = apply_java_display_plan(
            RustExpr::Var("item".to_string()),
            JavaDisplayPlan::OptionalBorrowedValue,
        );
        assert_eq!(
            emit::emit_expr(&optional_borrowed_value),
            "item.map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        assert_eq!(
            optional_borrowed_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("item".to_string())),
                    method: "map".to_string(),
                    args: vec![RustExpr::Raw("|value| value.to_string()".to_string())],
                }),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Raw("|| \"null\".to_string()".to_string())],
            }
        );

        assert_eq!(
            emit::emit_expr(&apply_java_display_plan(
                RustExpr::Raw("make_name()".to_string()),
                JavaDisplayPlan::OptionalString,
            )),
            "make_name().as_deref().unwrap_or(\"null\")"
        );
        assert_eq!(
            emit::emit_expr(&apply_java_display_plan(
                RustExpr::Raw("make_item()".to_string()),
                JavaDisplayPlan::OptionalValue,
            )),
            "make_item().as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        assert_eq!(
            emit::emit_expr(&apply_java_display_plan(
                RustExpr::Raw("make_item()".to_string()),
                JavaDisplayPlan::OptionalBorrowedValue,
            )),
            "make_item().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
    }

    #[test]
    fn java_display_operand_removes_redundant_arithmetic_to_string() {
        let structured = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var("left".to_string())),
                right: Box::new(RustExpr::Var("right".to_string())),
            }),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(render_java_display_operand(&structured), "left + right");

        let expr = RustExpr::Raw("(value * 100).to_string()".to_string());
        assert_eq!(render_java_display_operand(&expr), "(value * 100)");

        let structured_raw_arithmetic = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("(value * 100)".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            render_java_display_operand(&structured_raw_arithmetic),
            "value * 100"
        );

        let generated_numeric_cast = RustExpr::Raw("value * (100 as f64).to_string()".to_string());
        assert_eq!(
            render_java_display_operand(&generated_numeric_cast),
            "value * (100 as f64)"
        );

        let unparenthesized_right_call = RustExpr::Raw("left + right.to_string()".to_string());
        assert_eq!(
            render_java_display_operand(&unparenthesized_right_call),
            "left + right.to_string()"
        );

        let string_literal = RustExpr::Raw("(\"left + right\").to_string()".to_string());
        assert_eq!(
            render_java_display_operand(&string_literal),
            "(\"left + right\").to_string()"
        );

        let raw_string_literal = RustExpr::Raw("(r#\"left + right\"#).to_string()".to_string());
        assert_eq!(
            render_java_display_operand(&raw_string_literal),
            "(r#\"left + right\"#).to_string()"
        );
    }

    #[test]
    fn audited_clone_recovery_adds_translation_marker() {
        let expr = apply_audited_clone_recovery(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&expr),
            "value.clone()/* TODO(translation): validate added clone */"
        );
    }

    #[test]
    fn direct_clone_projection_records_recovery_shape() {
        let expr = apply_clone_projection(RustExpr::Var("value".to_string()));
        assert_eq!(emit::emit_expr(&expr), "value.clone()");
        assert_eq!(
            expr.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "clone".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(rust_expr_has_clone_recovery(&expr));
        assert!(rust_expr_has_direct_clone_recovery(&expr));
        assert!(!rust_expr_has_direct_clone_recovery(&RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "to_vec".to_string(),
                args: Vec::new(),
            },
        )));

        let cloned = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "cloned".to_string(),
            args: Vec::new(),
        };
        assert!(rust_expr_has_clone_recovery(&cloned));
        assert!(!rust_expr_has_direct_clone_recovery(&cloned));

        let nullable_recovered = RustExpr::MethodCall {
            receiver: Box::new(cloned),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        };
        assert!(rust_expr_has_clone_recovery(&nullable_recovered));

        let other_expect = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "cloned".to_string(),
                args: Vec::new(),
            }),
            method: "expect".to_string(),
            args: vec![RustExpr::Literal("\"not Java null\"".to_string())],
        };
        assert!(!rust_expr_has_clone_recovery(&other_expect));

        let unrelated = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("value".to_string())),
            method: "iter".to_string(),
            args: Vec::new(),
        };
        assert!(!rust_expr_has_clone_recovery(&unrelated));
    }

    #[test]
    fn clone_recovery_parenthesizes_only_receiver_level_casts() {
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "value as Target".to_string()
            ))),
            "(value as Target).clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "format!(\" as \")".to_string()
            ))),
            "format!(\" as \").clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "\" as \"".to_string()
            ))),
            "\" as \".clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "r#\" as \"#".to_string()
            ))),
            "r#\" as \"#.clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "r#\"quoted \" as text\"#".to_string()
            ))),
            "r#\"quoted \" as text\"#.clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "Table::VALUES[index as usize]".to_string()
            ))),
            "Table::VALUES[index as usize].clone()"
        );
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw("*value".to_string()))),
            "(*value).clone()"
        );
    }

    #[test]
    fn deref_borrowed_projection_keeps_deref_structured() {
        let simple = deref_borrowed_projection(RustExpr::Raw("value".to_string()));
        assert_eq!(
            simple.clone().into_unshaped(),
            RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Raw("value".to_string())),
            }
        );
        assert_eq!(emit::emit_expr(&simple), "*value");

        let compound = deref_borrowed_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(
            compound.clone().into_unshaped(),
            RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Raw("(left + right)".to_string())),
            }
        );
        assert_eq!(emit::emit_expr(&compound), "*(left + right)");
    }

    #[test]
    fn audited_clone_recovery_is_idempotent() {
        let expr = apply_audited_clone_recovery(RustExpr::Raw(
            "value.clone()/* TODO(translation): validate added clone */".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&expr),
            "value.clone()/* TODO(translation): validate added clone */"
        );
    }

    #[test]
    fn clone_recovery_comment_stripping_ignores_literal_comment_text() {
        assert_eq!(
            strip_trailing_block_comments(
                "value.clone()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
            ),
            "value.clone().expect(\"Java null receiver\")"
        );

        let literal = "format!(\"/* not a comment */\").clone()";
        assert_eq!(strip_trailing_block_comments(literal), literal);

        let raw_literal = "format!(r#\"/* not a comment */\"#).clone()";
        assert_eq!(strip_trailing_block_comments(raw_literal), raw_literal);
    }

    #[test]
    fn audited_clone_recovery_for_constructed_optional_clones_element() {
        let expr = apply_audited_clone_recovery(RustExpr::Raw("Some(value)".to_string()));
        assert_eq!(
            emit::emit_expr(&expr),
            "Some(value.clone()/* TODO(translation): validate added clone */)"
        );
    }

    #[test]
    fn optional_borrowed_element_unwrap_clone_recovery_is_idempotent() {
        let expr = apply_audited_clone_recovery(RustExpr::Raw(
            "value.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&expr),
            "value.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        );
    }

    #[test]
    fn optional_clone_recovery_owned_element_projection_records_recovery_shape() {
        let expr = optional_clone_recovery_owned_element_projection(
            RustExpr::Var("value".to_string()),
            false,
        );
        assert_eq!(
            emit::emit_expr(&expr),
            "value.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
    }

    #[test]
    fn optional_unwrap_detection_ignores_literal_text() {
        assert!(rendered_expr_has_optional_unwrap_or_java_null_expect(
            "value.unwrap()"
        ));
        assert!(rendered_expr_has_optional_unwrap_or_java_null_expect(
            "value.expect(\"Java null receiver\")"
        ));

        assert!(!rendered_expr_has_optional_unwrap_or_java_null_expect(
            "\"value.unwrap()\".to_string()"
        ));
        assert!(!rendered_expr_has_optional_unwrap_or_java_null_expect(
            "format!(\"{}\", \".expect(\\\"Java null receiver\\\")\")"
        ));
        assert!(!rendered_expr_has_optional_unwrap_or_java_null_expect(
            "r#\"value.unwrap()\"#"
        ));
    }

    #[test]
    fn optional_clone_recovery_unwrap_rewrite_ignores_literal_text() {
        assert_eq!(
            replace_optional_clone_recovery_unwrap("helper(value.unwrap())", "value", false, false),
            Some(
                "helper(value.as_ref().cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\"))"
                    .to_string()
            )
        );
        assert_eq!(
            replace_optional_clone_recovery_unwrap(
                "helper(value.clone().unwrap())",
                "value",
                true,
                true,
            ),
            Some(
                "helper(value.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\"))"
                    .to_string()
            )
        );

        assert_eq!(
            replace_optional_clone_recovery_unwrap(
                "format!(\"{}\", \"value.unwrap()\")",
                "value",
                false,
                false,
            ),
            None
        );
        assert_eq!(
            replace_optional_clone_recovery_unwrap(
                "r#\"value.clone().unwrap()\"#",
                "value",
                true,
                false,
            ),
            None
        );
    }

    #[test]
    fn downcast_expect_message_rewrite_ignores_literal_text() {
        let real = RustExpr::Raw(format!(
            "value.downcast::<Target>(){}",
            crate::output_markers::invalid_java_cast_expect()
        ));
        assert_eq!(
            emit::emit_expr(&rewrite_downcast_expect_message(real, "replacement")),
            "value.downcast::<Target>().expect(\"replacement\")"
        );

        let marker_text = RustExpr::Raw(format!(
            "format!(\"{} {}\")",
            crate::output_markers::DOWNCAST_TURBOFISH,
            crate::output_markers::invalid_java_cast_expect()
        ));
        assert_eq!(
            emit::emit_expr(&rewrite_downcast_expect_message(
                marker_text.clone(),
                "replacement"
            )),
            emit::emit_expr(&marker_text)
        );

        let raw_marker_text = RustExpr::Raw(format!(
            "r#\"{} {}\"#",
            crate::output_markers::DOWNCAST_TURBOFISH,
            crate::output_markers::invalid_java_cast_expect()
        ));
        assert_eq!(
            emit::emit_expr(&rewrite_downcast_expect_message(
                raw_marker_text.clone(),
                "replacement"
            )),
            emit::emit_expr(&raw_marker_text)
        );
    }

    #[test]
    fn optional_erased_string_local_rewrite_ignores_literal_text() {
        let erased_string_box =
            "name.map(|value| Box::new(value.clone()) as Box<dyn std::any::Any>)";
        let rendered = format!(
            "let text = {erased_string_box:?};\n\
             let raw = r#\"{erased_string_box}\"#;\n\
             let real = {erased_string_box};"
        );

        let rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Raw(rendered),
            &["name".to_string()],
            &[],
        );

        assert_eq!(
            emit::emit_expr(&rewritten),
            format!(
                "let text = {erased_string_box:?};\n\
                 let raw = r#\"{erased_string_box}\"#;\n\
                 let real = name.as_ref().map(|value| value.as_str());"
            )
        );
    }

    #[test]
    fn optional_erased_string_suffix_rewrite_ignores_literal_text() {
        let suffix = format!(
            ".unwrap().downcast::<String>(){}.as_str()",
            crate::output_markers::invalid_java_cast_expect()
        );
        let literal = format!("name{suffix}");
        let rendered = format!(
            "let text = {literal:?};\n\
             let raw = r#\"{literal}\"#;\n\
             let real = name{suffix};"
        );

        let rewritten = rewrite_rendered_optional_erased_string_views(&rendered)
            .expect("the real suffix outside literals should be rewritten");

        assert_eq!(
            rewritten,
            format!(
                "let text = {literal:?};\n\
                 let raw = r#\"{literal}\"#;\n\
                 let real = name.as_ref().and_then(|value| value.downcast_ref::<String>()){}.as_str();",
                crate::output_markers::invalid_java_cast_expect()
            )
        );
    }

    #[test]
    fn erased_object_clone_recovery_records_recovery_shape() {
        let expr = clone_erased_object_ref_once(RustExpr::Raw("&value".to_string()));
        assert_eq!(
            emit::emit_expr(&expr),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));

        let already_recovered = clone_erased_object_ref_once(RustExpr::Raw(
            "java_clone_erased_object_ref(&value)".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&already_recovered),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(
            already_recovered.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );

        let literal = clone_erased_object_ref_once(RustExpr::Raw(
            "\"java_clone_erased_object_ref(\".to_string()".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&literal),
            "java_clone_erased_object_ref(&\"java_clone_erased_object_ref(\".to_string())"
        );
        assert_eq!(literal.recorded_shape(), Some(RustExprShape::OwnedRecovery));

        let structured = clone_erased_object_ref_once(RustExpr::Var("value".to_string()));
        assert_eq!(
            structured.unshaped(),
            &RustExpr::Call {
                function: Box::new(RustExpr::path(["java_clone_erased_object_ref"])),
                args: vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&structured),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(
            structured.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );

        let structured_borrowed = clone_erased_object_ref_once(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::Var("value".to_string())),
        });
        assert_eq!(
            structured_borrowed.unshaped(),
            &RustExpr::Call {
                function: Box::new(RustExpr::path(["java_clone_erased_object_ref"])),
                args: vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                }],
            }
        );
        assert_eq!(
            emit::emit_expr(&structured_borrowed),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(
            structured_borrowed.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );

        let structured_already_recovered = clone_erased_object_ref_once(RustExpr::Call {
            function: Box::new(RustExpr::path(["java_clone_erased_object_ref"])),
            args: vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            }],
        });
        assert_eq!(
            emit::emit_expr(&structured_already_recovered),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(
            structured_already_recovered.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
    }

    #[test]
    fn optional_borrowed_slice_unwrap_does_not_reproject_slice_view() {
        let array_ty = JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Byte)));
        let mut carrier_shape =
            RustSlotShape::conservative(crate::types::optional_java_type(array_ty.clone()));
        carrier_shape.ownership = OwnershipShape::SharedBorrow;
        carrier_shape.view = ViewShape::Slice;

        let mut element_shape = RustSlotShape::conservative(array_ty.clone());
        element_shape.ownership = OwnershipShape::SharedBorrow;
        element_shape.view = ViewShape::Slice;

        let mut target = RustSlotShape::conservative(array_ty);
        target.ownership = OwnershipShape::SharedBorrow;
        target.view = ViewShape::Slice;

        let policy = crate::conversion_plan::borrowed_element_slot_adapter_policy(
            BorrowedElementCarrier::Option,
            &target,
            &carrier_shape,
            &element_shape,
            &FactSet::default(),
        );
        let expr =
            apply_borrowed_element_slot_adapter(RustExpr::Var("source".to_string()), &policy);

        assert_eq!(
            emit::emit_expr(&expr),
            "source.expect(\"Java null receiver\")"
        );
    }

    #[test]
    fn erased_object_downcast_parenthesizes_cast_receiver() {
        let expr = apply_erased_object_downcast(
            RustExpr::Raw("Box::new(value) as Box<dyn std::any::Any>".to_string()),
            "Target",
            &InvalidCastPolicy::Panic,
        );

        assert_eq!(
            emit::emit_expr(&expr),
            "*(Box::new(value) as Box<dyn std::any::Any>).downcast::<Target>().expect(\"invalid Java cast\")"
        );

        let nested = apply_erased_object_downcast(
            RustExpr::Var("value".to_string()),
            "GuideTree.Node",
            &InvalidCastPolicy::Panic,
        );
        assert_eq!(
            emit::emit_expr(&nested),
            "*value.downcast::<GuideTreeNode>().expect(\"invalid Java cast\")"
        );
    }

    #[test]
    fn clone_box_parenthesizes_dyn_trait_cast_receiver() {
        let expr = apply_one(
            RustExpr::Raw("&line as &dyn VCFIDHeaderLine".to_string()),
            &Coercion::CloneBoxDynTrait {
                trait_name: "VCFIDHeaderLine".to_string(),
            },
        );

        assert_eq!(
            emit::emit_expr(&expr),
            "(&line as &dyn VCFIDHeaderLine).clone_box()"
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(rust_expr_has_clone_recovery(&expr));

        let anonymous = apply_one(
            RustExpr::Raw("<dyn Callback>::anonymous()".to_string()),
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(emit::emit_expr(&anonymous), "<dyn Callback>::anonymous()");

        let tagged_anonymous = anonymous_dyn_trait_projection("Callback");
        let recloned_tagged_anonymous = apply_one(
            tagged_anonymous.clone(),
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(recloned_tagged_anonymous, tagged_anonymous);

        let literal_mention = apply_one(
            RustExpr::Raw("<dyn Callback>::named(\" >::anonymous()\")".to_string()),
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(
            emit::emit_expr(&literal_mention),
            "<dyn Callback>::named(\" >::anonymous()\").clone_box()"
        );
        assert_eq!(
            literal_mention.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
    }

    #[test]
    fn clone_erased_object_coercion_records_recovery_shape() {
        let expr = apply_one(
            RustExpr::Var("value".to_string()),
            &Coercion::CloneErasedObject { sequence: false },
        );
        assert_eq!(
            emit::emit_expr(&expr),
            "java_clone_erased_object_ref(&value)"
        );
        assert!(
            matches!(expr.unshaped(), RustExpr::Call { function, .. } if function.as_ref() == &RustExpr::path(["java_clone_erased_object_ref"]))
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(rust_expr_has_clone_recovery(&expr));

        let seq = apply_one(
            RustExpr::Var("values".to_string()),
            &Coercion::CloneErasedObject { sequence: true },
        );
        assert_eq!(
            emit::emit_expr(&seq),
            "java_clone_erased_object_vec_ref(&values)"
        );
        assert!(
            matches!(seq.unshaped(), RustExpr::Call { function, .. } if function.as_ref() == &RustExpr::path(["java_clone_erased_object_vec_ref"]))
        );
        assert_eq!(seq.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(rust_expr_has_clone_recovery(&seq));
    }

    #[test]
    fn to_owned_sequence_projection_keeps_proven_recovery_structured() {
        let proven = to_owned_sequence_projection(
            RustExpr::Var("values".to_string()),
            &CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
            },
        );
        assert_eq!(emit::emit_expr(&proven), "values.to_vec()");
        assert_eq!(
            proven.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "to_vec".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(proven.recorded_shape(), Some(RustExprShape::OwnedRecovery));

        let already = to_owned_sequence_projection(
            proven.clone(),
            &CloneRecoveryPolicy::ProvenCloneCapability {
                java_ty: JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
            },
        );
        assert_eq!(already, proven);

        let audited = to_owned_sequence_projection(
            RustExpr::Var("values".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(
            audited.clone().into_unshaped(),
            RustExpr::Raw(
                "values.to_vec()/* TODO(translation): validate added clone */".to_string()
            )
        );
        assert_eq!(audited.recorded_shape(), Some(RustExprShape::OwnedRecovery));
    }

    #[test]
    fn audited_clone_recovery_preserves_into_suffix() {
        assert_eq!(
            render_audited_clone_recovery("value.into()"),
            "value.clone()/* TODO(translation): validate added clone */.into()"
        );
        let expr =
            apply_audited_clone_recovery(into_projection(RustExpr::Var("value".to_string())));
        assert_eq!(
            emit::emit_expr(&expr),
            "value.clone()/* TODO(translation): validate added clone */.into()"
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
    }
    /// The point of the tag: an adapter that does not spell its result
    /// `.as_str()` is still recognised as a string view, where the textual
    /// answer cannot see it. This is the case that makes new adapters safe to
    /// add without teaching every consumer their spelling.
    #[test]
    fn a_recorded_string_view_is_recognised_without_matching_the_text() {
        let opaque = RustExpr::Raw("java_string_view_helper(value)".to_string());
        assert!(
            !rendered_expr_is_already_string_view(&emit::emit_expr(&opaque)),
            "the text check cannot recognise this spelling, which is the premise"
        );
        assert!(expr_is_string_view(&string_view_expr(opaque)));
    }

    /// The fallback still answers for the untagged majority, so tagging can
    /// proceed one producer at a time instead of all at once.
    #[test]
    fn an_untagged_string_view_still_falls_back_to_the_text() {
        assert!(expr_is_string_view(&RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "as_str".to_string(),
            args: Vec::new(),
        }));
        assert!(expr_is_string_view(&RustExpr::Literal(
            "\"literal\"".to_string()
        )));
        assert!(expr_is_string_view(&RustExpr::Raw(
            "name.as_str()".to_string()
        )));
        assert!(expr_is_string_view(&RustExpr::Raw(
            "\"quoted \\\"literal\\\"\"".to_string()
        )));
        assert!(expr_is_string_view(&RustExpr::Raw(
            "r#\"literal\"#".to_string()
        )));
        assert!(!expr_is_string_view(&RustExpr::Raw(
            "\"left\" + \"right\"".to_string()
        )));
        assert!(!expr_is_string_view(&RustExpr::Raw(
            "\"literal\".to_string()".to_string()
        )));
        assert!(!expr_is_string_view(&RustExpr::Raw(
            "r#\"literal\"#.to_string()".to_string()
        )));
        assert!(!expr_is_string_view(&RustExpr::Raw(
            "r#\"unterminated".to_string()
        )));
        assert!(!expr_is_string_view(&RustExpr::Raw("name".to_string())));
    }

    #[test]
    fn borrowed_map_key_literal_fallback_requires_complete_literal() {
        let complete = ensure_borrow_projection(
            RustExpr::Raw("\"key\"".to_string()),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(emit::emit_expr(&complete), "\"key\"");

        let suffix = ensure_borrow_projection(
            RustExpr::Raw("\"key\".to_string()".to_string()),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(emit::emit_expr(&suffix), "&(\"key\".to_string())");
    }

    /// A tag is a positive answer about *its own* shape only; it must not be
    /// read as a denial of every other shape when it disagrees.
    #[test]
    fn a_slice_view_tag_does_not_answer_the_string_view_question() {
        let tagged = slice_view_expr(RustExpr::Raw("values.as_slice()".to_string()));
        assert!(expr_is_slice_view(&tagged));
        assert!(!expr_is_string_view(&tagged));
        assert!(!expr_is_slice_view(&RustExpr::Raw(
            "values.as_slice()".to_string()
        )));
    }

    #[test]
    fn as_slice_path_compatibility_does_not_trust_literal_text() {
        let literal = apply_one(
            RustExpr::Raw("\"Owner::VALUES\"".to_string()),
            &Coercion::AsSlice,
        );
        assert_eq!(emit::emit_expr(&literal), "\"Owner::VALUES\".as_slice()");
        assert_eq!(literal.recorded_shape(), Some(RustExprShape::SliceView));

        let associated_path = apply_one(
            RustExpr::Raw("Owner::VALUES".to_string()),
            &Coercion::AsSlice,
        );
        assert_eq!(emit::emit_expr(&associated_path), "Owner::VALUES");
        assert_eq!(
            associated_path.recorded_shape(),
            Some(RustExprShape::SliceView)
        );
    }

    /// Mutable slice idempotence is also producer-recorded. A raw spelling that
    /// happens to end in `.as_mut_slice()` is not enough semantic evidence.
    #[test]
    fn mutable_slice_projection_uses_the_recorded_shape_not_the_suffix() {
        let tagged = mutable_slice_view_expr(RustExpr::Raw("custom_mut_slice(values)".to_string()));
        assert_eq!(
            ensure_borrow_projection(
                tagged.clone(),
                None,
                None,
                BorrowProjectionUse::MutableSliceView
            ),
            tagged
        );

        let raw = RustExpr::Raw("values.as_mut_slice()".to_string());
        assert_eq!(
            emit::emit_expr(&ensure_borrow_projection(
                raw,
                None,
                None,
                BorrowProjectionUse::MutableSliceView
            )),
            "&mut values.as_mut_slice()"
        );
    }

    /// `?` must be applied at most once. The old check only looked inside
    /// `RustExpr::Raw`, so a propagated expression carried by any other variant
    /// was propagated again.
    #[test]
    fn result_propagation_is_idempotent() {
        let once = propagate_result_projection(RustExpr::Raw("call()".to_string()));
        assert_eq!(emit::emit_expr(&once), "call()?");
        let twice = propagate_result_projection(once.clone());
        assert_eq!(emit::emit_expr(&twice), "call()?");
        assert_eq!(once, twice);
    }

    /// The recorded answer does not depend on which variant carries it, which
    /// is the difference between asking the expression and matching its text.
    #[test]
    fn a_recorded_propagation_is_recognised_on_any_variant() {
        let propagated = RustExpr::shaped(
            RustExprShape::ResultPropagated,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "take".to_string(),
                args: Vec::new(),
            },
        );
        assert!(expr_is_result_propagated(&propagated));
        assert!(!expr_is_result_propagated(&RustExpr::Var(
            "value".to_string()
        )));
        assert!(!expr_is_result_propagated(&RustExpr::Raw(
            "call()?".to_string()
        )));
    }
}
