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
use crate::java_naming::{RustKeywordEscape, rename_non_rawable_rust_keyword, rust_keyword_escape};
use crate::rust_ir::{
    RustClosureBody, RustClosureParam, RustExpr, RustExprShape, RustMatchArm, RustStmt, RustType,
};
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

#[derive(Debug, Clone, PartialEq)]
pub struct BorrowedStringLiteralTemporaryPlan {
    pub replacement: RustExpr,
    pub initializer: RustExpr,
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
            return optional_element_projection(expr, mapped_value, &element_plan.target);
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
    Some(RustExpr::shaped_as(
        RustExprShape::SliceView,
        Some(plan.target.clone()),
        borrowed_set_slice_temporary_vec_view_projection(expr),
    ))
}

fn borrowed_set_slice_temporary_vec_view_projection(expr: RustExpr) -> RustExpr {
    shared_borrow_expr(typed_collect_projection(
        sequence_iter_cloned_projection(parenthesize_raw_method_receiver(expr)),
        "Vec<_>",
    ))
}

fn optional_element_projection(
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
                zero_arg_method_projection(expr, "as_deref"),
                target_element,
            );
        }
        OptionalMappedValueProjection::MutableSliceView => {
            return optional_projected_structured_expr(
                zero_arg_method_projection(expr, "as_deref_mut"),
                target_element,
            );
        }
        OptionalMappedValueProjection::Other => {}
    }
    match mapped_value.recorded_shape() {
        Some(RustExprShape::Borrowed)
            if target_element.ownership == OwnershipShape::SharedBorrow
                && target_element.view == ViewShape::Native =>
        {
            return optional_projected_structured_expr(
                optional_map_projection_expr(
                    expr,
                    OptionalMapReceiverProjection::AsRef,
                    mapped_value,
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
                    mapped_value,
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
                    mapped_value,
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
                    mapped_value,
                ),
                target_element,
            );
        }
        _ => {}
    }
    optional_projected_structured_expr(
        optional_map_projection_expr(expr, OptionalMapReceiverProjection::Direct, mapped_value),
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
    mapped_value: RustExpr,
) -> RustExpr {
    let receiver = match receiver_projection {
        OptionalMapReceiverProjection::Direct => expr,
        OptionalMapReceiverProjection::AsRef => zero_arg_method_projection(expr, "as_ref"),
        OptionalMapReceiverProjection::AsMut => zero_arg_method_projection(expr, "as_mut"),
    };
    RustExpr::MethodCall {
        receiver: Box::new(receiver),
        method: "map".to_string(),
        args: vec![single_value_closure_projection(mapped_value)],
    }
}

fn single_value_closure_projection(body: RustExpr) -> RustExpr {
    single_param_closure_expr("value", body)
}

fn param_to_string_closure_expr(param_name: &str) -> RustExpr {
    single_param_closure_expr(
        param_name,
        zero_arg_method_projection(RustExpr::Var(param_name.to_string()), "to_string"),
    )
}

fn null_string_closure_expr() -> RustExpr {
    zero_arg_closure_expr(zero_arg_method_projection(
        RustExpr::Literal("\"null\"".to_string()),
        "to_string",
    ))
}

fn single_param_closure_expr(param_name: &str, body: RustExpr) -> RustExpr {
    closure_expr(
        vec![RustClosureParam {
            pattern: param_name.to_string(),
            ty: None,
        }],
        body,
    )
}

fn typed_single_param_closure_expr(param_name: &str, param_type: &str, body: RustExpr) -> RustExpr {
    closure_expr(
        vec![RustClosureParam {
            pattern: param_name.to_string(),
            ty: Some(param_type.to_string()),
        }],
        body,
    )
}

fn zero_arg_closure_expr(body: RustExpr) -> RustExpr {
    closure_expr(Vec::new(), body)
}

fn closure_expr(params: Vec<RustClosureParam>, body: RustExpr) -> RustExpr {
    RustExpr::Closure {
        is_move: false,
        return_type: None,
        params,
        body: RustClosureBody::Expr(Box::new(body)),
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
            crate::slot_render::storage_placement_construction_expr(*placement, expr)
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
    option_some_projection(expr)
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
    if let RustExpr::Raw(rendered) = expr.unshaped() {
        if let Some(recovered) = recover_raw_optional_constructed_string_view_element(rendered) {
            return recovered;
        }
    }
    expr
}

fn recover_raw_optional_constructed_string_view_element(rendered: &str) -> Option<RustExpr> {
    let inner = rendered
        .strip_prefix('&')
        .and_then(|value| value.strip_suffix(".as_str()"))?;
    Some(string_view_expr(zero_arg_method_projection(
        rendered_projection_or_raw_leaf(inner),
        "as_str",
    )))
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
    rendered_complete_some_call_inner(rendered).map(rendered_some_inner_projection)
}

fn rendered_some_inner_projection(inner: &str) -> RustExpr {
    rendered_plus_expr_projection_or_raw_leaf(inner)
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
        RustExpr::Raw(raw) => raw_expr_is_constructed_optional_value(raw),
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

fn raw_expr_is_constructed_optional_value(rendered: &str) -> bool {
    let recovered = rendered_plus_expr_projection_or_raw_leaf(rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
        rust_expr_is_constructed_optional_value(&recovered)
    } else {
        rendered_expr_is_constructed_optional_value(rendered)
    }
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
        RustExpr::Unary { op, expr } if op == "&" || op == "&mut " => {
            RustExpr::Grouped(Box::new(RustExpr::Unary { op, expr }))
        }
        RustExpr::Raw(rendered)
            if rendered.starts_with('&')
                && !(rendered.starts_with("(&") && rendered.ends_with(')')) =>
        {
            grouped_raw_leaf(rendered)
        }
        other => other,
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
    RustExpr::shaped(RustExprShape::Borrowed, shared_borrow_expr(expr))
}

pub fn propagate_result_projection(expr: RustExpr) -> RustExpr {
    if expr_is_result_propagated(&expr) {
        return expr;
    }
    RustExpr::shaped(
        RustExprShape::ResultPropagated,
        RustExpr::Try(Box::new(expr)),
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

pub fn free_function_path_projection(name: &str) -> RustExpr {
    RustExpr::path(name.split("::"))
}

pub fn rust_type_associated_path_projection(owner_ty: RustType, member: &str) -> RustExpr {
    match owner_ty {
        RustType::Vec(inner) => RustExpr::generic_path(["Vec"], vec![*inner], Some(member)),
        RustType::Generic { base, args } => RustExpr::GenericPath {
            base: base.split("::").map(str::to_string).collect(),
            args,
            member: Some(member.to_string()),
        },
        RustType::Named(name) => {
            let mut segments = name.split("::").map(str::to_string).collect::<Vec<_>>();
            segments.push(member.to_string());
            RustExpr::Path(segments)
        }
        other => qualified_rust_type_associated_path_projection(other, member),
    }
}

pub fn qualified_rust_type_associated_path_projection(
    owner_ty: RustType,
    member: &str,
) -> RustExpr {
    RustExpr::TypeAssociatedPath {
        ty: owner_ty,
        member: member.to_string(),
    }
}

pub fn java_class_static_path_projection(member: &str) -> RustExpr {
    java_class_path_projection(RustType::Unit, member)
}

pub fn java_class_literal_path_projection(rust_type: RustType) -> RustExpr {
    java_class_path_projection(rust_type, "literal")
}

pub fn rust_string_literal_projection(value: &str) -> RustExpr {
    RustExpr::Literal(format!("{value:?}"))
}

pub fn java_class_for_name_path_projection(rust_type: &str) -> RustExpr {
    java_class_path_projection(RustType::Raw(rust_type.to_string()), "for_name")
}

fn java_class_path_projection(rust_type: RustType, member: &str) -> RustExpr {
    RustExpr::generic_path(["JavaClass"], vec![rust_type], Some(member))
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

pub fn static_storage_assignment_value_projection(
    setter: RustExpr,
    value: RustExpr,
    getter: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Expr(RustExpr::Call {
            function: Box::new(setter),
            args: vec![value],
        })],
        tail: Some(Box::new(getter)),
    }
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
    method_call_projection(
        zero_arg_method_projection(static_storage_guard_projection(storage_name), "clone"),
        "unwrap_or_else",
        vec![zero_arg_closure_expr(initial)],
    )
}

fn static_storage_guard_projection(storage_name: &str) -> RustExpr {
    method_call_projection(
        zero_arg_method_projection(RustExpr::Var(storage_name.to_string()), "lock"),
        "unwrap_or_else",
        vec![single_param_closure_expr(
            "poisoned",
            zero_arg_method_projection(RustExpr::Var("poisoned".to_string()), "into_inner"),
        )],
    )
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
    initial: RustExpr,
    index: RustExpr,
    value: RustExpr,
) -> RustStmt {
    let initialized = method_call_projection(
        static_storage_guard_projection(storage_name),
        "get_or_insert_with",
        vec![zero_arg_closure_expr(initial)],
    );
    RustStmt::Assign {
        target: RustExpr::Index(Box::new(initialized), Box::new(index)),
        value,
    }
}

pub fn static_storage_setter_body_projection(storage_name: &str) -> RustStmt {
    RustStmt::Assign {
        target: RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(static_storage_guard_projection(storage_name)),
        },
        value: function_call_projection(
            RustExpr::Var("Some".to_string()),
            vec![RustExpr::Var("value".to_string())],
        ),
    }
}

pub fn associated_const_projection(owner: &str, member: &str) -> RustExpr {
    associated_path_projection([owner, member])
}

pub fn unbound_method_reference_projection(method: &str) -> RustExpr {
    unbound_method_reference_closure_leaf(method)
}

fn unbound_method_reference_closure_leaf(method: &str) -> RustExpr {
    single_param_closure_expr(
        "value",
        zero_arg_method_projection(RustExpr::Var("value".to_string()), method),
    )
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
    current_receiver_method_reference_closure_leaf(method, &args)
}

fn current_receiver_method_reference_closure_leaf(method: &str, args: &[String]) -> RustExpr {
    closure_expr(
        args.iter()
            .map(|arg| RustClosureParam {
                pattern: arg.clone(),
                ty: None,
            })
            .collect(),
        current_receiver_method_call_projection(
            method,
            args.iter()
                .map(|arg| RustExpr::Var(arg.clone()))
                .collect::<Vec<_>>(),
        ),
    )
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

#[cfg(test)]
pub fn render_function_call_projection(function: RustExpr, args: Vec<String>) -> String {
    emit::emit_expr(&function_call_projection(
        function,
        rendered_args_to_projection_leaves(args),
    ))
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

#[cfg(test)]
pub fn render_receiver_method_call_projection(
    receiver: String,
    method: &str,
    args: Vec<String>,
) -> String {
    emit::emit_expr(&receiver_method_call_projection(
        rendered_projection_or_raw_leaf(&receiver),
        method,
        rendered_args_to_projection_leaves(args),
    ))
}

#[cfg(test)]
pub fn render_field_projection(receiver: String, field_name: &str) -> String {
    emit::emit_expr(&field_projection(
        rendered_projection_or_raw_leaf(&receiver),
        field_name,
    ))
}

pub fn current_receiver_method_call_projection(method: &str, args: Vec<RustExpr>) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var("self".to_string())),
        method: method.to_string(),
        args,
    }
}

#[cfg(test)]
pub fn render_current_receiver_method_call_projection(method: &str, args: Vec<String>) -> String {
    emit::emit_expr(&current_receiver_method_call_projection(
        method,
        rendered_args_to_projection_leaves(args),
    ))
}

#[cfg(test)]
fn rendered_args_to_projection_leaves(args: Vec<String>) -> Vec<RustExpr> {
    args.into_iter()
        .map(|arg| rendered_projection_or_raw_leaf(&arg))
        .collect()
}

#[cfg(test)]
pub fn typed_single_input_callback_projection(param_type: &str, body: &str) -> RustExpr {
    typed_single_input_callback_expr(param_type, rendered_projection_or_raw_leaf(body))
}

pub fn typed_single_input_callback_expr(param_type: &str, body: RustExpr) -> RustExpr {
    typed_single_param_closure_expr("value", param_type, body)
}

pub fn outer_this_field_projection(field_name: &str) -> RustExpr {
    field_projection(
        outer_this_borrow_projection("borrow"),
        &crate::parser::to_snake_case(field_name),
    )
}

pub fn outer_this_field_mut_projection(field_name: &str) -> RustExpr {
    field_projection(
        outer_this_borrow_projection("borrow_mut"),
        &crate::parser::to_snake_case(field_name),
    )
}

fn outer_this_borrow_projection(method: &str) -> RustExpr {
    zero_arg_method_projection(
        field_projection(RustExpr::Var("self".to_string()), "__outer"),
        method,
    )
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
    typed_collect_projection(zero_arg_method_projection(chars, "iter"), "String")
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
        RustExpr::Cast {
            expr: Box::new(borrow_expr_leaf_for_dyn_cast(expr)),
            ty: format!("&dyn {trait_name}"),
        },
    )
}

fn borrow_boxed_as_dyn_trait(expr: RustExpr, trait_name: &str) -> RustExpr {
    let trait_name = normalize_emitted_java_type_name(trait_name);
    RustExpr::shaped(
        RustExprShape::Borrowed,
        RustExpr::Cast {
            expr: Box::new(zero_arg_method_projection(
                parenthesize_raw_method_receiver(expr),
                "as_ref",
            )),
            ty: format!("&dyn {trait_name}"),
        },
    )
}

fn borrow_expr_leaf_for_dyn_cast(expr: RustExpr) -> RustExpr {
    borrowed_raw_safe_expr(expr)
}

fn borrowed_raw_safe_expr(expr: RustExpr) -> RustExpr {
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(borrow_operand_expr(expr)),
    }
}

pub fn anonymous_dyn_trait_projection(trait_name: &str) -> RustExpr {
    let trait_name = normalize_emitted_java_type_name(trait_name);
    RustExpr::shaped(
        RustExprShape::AnonymousDynTrait,
        RustExpr::Call {
            function: Box::new(RustExpr::Path(vec![
                format!("<dyn {trait_name}>"),
                "anonymous".to_string(),
            ])),
            args: Vec::new(),
        },
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
        RustExpr::generic_path(
            ["None"],
            vec![RustType::Raw(rust_ty.to_string())],
            None::<String>,
        ),
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
        vec![single_param_closure_expr(
            "value",
            zero_arg_method_projection(
                RustExpr::Grouped(Box::new(RustExpr::Unary {
                    op: "*".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                })),
                "clone",
            ),
        )],
    )
}

pub fn rust_expr_mentions_constructor_self_temporary(expr: &RustExpr) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => rust_expr_mentions_constructor_self_temporary(expr),
        RustExpr::MarkedBlock { expr, .. } => rust_expr_mentions_constructor_self_temporary(expr),
        RustExpr::PostfixComment { expr, .. } => {
            rust_expr_mentions_constructor_self_temporary(expr)
        }
        RustExpr::Grouped(expr) => rust_expr_mentions_constructor_self_temporary(expr),
        RustExpr::Var(name) => name == "__java_self",
        RustExpr::Path(_) | RustExpr::GenericPath { .. } | RustExpr::TypeAssociatedPath { .. } => {
            false
        }
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
        RustExpr::Range { start, end, .. } => {
            rust_expr_mentions_constructor_self_temporary(start)
                || rust_expr_mentions_constructor_self_temporary(end)
        }
        RustExpr::Unary { expr, .. } | RustExpr::Cast { expr, .. } | RustExpr::Try(expr) => {
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
        RustExpr::UnsupportedJavaArrayCreation { dimensions, .. } => dimensions
            .iter()
            .any(rust_expr_mentions_constructor_self_temporary),
        RustExpr::Call { function, args } => {
            rust_expr_mentions_constructor_self_temporary(function)
                || args
                    .iter()
                    .any(rust_expr_mentions_constructor_self_temporary)
        }
        RustExpr::MacroCall { args, .. } => args
            .iter()
            .any(rust_expr_mentions_constructor_self_temporary),
        RustExpr::Closure { body, .. } => match body {
            RustClosureBody::Expr(expr) => rust_expr_mentions_constructor_self_temporary(expr),
        },
        RustExpr::Match { expr, arms } => {
            rust_expr_mentions_constructor_self_temporary(expr)
                || arms
                    .iter()
                    .any(|arm| rust_expr_mentions_constructor_self_temporary(&arm.body))
        }
        RustExpr::Block { stmts, tail } => {
            stmts
                .iter()
                .any(rust_stmt_mentions_constructor_self_temporary)
                || tail
                    .as_ref()
                    .is_some_and(|tail| rust_expr_mentions_constructor_self_temporary(tail))
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
        RustExpr::Literal(_) | RustExpr::Pattern(_) => false,
    }
}

fn rust_stmt_mentions_constructor_self_temporary(stmt: &RustStmt) -> bool {
    match stmt {
        RustStmt::Let { value, .. } => value
            .as_ref()
            .is_some_and(rust_expr_mentions_constructor_self_temporary),
        RustStmt::Assign { target, value } | RustStmt::AssignOp { target, value, .. } => {
            rust_expr_mentions_constructor_self_temporary(target)
                || rust_expr_mentions_constructor_self_temporary(value)
        }
        RustStmt::Expr(expr) | RustStmt::Return(Some(expr)) => {
            rust_expr_mentions_constructor_self_temporary(expr)
        }
        RustStmt::If {
            condition,
            then_body,
            else_body,
        } => {
            rust_expr_mentions_constructor_self_temporary(condition)
                || then_body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
                || else_body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
        }
        RustStmt::IfLet {
            expr,
            then_body,
            else_body,
            ..
        } => {
            rust_expr_mentions_constructor_self_temporary(expr)
                || then_body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
                || else_body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
        }
        RustStmt::While { condition, body } => {
            rust_expr_mentions_constructor_self_temporary(condition)
                || body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
        }
        RustStmt::WhileLet { expr, body, .. } => {
            rust_expr_mentions_constructor_self_temporary(expr)
                || body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
        }
        RustStmt::For { iterable, body, .. } => {
            rust_expr_mentions_constructor_self_temporary(iterable)
                || body
                    .iter()
                    .any(rust_stmt_mentions_constructor_self_temporary)
        }
        RustStmt::Loop { body, .. } => body
            .iter()
            .any(rust_stmt_mentions_constructor_self_temporary),
        RustStmt::BreakValue { value, .. } => rust_expr_mentions_constructor_self_temporary(value),
        RustStmt::Raw(rendered) => raw_mentions_identifier(rendered, "__java_self"),
        RustStmt::Return(None) | RustStmt::Break(_) | RustStmt::Continue(_) => false,
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
    field_projection(
        RustExpr::Var("__java_self".to_string()),
        &rust_field_projection_ident(field_name),
    )
}

fn rust_field_projection_ident(name: &str) -> String {
    if name.starts_with("r#") || name.contains("::") || name.is_empty() {
        return name.to_string();
    }
    match rust_keyword_escape(name) {
        Some(RustKeywordEscape::Raw) => format!("r#{name}"),
        Some(RustKeywordEscape::Rename) => rename_non_rawable_rust_keyword(name),
        None => name.to_string(),
    }
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
            string_view_expr(rust_string_literal_projection(value))
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
        other => other,
    }
}

pub fn normalize_map_key_borrow_projection(
    expr: RustExpr,
    local_name: Option<&str>,
    actual: Option<&RustSlotShape>,
) -> RustExpr {
    let expr = match expr {
        RustExpr::Raw(rendered) => rendered_projection_or_raw_leaf(&rendered),
        other => other,
    };
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
        _ => false,
    }
}

#[cfg(test)]
pub fn render_index_to_usize_projection(
    rendered: &str,
    projection: IndexToUsizeProjection,
) -> String {
    emit::emit_expr(&index_to_usize_projection(
        rendered_projection_or_raw_leaf(rendered),
        projection,
    ))
}

pub fn index_to_usize_projection(index: RustExpr, projection: IndexToUsizeProjection) -> RustExpr {
    let index = recover_raw_index_to_usize_operand(index);
    let index = decimal_integer_literal_projection(index);
    if let Some(cast) = usize_cast_projection(index.clone()) {
        return cast;
    }
    let expr = match projection {
        IndexToUsizeProjection::JavaStringCharAt
            if rust_expr_is_decimal_integer_literal(&index) =>
        {
            index
        }
        IndexToUsizeProjection::StreamLambdaArgument
            if rust_expr_is_simple_usize_cast_operand(&index) =>
        {
            index
        }
        _ => RustExpr::Grouped(Box::new(index)),
    };
    RustExpr::Cast {
        expr: Box::new(expr),
        ty: "usize".to_string(),
    }
}

fn recover_raw_index_to_usize_operand(index: RustExpr) -> RustExpr {
    match index {
        RustExpr::Raw(rendered) => rendered_plus_expr_projection_or_raw_leaf(&rendered),
        other => other,
    }
}

fn decimal_integer_literal_projection(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) if rendered_expr_is_decimal_integer_literal(&rendered) => {
            RustExpr::Literal(rendered)
        }
        other => other,
    }
}

fn rust_expr_is_decimal_integer_literal(expr: &RustExpr) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::Literal(rendered) if rendered_expr_is_decimal_integer_literal(rendered)
    )
}

fn rendered_expr_is_decimal_integer_literal(rendered: &str) -> bool {
    let Some(digits) = rendered
        .strip_suffix("usize")
        .or_else(|| rendered.strip_suffix("isize"))
        .or_else(|| rendered.strip_suffix("u128"))
        .or_else(|| rendered.strip_suffix("i128"))
        .or_else(|| rendered.strip_suffix("u64"))
        .or_else(|| rendered.strip_suffix("i64"))
        .or_else(|| rendered.strip_suffix("u32"))
        .or_else(|| rendered.strip_suffix("i32"))
        .or_else(|| rendered.strip_suffix("u16"))
        .or_else(|| rendered.strip_suffix("i16"))
        .or_else(|| rendered.strip_suffix("u8"))
        .or_else(|| rendered.strip_suffix("i8"))
        .or(Some(rendered))
    else {
        return false;
    };
    digits.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        && digits.chars().all(|ch| ch == '_' || ch.is_ascii_digit())
}

fn rust_expr_is_simple_usize_cast_operand(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Raw(rendered) => {
            if let Some(recovered) = recover_rendered_usize_cast_operand_projection(rendered) {
                rust_expr_is_simple_usize_cast_operand(&recovered)
            } else {
                !rendered.starts_with('*') && !rendered.contains(' ')
            }
        }
        RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::Field(_, _)
        | RustExpr::Index(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. } => true,
        _ => false,
    }
}

fn rust_expr_is_usize_cast(expr: &RustExpr) -> bool {
    usize_cast_projection(expr.clone()).is_some()
}

fn usize_cast_projection(expr: RustExpr) -> Option<RustExpr> {
    if matches!(expr.unshaped(), RustExpr::Cast { ty, .. } if ty == "usize") {
        return Some(expr);
    }
    if let RustExpr::Raw(raw) = expr.unshaped() {
        if let Some(recovered) = recover_rendered_usize_cast_projection(raw) {
            return Some(recovered);
        }
    }
    None
}

fn recover_rendered_usize_cast_projection(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    (!matches!(recovered.unshaped(), RustExpr::Raw(_)) && rust_expr_is_usize_cast(&recovered))
        .then_some(recovered)
}

fn recover_rendered_usize_cast_operand_projection(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    (!matches!(recovered.unshaped(), RustExpr::Raw(_))).then_some(recovered)
}

#[cfg(test)]
pub fn render_indexed_access_projection(
    array: &str,
    index: &str,
    projection: IndexToUsizeProjection,
) -> String {
    emit::emit_expr(&indexed_access_projection(
        rendered_projection_or_raw_leaf(array),
        rendered_projection_or_raw_leaf(index),
        projection,
    ))
}

pub fn indexed_access_projection(
    array: RustExpr,
    index: RustExpr,
    projection: IndexToUsizeProjection,
) -> RustExpr {
    RustExpr::Index(
        Box::new(array),
        Box::new(index_to_usize_projection(index, projection)),
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
    let RustExpr::Raw(rendered) = arg.unshaped() else {
        return false;
    };
    rendered_mutable_alias_projection(rendered, base).is_some()
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
    if expr_is_optional_mut_reborrow(expr) {
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

fn expr_is_optional_mut_reborrow(expr: &RustExpr) -> bool {
    rust_expr_is_zero_arg_method_call(expr, "as_deref_mut")
        || matches!(
            expr.unshaped(),
            RustExpr::Raw(rendered) if rendered.trim().ends_with(".as_deref_mut()")
        )
}

pub fn clone_erased_object_ref_once(expr: RustExpr) -> RustExpr {
    clone_erased_object_helper_once(expr, "java_clone_erased_object_ref")
}

fn clone_erased_object_helper_once(expr: RustExpr, helper: &str) -> RustExpr {
    if rust_expr_is_erased_object_clone_helper_call(&expr) {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
    }
    let expr = if let RustExpr::Raw(rendered) = expr.unshaped() {
        if raw_mentions_call(&rendered, "java_clone_erased_object_ref")
            || raw_mentions_call(&rendered, "java_clone_erased_object_vec_ref")
        {
            return RustExpr::shaped(RustExprShape::OwnedRecovery, expr);
        }
        let borrow_operand = rendered.strip_prefix('&').unwrap_or(&rendered);
        parenthesized_borrow_operand_leaf(borrow_operand)
    } else {
        expr
    };
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::Call {
            function: Box::new(RustExpr::path([helper])),
            args: vec![borrowed_erased_object_clone_arg(expr)],
        },
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
    let as_ref = zero_arg_method_projection(option_expr, "as_ref");
    let recovered = if erased_object {
        method_call_projection(
            as_ref,
            "map",
            vec![RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: vec![RustClosureParam {
                    pattern: "value".to_string(),
                    ty: None,
                }],
                body: RustClosureBody::Expr(Box::new(function_call_projection(
                    RustExpr::path(["java_clone_erased_object_ref"]),
                    vec![RustExpr::Var("value".to_string())],
                ))),
            }],
        )
    } else {
        zero_arg_method_projection(as_ref, "cloned")
    };
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        method_call_projection(
            clone_recovery_postfix_marker_expr(recovered, &CloneRecoveryPolicy::AuditRequired),
            "expect",
            vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
        ),
    )
}

#[cfg(test)]
pub fn render_parse_receiver(expr: RustExpr) -> String {
    emit::emit_expr(&parse_receiver_projection(expr))
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
    primitive_parse_chain_projection(parse_receiver_projection(arg), rust_ty)
}

fn parse_receiver_projection(arg: RustExpr) -> RustExpr {
    match arg {
        RustExpr::Unary { op, expr } if op == "&" => *expr,
        RustExpr::Raw(raw) => {
            let stripped = raw
                .trim()
                .strip_prefix('&')
                .map(str::trim)
                .unwrap_or(raw.trim());
            recover_rendered_parse_receiver_projection(stripped)
        }
        _ => arg,
    }
}

fn recover_rendered_parse_receiver_projection(rendered: &str) -> RustExpr {
    primitive_parse_raw_receiver_projection(rendered)
        .unwrap_or_else(|| rendered_plus_expr_projection_or_raw_leaf(rendered))
}

fn primitive_parse_raw_receiver_projection(raw: &str) -> Option<RustExpr> {
    dotted_zero_arg_or_field_projection(raw.trim())
}

fn dotted_zero_arg_or_field_projection(raw: &str) -> Option<RustExpr> {
    let mut parts = raw.split('.');
    let first = parts.next()?;
    let mut expr = plain_identifier_expr(first)?;
    for part in parts {
        if let Some(method) = part.strip_suffix("()") {
            if !rust_plain_identifier(method) {
                return None;
            }
            expr = zero_arg_method_projection(expr, method);
        } else {
            if !rust_plain_identifier(part) {
                return None;
            }
            expr = field_projection(expr, part);
        }
    }
    Some(expr)
}

fn plain_identifier_expr(raw: &str) -> Option<RustExpr> {
    rust_plain_identifier(raw).then(|| RustExpr::Var(raw.to_string()))
}

fn rust_plain_identifier(raw: &str) -> bool {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn primitive_parse_chain_projection(receiver: RustExpr, rust_ty: &str) -> RustExpr {
    let receiver = parenthesize_raw_method_receiver(receiver);
    zero_arg_method_projection(
        typed_zero_arg_method_projection(receiver, "parse", rust_ty),
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

#[cfg(test)]
pub fn render_string_join_vararg_value(value: RustExpr) -> String {
    emit::emit_expr(&string_join_vararg_value_projection(value))
}

fn string_join_vararg_value_projection(value: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_source(&value) {
        return zero_arg_method_projection(rust_string_literal_projection(&literal), "to_string");
    }
    if let Some(to_string_call) = to_string_call_projection(value.clone()) {
        to_string_call
    } else {
        zero_arg_method_projection(RustExpr::Grouped(Box::new(value)), "to_string")
    }
}

fn to_string_call_projection(value: RustExpr) -> Option<RustExpr> {
    if rust_expr_is_zero_arg_method_call(&value, "to_string") {
        return Some(value);
    }
    if let RustExpr::Raw(raw) = value.unshaped() {
        if let Some(to_string_call) = recover_rendered_to_string_call_projection(raw) {
            return Some(to_string_call);
        }
    }
    None
}

fn recover_rendered_to_string_call_projection(rendered: &str) -> Option<RustExpr> {
    terminal_raw_to_string_call_projection(rendered)
}

fn terminal_raw_to_string_call_projection(raw: &str) -> Option<RustExpr> {
    let receiver = raw.trim().strip_suffix(".to_string()")?.trim_end();
    let receiver = terminal_to_string_receiver_projection(receiver)?;
    Some(zero_arg_method_projection(receiver, "to_string"))
}

fn terminal_to_string_receiver_projection(receiver: &str) -> Option<RustExpr> {
    if receiver.is_empty()
        || rendered_expr_is_complete_string_literal(receiver)
        || rendered_parenthesized_expr_is_complete_string_literal(receiver)
        || (raw_contains_top_level_binary_operator(receiver)
            && !balanced_outer_parentheses_wrap_all(receiver))
    {
        return None;
    }
    let projected = if balanced_outer_parentheses_wrap_all(receiver) {
        terminal_grouped_to_string_receiver_projection(receiver)?
    } else {
        rendered_function_call_projection(receiver)
            .or_else(|| dotted_zero_arg_or_field_projection(receiver))
            .or_else(|| rendered_index_leaf_projection(receiver))
            .or_else(|| rendered_path_leaf_projection(receiver))?
    };
    terminal_to_string_receiver_is_structured(&projected).then_some(projected)
}

fn terminal_grouped_to_string_receiver_projection(receiver: &str) -> Option<RustExpr> {
    let RustExpr::Grouped(inner) = grouped_rendered_inner_projection(receiver)? else {
        return None;
    };
    terminal_grouped_to_string_receiver_is_structured(&inner).then(|| RustExpr::Grouped(inner))
}

fn terminal_grouped_to_string_receiver_is_structured(receiver: &RustExpr) -> bool {
    matches!(
        receiver.unshaped(),
        RustExpr::Var(_)
            | RustExpr::Path(_)
            | RustExpr::GenericPath { .. }
            | RustExpr::TypeAssociatedPath { .. }
            | RustExpr::Field(_, _)
            | RustExpr::Index(_, _)
            | RustExpr::Call { .. }
            | RustExpr::MethodCall { .. }
            | RustExpr::Binary { .. }
    )
}

fn rendered_parenthesized_expr_is_complete_string_literal(rendered: &str) -> bool {
    balanced_outer_parentheses_wrap_all(rendered)
        && rendered_expr_is_complete_string_literal(&rendered[1..rendered.len() - 1])
}

fn terminal_to_string_receiver_is_structured(receiver: &RustExpr) -> bool {
    match receiver.unshaped() {
        RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::Field(_, _)
        | RustExpr::Index(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. }
        | RustExpr::Grouped(_) => true,
        RustExpr::Literal(_)
        | RustExpr::Raw(_)
        | RustExpr::MacroCall { .. }
        | RustExpr::StructLit { .. }
        | RustExpr::Tuple(_)
        | RustExpr::ArrayLiteral(_)
        | RustExpr::VecLiteral(_)
        | RustExpr::VecRepeat { .. }
        | RustExpr::Unary { .. }
        | RustExpr::Binary { .. }
        | RustExpr::Range { .. }
        | RustExpr::Cast { .. }
        | RustExpr::Try(_)
        | RustExpr::If { .. }
        | RustExpr::Closure { .. }
        | RustExpr::Match { .. }
        | RustExpr::Block { .. }
        | RustExpr::UnsupportedJavaArrayCreation { .. }
        | RustExpr::Shaped { .. }
        | RustExpr::PostfixComment { .. }
        | RustExpr::MarkedBlock { .. }
        | RustExpr::Pattern(_) => false,
    }
}

pub fn string_join_delimiter_projection(delimiter: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_source(&delimiter) {
        rust_string_literal_projection(&literal)
    } else {
        delimiter
    }
}

pub fn string_join_collection_projection(delimiter: RustExpr, values: RustExpr) -> RustExpr {
    method_call_projection(
        values,
        "join",
        vec![string_join_delimiter_projection(delimiter)],
    )
}

pub fn string_join_varargs_projection(delimiter: RustExpr, values: Vec<RustExpr>) -> RustExpr {
    let values = RustExpr::VecLiteral(
        values
            .into_iter()
            .map(string_join_vararg_value_projection)
            .collect(),
    );
    string_join_collection_projection(delimiter, values)
}

#[cfg(test)]
pub fn render_string_equality_operand_projection(
    expr: RustExpr,
    already_string_view: bool,
) -> String {
    emit::emit_expr(&string_equality_operand_projection(
        expr,
        already_string_view,
    ))
}

pub fn string_equality_operand_projection(expr: RustExpr, already_string_view: bool) -> RustExpr {
    if already_string_view {
        expr
    } else {
        zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_str")
    }
}

pub fn optional_string_view_projection(expr: RustExpr) -> RustExpr {
    RustExpr::shaped(
        RustExprShape::OptionalProjected,
        optional_string_view_projection_expr(expr),
    )
}

fn optional_string_view_projection_expr(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(zero_arg_method_projection(
            parenthesize_raw_method_receiver(expr),
            "as_ref",
        )),
        method: "map".to_string(),
        args: vec![single_param_closure_expr(
            "value",
            zero_arg_method_projection(RustExpr::Var("value".to_string()), "as_str"),
        )],
    }
}

pub fn string_helper_receiver_projection(receiver: RustExpr) -> RustExpr {
    let receiver = recover_string_helper_receiver(receiver);
    if expr_is_string_view(&receiver) {
        return receiver;
    }
    if expr_is_as_ref_unwrap_projection(&receiver) {
        return string_view_expr(zero_arg_method_projection(receiver, "as_str"));
    }
    string_view_expr(force_shared_borrow_projection(receiver))
}

fn recover_string_helper_receiver(receiver: RustExpr) -> RustExpr {
    match receiver {
        RustExpr::Raw(raw) => rendered_plus_expr_projection_or_raw_leaf(&raw),
        other => other,
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
    function_call_projection(
        RustExpr::path(["java_string_hash_code"]),
        vec![string_helper_receiver_projection(receiver)],
    )
}

pub fn java_list_hash_code_projection(receiver: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_list_hash_code"]),
        vec![force_shared_borrow_projection(receiver)],
    )
}

pub fn java_set_hash_code_projection(receiver: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_set_hash_code"]),
        vec![force_shared_borrow_projection(receiver)],
    )
}

pub fn java_map_hash_code_projection(receiver: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_map_hash_code"]),
        vec![force_shared_borrow_projection(receiver)],
    )
}

pub fn java_value_hash_code_projection(receiver: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_value_hash_code"]),
        vec![force_shared_borrow_projection(receiver)],
    )
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
    function_call_projection(
        RustExpr::path([function]),
        vec![
            string_helper_receiver_projection(receiver),
            regex,
            replacement,
        ],
    )
}

pub fn string_matches_projection(receiver: RustExpr, pattern: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_string_matches"]),
        vec![string_helper_receiver_projection(receiver), pattern],
    )
}

pub fn string_split_projection(receiver: RustExpr, regex: RustExpr, limit: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_string_split"]),
        vec![string_helper_receiver_projection(receiver), regex, limit],
    )
}

pub fn string_from_bytes_projection(bytes: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_string_from_bytes"]),
        vec![force_shared_borrow_projection(bytes)],
    )
}

pub fn string_from_byte_range_projection(
    bytes: RustExpr,
    offset: RustExpr,
    len: RustExpr,
) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_string_from_byte_range"]),
        vec![force_shared_borrow_projection(bytes), offset, len],
    )
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
    function_call_projection(
        RustExpr::path([function]),
        vec![string_helper_receiver_projection(receiver), other],
    )
}

pub fn string_get_bytes_projection(receiver: RustExpr) -> RustExpr {
    typed_collect_projection(
        RustExpr::MethodCall {
            receiver: Box::new(zero_arg_method_projection(
                zero_arg_method_projection(receiver, "as_bytes"),
                "iter",
            )),
            method: "map".to_string(),
            args: vec![single_param_closure_expr(
                "byte",
                RustExpr::Cast {
                    expr: Box::new(RustExpr::Unary {
                        op: "*".to_string(),
                        expr: Box::new(RustExpr::Var("byte".to_string())),
                    }),
                    ty: "i8".to_string(),
                },
            )],
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
                args: vec![sequence_index_position_predicate_leaf(needle)],
            }),
            method: "map".to_string(),
            args: vec![sequence_index_i32_map_leaf()],
        }),
        method: "unwrap_or".to_string(),
        args: vec![RustExpr::Literal("-1".to_string())],
    }
}

fn sequence_index_position_predicate_leaf(needle: RustExpr) -> RustExpr {
    single_param_closure_expr(
        "item",
        RustExpr::Binary {
            op: "==".to_string(),
            left: Box::new(RustExpr::Var("item".to_string())),
            right: Box::new(needle),
        },
    )
}

fn sequence_index_i32_map_leaf() -> RustExpr {
    single_param_closure_expr(
        "index",
        RustExpr::Cast {
            expr: Box::new(RustExpr::Var("index".to_string())),
            ty: "i32".to_string(),
        },
    )
}

pub fn string_char_at_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    let index = string_char_at_index_to_usize_projection(index);
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

fn string_char_at_index_to_usize_projection(index: RustExpr) -> RustExpr {
    index_to_usize_projection(index, IndexToUsizeProjection::JavaStringCharAt)
}

fn parenthesize_raw_method_receiver(expr: RustExpr) -> RustExpr {
    raw_leaf_or_grouped(expr, |trimmed| {
        !(balanced_outer_parentheses_wrap_all(trimmed)
            || (!raw_contains_top_level_binary_operator(trimmed)
                && !raw_contains_top_level_as_cast(trimmed)
                && !trimmed.starts_with('*')))
    })
}

fn grouped_raw_leaf(raw: String) -> RustExpr {
    RustExpr::Grouped(Box::new(recover_parseable_rendered_operand_projection(
        &raw,
    )))
}

fn grouped_stable_rendered_raw_leaf(raw: String) -> RustExpr {
    let trimmed = raw.trim();
    let recovered = rendered_plus_expr_projection_or_raw_leaf(trimmed);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) && emit::emit_expr(&recovered) == trimmed {
        RustExpr::Grouped(Box::new(recovered))
    } else {
        RustExpr::Grouped(Box::new(RustExpr::raw_leaf(trimmed)))
    }
}

fn raw_leaf_or_grouped(expr: RustExpr, should_group: impl FnOnce(&str) -> bool) -> RustExpr {
    let RustExpr::Raw(raw) = &expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if should_group(trimmed) {
        grouped_raw_leaf(trimmed.to_string())
    } else {
        expr
    }
}

pub(crate) fn raw_receiver_projection(
    expr: RustExpr,
    raw_needs_parentheses: impl FnOnce(&str) -> bool,
) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if raw_needs_parentheses(trimmed) {
        RustExpr::Grouped(Box::new(recover_parseable_rendered_operand_projection(
            trimmed,
        )))
    } else {
        recover_parseable_rendered_operand_projection(&raw)
    }
}

pub fn recover_parseable_raw_operand_projection(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => recover_parseable_rendered_operand_projection(&rendered),
        other => other,
    }
}

fn recover_parseable_rendered_operand_projection(rendered: &str) -> RustExpr {
    rendered_plus_expr_projection_or_raw_leaf(rendered)
}

pub fn string_index_of_projection(
    receiver: RustExpr,
    needle: RustExpr,
    from_index: Option<RustExpr>,
) -> RustExpr {
    let receiver = string_view_argument_projection(receiver);
    let needle = string_view_argument_projection(string_search_needle_projection(needle));
    if let Some(from_index) = from_index {
        return function_call_projection(
            RustExpr::path(["java_string_index_of_from"]),
            vec![receiver, needle, from_index],
        );
    }
    function_call_projection(
        RustExpr::path(["java_string_index_of"]),
        vec![receiver, needle],
    )
}

pub fn string_last_index_of_projection(
    receiver: RustExpr,
    needle: RustExpr,
    from_index: Option<RustExpr>,
) -> RustExpr {
    let receiver = string_view_argument_projection(receiver);
    let needle = string_view_argument_projection(string_search_needle_projection(needle));
    if let Some(from_index) = from_index {
        return function_call_projection(
            RustExpr::path(["java_string_last_index_of_from"]),
            vec![receiver, needle, from_index],
        );
    }
    function_call_projection(
        RustExpr::path(["java_string_last_index_of"]),
        vec![receiver, needle],
    )
}

fn string_search_needle_projection(needle: RustExpr) -> RustExpr {
    if let Some(literal) = string_literal_from_rust_char_literal_expr(&needle) {
        RustExpr::Literal(literal)
    } else {
        needle
    }
}

fn string_literal_from_rust_char_literal_expr(expr: &RustExpr) -> Option<String> {
    match expr.unshaped() {
        RustExpr::Literal(rendered) => string_literal_from_rust_char_literal(rendered),
        RustExpr::Unary { op, expr } if op == "&" => {
            string_literal_from_rust_char_literal_expr(expr)
        }
        RustExpr::Raw(rendered) => string_literal_from_rust_char_literal(rendered),
        _ => None,
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
    let operand = parenthesize_raw_primitive_number_cast_operand(expr);
    let cast_operand = RustExpr::Cast {
        expr: Box::new(operand.clone()),
        ty: "u32".to_string(),
    };
    let left = if matches!(operand, RustExpr::Grouped(_)) {
        RustExpr::Grouped(Box::new(cast_operand))
    } else {
        cast_operand
    };
    RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Call {
            function: Box::new(RustExpr::path(["char", "from_u32"])),
            args: vec![RustExpr::Binary {
                op: "&".to_string(),
                left: Box::new(left),
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
            expr: Box::new(parenthesize_raw_java_char_numeric_cast_operand(expr)),
            ty: "u32".to_string(),
        }),
        ty: rust_primitive_type_name_policy(target)
            .rust_name
            .to_string(),
    }
}

fn parenthesize_raw_java_char_numeric_cast_operand(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(raw) => grouped_raw_leaf(raw),
        other => other,
    }
}

fn parenthesize_raw_primitive_number_cast_operand(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    let recovered = recover_parseable_rendered_operand_projection(trimmed);
    if !(balanced_outer_parentheses_wrap_all(trimmed)
        || (!raw_contains_top_level_binary_operator(trimmed)
            && !raw_contains_top_level_as_cast(trimmed)))
    {
        RustExpr::Grouped(Box::new(recovered))
    } else {
        recovered
    }
}

pub fn facade_receiver_method_call_projection(
    receiver: RustExpr,
    rust_path: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    receiver_method_call_projection_exact(receiver, &facade_method_name(rust_path), args)
}

pub fn external_facade_rust_path_projection(rust_path: &str) -> RustExpr {
    split_external_facade_rust_path(rust_path)
        .map(RustExpr::Path)
        .unwrap_or_else(|| rendered_projection_or_raw_leaf(rust_path))
}

fn split_external_facade_rust_path(rust_path: &str) -> Option<Vec<String>> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0usize;
    let chars = rust_path.char_indices().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        let (byte_index, ch) = chars[index];
        match ch {
            '<' => angle_depth = angle_depth.saturating_add(1),
            '>' => angle_depth = angle_depth.checked_sub(1)?,
            ':' if angle_depth == 0
                && chars.get(index + 1).is_some_and(|(_, next)| *next == ':')
                && chars.get(index + 2).is_none_or(|(_, next)| *next != '<') =>
            {
                segments.push(&rust_path[start..byte_index]);
                start = byte_index + 2;
                index += 1;
            }
            _ => {}
        }
        index += 1;
    }
    if angle_depth != 0 {
        return None;
    }
    segments.push(&rust_path[start..]);
    if segments
        .iter()
        .all(|segment| rust_path_segment_is_path_like(segment))
    {
        Some(segments.into_iter().map(str::to_string).collect())
    } else {
        None
    }
}

fn rust_path_segment_is_path_like(segment: &str) -> bool {
    if rust_path_segment_is_plain_ident(segment) {
        return true;
    }
    if let Some((ident, generic_args)) = segment.split_once("::<") {
        return rust_path_segment_is_plain_ident(ident)
            && !generic_args.is_empty()
            && generic_args.ends_with('>')
            && generic_args_is_balanced(generic_args);
    }
    if let Some((ident, generic_args)) = segment.split_once('<') {
        return rust_path_segment_is_plain_ident(ident)
            && !generic_args.is_empty()
            && generic_args.ends_with('>')
            && generic_args_is_balanced(generic_args);
    }
    false
}

fn generic_args_is_balanced(generic_args: &str) -> bool {
    let mut depth = 1usize;
    for ch in generic_args.chars() {
        match ch {
            '<' => depth = depth.saturating_add(1),
            '>' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ => {}
        }
    }
    depth == 0
}

pub fn facade_method_name(rust_path: &str) -> String {
    split_external_facade_rust_path(rust_path)
        .and_then(|segments| segments.into_iter().last())
        .unwrap_or_else(|| {
            rust_path
                .rsplit("::")
                .next()
                .unwrap_or(rust_path)
                .to_string()
        })
}

pub fn primitive_hash_code_projection(receiver: RustExpr, source: PrimitiveType) -> RustExpr {
    match source {
        PrimitiveType::Boolean => RustExpr::If {
            condition: Box::new(receiver),
            then_expr: Box::new(RustExpr::Literal("1231".to_string())),
            else_expr: Box::new(RustExpr::Literal("1237".to_string())),
        },
        PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Char | PrimitiveType::Int => {
            primitive_hash_i32_cast_projection(receiver)
        }
        PrimitiveType::Long => primitive_hash_long_bits_projection(receiver),
        PrimitiveType::Float => primitive_hash_float_bits_projection(receiver),
        PrimitiveType::Double => primitive_hash_double_bits_projection(receiver),
    }
}

fn primitive_hash_i32_cast_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(parenthesize_raw_primitive_number_cast_operand(receiver)),
        ty: "i32".to_string(),
    }
}

fn primitive_hash_float_bits_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(zero_arg_method_projection(
            parenthesize_raw_method_receiver(receiver),
            "to_bits",
        )),
        ty: "i32".to_string(),
    }
}

fn primitive_hash_long_bits_projection(receiver: RustExpr) -> RustExpr {
    let wide = RustExpr::Cast {
        expr: Box::new(parenthesize_raw_primitive_number_cast_operand(receiver)),
        ty: "i64".to_string(),
    };
    primitive_hash_folded_bits_projection(wide)
}

fn primitive_hash_double_bits_projection(receiver: RustExpr) -> RustExpr {
    let bits = zero_arg_method_projection(parenthesize_raw_method_receiver(receiver), "to_bits");
    primitive_hash_folded_bits_projection(bits)
}

fn primitive_hash_folded_bits_projection(bits: RustExpr) -> RustExpr {
    RustExpr::Cast {
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
    }
}

pub fn primitive_compare_to_projection(receiver: RustExpr, other: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_primitive_compare_to"]),
        vec![receiver, other],
    )
}

pub fn primitive_f64_method_projection(arg: RustExpr, method: &str) -> RustExpr {
    zero_arg_method_projection(f64_cast_projection(arg), method)
}

pub fn primitive_f64_binary_method_projection(
    left: RustExpr,
    right: RustExpr,
    method: &str,
) -> RustExpr {
    method_call_projection(
        f64_cast_projection(left),
        method,
        vec![f64_cast_projection(right)],
    )
}

pub fn math_pow_projection(base: RustExpr, exponent: RustExpr) -> RustExpr {
    method_call_projection(
        f64_cast_projection(base),
        "powf",
        vec![f64_cast_projection(exponent)],
    )
}

fn f64_cast_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(parenthesize_raw_primitive_number_cast_operand(expr)),
        ty: "f64".to_string(),
    }
}

pub fn math_round_projection(arg: RustExpr, source: PrimitiveType) -> RustExpr {
    let (helper, arg) = match source {
        PrimitiveType::Float => ("java_math_round_f32", arg),
        PrimitiveType::Double => ("java_math_round_f64", arg),
        _ => ("java_math_round_f64", f64_cast_projection(arg)),
    };
    function_call_projection(RustExpr::path([helper]), vec![arg])
}

pub fn math_abs_projection(arg: RustExpr, source: PrimitiveType) -> RustExpr {
    let method = match source {
        PrimitiveType::Float | PrimitiveType::Double => "abs",
        _ => "wrapping_abs",
    };
    zero_arg_method_projection(parenthesize_raw_method_receiver(arg), method)
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
        RustExpr::Cast {
            expr: Box::new(call),
            ty: "i32".to_string(),
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
    let (helper, cast_ty) = match primitive {
        PrimitiveType::Long => (format!("java_math_floor_{operation}_i64"), "i64"),
        _ => (format!("java_math_floor_{operation}_i32"), "i32"),
    };
    function_call_projection(
        RustExpr::path([helper]),
        vec![
            RustExpr::Cast {
                expr: Box::new(parenthesize_raw_primitive_number_cast_operand(left)),
                ty: cast_ty.to_string(),
            },
            RustExpr::Cast {
                expr: Box::new(parenthesize_raw_primitive_number_cast_operand(right)),
                ty: cast_ty.to_string(),
            },
        ],
    )
}

pub fn len_i32_projection(expr: RustExpr) -> RustExpr {
    method_call_as_i32_projection(expr, "len")
}

#[cfg(test)]
pub fn render_len_i32_projection(rendered: &str) -> String {
    emit::emit_expr(&len_i32_projection(parenthesize_raw_method_receiver(
        rendered_projection_or_raw_leaf(rendered),
    )))
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
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        typed_collect_projection(
            method_call_projection(
                zero_arg_method_projection(parenthesize_raw_method_receiver(receiver), "iter"),
                "map",
                vec![single_param_closure_expr(
                    "value",
                    function_call_projection(
                        RustExpr::Var("java_clone_erased_object_ref".to_string()),
                        vec![RustExpr::Var("value".to_string())],
                    ),
                )],
            ),
            "Vec<_>",
        ),
    )
}

fn typed_collect_projection(receiver: RustExpr, ty: &str) -> RustExpr {
    typed_zero_arg_method_projection(receiver, "collect", ty)
}

fn typed_zero_arg_method_projection(receiver: RustExpr, method: &str, ty: &str) -> RustExpr {
    zero_arg_method_projection(receiver, &typed_method_name(method, ty))
}

fn typed_method_name(method: &str, ty: &str) -> String {
    format!("{method}::<{ty}>")
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
    method_call_projection(
        zero_arg_method_projection(receiver, "iter"),
        "map",
        vec![single_param_closure_expr(
            "value",
            function_call_projection(
                RustExpr::path(["java_clone_erased_object_ref"]),
                vec![RustExpr::Var("value".to_string())],
            ),
        )],
    )
}

pub fn sequence_iter_copied_projection(receiver: RustExpr) -> RustExpr {
    zero_arg_method_projection(zero_arg_method_projection(receiver, "iter"), "copied")
}

pub fn iterator_next_expect_projection(iterator: RustExpr) -> RustExpr {
    let next = zero_arg_method_projection(iterator, "next");
    method_expect_projection(next, "Java iterator exhausted")
}

pub fn tuple_field_projection(tuple: RustExpr, field_index: usize) -> RustExpr {
    RustExpr::Field(Box::new(tuple), field_index.to_string())
}

pub fn subtype_owned_constructor_from_borrowed_projection(
    target_ty: &str,
    borrowed: RustExpr,
) -> RustExpr {
    function_call_projection(
        associated_rust_path_expr(target_ty, "new_1"),
        vec![apply_clone_projection(borrowed)],
    )
}

fn associated_rust_path_expr(owner: &str, member: &str) -> RustExpr {
    if let Some(mut segments) = split_external_facade_rust_path(owner) {
        segments.push(member.to_string());
        return RustExpr::Path(segments);
    }
    qualified_rust_type_associated_path_projection(RustType::Raw(owner.to_string()), member)
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
        args: vec![rust_string_literal_projection(message)],
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
        RustExpr::Raw(raw) => raw
            .strip_suffix(".unwrap()")
            .map(rendered_projection_or_raw_leaf)
            .unwrap_or_else(|| rendered_projection_or_raw_leaf(&raw)),
        other => other,
    }
}

pub fn file_exists_projection(receiver: RustExpr) -> RustExpr {
    zero_arg_method_projection(
        function_call_projection(
            RustExpr::path(["std", "path", "Path", "new"]),
            vec![zero_arg_method_projection(
                zero_arg_method_projection(receiver, "get_path"),
                "as_str",
            )],
        ),
        "exists",
    )
}

pub fn system_exit_projection(status: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["std", "process", "exit"]),
        vec![primitive_number_value_projection(
            status,
            PrimitiveType::Int,
        )],
    )
}

pub fn thread_sleep_millis_projection(millis: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["std", "thread", "sleep"]),
        vec![function_call_projection(
            RustExpr::path(["std", "time", "Duration", "from_millis"]),
            vec![RustExpr::Cast {
                expr: Box::new(group_raw_cast_operand_if_needed(millis)),
                ty: "u64".to_string(),
            }],
        )],
    )
}

pub fn print_macro_projection(macro_name: &str, arg: Option<RustExpr>) -> RustExpr {
    let mut args = Vec::new();
    if let Some(arg) = arg {
        args.push(RustExpr::Literal("\"{}\"".to_string()));
        args.push(arg);
    }
    macro_call_projection(macro_name, args)
}

pub fn macro_call_projection(macro_name: &str, args: Vec<RustExpr>) -> RustExpr {
    RustExpr::MacroCall {
        macro_name: macro_name.to_string(),
        args,
    }
}

pub fn logical_not_projection(expr: RustExpr) -> RustExpr {
    let expr = match expr {
        RustExpr::Raw(raw) => grouped_raw_leaf(raw),
        other => other,
    };
    RustExpr::Unary {
        op: "!".to_string(),
        expr: Box::new(expr),
    }
}

pub fn math_max_projection(left: RustExpr, right: RustExpr, primitive: PrimitiveType) -> RustExpr {
    match primitive {
        PrimitiveType::Float => {
            function_call_projection(RustExpr::path(["java_math_max_f32"]), vec![left, right])
        }
        PrimitiveType::Double => {
            function_call_projection(RustExpr::path(["java_math_max_f64"]), vec![left, right])
        }
        _ => function_call_projection(RustExpr::path(["std", "cmp", "max"]), vec![left, right]),
    }
}

pub fn math_min_projection(left: RustExpr, right: RustExpr, primitive: PrimitiveType) -> RustExpr {
    match primitive {
        PrimitiveType::Float => {
            function_call_projection(RustExpr::path(["java_math_min_f32"]), vec![left, right])
        }
        PrimitiveType::Double => {
            function_call_projection(RustExpr::path(["java_math_min_f64"]), vec![left, right])
        }
        _ => function_call_projection(RustExpr::path(["std", "cmp", "min"]), vec![left, right]),
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
    zero_arg_method_projection(arg, "is_some")
}

pub fn objects_is_null_projection(arg: RustExpr) -> RustExpr {
    zero_arg_method_projection(arg, "is_none")
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
            zero_arg_method_projection(source, "into_iter"),
            "std::collections::HashSet<_>",
        ),
        crate::stdlib::JavaCollectionKind::Map => source,
    }
}

pub fn arrays_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    function_call_projection(RustExpr::path(["java_arrays_equals"]), vec![left, right])
}

pub fn arrays_deep_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["java_arrays_deep_equals"]),
        vec![left, right],
    )
}

pub fn arrays_deep_hash_code_projection(source: RustExpr) -> RustExpr {
    function_call_projection(RustExpr::path(["java_arrays_deep_hash_code"]), vec![source])
}

pub fn arrays_equals_range_projection(
    left: RustExpr,
    left_from: RustExpr,
    left_to: RustExpr,
    right: RustExpr,
    right_from: RustExpr,
    right_to: RustExpr,
) -> RustExpr {
    arrays_dual_range_runtime_call_projection(
        "java_arrays_equals",
        left,
        left_from,
        left_to,
        right,
        right_from,
        right_to,
    )
}

pub fn arrays_binary_search_projection(source: RustExpr, key: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_source".to_string(),
                ty: None,
                value: Some(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(source),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_key".to_string(),
                ty: None,
                value: Some(key),
            },
        ],
        tail: Some(Box::new(arrays_binary_search_match_expr(
            RustExpr::Var("__java_arrays_source".to_string()),
            RustExpr::Var("__java_arrays_key".to_string()),
            None,
        ))),
    }
}

fn arrays_binary_search_match_expr(
    source: RustExpr,
    key: RustExpr,
    base_offset: Option<RustExpr>,
) -> RustExpr {
    RustExpr::Match {
        expr: Box::new(receiver_method_call_projection_exact(
            source,
            "binary_search",
            vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(key),
            }],
        )),
        arms: vec![
            RustMatchArm {
                pattern: "Ok(__java_arrays_index)".to_string(),
                body: arrays_binary_search_index_result_expr(
                    RustExpr::Var("__java_arrays_index".to_string()),
                    base_offset.clone(),
                ),
            },
            RustMatchArm {
                pattern: "Err(__java_arrays_insert)".to_string(),
                body: RustExpr::Unary {
                    op: "-".to_string(),
                    expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                        op: "+".to_string(),
                        left: Box::new(arrays_binary_search_index_result_expr(
                            RustExpr::Var("__java_arrays_insert".to_string()),
                            base_offset,
                        )),
                        right: Box::new(RustExpr::Literal("1".to_string())),
                    }))),
                },
            },
        ],
    }
}

fn arrays_binary_search_index_result_expr(
    index: RustExpr,
    base_offset: Option<RustExpr>,
) -> RustExpr {
    let indexed = match base_offset {
        Some(base_offset) => RustExpr::Binary {
            op: "+".to_string(),
            left: Box::new(base_offset),
            right: Box::new(index),
        },
        None => index,
    };
    RustExpr::Cast {
        expr: Box::new(RustExpr::Grouped(Box::new(indexed))),
        ty: "i32".to_string(),
    }
}

pub fn arrays_binary_search_range_projection(
    source: RustExpr,
    from: RustExpr,
    to: RustExpr,
    key: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_source".to_string(),
                ty: None,
                value: Some(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(source),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(from),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(to),
            },
            RustStmt::If {
                condition: RustExpr::Binary {
                    op: ">".to_string(),
                    left: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                    right: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                },
                then_body: vec![RustStmt::Expr(arrays_panic_expr(
                    "Java IllegalArgumentException: fromIndex > toIndex",
                ))],
                else_body: Vec::new(),
            },
            RustStmt::If {
                condition: RustExpr::Binary {
                    op: "||".to_string(),
                    left: Box::new(RustExpr::Binary {
                        op: "<".to_string(),
                        left: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                        right: Box::new(RustExpr::Literal("0".to_string())),
                    }),
                    right: Box::new(RustExpr::Binary {
                        op: ">".to_string(),
                        left: Box::new(RustExpr::Cast {
                            expr: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                            ty: "usize".to_string(),
                        }),
                        right: Box::new(receiver_method_call_projection_exact(
                            RustExpr::Var("__java_arrays_source".to_string()),
                            "len",
                            Vec::new(),
                        )),
                    }),
                },
                then_body: vec![RustStmt::Expr(arrays_panic_expr(
                    "Java ArrayIndexOutOfBoundsException",
                ))],
                else_body: Vec::new(),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                    ty: "usize".to_string(),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    ty: "usize".to_string(),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_key".to_string(),
                ty: None,
                value: Some(key),
            },
        ],
        tail: Some(Box::new(arrays_binary_search_match_expr(
            RustExpr::Index(
                Box::new(RustExpr::Var("__java_arrays_source".to_string())),
                Box::new(RustExpr::Range {
                    start: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                    end: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    inclusive: false,
                }),
            ),
            RustExpr::Var("__java_arrays_key".to_string()),
            Some(RustExpr::Var("__java_arrays_from".to_string())),
        ))),
    }
}

fn arrays_panic_expr(message: &str) -> RustExpr {
    RustExpr::MacroCall {
        macro_name: "panic".to_string(),
        args: vec![rust_string_literal_projection(message)],
    }
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
    arrays_dual_range_runtime_call_projection(
        "java_arrays_compare",
        left,
        left_from,
        left_to,
        right,
        right_from,
        right_to,
    )
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
    arrays_dual_range_runtime_call_projection(
        "java_arrays_mismatch",
        left,
        left_from,
        left_to,
        right,
        right_from,
        right_to,
    )
}

fn arrays_dual_range_runtime_call_projection(
    runtime_function: &str,
    left: RustExpr,
    left_from: RustExpr,
    left_to: RustExpr,
    right: RustExpr,
    right_from: RustExpr,
    right_to: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            arrays_let_borrowed_source_stmt("__java_arrays_left", left),
            arrays_let_value_stmt("__java_arrays_left_from", left_from),
            arrays_let_value_stmt("__java_arrays_left_to", left_to),
            arrays_let_borrowed_source_stmt("__java_arrays_right", right),
            arrays_let_value_stmt("__java_arrays_right_from", right_from),
            arrays_let_value_stmt("__java_arrays_right_to", right_to),
            arrays_dual_range_order_guard_stmt(),
            arrays_dual_range_bounds_guard_stmt(),
            arrays_let_usize_shadow_stmt("__java_arrays_left_from"),
            arrays_let_usize_shadow_stmt("__java_arrays_left_to"),
            arrays_let_usize_shadow_stmt("__java_arrays_right_from"),
            arrays_let_usize_shadow_stmt("__java_arrays_right_to"),
        ],
        tail: Some(Box::new(RustExpr::Call {
            function: Box::new(RustExpr::path([runtime_function])),
            args: vec![
                arrays_borrowed_range_slice_expr(
                    "__java_arrays_left",
                    "__java_arrays_left_from",
                    "__java_arrays_left_to",
                ),
                arrays_borrowed_range_slice_expr(
                    "__java_arrays_right",
                    "__java_arrays_right_from",
                    "__java_arrays_right_to",
                ),
            ],
        })),
    }
}

fn arrays_let_borrowed_source_stmt(name: &str, value: RustExpr) -> RustStmt {
    arrays_let_value_stmt(
        name,
        RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(value),
        },
    )
}

fn arrays_let_value_stmt(name: &str, value: RustExpr) -> RustStmt {
    RustStmt::Let {
        mutable: false,
        name: name.to_string(),
        ty: None,
        value: Some(value),
    }
}

fn arrays_dual_range_order_guard_stmt() -> RustStmt {
    RustStmt::If {
        condition: RustExpr::Binary {
            op: "||".to_string(),
            left: Box::new(RustExpr::Binary {
                op: ">".to_string(),
                left: Box::new(RustExpr::Var("__java_arrays_left_from".to_string())),
                right: Box::new(RustExpr::Var("__java_arrays_left_to".to_string())),
            }),
            right: Box::new(RustExpr::Binary {
                op: ">".to_string(),
                left: Box::new(RustExpr::Var("__java_arrays_right_from".to_string())),
                right: Box::new(RustExpr::Var("__java_arrays_right_to".to_string())),
            }),
        },
        then_body: vec![RustStmt::Expr(arrays_panic_expr(
            "Java IllegalArgumentException: fromIndex > toIndex",
        ))],
        else_body: Vec::new(),
    }
}

fn arrays_dual_range_bounds_guard_stmt() -> RustStmt {
    RustStmt::If {
        condition: arrays_or_chain_expr(vec![
            arrays_less_than_zero_expr("__java_arrays_left_from"),
            arrays_usize_greater_than_len_expr("__java_arrays_left_to", "__java_arrays_left"),
            arrays_less_than_zero_expr("__java_arrays_right_from"),
            arrays_usize_greater_than_len_expr("__java_arrays_right_to", "__java_arrays_right"),
        ]),
        then_body: vec![RustStmt::Expr(arrays_panic_expr(
            "Java ArrayIndexOutOfBoundsException",
        ))],
        else_body: Vec::new(),
    }
}

fn arrays_or_chain_expr(mut conditions: Vec<RustExpr>) -> RustExpr {
    let first = conditions
        .drain(..1)
        .next()
        .expect("array guard should have at least one condition");
    conditions
        .into_iter()
        .fold(first, |left, right| RustExpr::Binary {
            op: "||".to_string(),
            left: Box::new(left),
            right: Box::new(right),
        })
}

fn arrays_less_than_zero_expr(name: &str) -> RustExpr {
    RustExpr::Binary {
        op: "<".to_string(),
        left: Box::new(RustExpr::Var(name.to_string())),
        right: Box::new(RustExpr::Literal("0".to_string())),
    }
}

fn arrays_usize_greater_than_len_expr(index_name: &str, source_name: &str) -> RustExpr {
    RustExpr::Binary {
        op: ">".to_string(),
        left: Box::new(RustExpr::Cast {
            expr: Box::new(RustExpr::Var(index_name.to_string())),
            ty: "usize".to_string(),
        }),
        right: Box::new(receiver_method_call_projection_exact(
            RustExpr::Var(source_name.to_string()),
            "len",
            Vec::new(),
        )),
    }
}

fn arrays_let_usize_shadow_stmt(name: &str) -> RustStmt {
    arrays_let_value_stmt(
        name,
        RustExpr::Cast {
            expr: Box::new(RustExpr::Var(name.to_string())),
            ty: "usize".to_string(),
        },
    )
}

fn arrays_borrowed_range_slice_expr(source_name: &str, from_name: &str, to_name: &str) -> RustExpr {
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(RustExpr::Index(
            Box::new(RustExpr::Var(source_name.to_string())),
            Box::new(RustExpr::Range {
                start: Box::new(RustExpr::Var(from_name.to_string())),
                end: Box::new(RustExpr::Var(to_name.to_string())),
                inclusive: false,
            }),
        )),
    }
}

pub fn arrays_copy_of_projection(source: RustExpr, len: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            arrays_let_borrowed_source_stmt("__java_arrays_source", source),
            arrays_let_value_stmt("__java_arrays_len", len),
            arrays_negative_length_guard_stmt("__java_arrays_len"),
            arrays_let_usize_shadow_stmt("__java_arrays_len"),
            arrays_let_mut_value_stmt(
                "__java_arrays_out",
                receiver_method_call_projection_exact(
                    receiver_method_call_projection_exact(
                        receiver_method_call_projection_exact(
                            receiver_method_call_projection_exact(
                                RustExpr::Var("__java_arrays_source".to_string()),
                                "iter",
                                Vec::new(),
                            ),
                            "cloned",
                            Vec::new(),
                        ),
                        "take",
                        vec![RustExpr::Var("__java_arrays_len".to_string())],
                    ),
                    "collect::<Vec<_>>",
                    Vec::new(),
                ),
            ),
            arrays_resize_out_stmt(),
        ],
        tail: Some(Box::new(RustExpr::Var("__java_arrays_out".to_string()))),
    }
}

pub fn arrays_copy_of_range_projection(source: RustExpr, from: RustExpr, to: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            arrays_let_borrowed_source_stmt("__java_arrays_source", source),
            arrays_let_value_stmt("__java_arrays_from", from),
            arrays_let_value_stmt("__java_arrays_to", to),
            arrays_single_range_order_guard_stmt(),
            arrays_copy_of_range_bounds_guard_stmt(),
            arrays_let_usize_shadow_stmt("__java_arrays_from"),
            arrays_let_usize_shadow_stmt("__java_arrays_to"),
            arrays_let_value_stmt(
                "__java_arrays_len",
                RustExpr::Binary {
                    op: "-".to_string(),
                    left: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    right: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                },
            ),
            arrays_let_mut_value_stmt(
                "__java_arrays_out",
                receiver_method_call_projection_exact(
                    RustExpr::Index(
                        Box::new(RustExpr::Var("__java_arrays_source".to_string())),
                        Box::new(RustExpr::Range {
                            start: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                            end: Box::new(receiver_method_call_projection_exact(
                                receiver_method_call_projection_exact(
                                    RustExpr::Var("__java_arrays_source".to_string()),
                                    "len",
                                    Vec::new(),
                                ),
                                "min",
                                vec![RustExpr::Var("__java_arrays_to".to_string())],
                            )),
                            inclusive: false,
                        }),
                    ),
                    "to_vec",
                    Vec::new(),
                ),
            ),
            arrays_resize_out_stmt(),
        ],
        tail: Some(Box::new(RustExpr::Var("__java_arrays_out".to_string()))),
    }
}

fn arrays_let_mut_value_stmt(name: &str, value: RustExpr) -> RustStmt {
    RustStmt::Let {
        mutable: true,
        name: name.to_string(),
        ty: None,
        value: Some(value),
    }
}

fn arrays_negative_length_guard_stmt(len_name: &str) -> RustStmt {
    RustStmt::If {
        condition: arrays_less_than_zero_expr(len_name),
        then_body: vec![RustStmt::Expr(arrays_panic_expr(
            "Java NegativeArraySizeException",
        ))],
        else_body: Vec::new(),
    }
}

fn arrays_single_range_order_guard_stmt() -> RustStmt {
    RustStmt::If {
        condition: RustExpr::Binary {
            op: ">".to_string(),
            left: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
            right: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
        },
        then_body: vec![RustStmt::Expr(arrays_panic_expr(
            "Java IllegalArgumentException: fromIndex > toIndex",
        ))],
        else_body: Vec::new(),
    }
}

fn arrays_copy_of_range_bounds_guard_stmt() -> RustStmt {
    RustStmt::If {
        condition: RustExpr::Binary {
            op: "||".to_string(),
            left: Box::new(arrays_less_than_zero_expr("__java_arrays_from")),
            right: Box::new(arrays_usize_greater_than_len_expr(
                "__java_arrays_from",
                "__java_arrays_source",
            )),
        },
        then_body: vec![RustStmt::Expr(arrays_panic_expr(
            "Java ArrayIndexOutOfBoundsException",
        ))],
        else_body: Vec::new(),
    }
}

fn arrays_resize_out_stmt() -> RustStmt {
    RustStmt::Expr(receiver_method_call_projection_exact(
        RustExpr::Var("__java_arrays_out".to_string()),
        "resize",
        vec![
            RustExpr::Var("__java_arrays_len".to_string()),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Default", "default"])),
                args: Vec::new(),
            },
        ],
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
            expr: Box::new(group_raw_cast_operand_if_needed(count)),
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
    let capacity = recover_parseable_raw_operand_projection(capacity);
    let capacity = RustExpr::Cast {
        expr: Box::new(group_raw_cast_operand_if_needed(capacity)),
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
    discard_value_block_leaf(value)
}

fn discard_value_block_leaf(value: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: "_".to_string(),
            ty: None,
            value: Some(value),
        }],
        tail: None,
    }
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
    varargs_spread_extend_block_leaf(packed_vec, tail)
}

fn varargs_spread_extend_block_leaf(packed_vec: RustExpr, tail: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: true,
                name: "__java_varargs".to_string(),
                ty: None,
                value: Some(packed_vec),
            },
            RustStmt::Expr(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("__java_varargs".to_string())),
                method: "extend".to_string(),
                args: vec![tail],
            }),
        ],
        tail: Some(Box::new(RustExpr::Var("__java_varargs".to_string()))),
    }
}

pub fn collection_remove_if_projection(
    receiver: RustExpr,
    param_name: &str,
    predicate: RustExpr,
) -> RustExpr {
    receiver_method_call_projection_exact(
        receiver,
        "retain",
        vec![collection_remove_if_predicate_leaf(param_name, predicate)],
    )
}

fn collection_remove_if_predicate_leaf(param_name: &str, predicate: RustExpr) -> RustExpr {
    single_param_closure_expr(
        param_name,
        RustExpr::Unary {
            op: "!".to_string(),
            expr: Box::new(RustExpr::Grouped(Box::new(predicate))),
        },
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
    collection_sort_range_block_leaf(target, from, to)
}

fn collection_sort_range_block_leaf(target: RustExpr, from: RustExpr, to: RustExpr) -> RustExpr {
    checked_array_range_block_expr(
        target,
        from,
        to,
        "__java_arrays_target",
        RustExpr::MethodCall {
            receiver: Box::new(checked_array_range_slice_expr("__java_arrays_target")),
            method: "sort".to_string(),
            args: Vec::new(),
        },
    )
}

pub fn collection_sort_by_comparator_projection(
    target: RustExpr,
    comparator: RustExpr,
) -> RustExpr {
    collection_sort_by_comparator_block_leaf(target, comparator)
}

fn collection_sort_by_comparator_block_leaf(target: RustExpr, comparator: RustExpr) -> RustExpr {
    comparator_sort_block_expr(
        target,
        comparator,
        false,
        receiver_method_call_projection_exact(
            RustExpr::Var("__java_comparator".to_string()),
            "compare",
            vec![clone_var_expr("a"), clone_var_expr("b")],
        ),
    )
}

pub fn collection_sort_by_callback_projection(target: RustExpr, comparator: RustExpr) -> RustExpr {
    collection_sort_by_callback_block_leaf(target, comparator)
}

fn collection_sort_by_callback_block_leaf(target: RustExpr, comparator: RustExpr) -> RustExpr {
    comparator_sort_block_expr(
        target,
        comparator,
        true,
        RustExpr::Call {
            function: Box::new(RustExpr::Var("__java_comparator".to_string())),
            args: vec![clone_var_expr("a"), clone_var_expr("b")],
        },
    )
}

fn comparator_sort_block_expr(
    target: RustExpr,
    comparator: RustExpr,
    mutable_comparator: bool,
    comparison_value: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: mutable_comparator,
                name: "__java_comparator".to_string(),
                ty: None,
                value: Some(comparator),
            },
            RustStmt::Expr(receiver_method_call_projection_exact(
                target,
                "sort_by",
                vec![closure_expr(
                    vec![
                        RustClosureParam {
                            pattern: "a".to_string(),
                            ty: None,
                        },
                        RustClosureParam {
                            pattern: "b".to_string(),
                            ty: None,
                        },
                    ],
                    receiver_method_call_projection_exact(
                        comparison_value,
                        "cmp",
                        vec![RustExpr::Unary {
                            op: "&".to_string(),
                            expr: Box::new(RustExpr::Literal("0".to_string())),
                        }],
                    ),
                )],
            )),
        ],
        tail: None,
    }
}

fn clone_var_expr(name: &str) -> RustExpr {
    zero_arg_method_projection(RustExpr::Var(name.to_string()), "clone")
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
    array_fill_range_block_leaf(target, from, to, value)
}

fn array_fill_range_block_leaf(
    target: RustExpr,
    from: RustExpr,
    to: RustExpr,
    value: RustExpr,
) -> RustExpr {
    checked_array_range_block_expr(
        target,
        from,
        to,
        "__java_arrays_target",
        RustExpr::MethodCall {
            receiver: Box::new(checked_array_range_slice_expr("__java_arrays_target")),
            method: "fill".to_string(),
            args: vec![value],
        },
    )
}

pub fn arrays_stream_range_projection(source: RustExpr, from: RustExpr, to: RustExpr) -> RustExpr {
    checked_array_range_source_slice_expr(source, from, to)
}

fn checked_array_range_source_slice_expr(
    source: RustExpr,
    from: RustExpr,
    to: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_source".to_string(),
                ty: None,
                value: Some(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(source),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(from),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(to),
            },
            checked_array_from_exceeds_to_stmt(),
            checked_array_range_out_of_bounds_stmt("__java_arrays_source"),
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                    ty: "usize".to_string(),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    ty: "usize".to_string(),
                }),
            },
        ],
        tail: Some(Box::new(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(checked_array_range_slice_expr("__java_arrays_source")),
        })),
    }
}

fn checked_array_range_block_expr(
    target: RustExpr,
    from: RustExpr,
    to: RustExpr,
    target_name: &str,
    body: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: target_name.to_string(),
                ty: None,
                value: Some(RustExpr::Unary {
                    op: "&mut ".to_string(),
                    expr: Box::new(target),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(from),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(to),
            },
            checked_array_from_exceeds_to_stmt(),
            checked_array_range_out_of_bounds_stmt(target_name),
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_from".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                    ty: "usize".to_string(),
                }),
            },
            RustStmt::Let {
                mutable: false,
                name: "__java_arrays_to".to_string(),
                ty: None,
                value: Some(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    ty: "usize".to_string(),
                }),
            },
            RustStmt::Expr(body),
        ],
        tail: None,
    }
}

fn checked_array_from_exceeds_to_stmt() -> RustStmt {
    RustStmt::If {
        condition: RustExpr::Binary {
            op: ">".to_string(),
            left: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
            right: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
        },
        then_body: vec![panic_stmt(
            "\"Java IllegalArgumentException: fromIndex > toIndex\"",
        )],
        else_body: Vec::new(),
    }
}

fn checked_array_range_out_of_bounds_stmt(target_name: &str) -> RustStmt {
    RustStmt::If {
        condition: RustExpr::Binary {
            op: "||".to_string(),
            left: Box::new(RustExpr::Binary {
                op: "<".to_string(),
                left: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
                right: Box::new(RustExpr::Literal("0".to_string())),
            }),
            right: Box::new(RustExpr::Binary {
                op: ">".to_string(),
                left: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
                    ty: "usize".to_string(),
                }),
                right: Box::new(zero_arg_method_projection(
                    RustExpr::Var(target_name.to_string()),
                    "len",
                )),
            }),
        },
        then_body: vec![panic_stmt("\"Java ArrayIndexOutOfBoundsException\"")],
        else_body: Vec::new(),
    }
}

fn checked_array_range_slice_expr(target_name: &str) -> RustExpr {
    RustExpr::Index(
        Box::new(RustExpr::Var(target_name.to_string())),
        Box::new(RustExpr::Range {
            start: Box::new(RustExpr::Var("__java_arrays_from".to_string())),
            end: Box::new(RustExpr::Var("__java_arrays_to".to_string())),
            inclusive: false,
        }),
    )
}

fn panic_stmt(message: &str) -> RustStmt {
    RustStmt::Expr(RustExpr::MacroCall {
        macro_name: "panic".to_string(),
        args: vec![RustExpr::Literal(message.to_string())],
    })
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
    let expr = recover_parseable_raw_operand_projection(expr);
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        method_expect_projection(
            zero_arg_method_projection(expr, "copied"),
            "Java null receiver",
        ),
    )
}

#[cfg(test)]
pub fn render_string_format_arg_value(arg: RustExpr) -> String {
    emit::emit_expr(&string_format_arg_value_projection(arg))
}

pub fn string_format_arg_value_projection(arg: RustExpr) -> RustExpr {
    zero_arg_method_projection(parenthesize_raw_method_receiver(arg), "to_string")
}

pub fn string_format_projection(mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() {
        return function_call_projection(RustExpr::path(["String", "new"]), Vec::new());
    }
    if args.len() >= 2 && is_java_locale_format_arg(&args[0]) {
        args.remove(0);
    }
    let format_arg = args.remove(0);
    let Some(java_format) = string_literal_source(&format_arg) else {
        return function_call_projection(
            RustExpr::path(["java_string_format"]),
            vec![
                format_arg,
                RustExpr::VecLiteral(
                    args.into_iter()
                        .map(string_format_arg_value_projection)
                        .collect(),
                ),
            ],
        );
    };
    let rust_format = java_format_to_rust_format(&java_format);
    let mut rendered_args = vec![rust_string_literal_projection(&rust_format)];
    rendered_args.extend(args);
    macro_call_projection("format", rendered_args)
}

fn is_java_locale_format_arg(arg: &RustExpr) -> bool {
    match arg.unshaped() {
        RustExpr::Var(name) => matches!(name.as_str(), "Locale" | "JavaUtilLocale"),
        RustExpr::Path(segments) => matches!(
            segments.first().map(String::as_str),
            Some("Locale" | "JavaUtilLocale")
        ),
        RustExpr::Raw(raw) => {
            raw == "Locale"
                || raw.starts_with("Locale::")
                || raw == "JavaUtilLocale"
                || raw.starts_with("JavaUtilLocale::")
        }
        _ => false,
    }
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
    let iter = zero_arg_method_projection(borrow_operand_expr(arg), "iter");
    match borrowed_element_recovery_policy(element_shape, facts) {
        BorrowedElementRecoveryPolicy::BorrowElement => iter,
        BorrowedElementRecoveryPolicy::StringViewToOwned => receiver_method_call_projection_exact(
            iter,
            "map",
            vec![param_to_string_closure_expr("v")],
        ),
        BorrowedElementRecoveryPolicy::CopyElements => zero_arg_method_projection(iter, "copied"),
        BorrowedElementRecoveryPolicy::CloneElements { .. } => {
            zero_arg_method_projection(iter, "cloned")
        }
        BorrowedElementRecoveryPolicy::Unsupported { .. } => RustExpr::PostfixComment {
            expr: Box::new(zero_arg_method_projection(iter, "cloned")),
            comment: " /* java2rust: unsupported borrowed element recovery for extend */"
                .to_string(),
        },
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

pub fn sequence_iteration_source_projection(
    source: RustExpr,
    optional_sequence: bool,
    projection: SequenceIterationProjection,
) -> RustExpr {
    let source = parenthesize_raw_method_receiver(source);
    match (optional_sequence, projection) {
        (true, SequenceIterationProjection::Borrowed) => zero_arg_method_projection(
            method_expect_projection(
                zero_arg_method_projection(source, "as_ref"),
                "Java null receiver",
            ),
            "iter",
        ),
        (true, SequenceIterationProjection::OwnedValue) => {
            zero_arg_method_projection(zero_arg_method_projection(source, "unwrap"), "into_iter")
        }
        (false, SequenceIterationProjection::Borrowed) => {
            zero_arg_method_projection(source, "iter")
        }
        (false, SequenceIterationProjection::OwnedValue) => {
            zero_arg_method_projection(source, "into_iter")
        }
    }
}

#[cfg(test)]
pub fn render_optional_non_null_filter_map(item_is_borrowed: bool) -> String {
    let projection = optional_non_null_filter_map_projection(
        RustExpr::Var("__java_stream".to_string()),
        item_is_borrowed,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn optional_non_null_filter_map_projection(
    receiver: RustExpr,
    item_is_borrowed: bool,
) -> RustExpr {
    let body = if item_is_borrowed {
        zero_arg_method_projection(RustExpr::Var("item".to_string()), "as_ref")
    } else {
        RustExpr::Var("item".to_string())
    };
    method_call_projection(
        receiver,
        "filter_map",
        vec![single_param_closure_expr("item", body)],
    )
}

pub fn nullable_stream_map_body_projection(
    before_guard: Vec<RustStmt>,
    condition: RustExpr,
    else_body: Vec<RustStmt>,
    final_expr: RustExpr,
) -> RustExpr {
    RustExpr::Block {
        stmts: before_guard,
        tail: Some(Box::new(RustExpr::If {
            condition: Box::new(condition),
            then_expr: Box::new(RustExpr::path(["None"])),
            else_expr: Box::new(if else_body.is_empty() {
                some_expr(final_expr)
            } else {
                RustExpr::Block {
                    stmts: else_body,
                    tail: Some(Box::new(some_expr(final_expr))),
                }
            }),
        })),
    }
}

fn some_expr(value: RustExpr) -> RustExpr {
    function_call_projection(RustExpr::path(["Some"]), vec![value])
}

pub fn borrowed_collection_flat_map_projection(receiver: RustExpr, param_name: &str) -> RustExpr {
    method_call_projection(
        receiver,
        "flat_map",
        vec![single_param_closure_expr(
            param_name,
            zero_arg_method_projection(RustExpr::Var(param_name.to_string()), "iter"),
        )],
    )
}

#[cfg(test)]
pub fn render_borrowed_collection_flat_map(param_name: &str) -> String {
    let projection = borrowed_collection_flat_map_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_name,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_collect_to_map_projection(
    receiver: RustExpr,
    param_name: &str,
    key: RustExpr,
    value: RustExpr,
) -> RustExpr {
    let mapped = method_call_projection(
        receiver,
        "map",
        vec![single_param_closure_expr(
            param_name,
            RustExpr::Tuple(vec![key, value]),
        )],
    );
    typed_collect_projection(mapped, "std::collections::HashMap<_, _>")
}

#[cfg(test)]
pub fn render_stream_collect_to_map_terminal(
    param_name: &str,
    key: RustExpr,
    value: RustExpr,
) -> String {
    let projection = stream_collect_to_map_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_name,
        key,
        value,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_join_terminal_projection(
    receiver: RustExpr,
    item_display: RustExpr,
    delimiter: RustExpr,
) -> RustExpr {
    let mapped = method_call_projection(
        receiver,
        "map",
        vec![single_param_closure_expr("item", item_display)],
    );
    method_call_projection(
        typed_collect_projection(mapped, "Vec<_>"),
        "join",
        vec![delimiter],
    )
}

#[cfg(test)]
pub fn render_stream_join_terminal(item_display: RustExpr, delimiter: RustExpr) -> String {
    let projection = stream_join_terminal_projection(
        RustExpr::Var("__java_stream".to_string()),
        item_display,
        delimiter,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_match_terminal_projection(
    receiver: RustExpr,
    terminal_method: &str,
    param_name: &str,
    predicate: RustExpr,
) -> RustExpr {
    method_call_projection(
        receiver,
        terminal_method,
        vec![single_param_closure_expr(param_name, predicate)],
    )
}

#[cfg(test)]
pub fn render_stream_match_terminal(
    terminal_method: &str,
    param_name: &str,
    predicate: RustExpr,
) -> String {
    let projection = stream_match_terminal_projection(
        RustExpr::Var("__java_stream".to_string()),
        terminal_method,
        param_name,
        predicate,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_reduce_terminal_projection(
    receiver: RustExpr,
    identity: RustExpr,
    left_param: &str,
    right_param: &str,
    body: RustExpr,
) -> RustExpr {
    method_call_projection(
        receiver,
        "fold",
        vec![
            identity,
            closure_expr(
                vec![
                    RustClosureParam {
                        pattern: left_param.to_string(),
                        ty: None,
                    },
                    RustClosureParam {
                        pattern: right_param.to_string(),
                        ty: None,
                    },
                ],
                body,
            ),
        ],
    )
}

#[cfg(test)]
pub fn render_stream_reduce_terminal(
    identity: RustExpr,
    left_param: &str,
    right_param: &str,
    body: RustExpr,
) -> String {
    let projection = stream_reduce_terminal_projection(
        RustExpr::Var("__java_stream".to_string()),
        identity,
        left_param,
        right_param,
        body,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_filter_op_projection(
    receiver: RustExpr,
    param_pattern: &str,
    predicate: RustExpr,
    move_closure: bool,
) -> RustExpr {
    method_call_projection(
        receiver,
        "filter",
        vec![RustExpr::Closure {
            is_move: move_closure,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: param_pattern.to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(predicate)),
        }],
    )
}

#[cfg(test)]
pub fn render_stream_filter_op(
    param_pattern: &str,
    predicate: RustExpr,
    move_closure: bool,
) -> String {
    let projection = stream_filter_op_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_pattern,
        predicate,
        move_closure,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_map_op_projection(
    receiver: RustExpr,
    param_pattern: &str,
    body: RustExpr,
) -> RustExpr {
    stream_map_op_projection_with_param(
        receiver,
        RustClosureParam {
            pattern: param_pattern.to_string(),
            ty: None,
        },
        body,
    )
}

pub fn stream_map_op_projection_with_param(
    receiver: RustExpr,
    param: RustClosureParam,
    body: RustExpr,
) -> RustExpr {
    method_call_projection(
        receiver,
        "map",
        vec![RustExpr::Closure {
            is_move: false,
            params: vec![param],
            return_type: None,
            body: RustClosureBody::Expr(Box::new(body)),
        }],
    )
}

#[cfg(test)]
pub fn render_stream_map_op(param_pattern: &str, body: RustExpr) -> String {
    let projection = stream_map_op_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_pattern,
        body,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_flat_map_op_projection(
    receiver: RustExpr,
    param_pattern: &str,
    body: RustExpr,
) -> RustExpr {
    method_call_projection(
        receiver,
        "flat_map",
        vec![single_param_closure_expr(param_pattern, body)],
    )
}

#[cfg(test)]
pub fn render_stream_flat_map_op(param_pattern: &str, body: RustExpr) -> String {
    let projection = stream_flat_map_op_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_pattern,
        body,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

pub fn stream_prefixed_flat_map_op_projection(
    receiver: RustExpr,
    param_pattern: &str,
    prefix_stmts: Vec<RustStmt>,
    tail: RustExpr,
) -> RustExpr {
    stream_flat_map_op_projection(
        receiver,
        param_pattern,
        RustExpr::Block {
            stmts: prefix_stmts,
            tail: Some(Box::new(tail)),
        },
    )
}

#[cfg(test)]
pub fn render_stream_prefixed_flat_map_op(
    param_pattern: &str,
    prefix_stmts: Vec<RustStmt>,
    tail: RustExpr,
) -> String {
    let projection = stream_prefixed_flat_map_op_projection(
        RustExpr::Var("__java_stream".to_string()),
        param_pattern,
        prefix_stmts,
        tail,
    );
    let rendered = emit::emit_expr(&projection);
    rendered
        .strip_prefix("__java_stream")
        .unwrap_or(&rendered)
        .to_string()
}

#[cfg(test)]
pub fn render_stream_prefixed_flat_map_op_from_prefix_string(
    param_pattern: &str,
    prefix: String,
    tail: RustExpr,
) -> String {
    render_stream_prefixed_flat_map_op(param_pattern, prefixed_stream_block_stmts(prefix), tail)
}

#[cfg(test)]
fn prefixed_stream_block_stmts(prefix: String) -> Vec<RustStmt> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Vec::new();
    }
    prefix
        .split(';')
        .map(str::trim)
        .filter(|stmt| !stmt.is_empty())
        .map(|stmt| {
            prefixed_stream_block_let_stmt(stmt)
                .unwrap_or_else(|| prefixed_stream_block_expr_stmt(stmt))
        })
        .collect()
}

#[cfg(test)]
fn prefixed_stream_block_let_stmt(stmt: &str) -> Option<RustStmt> {
    let stmt = stmt.strip_prefix("let ")?;
    let (mutable, stmt) = stmt
        .strip_prefix("mut ")
        .map(|stmt| (true, stmt))
        .unwrap_or((false, stmt));
    let (name, value) = stmt.split_once(" = ")?;
    Some(RustStmt::Let {
        mutable,
        name: name.to_string(),
        ty: None,
        value: Some(rendered_projection_or_raw_leaf(value)),
    })
}

#[cfg(test)]
fn prefixed_stream_block_expr_stmt(stmt: &str) -> RustStmt {
    RustStmt::Expr(rendered_projection_or_raw_leaf(stmt))
}

pub fn stream_lambda_param_expr(
    projection: StreamLambdaParamProjection,
    param: &str,
    parenthesized: bool,
) -> RustExpr {
    let expr = match projection {
        StreamLambdaParamProjection::FilterBorrowed => RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Var(param.to_string())),
            }),
        },
        StreamLambdaParamProjection::BorrowedItem => RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(RustExpr::Var(param.to_string())),
        },
        StreamLambdaParamProjection::OwnedItem => RustExpr::Var(param.to_string()),
    };
    if parenthesized && !matches!(projection, StreamLambdaParamProjection::OwnedItem) {
        RustExpr::Grouped(Box::new(expr))
    } else {
        expr
    }
}

pub fn project_optional_receiver(
    expr: RustExpr,
    projection: OptionalReceiverProjection,
) -> RustExpr {
    let receiver = match strip_terminal_unwrap_projection(expr) {
        RustExpr::Raw(rendered) => rendered_plus_expr_projection_or_raw_leaf(&rendered),
        other => other,
    };
    let receiver = parenthesize_raw_method_receiver(receiver);
    match projection {
        OptionalReceiverProjection::Shared => method_expect_projection(
            zero_arg_method_projection(receiver, "as_ref"),
            "Java null receiver",
        ),
        OptionalReceiverProjection::Mutable => method_expect_projection(
            zero_arg_method_projection(receiver, "as_mut"),
            "Java null receiver",
        ),
        OptionalReceiverProjection::Borrowed => {
            method_expect_projection(receiver, "Java null receiver")
        }
        OptionalReceiverProjection::SharedUnchecked => {
            zero_arg_method_projection(zero_arg_method_projection(receiver, "as_ref"), "unwrap")
        }
        OptionalReceiverProjection::MutableUnchecked => {
            zero_arg_method_projection(zero_arg_method_projection(receiver, "as_mut"), "unwrap")
        }
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
    if let Some(rewritten) = rewrite_downcast_expect_message_projection(&expr, replacement_message)
    {
        return rewritten;
    }
    let RustExpr::Raw(rendered) = expr.unshaped() else {
        return expr;
    };
    let rendered = rendered.clone();
    if let Some(rewritten) =
        rendered_downcast_expect_message_projection(&rendered, replacement_message)
    {
        return rewritten;
    }
    let recovered = rendered_plus_expr_projection_or_raw_leaf(&rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_))
        && let Some(rewritten) =
            rewrite_downcast_expect_message_projection(&recovered, replacement_message)
    {
        return rewritten;
    }
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
    .map(|rendered| rendered_projection_or_raw_leaf(&rendered))
    .unwrap_or(expr)
}

fn rendered_downcast_expect_message_projection(
    rendered: &str,
    replacement_message: &'static str,
) -> Option<RustExpr> {
    let (expect_function, expect_args) = rendered_call_parts(rendered.trim())?;
    let (downcast_expr, expect_method) = rendered_method_call_target(expect_function)?;
    if expect_method != "expect"
        || expect_args.trim() != format!("{:?}", crate::output_markers::INVALID_JAVA_CAST_MESSAGE)
    {
        return None;
    }

    let (downcast_function, downcast_args) = rendered_call_parts(downcast_expr)?;
    let (receiver, downcast_method) = rendered_method_call_target(downcast_function)?;
    if !downcast_args.trim().is_empty() || !downcast_method.starts_with("downcast::<") {
        return None;
    }

    let receiver = rendered_projection_or_raw_leaf(receiver);
    if matches!(receiver.unshaped(), RustExpr::Raw(_)) {
        return None;
    }
    Some(method_expect_projection(
        zero_arg_method_projection(parenthesize_raw_method_receiver(receiver), downcast_method),
        replacement_message,
    ))
}

fn rewrite_downcast_expect_message_projection(
    expr: &RustExpr,
    replacement_message: &'static str,
) -> Option<RustExpr> {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "expect"
            && matches!(args.as_slice(), [RustExpr::Literal(message)] if message == &format!("{:?}", crate::output_markers::INVALID_JAVA_CAST_MESSAGE))
            && rust_expr_contains_downcast_call(receiver) =>
        {
            Some(RustExpr::MethodCall {
                receiver: receiver.clone(),
                method: method.clone(),
                args: vec![RustExpr::Literal(format!("{replacement_message:?}"))],
            })
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let rewritten_receiver =
                rewrite_downcast_expect_message_projection(receiver, replacement_message);
            let mut changed = rewritten_receiver.is_some();
            let rewritten_args = args
                .iter()
                .map(|arg| {
                    rewrite_downcast_expect_message_projection(arg, replacement_message)
                        .inspect(|_| changed = true)
                        .unwrap_or_else(|| arg.clone())
                })
                .collect::<Vec<_>>();
            changed.then(|| RustExpr::MethodCall {
                receiver: Box::new(rewritten_receiver.unwrap_or_else(|| receiver.as_ref().clone())),
                method: method.clone(),
                args: rewritten_args,
            })
        }
        RustExpr::Call { function, args } => {
            let rewritten_function =
                rewrite_downcast_expect_message_projection(function, replacement_message);
            let mut changed = rewritten_function.is_some();
            let rewritten_args = args
                .iter()
                .map(|arg| {
                    rewrite_downcast_expect_message_projection(arg, replacement_message)
                        .inspect(|_| changed = true)
                        .unwrap_or_else(|| arg.clone())
                })
                .collect::<Vec<_>>();
            changed.then(|| RustExpr::Call {
                function: Box::new(rewritten_function.unwrap_or_else(|| function.as_ref().clone())),
                args: rewritten_args,
            })
        }
        RustExpr::Binary { op, left, right } => {
            if let Some(left) =
                rewrite_downcast_expect_message_projection(left, replacement_message)
            {
                return Some(RustExpr::Binary {
                    op: op.clone(),
                    left: Box::new(left),
                    right: right.clone(),
                });
            }
            rewrite_downcast_expect_message_projection(right, replacement_message).map(|right| {
                RustExpr::Binary {
                    op: op.clone(),
                    left: left.clone(),
                    right: Box::new(right),
                }
            })
        }
        RustExpr::Grouped(inner) => {
            rewrite_downcast_expect_message_projection(inner, replacement_message)
                .map(|rewritten| RustExpr::Grouped(Box::new(rewritten)))
        }
        RustExpr::Shaped {
            shape,
            actual,
            expr,
        } => {
            rewrite_downcast_expect_message_projection(expr, replacement_message).map(|rewritten| {
                RustExpr::Shaped {
                    shape: *shape,
                    actual: actual.clone(),
                    expr: Box::new(rewritten),
                }
            })
        }
        RustExpr::MarkedBlock { marker, expr } => {
            rewrite_downcast_expect_message_projection(expr, replacement_message).map(|rewritten| {
                RustExpr::MarkedBlock {
                    marker: marker.clone(),
                    expr: Box::new(rewritten),
                }
            })
        }
        RustExpr::PostfixComment { expr, comment } => {
            rewrite_downcast_expect_message_projection(expr, replacement_message).map(|rewritten| {
                RustExpr::PostfixComment {
                    expr: Box::new(rewritten),
                    comment: comment.clone(),
                }
            })
        }
        _ => None,
    }
}

fn rust_expr_contains_downcast_call(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            method.starts_with("downcast::<")
                || rust_expr_contains_downcast_call(receiver)
                || args.iter().any(rust_expr_contains_downcast_call)
        }
        RustExpr::Call { function, args } => {
            rust_expr_contains_downcast_call(function)
                || args.iter().any(rust_expr_contains_downcast_call)
        }
        RustExpr::Grouped(expr)
        | RustExpr::Shaped { expr, .. }
        | RustExpr::MarkedBlock { expr, .. }
        | RustExpr::PostfixComment { expr, .. } => rust_expr_contains_downcast_call(expr),
        _ => false,
    }
}

pub fn rewrite_optional_erased_borrowed_string_view_call_args(
    lowered: RustExpr,
    optional_string_locals: &[String],
    erased_string_cast_locals: &[String],
) -> RustExpr {
    if let Some(rewritten) = rewrite_optional_erased_borrowed_string_view_call_args_projection(
        &lowered,
        optional_string_locals,
        erased_string_cast_locals,
    ) {
        return rewritten;
    }
    let RustExpr::Raw(rendered) = lowered.unshaped() else {
        return lowered;
    };
    let mut rendered = rendered.clone();
    let mut changed = false;
    for name in optional_string_locals {
        let erased_string_box = emit::emit_expr(&optional_string_erased_box_projection(name));
        let borrowed_string_view = emit::emit_expr(&optional_string_borrowed_view_projection(name));
        if let Some(rewritten) =
            raw_replace_all_outside_literals(&rendered, &erased_string_box, &borrowed_string_view)
        {
            rendered = rewritten;
            changed = true;
        }
    }
    for name in erased_string_cast_locals {
        let consuming_view = format!(
            "{name}.unwrap().downcast::<String>(){}.as_str()",
            crate::output_markers::invalid_java_cast_expect()
        );
        let borrowed_view = emit::emit_expr(&optional_erased_string_borrowed_view_projection(name));
        if let Some(rewritten) =
            raw_replace_all_outside_literals(&rendered, &consuming_view, &borrowed_view)
        {
            rendered = rewritten;
            changed = true;
        }
    }
    if changed {
        rendered_projection_or_raw_leaf(&rendered)
    } else {
        lowered
    }
}

fn rewrite_optional_erased_borrowed_string_view_call_args_projection(
    lowered: &RustExpr,
    optional_string_locals: &[String],
    erased_string_cast_locals: &[String],
) -> Option<RustExpr> {
    if let RustExpr::Shaped {
        shape,
        actual,
        expr,
    } = lowered
    {
        return rewrite_optional_erased_borrowed_string_view_call_args_projection(
            expr,
            optional_string_locals,
            erased_string_cast_locals,
        )
        .map(|rewritten| RustExpr::Shaped {
            shape: *shape,
            actual: actual.clone(),
            expr: Box::new(rewritten),
        });
    }
    if let Some(rewritten) =
        optional_string_erased_box_map_string_view_projection(lowered, optional_string_locals)
    {
        return Some(rewritten);
    }
    if let Some(rewritten) =
        erased_string_cast_local_string_view_projection(lowered, erased_string_cast_locals)
    {
        return Some(rewritten);
    }
    if let Some(rewritten) = optional_erased_string_consuming_view_expr_projection(lowered) {
        return Some(rewritten);
    }
    match lowered {
        RustExpr::Grouped(expr) => {
            rewrite_optional_erased_borrowed_string_view_call_args_projection(
                expr,
                optional_string_locals,
                erased_string_cast_locals,
            )
            .map(|rewritten| RustExpr::Grouped(Box::new(rewritten)))
        }
        RustExpr::Unary { op, expr } => {
            rewrite_optional_erased_borrowed_string_view_call_args_projection(
                expr,
                optional_string_locals,
                erased_string_cast_locals,
            )
            .map(|rewritten| RustExpr::Unary {
                op: op.clone(),
                expr: Box::new(rewritten),
            })
        }
        RustExpr::Cast { expr, ty } => {
            rewrite_optional_erased_borrowed_string_view_call_args_projection(
                expr,
                optional_string_locals,
                erased_string_cast_locals,
            )
            .map(|rewritten| RustExpr::Cast {
                expr: Box::new(rewritten),
                ty: ty.clone(),
            })
        }
        RustExpr::Try(expr) => rewrite_optional_erased_borrowed_string_view_call_args_projection(
            expr,
            optional_string_locals,
            erased_string_cast_locals,
        )
        .map(|rewritten| RustExpr::Try(Box::new(rewritten))),
        RustExpr::Field(receiver, field) => {
            rewrite_optional_erased_borrowed_string_view_call_args_projection(
                receiver,
                optional_string_locals,
                erased_string_cast_locals,
            )
            .map(|rewritten| RustExpr::Field(Box::new(rewritten), field.clone()))
        }
        RustExpr::Index(target, index) => {
            let rewritten_target =
                rewrite_optional_erased_borrowed_string_view_call_args_projection(
                    target,
                    optional_string_locals,
                    erased_string_cast_locals,
                );
            let rewritten_index = rewrite_optional_erased_borrowed_string_view_call_args_projection(
                index,
                optional_string_locals,
                erased_string_cast_locals,
            );
            if rewritten_target.is_none() && rewritten_index.is_none() {
                return None;
            }
            Some(RustExpr::Index(
                Box::new(rewritten_target.unwrap_or_else(|| target.as_ref().clone())),
                Box::new(rewritten_index.unwrap_or_else(|| index.as_ref().clone())),
            ))
        }
        RustExpr::Binary { op, left, right } => {
            let rewritten_left = rewrite_optional_erased_borrowed_string_view_call_args_projection(
                left,
                optional_string_locals,
                erased_string_cast_locals,
            );
            let rewritten_right = rewrite_optional_erased_borrowed_string_view_call_args_projection(
                right,
                optional_string_locals,
                erased_string_cast_locals,
            );
            if rewritten_left.is_none() && rewritten_right.is_none() {
                return None;
            }
            Some(RustExpr::Binary {
                op: op.clone(),
                left: Box::new(rewritten_left.unwrap_or_else(|| left.as_ref().clone())),
                right: Box::new(rewritten_right.unwrap_or_else(|| right.as_ref().clone())),
            })
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            let rewritten_receiver =
                rewrite_optional_erased_borrowed_string_view_call_args_projection(
                    receiver,
                    optional_string_locals,
                    erased_string_cast_locals,
                );
            let mut changed = rewritten_receiver.is_some();
            let rewritten_args = args
                .iter()
                .map(|arg| {
                    rewrite_optional_erased_borrowed_string_view_call_args_projection(
                        arg,
                        optional_string_locals,
                        erased_string_cast_locals,
                    )
                    .inspect(|_| changed = true)
                    .unwrap_or_else(|| arg.clone())
                })
                .collect::<Vec<_>>();
            changed.then(|| RustExpr::MethodCall {
                receiver: Box::new(rewritten_receiver.unwrap_or_else(|| receiver.as_ref().clone())),
                method: method.clone(),
                args: rewritten_args,
            })
        }
        RustExpr::Call { function, args } => {
            let rewritten_function =
                rewrite_optional_erased_borrowed_string_view_call_args_projection(
                    function,
                    optional_string_locals,
                    erased_string_cast_locals,
                );
            let mut changed = rewritten_function.is_some();
            let rewritten_args = args
                .iter()
                .map(|arg| {
                    rewrite_optional_erased_borrowed_string_view_call_args_projection(
                        arg,
                        optional_string_locals,
                        erased_string_cast_locals,
                    )
                    .inspect(|_| changed = true)
                    .unwrap_or_else(|| arg.clone())
                })
                .collect::<Vec<_>>();
            changed.then(|| RustExpr::Call {
                function: Box::new(rewritten_function.unwrap_or_else(|| function.as_ref().clone())),
                args: rewritten_args,
            })
        }
        RustExpr::Raw(rendered) => optional_erased_string_consuming_view_projection(rendered)
            .or_else(|| {
                let recovered = rendered_plus_expr_projection_or_raw_leaf(rendered);
                (!matches!(recovered.unshaped(), RustExpr::Raw(_)))
                    .then(|| {
                        rewrite_optional_erased_borrowed_string_view_call_args_projection(
                            &recovered,
                            optional_string_locals,
                            erased_string_cast_locals,
                        )
                    })
                    .flatten()
            }),
        _ => None,
    }
}

fn optional_string_erased_box_map_string_view_projection(
    lowered: &RustExpr,
    optional_string_locals: &[String],
) -> Option<RustExpr> {
    let RustExpr::MethodCall {
        receiver,
        method,
        args,
    } = lowered.unshaped()
    else {
        return None;
    };
    if method != "map" {
        return None;
    }
    let RustExpr::Var(name) = receiver.unshaped() else {
        return None;
    };
    if !optional_string_locals.iter().any(|local| local == name) {
        return None;
    }
    let [closure] = args.as_slice() else {
        return None;
    };
    if !closure_boxes_cloned_param_as_erased_object(closure, "value") {
        return None;
    }
    Some(method_call_projection(
        zero_arg_method_projection(RustExpr::Var(name.clone()), "as_ref"),
        "map",
        vec![optional_string_borrowed_view_closure_projection()],
    ))
}

fn optional_string_erased_box_projection(name: &str) -> RustExpr {
    method_call_projection(
        RustExpr::Var(name.to_string()),
        "map",
        vec![single_param_closure_expr(
            "value",
            RustExpr::Cast {
                expr: Box::new(function_call_projection(
                    RustExpr::path(["Box", "new"]),
                    vec![zero_arg_method_projection(
                        RustExpr::Var("value".to_string()),
                        "clone",
                    )],
                )),
                ty: "Box<dyn std::any::Any>".to_string(),
            },
        )],
    )
}

fn optional_string_borrowed_view_projection(name: &str) -> RustExpr {
    method_call_projection(
        zero_arg_method_projection(RustExpr::Var(name.to_string()), "as_ref"),
        "map",
        vec![optional_string_borrowed_view_closure_projection()],
    )
}

fn optional_string_borrowed_view_closure_projection() -> RustExpr {
    single_param_closure_expr(
        "value",
        zero_arg_method_projection(RustExpr::Var("value".to_string()), "as_str"),
    )
}

fn closure_boxes_cloned_param_as_erased_object(closure: &RustExpr, param_name: &str) -> bool {
    let RustExpr::Closure { params, body, .. } = closure.unshaped() else {
        return false;
    };
    if !matches!(params.as_slice(), [param] if param.pattern == param_name) {
        return false;
    }
    let RustClosureBody::Expr(body) = body;
    let RustExpr::Cast { expr, ty } = body.as_ref().unshaped() else {
        return false;
    };
    if ty != "Box<dyn std::any::Any>" {
        return false;
    }
    let RustExpr::Call { function, args } = expr.as_ref().unshaped() else {
        return false;
    };
    matches!(function.unshaped(), RustExpr::Path(path) if path == &["Box".to_string(), "new".to_string()])
        && matches!(args.as_slice(), [RustExpr::MethodCall { receiver, method, args }]
            if method == "clone"
                && args.is_empty()
                && matches!(receiver.unshaped(), RustExpr::Var(name) if name == param_name))
}

fn erased_string_cast_local_string_view_projection(
    lowered: &RustExpr,
    erased_string_cast_locals: &[String],
) -> Option<RustExpr> {
    let name = optional_erased_string_consuming_view_local_name(lowered)?;
    if !erased_string_cast_locals.iter().any(|local| local == name) {
        return None;
    }
    Some(optional_erased_string_borrowed_view_projection(name))
}

fn optional_erased_string_consuming_view_expr_projection(lowered: &RustExpr) -> Option<RustExpr> {
    optional_erased_string_consuming_view_local_name(lowered)
        .map(optional_erased_string_borrowed_view_projection)
}

fn optional_erased_string_consuming_view_local_name(lowered: &RustExpr) -> Option<&str> {
    let RustExpr::MethodCall {
        receiver,
        method,
        args,
    } = lowered.unshaped()
    else {
        return None;
    };
    if method != "as_str" || !args.is_empty() {
        return None;
    }
    let RustExpr::MethodCall {
        receiver,
        method,
        args,
    } = receiver.unshaped()
    else {
        return None;
    };
    let invalid_cast_message = format!("{:?}", crate::output_markers::INVALID_JAVA_CAST_MESSAGE);
    if method != "expect"
        || !matches!(args.as_slice(), [RustExpr::Literal(message)]
            if message == &invalid_cast_message)
    {
        return None;
    }
    let RustExpr::MethodCall {
        receiver,
        method,
        args,
    } = receiver.unshaped()
    else {
        return None;
    };
    if method != "downcast::<String>" || !args.is_empty() {
        return None;
    }
    let RustExpr::MethodCall {
        receiver,
        method,
        args,
    } = receiver.unshaped()
    else {
        return None;
    };
    if method != "unwrap" || !args.is_empty() {
        return None;
    }
    let RustExpr::Var(name) = receiver.unshaped() else {
        return None;
    };
    Some(name)
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
    let recovered = recover_parseable_rendered_operand_projection(trimmed);
    if !(balanced_outer_parentheses_wrap_all(trimmed)
        || !raw_contains_top_level_binary_operator(trimmed))
    {
        RustExpr::Grouped(Box::new(recovered))
    } else {
        recovered
    }
}

pub fn unwrap_result_or_panic_uncaught_java_exception(expr: RustExpr) -> RustExpr {
    let panic_recovery = ".unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))";
    if let Some(expr) = recover_nested_result_try_operator(expr.clone(), panic_recovery) {
        return expr;
    }
    if rust_expr_is_result_panic_recovered(&expr) {
        return expr;
    }
    if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return method_call_projection(
            expr,
            "unwrap_or_else",
            vec![panic_uncaught_java_exception_closure_expr()],
        );
    }
    let RustExpr::Raw(rendered) = expr.unshaped() else {
        unreachable!("structured result recovery returns before raw fallback");
    };
    let rendered = rendered.clone();
    // Already recovered: re-applying would splice a second copy into the `{:?}`
    // of the recovery text this function previously appended.
    if rendered.ends_with(panic_recovery) {
        return panic_recovered_result_expr_or_raw_leaf(rendered);
    }
    let recovered = rendered_plus_expr_projection_or_raw_leaf(&rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
        return unwrap_result_or_panic_uncaught_java_exception(recovered);
    }
    method_call_projection(
        recovered,
        "unwrap_or_else",
        vec![panic_uncaught_java_exception_closure_expr()],
    )
}

fn panic_recovered_result_expr_or_raw_leaf(rendered: String) -> RustExpr {
    rendered_projection_or_raw_leaf(&rendered)
}

fn rust_expr_is_result_panic_recovered(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall { method, args, .. } => {
            method == "unwrap_or_else"
                && matches!(args.as_slice(), [arg] if arg == &panic_uncaught_java_exception_closure_expr())
        }
        _ => false,
    }
}

fn panic_uncaught_java_exception_closure_expr() -> RustExpr {
    single_param_closure_expr("ex", panic_uncaught_java_exception_expr())
}

pub fn if_err_panic_uncaught_java_exception_stmt(expr: RustExpr) -> RustStmt {
    RustStmt::IfLet {
        pattern: "Err(ex)".to_string(),
        expr,
        then_body: vec![RustStmt::Expr(panic_uncaught_java_exception_expr())],
        else_body: Vec::new(),
    }
}

fn panic_uncaught_java_exception_expr() -> RustExpr {
    RustExpr::MacroCall {
        macro_name: "panic".to_string(),
        args: vec![
            RustExpr::Literal("\"uncaught Java exception: {:?}\"".to_string()),
            RustExpr::Var("ex".to_string()),
        ],
    }
}

fn recover_nested_result_try_operator(expr: RustExpr, recovery: &str) -> Option<RustExpr> {
    match expr {
        // Rewriting the inside cannot preserve what the tag asserted, so the
        // rewritten expression comes back untagged rather than mislabelled.
        RustExpr::Shaped { expr, .. } => recover_nested_result_try_operator(*expr, recovery),
        RustExpr::Raw(raw) => {
            let recovered = rendered_plus_expr_projection_or_raw_leaf(&raw);
            (!matches!(recovered.unshaped(), RustExpr::Raw(_)))
                .then(|| recover_nested_result_try_operator(recovered, recovery))
                .flatten()
        }
        RustExpr::Try(expr) => (recovery
            == ".unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))")
            .then(|| {
                let recovered_inner =
                    recover_nested_result_try_operator((*expr).clone(), recovery).unwrap_or(*expr);
                method_call_projection(
                    recovered_inner,
                    "unwrap_or_else",
                    vec![panic_uncaught_java_exception_closure_expr()],
                )
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
        RustExpr::Range {
            start,
            end,
            inclusive,
        } => {
            if let Some(start) = recover_nested_result_try_operator((*start).clone(), recovery) {
                return Some(RustExpr::Range {
                    start: Box::new(start),
                    end,
                    inclusive,
                });
            }
            recover_nested_result_try_operator(*end, recovery).map(|end| RustExpr::Range {
                start,
                end: Box::new(end),
                inclusive,
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
        RustExpr::Grouped(expr) => recover_nested_result_try_operator(*expr, recovery)
            .map(|expr| RustExpr::Grouped(Box::new(expr))),
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
        RustExpr::UnsupportedJavaArrayCreation {
            element_type,
            dimensions,
        } => recover_first_arg_try_operator(dimensions, recovery).map(|dimensions| {
            RustExpr::UnsupportedJavaArrayCreation {
                element_type,
                dimensions,
            }
        }),
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
        RustExpr::MacroCall { macro_name, args } => recover_first_arg_try_operator(args, recovery)
            .map(|args| RustExpr::MacroCall { macro_name, args }),
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
        RustExpr::PostfixComment { expr, comment } => {
            recover_nested_result_try_operator(*expr, recovery).map(|expr| {
                RustExpr::PostfixComment {
                    expr: Box::new(expr),
                    comment,
                }
            })
        }
        RustExpr::Closure {
            is_move,
            params,
            return_type,
            body,
        } => match body {
            RustClosureBody::Expr(expr) => {
                recover_nested_result_try_operator(*expr, recovery).map(|expr| RustExpr::Closure {
                    is_move,
                    params,
                    return_type,
                    body: RustClosureBody::Expr(Box::new(expr)),
                })
            }
        },
        RustExpr::Match { expr, arms } => {
            if let Some(expr) = recover_nested_result_try_operator((*expr).clone(), recovery) {
                return Some(RustExpr::Match {
                    expr: Box::new(expr),
                    arms,
                });
            }
            let mut updated = Vec::with_capacity(arms.len());
            let mut recovered = false;
            for arm in arms {
                if !recovered
                    && let Some(body) =
                        recover_nested_result_try_operator(arm.body.clone(), recovery)
                {
                    updated.push(crate::rust_ir::RustMatchArm { body, ..arm });
                    recovered = true;
                } else {
                    updated.push(arm);
                }
            }
            recovered.then_some(RustExpr::Match {
                expr,
                arms: updated,
            })
        }
        RustExpr::Block { stmts, tail } => {
            let mut updated_stmts = Vec::with_capacity(stmts.len());
            let mut recovered = false;
            for stmt in stmts {
                if !recovered
                    && let Some(stmt) =
                        recover_nested_result_try_operator_stmt(stmt.clone(), recovery)
                {
                    updated_stmts.push(stmt);
                    recovered = true;
                } else {
                    updated_stmts.push(stmt);
                }
            }
            if !recovered
                && let Some(tail_expr) = tail
                    .as_ref()
                    .and_then(|tail| recover_nested_result_try_operator((**tail).clone(), recovery))
            {
                return Some(RustExpr::Block {
                    stmts: updated_stmts,
                    tail: Some(Box::new(tail_expr)),
                });
            }
            recovered.then_some(RustExpr::Block {
                stmts: updated_stmts,
                tail,
            })
        }
        RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::Pattern(_) => None,
    }
}

fn recover_nested_result_try_operator_stmt(stmt: RustStmt, recovery: &str) -> Option<RustStmt> {
    match stmt {
        RustStmt::Let {
            mutable,
            name,
            ty,
            value: Some(value),
        } => recover_nested_result_try_operator(value, recovery).map(|value| RustStmt::Let {
            mutable,
            name,
            ty,
            value: Some(value),
        }),
        RustStmt::Assign { target, value } => {
            if let Some(target) = recover_nested_result_try_operator(target.clone(), recovery) {
                return Some(RustStmt::Assign { target, value });
            }
            recover_nested_result_try_operator(value, recovery)
                .map(|value| RustStmt::Assign { target, value })
        }
        RustStmt::Expr(expr) => {
            recover_nested_result_try_operator(expr, recovery).map(RustStmt::Expr)
        }
        RustStmt::Return(Some(expr)) => recover_nested_result_try_operator(expr, recovery)
            .map(|expr| RustStmt::Return(Some(expr))),
        RustStmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            if let Some(expr) = recover_nested_result_try_operator(expr.clone(), recovery) {
                return Some(RustStmt::IfLet {
                    pattern,
                    expr,
                    then_body,
                    else_body,
                });
            }
            let mut updated_then = Vec::with_capacity(then_body.len());
            let mut recovered = false;
            for stmt in then_body {
                if !recovered
                    && let Some(stmt) =
                        recover_nested_result_try_operator_stmt(stmt.clone(), recovery)
                {
                    updated_then.push(stmt);
                    recovered = true;
                } else {
                    updated_then.push(stmt);
                }
            }
            if recovered {
                return Some(RustStmt::IfLet {
                    pattern,
                    expr,
                    then_body: updated_then,
                    else_body,
                });
            }
            let mut updated_else = Vec::with_capacity(else_body.len());
            for stmt in else_body {
                if !recovered
                    && let Some(stmt) =
                        recover_nested_result_try_operator_stmt(stmt.clone(), recovery)
                {
                    updated_else.push(stmt);
                    recovered = true;
                } else {
                    updated_else.push(stmt);
                }
            }
            recovered.then_some(RustStmt::IfLet {
                pattern,
                expr,
                then_body: updated_then,
                else_body: updated_else,
            })
        }
        _ => None,
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
        return borrowed_string_literal_temporary_plan_from_parts(
            borrowed_string_optional_temporary_replacement(temporary_name),
            inner,
        );
    }
    if let Some(inner) = raw
        .strip_prefix("Some(&")
        .and_then(|rest| rest.strip_suffix(')'))
        && string_literal_to_string_expr(inner)
    {
        return borrowed_string_literal_temporary_plan_from_parts(
            borrowed_string_optional_temporary_replacement(temporary_name),
            inner,
        );
    }
    if let Some(inner) = raw.strip_prefix('&')
        && string_literal_to_string_expr(inner)
    {
        return borrowed_string_literal_temporary_plan_from_parts(
            borrowed_string_temporary_replacement(temporary_name),
            inner,
        );
    }
    None
}

fn borrowed_string_literal_temporary_plan_from_parts(
    replacement: RustExpr,
    initializer_expr: &str,
) -> Option<BorrowedStringLiteralTemporaryPlan> {
    Some(BorrowedStringLiteralTemporaryPlan {
        replacement,
        initializer: string_literal_to_string_projection(initializer_expr)?,
    })
}

fn borrowed_string_optional_temporary_replacement(temporary_name: &str) -> RustExpr {
    function_call_projection(
        RustExpr::Var("Some".to_string()),
        vec![borrowed_string_temporary_replacement(temporary_name)],
    )
}

fn borrowed_string_temporary_replacement(temporary_name: &str) -> RustExpr {
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(RustExpr::Var(temporary_name.to_string())),
    }
}

fn string_literal_to_string_expr(raw: &str) -> bool {
    string_literal_to_string_projection(raw).is_some()
}

fn string_literal_to_string_projection(raw: &str) -> Option<RustExpr> {
    let literal = raw.strip_suffix(".to_string()")?;
    if !literal.starts_with('"') {
        return None;
    }
    Some(zero_arg_method_projection(
        RustExpr::Literal(literal.to_string()),
        "to_string",
    ))
}

pub fn erased_object_box_projection(value: RustExpr) -> RustExpr {
    boxed_cast_projection(value, "Box<dyn std::any::Any>".to_string())
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
    boxed_cast_projection(value, format!("Box<dyn {trait_name}>"))
}

fn boxed_cast_projection(value: RustExpr, ty: String) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(function_call_projection(
            RustExpr::path(["Box", "new"]),
            vec![preserve_raw_tuple_operand(value)],
        )),
        ty,
    }
}

fn preserve_raw_tuple_operand(value: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = value else {
        return value;
    };
    let tuple_source = if raw_contains_top_level_comma(&raw) {
        Some(raw.as_str())
    } else {
        let trimmed = raw.trim();
        balanced_outer_parentheses_wrap_all(trimmed)
            .then(|| &trimmed[1..trimmed.len() - 1])
            .filter(|inner| raw_contains_top_level_comma(inner))
    };
    if let Some(tuple_source) = tuple_source {
        RustExpr::Tuple(split_raw_top_level_commas(tuple_source))
    } else {
        rendered_projection_or_raw_leaf(&raw)
    }
}

fn split_raw_top_level_commas(raw: &str) -> Vec<RustExpr> {
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            while let Some((next_idx, _)) = chars.peek() {
                if *next_idx >= end {
                    break;
                }
                chars.next();
            }
            continue;
        }
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
                items.push(raw_tuple_leaf_expr(&raw[start..idx]));
                start = idx + 1;
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
            _ => {}
        }
    }
    items.push(raw_tuple_leaf_expr(&raw[start..]));
    items
}

fn raw_tuple_leaf_expr(raw: &str) -> RustExpr {
    rendered_plus_expr_projection_or_raw_leaf(raw.trim())
}

pub fn unsupported_deserialization_cast_handoff_projection(
    value: RustExpr,
    java_name: &str,
) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: "_java_deserialized".to_string(),
            ty: None,
            value: Some(value),
        }],
        tail: Some(Box::new(panic_message_projection(format!(
            "unsupported Java deserialization cast to {java_name}; provide a typed serialization facade"
        )))),
    }
}

fn panic_message_projection(message: String) -> RustExpr {
    RustExpr::MacroCall {
        macro_name: "panic".to_string(),
        args: vec![rust_string_literal_projection(&message)],
    }
}

pub fn abstract_method_stub_projection(owner: &str, method: &str) -> RustExpr {
    panic_message_projection(format!(
        "abstract method {owner}.{method} has no implementation"
    ))
}

pub fn abstract_interface_method_stub_projection(owner: &str, method: &str) -> RustExpr {
    panic_message_projection(format!(
        "abstract interface method {owner}.{method} has no implementation"
    ))
}

pub fn unsupported_outer_capture_constructor_projection(owner: &str, outer_type: &str) -> RustExpr {
    panic_message_projection(format!(
        "{} for {owner}; pass an outer {outer_type} handle",
        crate::output_markers::UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE,
    ))
}

pub fn unsupported_constructor_call_handoff_projection(message: &str) -> RustExpr {
    RustExpr::PostfixComment {
        expr: Box::new(panic_message_projection(message.to_string())),
        comment:
            "/* TODO(translation): constructor call targets unsupported Java object graph mutation */"
                .to_string(),
    }
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
    if policy.optional {
        return method_call_projection(
            parenthesize_raw_method_receiver(expr),
            "map",
            vec![single_param_closure_expr(
                "bytes",
                unsigned_byte_array_conversion_pipeline(RustExpr::Var("bytes".to_string())),
            )],
        );
    }
    unsigned_byte_array_conversion_pipeline(parenthesize_raw_method_receiver(expr))
}

fn unsigned_byte_array_conversion_pipeline(expr: RustExpr) -> RustExpr {
    typed_collect_projection(
        method_call_projection(
            zero_arg_method_projection(expr, "into_iter"),
            "map",
            vec![single_param_closure_expr(
                "byte",
                RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("byte".to_string())),
                    ty: "i8".to_string(),
                },
            )],
        ),
        "Vec<_>",
    )
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
        option_some_projection(erased)
    } else {
        erased
    }
}

fn apply_optional_rust_borrowed_facade_return(
    expr: RustExpr,
    policy: &RustBorrowedFacadeReturnPolicy,
) -> RustExpr {
    let mapped = RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "map".to_string(),
        args: vec![boxed_erased_any_value_closure_expr(policy.recovery)],
    };
    if policy.target.optional_element_type().is_some() {
        mapped
    } else {
        method_expect_projection(mapped, "Java null receiver")
    }
}

fn boxed_erased_any_value_closure_expr(recovery: RustBorrowedFacadeReturnRecovery) -> RustExpr {
    let recovered = match recovery {
        RustBorrowedFacadeReturnRecovery::ToPathBuf => {
            zero_arg_method_projection(RustExpr::Var("value".to_string()), "to_path_buf")
        }
        RustBorrowedFacadeReturnRecovery::Clone => {
            zero_arg_method_projection(RustExpr::Var("value".to_string()), "clone")
        }
    };
    single_param_closure_expr("value", erased_object_box_projection(recovered))
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
        return zero_arg_method_projection(
            zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "try_into"),
            "unwrap",
        );
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
        BorrowProjectionUse::MutableSliceView => mutable_borrow_expr(expr),
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

pub fn apply_borrowed_string_view_projection_expr(
    rendered: &str,
    projection: &BorrowedStringViewProjection,
) -> RustExpr {
    match projection {
        BorrowedStringViewProjection::AlreadyStringView => {
            rendered_projection_or_raw_leaf(rendered.trim_start_matches('*'))
        }
        BorrowedStringViewProjection::LocalStringValue { rust_name } => string_view_expr(
            zero_arg_method_projection(RustExpr::Var(rust_name.clone()), "as_str"),
        ),
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
            zero_arg_method_projection(grouped_display_method_receiver(expr), "to_string")
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
            let display_operand = java_display_operand_projection(expr);
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
        JavaDisplayPlan::OptionalString => RustExpr::MethodCall {
            receiver: Box::new(zero_arg_method_projection(
                parenthesize_raw_method_receiver(expr),
                "as_deref",
            )),
            method: "unwrap_or".to_string(),
            args: vec![RustExpr::Literal("\"null\"".to_string())],
        },
        JavaDisplayPlan::OptionalValue => optional_value_display_projection(expr),
        JavaDisplayPlan::OptionalBorrowedValue => optional_borrowed_value_display_projection(expr),
    }
}

fn grouped_display_method_receiver(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(raw) => grouped_raw_leaf(raw),
        other => RustExpr::Grouped(Box::new(other)),
    }
}

fn optional_value_display_projection(expr: RustExpr) -> RustExpr {
    optional_display_chain_projection(zero_arg_method_projection(
        parenthesize_raw_method_receiver(recover_optional_display_receiver(expr)),
        "as_ref",
    ))
}

fn optional_borrowed_value_display_projection(expr: RustExpr) -> RustExpr {
    optional_display_chain_projection(parenthesize_raw_method_receiver(
        recover_optional_display_receiver(expr),
    ))
}

fn optional_display_chain_projection(expr: RustExpr) -> RustExpr {
    method_call_projection(
        method_call_projection(expr, "map", vec![param_to_string_closure_expr("value")]),
        "unwrap_or_else",
        vec![null_string_closure_expr()],
    )
}

pub fn render_java_display_operand(expr: &RustExpr) -> String {
    emit::emit_expr(&java_display_operand_projection(expr.clone()))
}

fn recover_optional_display_receiver(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => rendered_plus_expr_projection_or_raw_leaf(&rendered),
        other => other,
    }
}

pub(crate) fn java_display_operand_projection(expr: RustExpr) -> RustExpr {
    if let Some(receiver) = arithmetic_display_conversion_receiver_expr(&expr) {
        return receiver;
    }
    expr
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
            RustExpr::Raw(rendered) if rendered_expr_has_direct_clone_recovery(rendered)
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
    clone_recovery_projection(expr.clone()).is_some()
}

fn clone_recovery_projection(expr: RustExpr) -> Option<RustExpr> {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery)
        || rust_expr_has_clone_recovery_projection(&expr)
    {
        return Some(expr);
    }
    if let RustExpr::Raw(rendered) = expr.unshaped() {
        if let Some(recovered) = recover_rendered_clone_recovery_projection(rendered) {
            return Some(recovered);
        }
        if rendered_expr_has_clone_recovery(rendered) {
            return Some(expr);
        }
    }
    None
}

fn recover_rendered_clone_recovery_projection(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    (!matches!(recovered.unshaped(), RustExpr::Raw(_)) && rust_expr_has_clone_recovery(&recovered))
        .then_some(recovered)
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
    if let Some(snapshot) = snapshot_shared_borrow_argument_projection(&lowered, name) {
        return snapshot;
    }
    let RustExpr::Raw(rendered) = lowered.unshaped() else {
        return lowered;
    };
    let recovered = rendered_projection_or_raw_leaf(rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_))
        && let Some(snapshot) = snapshot_shared_borrow_argument_projection(&recovered, name)
    {
        return snapshot;
    }
    recovered
}

fn snapshot_shared_borrow_argument_projection(lowered: &RustExpr, name: &str) -> Option<RustExpr> {
    if rust_expr_is_option_some_of(lowered, |inner| {
        rust_expr_is_shared_borrow_of_name(inner, name)
    }) {
        return Some(option_some_projection(snapshot_shared_borrow_expr(name)));
    }
    if rust_expr_is_shared_borrow_of_name(lowered, name) {
        return Some(snapshot_shared_borrow_expr(name));
    }
    if let Some(method) = rust_expr_string_or_slice_view_method_of_name(lowered, name) {
        return Some(zero_arg_method_projection(
            snapshot_clone_expr(name),
            method,
        ));
    }
    if let Some(method) = rust_expr_option_string_or_slice_view_method_of_name(lowered, name) {
        return Some(option_some_projection(zero_arg_method_projection(
            snapshot_clone_expr(name),
            method,
        )));
    }
    None
}

fn rust_expr_is_option_some_of(
    expr: &RustExpr,
    inner_matches: impl FnOnce(&RustExpr) -> bool,
) -> bool {
    match expr.unshaped() {
        RustExpr::Call { function, args } if args.len() == 1 => {
            matches!(function.unshaped(), RustExpr::Path(path) if path.as_slice() == ["Some"])
                && inner_matches(&args[0])
        }
        _ => false,
    }
}

fn rust_expr_is_shared_borrow_of_name(expr: &RustExpr, name: &str) -> bool {
    match expr.unshaped() {
        RustExpr::Unary { op, expr } if op == "&" => {
            matches!(expr.unshaped(), RustExpr::Var(var) if var == name)
        }
        _ => false,
    }
}

fn rust_expr_string_or_slice_view_method_of_name<'a>(
    expr: &'a RustExpr,
    name: &str,
) -> Option<&'a str> {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if args.is_empty()
            && matches!(method.as_str(), "as_slice" | "as_str")
            && matches!(receiver.unshaped(), RustExpr::Var(var) if var == name) =>
        {
            Some(method)
        }
        _ => None,
    }
}

fn rust_expr_option_string_or_slice_view_method_of_name<'a>(
    expr: &'a RustExpr,
    name: &str,
) -> Option<&'a str> {
    match expr.unshaped() {
        RustExpr::Call { function, args } if args.len() == 1 => {
            if !matches!(function.unshaped(), RustExpr::Path(path) if path.as_slice() == ["Some"]) {
                return None;
            }
            rust_expr_string_or_slice_view_method_of_name(&args[0], name)
        }
        _ => None,
    }
}

fn snapshot_shared_borrow_expr(name: &str) -> RustExpr {
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(snapshot_clone_expr(name)),
    }
}

fn snapshot_clone_expr(name: &str) -> RustExpr {
    RustExpr::PostfixComment {
        expr: Box::new(zero_arg_method_projection(
            RustExpr::Var(name.to_string()),
            "clone",
        )),
        comment: "/* TODO(translation): validate same-call alias snapshot */".to_string(),
    }
}

pub fn apply_audited_clone_recovery(expr: RustExpr) -> RustExpr {
    apply_clone_recovery(expr, &CloneRecoveryPolicy::AuditRequired)
}

pub fn apply_clone_projection(expr: RustExpr) -> RustExpr {
    let receiver = if let RustExpr::Raw(rendered) = expr.unshaped() {
        let rendered = rendered.clone();
        clone_method_receiver(
            expr,
            &rendered,
            true,
            rust_method_receiver_needs_parentheses,
        )
    } else {
        expr
    };
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        RustExpr::MethodCall {
            receiver: Box::new(receiver),
            method: "clone".to_string(),
            args: Vec::new(),
        },
    )
}

fn clone_method_receiver(
    expr: RustExpr,
    rendered: &str,
    string_literals_as_literals: bool,
    receiver_needs_parentheses: impl Fn(&str) -> bool,
) -> RustExpr {
    let RustExpr::Raw(raw) = &expr else {
        return expr;
    };
    let trimmed = rendered.trim();
    if rendered_expr_is_complete_string_literal(trimmed) {
        return if string_literals_as_literals {
            RustExpr::Literal(raw.clone())
        } else {
            expr
        };
    }
    if (receiver_needs_parentheses(trimmed) || raw_contains_top_level_binary_operator(trimmed))
        && !balanced_outer_parentheses_wrap_all(trimmed)
    {
        grouped_raw_leaf(trimmed.to_string())
    } else {
        let projected = rendered_projection_or_raw_leaf(trimmed);
        if matches!(projected.unshaped(), RustExpr::Raw(_)) {
            expr
        } else {
            projected
        }
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
    if prefix {
        java_prefix_update_block_projection(target, update_op)
    } else {
        java_postfix_update_block_projection(target, update_op)
    }
}

pub fn java_primitive_update_expression_projection(
    target: RustExpr,
    increment: bool,
    primitive: PrimitiveType,
    prefix: bool,
) -> Option<RustExpr> {
    let value = java_primitive_updated_value_projection(target.clone(), increment, primitive)?;
    if prefix {
        Some(java_prefix_assignment_update_block_projection(
            target, value,
        ))
    } else {
        Some(java_postfix_assignment_update_block_projection(
            target, value,
        ))
    }
}

fn java_prefix_update_block_projection(target: RustExpr, update_op: &str) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::AssignOp {
            target: target.clone(),
            op: update_op.to_string(),
            value: RustExpr::Literal("1".to_string()),
        }],
        tail: Some(Box::new(target)),
    }
}

fn java_postfix_update_block_projection(target: RustExpr, update_op: &str) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: "__java_old_value".to_string(),
                ty: None,
                value: Some(target.clone()),
            },
            RustStmt::AssignOp {
                target,
                op: update_op.to_string(),
                value: RustExpr::Literal("1".to_string()),
            },
        ],
        tail: Some(Box::new(RustExpr::Var("__java_old_value".to_string()))),
    }
}

fn java_prefix_assignment_update_block_projection(target: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Assign {
            target: target.clone(),
            value,
        }],
        tail: Some(Box::new(target)),
    }
}

fn java_postfix_assignment_update_block_projection(target: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: "__java_old_value".to_string(),
                ty: None,
                value: Some(target.clone()),
            },
            RustStmt::Assign { target, value },
        ],
        tail: Some(Box::new(RustExpr::Var("__java_old_value".to_string()))),
    }
}

pub fn java_primitive_updated_value_projection(
    target: RustExpr,
    increment: bool,
    primitive: PrimitiveType,
) -> Option<RustExpr> {
    let method = if increment {
        "wrapping_add"
    } else {
        "wrapping_sub"
    };
    match primitive {
        PrimitiveType::Byte | PrimitiveType::Short | PrimitiveType::Int | PrimitiveType::Long => {
            Some(receiver_method_call_projection_exact(
                parenthesized_update_method_receiver(target),
                method,
                vec![RustExpr::Literal("1".to_string())],
            ))
        }
        PrimitiveType::Char => Some(java_char_from_u32_mask_projection(
            char_update_wrapping_value_projection(target, method),
        )),
        PrimitiveType::Float | PrimitiveType::Double | PrimitiveType::Boolean => None,
    }
}

fn char_update_wrapping_value_projection(target: RustExpr, method: &str) -> RustExpr {
    let target_as_u32 = RustExpr::Cast {
        expr: Box::new(parenthesize_raw_primitive_number_cast_operand(target)),
        ty: "u32".to_string(),
    };
    let u32_as_i32 = RustExpr::Cast {
        expr: Box::new(RustExpr::Grouped(Box::new(target_as_u32))),
        ty: "i32".to_string(),
    };
    let wrapped = receiver_method_call_projection_exact(
        RustExpr::Grouped(Box::new(u32_as_i32)),
        method,
        vec![RustExpr::Literal("1".to_string())],
    );
    wrapped
}

fn parenthesized_update_method_receiver(target: RustExpr) -> RustExpr {
    match target {
        RustExpr::Raw(raw) => grouped_raw_leaf(raw),
        other => RustExpr::Grouped(Box::new(other)),
    }
}

pub fn exception_self_as_supertype_facade_boundary_projection() -> RustExpr {
    RustExpr::PostfixComment {
        expr: Box::new(RustExpr::Call {
            function: Box::new(RustExpr::path(["Default", "default"])),
            args: Vec::new(),
        }),
        comment: "/* TODO(translation): Java exception self-as-supertype facade boundary */"
            .to_string(),
    }
}

pub fn explicit_java_facade_cast_placeholder_projection(
    source: RustExpr,
    rust_type: &str,
) -> RustExpr {
    RustExpr::PostfixComment {
        expr: Box::new(RustExpr::Block {
            stmts: vec![RustStmt::Let {
                mutable: false,
                name: "_java_facade_cast_source".to_string(),
                ty: None,
                value: Some(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Grouped(Box::new(source))),
                }),
            }],
            tail: Some(Box::new(rust_type_default_call_expr(rust_type))),
        }),
        comment: "/* TODO(translation): validate explicit Java facade cast */".to_string(),
    }
}

fn rust_type_default_call_expr(rust_type: &str) -> RustExpr {
    RustExpr::Call {
        function: Box::new(associated_rust_path_expr(rust_type, "default")),
        args: Vec::new(),
    }
}

#[cfg(test)]
pub fn render_audited_clone_recovery(rendered: &str) -> String {
    render_clone_recovery_expr(rendered, &CloneRecoveryPolicy::AuditRequired)
}

fn arithmetic_display_conversion_receiver_expr(expr: &RustExpr) -> Option<RustExpr> {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "to_string" && args.is_empty() => arithmetic_display_receiver_expr(receiver),
        RustExpr::Raw(rendered) => {
            recover_rendered_arithmetic_display_conversion_receiver(rendered)
        }
        _ => None,
    }
}

fn recover_rendered_arithmetic_display_conversion_receiver(rendered: &str) -> Option<RustExpr> {
    raw_arithmetic_display_conversion_receiver(rendered)
        .map(|receiver| rendered_plus_expr_projection_or_raw_leaf(&receiver))
}

fn arithmetic_display_receiver_expr(receiver: &RustExpr) -> Option<RustExpr> {
    match receiver.unshaped() {
        RustExpr::Grouped(inner) => arithmetic_display_receiver_expr(inner),
        _ if rust_expr_is_arithmetic_expr(receiver) => {
            Some(RustExpr::Grouped(Box::new(receiver.clone())))
        }
        RustExpr::Raw(rendered)
            if balanced_outer_parentheses_wrap_all(rendered.trim())
                && raw_contains_top_level_arithmetic_operator(
                    &rendered.trim()[1..rendered.trim().len() - 1],
                ) =>
        {
            grouped_rendered_inner_projection(rendered.trim())
        }
        RustExpr::Raw(rendered) if raw_contains_top_level_arithmetic_operator(rendered.trim()) => {
            recover_rendered_arithmetic_display_receiver(rendered)
        }
        _ => None,
    }
}

fn recover_rendered_arithmetic_display_receiver(rendered: &str) -> Option<RustExpr> {
    Some(rendered_plus_expr_projection_or_raw_leaf(rendered.trim()))
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
        expr.unshaped(),
        RustExpr::Unary { op, .. } if op == "&" || op == "&mut "
    ) {
        return true;
    }
    if rust_expr_is_raw_borrow_leaf(expr) {
        return true;
    }
    match use_case {
        BorrowProjectionUse::MapKey => {
            rust_expr_is_already_borrowed_map_key(expr)
                || actual.is_some_and(map_key_shape_is_borrowed_string_view)
                || (actual.is_some_and(|shape| shape.ownership == OwnershipShape::SharedBorrow)
                    && rust_expr_is_simple_local_path(expr))
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

fn rust_expr_is_raw_borrow_leaf(expr: &RustExpr) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::Raw(rendered) if rendered.trim_start().starts_with('&')
    )
}

fn borrow_expr(expr: RustExpr) -> RustExpr {
    shared_borrow_expr(expr)
}

pub(crate) fn shared_borrow_expr(expr: RustExpr) -> RustExpr {
    RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(borrow_operand_expr(expr)),
    }
}

fn mutable_borrow_expr(expr: RustExpr) -> RustExpr {
    RustExpr::Unary {
        op: "&mut ".to_string(),
        expr: Box::new(borrow_operand_expr(expr)),
    }
}

fn borrow_operand_expr(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(raw) => parenthesized_borrow_operand_leaf(&raw),
        other => other,
    }
}

fn parenthesized_borrow_operand_leaf(rendered: &str) -> RustExpr {
    let recovered = rendered_plus_expr_projection_or_raw_leaf(rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
        return parenthesized_borrow_operand_expr(recovered);
    }
    RustExpr::raw_leaf(rendered.trim().to_string())
}

fn parenthesized_borrow_operand_expr(expr: RustExpr) -> RustExpr {
    if rust_expr_is_prefix_safe_borrow_operand(&expr) {
        expr
    } else {
        RustExpr::Grouped(Box::new(expr))
    }
}

fn rust_expr_is_prefix_safe_borrow_operand(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Literal(rendered) => rendered_expr_is_complete_string_literal(rendered),
        RustExpr::Var(_) | RustExpr::Path(_) | RustExpr::Field(_, _) | RustExpr::Index(_, _) => {
            true
        }
        RustExpr::GenericPath { .. } | RustExpr::TypeAssociatedPath { .. } => true,
        RustExpr::Grouped(_) => true,
        _ => false,
    }
}

fn rendered_expr_is_complete_string_literal(rendered: &str) -> bool {
    normal_string_literal_end_at(rendered, 0) == Some(rendered.len())
        || raw_string_literal_end_at(rendered, 0) == Some(rendered.len())
}

fn rendered_expr_is_complete_char_literal(rendered: &str) -> bool {
    normal_char_literal_end_at(rendered, 0) == Some(rendered.len())
}

fn normal_char_literal_end_at(raw: &str, idx: usize) -> Option<usize> {
    let rest = raw.get(idx..)?;
    if !rest.starts_with('\'') {
        return None;
    }
    let mut escaped = false;
    let mut saw_body = false;
    for (offset, ch) in rest[1..].char_indices() {
        if escaped {
            escaped = false;
            saw_body = true;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '\'' if saw_body => return Some(idx + 1 + offset + ch.len_utf8()),
            '\'' => return None,
            _ => saw_body = true,
        }
    }
    None
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

fn rendered_expr_is_already_borrowed_map_key(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()") || rendered_expr_is_complete_string_literal(trimmed)
}

fn rust_expr_is_already_borrowed_map_key(expr: &RustExpr) -> bool {
    if expr.recorded_shape() == Some(RustExprShape::StringView) {
        return true;
    }
    match expr.unshaped() {
        RustExpr::Literal(literal) => rendered_expr_is_complete_string_literal(literal),
        RustExpr::MethodCall { method, args, .. } if method == "as_str" && args.is_empty() => true,
        RustExpr::Call { function, .. } => rust_expr_is_map_key_string_helper(function),
        RustExpr::Raw(rendered) => {
            let recovered = rendered_projection_or_raw_leaf(rendered);
            if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
                rust_expr_is_already_borrowed_map_key(&recovered)
            } else {
                rendered_expr_is_already_borrowed_map_key(rendered)
            }
        }
        _ => false,
    }
}

fn rust_expr_is_map_key_string_helper(function: &RustExpr) -> bool {
    matches!(function.unshaped(), RustExpr::Path(segments) if segments.as_slice() == ["java_string_substring"])
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

fn rust_expr_is_simple_local_path(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Var(_) => true,
        RustExpr::Raw(rendered) => rendered_expr_is_simple_local_path(rendered),
        _ => false,
    }
}

fn rendered_expr_is_associated_path_without_call(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.contains("::")
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '#'))
}

fn expr_is_associated_path_without_call(expr: &RustExpr) -> bool {
    matches!(expr.unshaped(), RustExpr::Path(segments) if segments.len() > 1)
        || matches!(
            expr.unshaped(),
            RustExpr::Raw(rendered) if rendered_expr_is_associated_path_without_call(rendered)
        )
}

fn associated_path_view_projection(expr: RustExpr) -> RustExpr {
    match expr.into_unshaped() {
        path @ RustExpr::Path(_) => path,
        RustExpr::Raw(rendered) => external_facade_rust_path_projection(&rendered),
        other => other,
    }
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
            RustExpr::Raw(rendered) if raw_expr_is_string_view(rendered)
        )
}

fn rust_expr_is_string_view_projection(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall { .. } if rust_expr_is_zero_arg_method_call(expr, "as_str") => true,
        RustExpr::Literal(literal) => rendered_expr_is_complete_string_literal(literal),
        _ => false,
    }
}

fn expr_is_owned_string_recovery(expr: &RustExpr) -> bool {
    owned_string_recovery_projection(expr.clone()).is_some()
}

fn owned_string_recovery_projection(expr: RustExpr) -> Option<RustExpr> {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery)
        || rust_expr_is_zero_arg_method_call(&expr, "to_string")
    {
        return Some(expr);
    }
    if let RustExpr::Raw(rendered) = expr.unshaped() {
        if let Some(recovered) = recover_rendered_owned_string_recovery_projection(rendered) {
            return Some(recovered);
        }
        if let Some(to_string_call) = terminal_raw_to_string_call_projection(rendered) {
            return Some(to_string_call);
        }
        if rendered_expr_is_already_owned_string_recovery(rendered) {
            return Some(expr);
        }
    }
    None
}

fn recover_rendered_owned_string_recovery_projection(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    (!matches!(recovered.unshaped(), RustExpr::Raw(_)) && expr_is_owned_string_recovery(&recovered))
        .then_some(recovered)
}

fn rust_expr_is_zero_arg_method_call(expr: &RustExpr, expected_method: &str) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::MethodCall { method, args, .. } if method == expected_method && args.is_empty()
    )
}

/// The text-only form of [`expr_is_string_view`], for consumers that never had
/// the expression — only its rendering.
pub fn rendered_expr_is_already_string_view(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()") || rendered_expr_is_complete_string_literal(trimmed)
}

fn rendered_expr_is_already_owned_string_recovery(rendered: &str) -> bool {
    rendered.trim().ends_with(".to_string()")
}

fn raw_expr_is_string_view(rendered: &str) -> bool {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
        expr_is_string_view(&recovered)
    } else {
        rendered_expr_is_already_string_view(rendered)
    }
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

fn deref_string_view_projection(expr: &RustExpr) -> Option<RustExpr> {
    match expr.unshaped() {
        RustExpr::Unary { op, expr: inner } if op == "*" => {
            Some(string_view_expr(zero_arg_method_projection(
                parenthesize_raw_method_receiver(inner.as_ref().clone()),
                "as_str",
            )))
        }
        RustExpr::Raw(rendered) => raw_deref_string_view_projection(rendered),
        _ => None,
    }
}

fn raw_deref_string_view_projection(rendered: &str) -> Option<RustExpr> {
    if let Some(borrowed_item) = simple_deref_projection_source(rendered) {
        return Some(borrowed_item_string_view_projection(borrowed_item));
    }
    rendered
        .trim()
        .strip_prefix('*')
        .map(borrowed_item_string_view_projection)
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
    instanceof_check_projection(
        expr,
        &crate::conversion_plan::plan_instanceof_check(actual, target),
    )
}

pub fn instanceof_check_projection(expr: RustExpr, check: &InstanceOfCheck) -> RustExpr {
    match check {
        InstanceOfCheck::JavaSerializableMarker => function_call_projection(
            RustExpr::path(["java_is_serializable"]),
            vec![force_shared_borrow_projection(expr)],
        ),
        InstanceOfCheck::ClosedEnumVariant {
            enum_name,
            variant_name,
        } => RustExpr::MacroCall {
            macro_name: "matches".to_string(),
            args: vec![
                expr,
                closed_enum_variant_pattern_leaf(enum_name, variant_name),
            ],
        },
        InstanceOfCheck::DynTraitType { target_type } => typed_zero_arg_method_projection(
            zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_any"),
            "is",
            target_type,
        ),
        InstanceOfCheck::BoxedDynTraitType { target_type } => typed_zero_arg_method_projection(
            zero_arg_method_projection(
                zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
                "as_any",
            ),
            "is",
            target_type,
        ),
        InstanceOfCheck::ErasedObjectType { target_type } => {
            erased_object_is_type_projection(expr, target_type)
        }
        InstanceOfCheck::OptionalErasedObjectType { target_type } => method_call_projection(
            zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
            "is_some_and",
            vec![optional_erased_object_is_instance_closure_leaf(target_type)],
        ),
        InstanceOfCheck::AlwaysTrue => RustExpr::Literal("true".to_string()),
        InstanceOfCheck::Unsupported {
            actual_dispatch,
            target_dispatch,
            policy,
        } => render_instanceof_fallback_policy(policy, actual_dispatch, target_dispatch),
    }
}

fn closed_enum_variant_pattern_leaf(enum_name: &str, variant_name: &str) -> RustExpr {
    RustExpr::Pattern(crate::rust_ir::RustPattern::TupleVariantWildcard {
        enum_name: enum_name.to_string(),
        variant_name: variant_name.to_string(),
    })
}

fn optional_erased_object_is_instance_closure_leaf(target_type: &str) -> RustExpr {
    single_param_closure_expr(
        "value",
        erased_object_is_type_projection(RustExpr::Var("value".to_string()), target_type),
    )
}

pub fn instanceof_pattern_downcast_ref_projection(
    expr: RustExpr,
    check: &InstanceOfCheck,
) -> Option<RustExpr> {
    match check {
        InstanceOfCheck::ClosedEnumVariant {
            enum_name,
            variant_name,
        } => Some(closed_enum_pattern_downcast_ref_projection(
            expr,
            enum_name,
            variant_name,
        )),
        InstanceOfCheck::DynTraitType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(dyn_trait_downcast_ref_projection(expr, &target_type))
        }
        InstanceOfCheck::BoxedDynTraitType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(boxed_dyn_trait_downcast_ref_projection(expr, &target_type))
        }
        InstanceOfCheck::ErasedObjectType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(erased_object_downcast_ref_projection(
                parenthesize_raw_method_receiver(expr),
                &target_type,
            ))
        }
        InstanceOfCheck::OptionalErasedObjectType { target_type } => {
            let target_type = normalize_emitted_java_type_name(target_type);
            Some(method_call_projection(
                zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
                "and_then",
                vec![single_param_closure_expr(
                    "value",
                    erased_object_downcast_ref_projection(
                        RustExpr::Var("value".to_string()),
                        &target_type,
                    ),
                )],
            ))
        }
        _ => None,
    }
}

fn closed_enum_pattern_downcast_ref_projection(
    expr: RustExpr,
    enum_name: &str,
    variant_name: &str,
) -> RustExpr {
    RustExpr::Match {
        expr: Box::new(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(parenthesize_raw_method_receiver(expr)),
        }),
        arms: vec![
            RustMatchArm {
                pattern: format!("{enum_name}::{variant_name}(value)"),
                body: RustExpr::Call {
                    function: Box::new(RustExpr::path(["Some"])),
                    args: vec![RustExpr::Var("value".to_string())],
                },
            },
            RustMatchArm {
                pattern: "_".to_string(),
                body: RustExpr::Path(vec!["None".to_string()]),
            },
        ],
    }
}

fn closed_enum_downcast_projection(
    expr: RustExpr,
    enum_name: &str,
    variant_name: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    RustExpr::Match {
        expr: Box::new(expr),
        arms: vec![
            RustMatchArm {
                pattern: format!("{enum_name}::{variant_name}(value)"),
                body: RustExpr::Var("value".to_string()),
            },
            RustMatchArm {
                pattern: "_".to_string(),
                body: invalid_cast_policy_expr(invalid_cast),
            },
        ],
    }
}

fn dyn_trait_downcast_ref_projection(expr: RustExpr, target_type: &str) -> RustExpr {
    typed_zero_arg_method_projection(
        zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_any"),
        "downcast_ref",
        target_type,
    )
}

fn boxed_dyn_trait_downcast_ref_projection(expr: RustExpr, target_type: &str) -> RustExpr {
    dyn_trait_downcast_ref_projection(
        zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
        target_type,
    )
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
    let expr = recover_borrowed_element_slot_adapter_input(expr);
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

fn recover_borrowed_element_slot_adapter_input(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => recover_rendered_method_receiver_expr(&rendered)
            .unwrap_or_else(|| RustExpr::Raw(rendered)),
        other => other,
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
    match (adapter.carrier, policy.target.ownership) {
        (BorrowedElementCarrier::Option, OwnershipShape::SharedBorrow)
            if adapter.element_shape.ownership == OwnershipShape::SharedBorrow =>
        {
            optional_borrowed_element_projected_expr(
                method_expect_projection(
                    parenthesize_raw_method_receiver(expr),
                    "Java null receiver",
                ),
                &adapter.element_shape,
                adapter.element_shape.ownership,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow)
            if adapter.element_shape.ownership == OwnershipShape::MutableBorrow =>
        {
            optional_borrowed_element_projected_expr(
                method_expect_projection(
                    parenthesize_raw_method_receiver(expr),
                    "Java null receiver",
                ),
                &adapter.element_shape,
                adapter.element_shape.ownership,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::SharedBorrow) => {
            optional_borrowed_element_projected_expr(
                method_expect_projection(
                    zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
                    "Java null receiver",
                ),
                &adapter.element_shape,
                OwnershipShape::SharedBorrow,
            )
        }
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow) => {
            optional_borrowed_element_projected_expr(
                method_expect_projection(
                    zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_mut"),
                    "Java null receiver",
                ),
                &adapter.element_shape,
                OwnershipShape::MutableBorrow,
            )
        }
        _ => method_expect_projection(expr, "Java null receiver"),
    }
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
                    args: vec![param_to_string_closure_expr("v")],
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
                args: vec![param_to_string_closure_expr("v")],
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
    RustExpr::MethodCall {
        receiver: Box::new(borrow_operand_expr(expr)),
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
                RustExpr::Cast {
                    expr: Box::new(expr),
                    ty: format!("&dyn {trait_name}"),
                },
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
                            borrow_concrete_facade_block_expr(expr, &target_type),
                        )
                    }
                }
                _ => expr,
            }
        }
        _ => expr,
    }
}

fn borrow_concrete_facade_block_expr(expr: RustExpr, target_type: &str) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: "_java_value".to_string(),
            ty: None,
            value: Some(expr),
        }],
        tail: Some(Box::new(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(external_facade_rust_path_projection(target_type)),
        })),
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
            if let RustExpr::Raw(rendered) = expr.unshaped()
                && let Some(borrowed_erased_view) =
                    optional_erased_string_downcast_ref_view_projection(rendered)
            {
                return Some(RustExpr::shaped_as(
                    RustExprShape::StringView,
                    Some(policy.target.clone()),
                    borrowed_erased_view,
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

fn optional_erased_string_downcast_ref_view_projection(rendered: &str) -> Option<RustExpr> {
    let rendered = rendered
        .strip_prefix("&(*")
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(rendered);
    let local_name = rendered.strip_suffix(&format!(
        ".unwrap().downcast::<String>(){}",
        crate::output_markers::invalid_java_cast_expect()
    ))?;
    if !rust_plain_identifier(local_name) {
        return None;
    }
    Some(optional_erased_string_borrowed_view_projection(local_name))
}

fn optional_erased_string_borrowed_view_projection(local_name: &str) -> RustExpr {
    zero_arg_method_projection(
        method_expect_projection(
            method_call_projection(
                zero_arg_method_projection(RustExpr::Var(local_name.to_string()), "as_ref"),
                "and_then",
                vec![single_param_closure_expr(
                    "value",
                    erased_object_downcast_ref_projection(
                        RustExpr::Var("value".to_string()),
                        "String",
                    ),
                )],
            ),
            crate::output_markers::INVALID_JAVA_CAST_MESSAGE,
        ),
        "as_str",
    )
}

fn optional_erased_string_consuming_view_projection(rendered: &str) -> Option<RustExpr> {
    let rendered = rendered.strip_suffix(".as_str()")?;
    optional_erased_string_downcast_ref_view_projection(rendered)
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
            clone_recovery_postfix_marker_expr(zero_arg_method_projection(expr, "cloned"), policy),
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
            clone_recovery_postfix_marker_expr(zero_arg_method_projection(expr, "clone"), policy),
        ),
    }
}

fn clone_recovery_postfix_marker_expr(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    let comment = render_clone_recovery_policy(policy);
    if comment.is_empty() {
        expr
    } else {
        RustExpr::PostfixComment {
            expr: Box::new(expr),
            comment: comment.to_string(),
        }
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
    let mapped = method_call_projection(
        parenthesize_raw_method_receiver(expr),
        "map",
        vec![single_param_closure_expr(
            "value",
            zero_arg_method_projection(RustExpr::Var("value".to_string()), "to_vec"),
        )],
    );
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        clone_recovery_postfix_marker_expr(mapped, policy),
    )
}

fn apply_borrowed_vec_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery) {
        return expr;
    }
    if let Some(projected) =
        borrowed_collect_clone_compat_projection(&expr, "collect::<Vec<_>>", policy)
    {
        return projected;
    }
    borrowed_collect_clone_projection(
        zero_arg_method_projection(opaque_raw_method_receiver(expr), "into_iter"),
        "collect::<Vec<_>>",
        policy,
    )
}

fn apply_borrowed_set_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if expr.recorded_shape() == Some(RustExprShape::OwnedRecovery) {
        return expr;
    }
    if let Some(projected) = borrowed_collect_clone_compat_projection(
        &expr,
        "collect::<std::collections::HashSet<_>>",
        policy,
    ) {
        return projected;
    }
    borrowed_collect_clone_projection(
        zero_arg_method_projection(opaque_raw_method_receiver(expr), "into_iter"),
        "collect::<std::collections::HashSet<_>>",
        policy,
    )
}

fn opaque_raw_method_receiver(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return parenthesize_raw_method_receiver(expr);
    };
    let trimmed = raw.trim();
    if let Some(recovered) = recover_rendered_method_receiver_expr(trimmed) {
        return parenthesize_raw_method_receiver(recovered);
    }
    if balanced_outer_parentheses_wrap_all(trimmed) {
        RustExpr::raw_leaf(trimmed)
    } else {
        grouped_stable_rendered_raw_leaf(trimmed.to_string())
    }
}

fn recover_rendered_method_receiver_expr(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    (!matches!(recovered.unshaped(), RustExpr::Raw(_)) && emit::emit_expr(&recovered) == rendered)
        .then_some(recovered)
}

pub(crate) fn rendered_projection_or_raw_leaf(rendered: &str) -> RustExpr {
    let rendered = rendered.trim();
    if matches!(rendered, "true" | "false") {
        return RustExpr::Literal(rendered.to_string());
    }
    if let Some(expr) = rendered_literal_leaf_projection(rendered) {
        return expr;
    }
    if let Some(expr) = borrowed_rendered_leaf_projection(rendered) {
        return expr;
    }
    if let Some(expr) = rendered_try_projection(rendered) {
        return expr;
    }
    if let Some(expr) = rendered_cast_leaf_projection(rendered) {
        return expr;
    }
    if let Some(expr) = rendered_vec_macro_projection(rendered) {
        return expr;
    }
    if let Some(expr) = rendered_some_projection(rendered) {
        return expr;
    }
    if let Some(expr) = rendered_function_call_projection(rendered) {
        return expr;
    }
    dotted_zero_arg_or_field_projection(rendered)
        .or_else(|| rendered_index_leaf_projection(rendered))
        .or_else(|| grouped_rendered_leaf_projection(rendered))
        .or_else(|| rendered_path_leaf_projection(rendered))
        .unwrap_or_else(|| RustExpr::raw_leaf(rendered))
}

fn rendered_some_projection(rendered: &str) -> Option<RustExpr> {
    rendered_complete_some_call_inner(rendered)
        .map(|inner| option_some_projection(rendered_some_inner_projection(inner)))
}

pub(crate) fn rendered_plus_expr_projection_or_raw_leaf(rendered: &str) -> RustExpr {
    rendered_top_level_plus_expr_projection(rendered)
        .or_else(|| rendered_top_level_simple_binary_expr_projection(rendered))
        .or_else(|| {
            let projected = rendered_projection_or_raw_leaf(rendered);
            (!matches!(projected.unshaped(), RustExpr::Raw(_))).then_some(projected)
        })
        .unwrap_or_else(|| RustExpr::raw_leaf(rendered))
}

fn rendered_top_level_plus_expr_projection(rendered: &str) -> Option<RustExpr> {
    let mut terms = split_rendered_top_level_plus_terms(rendered)?;
    let first = terms.next()?;
    let mut expr = rendered_projection_or_raw_leaf(first);
    for term in terms {
        expr = RustExpr::Binary {
            op: "+".to_string(),
            left: Box::new(expr),
            right: Box::new(rendered_projection_or_raw_leaf(term)),
        };
    }
    Some(expr)
}

fn split_rendered_top_level_plus_terms<'a>(text: &'a str) -> Option<std::vec::IntoIter<&'a str>> {
    let mut terms = Vec::new();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut cursor = 0usize;
    let mut start = 0usize;
    while cursor < text.len() {
        if let Some(end) = raw_string_literal_end_at(text, cursor) {
            cursor = end;
            continue;
        }
        let ch = text[cursor..].chars().next()?;
        if ch == '"' {
            cursor = normal_string_literal_end_at(text, cursor).unwrap_or(text.len());
            continue;
        }
        if ch == '\'' {
            cursor = normal_char_literal_end_at(text, cursor).unwrap_or(text.len());
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '+' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                let term = text[start..cursor].trim();
                if term.is_empty() {
                    return None;
                }
                terms.push(term);
                start = cursor + ch.len_utf8();
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    let term = text[start..].trim();
    if term.is_empty() {
        return None;
    }
    terms.push(term);
    (terms.len() > 1).then(|| terms.into_iter())
}

fn rendered_top_level_simple_binary_expr_projection(rendered: &str) -> Option<RustExpr> {
    for op in [
        "==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "&", "^", "|", "*", "/", "%",
    ] {
        if let Some((left, right)) = split_rendered_top_level_binary_once(rendered, op) {
            return Some(RustExpr::Binary {
                op: op.to_string(),
                left: Box::new(rendered_projection_or_raw_leaf(left)),
                right: Box::new(rendered_projection_or_raw_leaf(right)),
            });
        }
    }
    None
}

fn split_rendered_top_level_binary_once<'a>(text: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut cursor = 0usize;
    let mut found = None;
    while cursor < text.len() {
        if let Some(end) = raw_string_literal_end_at(text, cursor) {
            cursor = end;
            continue;
        }
        let ch = text[cursor..].chars().next()?;
        if ch == '"' {
            cursor = normal_string_literal_end_at(text, cursor).unwrap_or(text.len());
            continue;
        }
        if ch == '\'' {
            cursor = normal_char_literal_end_at(text, cursor).unwrap_or(text.len());
            continue;
        }
        if paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
            && text[cursor..].starts_with(op)
        {
            if found.replace(cursor).is_some() {
                return None;
            }
            cursor += op.len();
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    let index = found?;
    let left = text[..index].trim();
    let right = text[index + op.len()..].trim();
    (!left.is_empty() && !right.is_empty()).then_some((left, right))
}

fn rendered_literal_leaf_projection(rendered: &str) -> Option<RustExpr> {
    (rendered_expr_is_decimal_integer_literal(rendered)
        || rendered_expr_is_complete_string_literal(rendered)
        || rendered_expr_is_complete_char_literal(rendered))
    .then(|| RustExpr::Literal(rendered.to_string()))
}

fn rendered_try_projection(rendered: &str) -> Option<RustExpr> {
    let inner = rendered.trim_end().strip_suffix('?')?.trim();
    (!inner.is_empty()).then_some(())?;
    let projected = rendered_projection_or_raw_leaf(inner);
    (!matches!(projected.unshaped(), RustExpr::Raw(_))).then(|| RustExpr::Try(Box::new(projected)))
}

fn rendered_cast_leaf_projection(rendered: &str) -> Option<RustExpr> {
    let (operand, ty) = rendered_top_level_as_cast_parts(rendered)?;
    if raw_contains_top_level_binary_operator(operand) || raw_contains_top_level_as_cast(operand) {
        return None;
    }
    let expr = rendered_projection_or_raw_leaf(operand);
    (!matches!(expr.unshaped(), RustExpr::Raw(_)) && simple_rust_cast_type(ty)).then(|| {
        RustExpr::Cast {
            expr: Box::new(expr),
            ty: ty.to_string(),
        }
    })
}

fn rendered_top_level_as_cast_parts(rendered: &str) -> Option<(&str, &str)> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut cursor = 0usize;
    while cursor < rendered.len() {
        if let Some(end) = raw_string_literal_end_at(rendered, cursor) {
            cursor = end;
            continue;
        }
        let ch = rendered[cursor..].chars().next()?;
        if ch == '"' {
            cursor = normal_string_literal_end_at(rendered, cursor)?;
            continue;
        }
        if ch == '\'' {
            cursor = normal_char_literal_end_at(rendered, cursor)?;
            continue;
        }
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
            'a' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0
                && rendered[cursor..].starts_with("as")
                && raw_identifier_boundary(rendered[..cursor].chars().next_back())
                && raw_identifier_boundary(rendered[cursor + 2..].chars().next()) =>
            {
                let operand = rendered[..cursor].trim();
                let ty = rendered[cursor + 2..].trim();
                return (!operand.is_empty() && !ty.is_empty()).then_some((operand, ty));
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    None
}

fn simple_rust_cast_type(ty: &str) -> bool {
    !ty.is_empty()
        && ty
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '<' | '>' | ','))
}

fn rendered_vec_macro_projection(rendered: &str) -> Option<RustExpr> {
    let body = rendered_vec_macro_body(rendered)?;
    let body = body.trim();
    if body.is_empty() {
        return Some(RustExpr::VecLiteral(Vec::new()));
    }
    if let Some((value, len)) = split_rendered_top_level_binary_once(body, ";") {
        return Some(RustExpr::VecRepeat {
            value: Box::new(rendered_plus_expr_projection_or_raw_leaf(value)),
            len: Box::new(rendered_plus_expr_projection_or_raw_leaf(len)),
        });
    }
    Some(RustExpr::VecLiteral(split_raw_top_level_commas(body)))
}

fn grouped_rendered_leaf_projection(rendered: &str) -> Option<RustExpr> {
    if let Some(borrowed) = rendered.strip_prefix('&')
        && balanced_outer_parentheses_wrap_all(borrowed.trim())
    {
        return Some(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(grouped_rendered_inner_projection(borrowed.trim())?),
        });
    }
    if balanced_outer_parentheses_wrap_all(rendered) {
        return Some(grouped_rendered_inner_projection(rendered)?);
    }
    None
}

fn grouped_rendered_inner_projection(rendered: &str) -> Option<RustExpr> {
    let inner = rendered.strip_prefix('(')?.strip_suffix(')')?;
    Some(RustExpr::Grouped(Box::new(
        rendered_plus_expr_projection_or_raw_leaf(inner),
    )))
}

fn rendered_function_call_projection(rendered: &str) -> Option<RustExpr> {
    let (function, args) = rendered_call_parts(rendered)?;
    if let Some(macro_name) = simple_macro_call_name(function) {
        return Some(macro_call_projection(
            macro_name,
            rendered_call_args_projection(args),
        ));
    }
    if let Some((receiver, method)) = rendered_method_call_target(function) {
        return Some(method_call_projection(
            parenthesize_raw_method_receiver(rendered_projection_or_raw_leaf(receiver)),
            method,
            rendered_call_args_projection(args),
        ));
    }
    let function = plain_identifier_expr(function).or_else(|| {
        split_external_facade_rust_path(function)
            .filter(|segments| segments.len() > 1)
            .map(RustExpr::Path)
    })?;
    let args = rendered_call_args_projection(args);
    Some(RustExpr::Call {
        function: Box::new(function),
        args,
    })
}

pub(crate) fn recover_rendered_call_projection(rendered: &str) -> Option<RustExpr> {
    rendered_function_call_projection(rendered)
}

pub(crate) fn recover_rendered_assignment_place_projection(rendered: &str) -> Option<RustExpr> {
    let rendered = rendered.trim();
    if rendered.is_empty() {
        return None;
    }
    if let Some(inner) = rendered.strip_prefix('*') {
        let inner = recover_rendered_assignment_place_projection(inner.trim())?;
        return Some(RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(inner),
        });
    }
    if balanced_outer_parentheses_wrap_all(rendered) {
        let inner = rendered.strip_prefix('(')?.strip_suffix(')')?;
        let inner = recover_rendered_assignment_place_projection(inner)?;
        return Some(RustExpr::Grouped(Box::new(inner)));
    }
    rendered_index_leaf_projection(rendered)
        .filter(rust_expr_is_assignment_place_projection)
        .or_else(|| rendered_field_place_projection(rendered))
        .or_else(|| rendered_path_leaf_projection(rendered))
        .or_else(|| plain_identifier_expr(rendered))
}

fn rendered_field_place_projection(rendered: &str) -> Option<RustExpr> {
    let dot = last_top_level_dot(rendered)?;
    let receiver = rendered[..dot].trim();
    let field = rendered[dot + 1..].trim();
    let receiver = recover_rendered_assignment_place_projection(receiver)?;
    rust_plain_identifier(field).then(|| field_projection(receiver, field))
}

fn rust_expr_is_assignment_place_projection(expr: &RustExpr) -> bool {
    match expr.unshaped() {
        RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::Field(_, _) => true,
        RustExpr::Index(base, _) => rust_expr_is_assignment_place_projection(base),
        RustExpr::Unary { op, expr } if op == "*" => rust_expr_is_assignment_place_projection(expr),
        RustExpr::Grouped(expr)
        | RustExpr::Shaped { expr, .. }
        | RustExpr::PostfixComment { expr, .. }
        | RustExpr::MarkedBlock { expr, .. } => rust_expr_is_assignment_place_projection(expr),
        RustExpr::Literal(_)
        | RustExpr::Raw(_)
        | RustExpr::MacroCall { .. }
        | RustExpr::StructLit { .. }
        | RustExpr::Tuple(_)
        | RustExpr::ArrayLiteral(_)
        | RustExpr::VecLiteral(_)
        | RustExpr::VecRepeat { .. }
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. }
        | RustExpr::Unary { .. }
        | RustExpr::Binary { .. }
        | RustExpr::Range { .. }
        | RustExpr::Cast { .. }
        | RustExpr::Try(_)
        | RustExpr::If { .. }
        | RustExpr::Closure { .. }
        | RustExpr::Match { .. }
        | RustExpr::Block { .. }
        | RustExpr::UnsupportedJavaArrayCreation { .. }
        | RustExpr::Pattern(_) => false,
    }
}

fn simple_macro_call_name(function: &str) -> Option<&str> {
    let macro_name = function.strip_suffix('!')?;
    rust_plain_identifier(macro_name).then_some(macro_name)
}

fn rendered_call_args_projection(args: &str) -> Vec<RustExpr> {
    if args.trim().is_empty() {
        Vec::new()
    } else {
        split_raw_top_level_commas(args)
    }
}

fn rendered_index_leaf_projection(rendered: &str) -> Option<RustExpr> {
    let (array, index) = rendered_final_index_parts(rendered)?;
    let array = rendered_plus_expr_projection_or_raw_leaf(array);
    let index = rendered_plus_expr_projection_or_raw_leaf(index);
    if matches!(array.unshaped(), RustExpr::Raw(_)) || matches!(index.unshaped(), RustExpr::Raw(_))
    {
        return None;
    }
    Some(RustExpr::Index(Box::new(array), Box::new(index)))
}

fn rendered_final_index_parts(rendered: &str) -> Option<(&str, &str)> {
    if !rendered.ends_with(']') {
        return None;
    }
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut open = None;
    let mut cursor = 0usize;
    while cursor < rendered.len() {
        if let Some(end) = raw_string_literal_end_at(rendered, cursor) {
            cursor = end;
            continue;
        }
        let ch = rendered[cursor..].chars().next()?;
        if ch == '"' {
            cursor = normal_string_literal_end_at(rendered, cursor)?;
            continue;
        }
        if ch == '\'' {
            cursor = normal_char_literal_end_at(rendered, cursor)?;
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.checked_sub(1)?,
            '[' => {
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 && angle_depth == 0 {
                    open = Some(cursor);
                }
                bracket_depth += 1;
            }
            ']' => {
                bracket_depth = bracket_depth.checked_sub(1)?;
                if bracket_depth == 0 && cursor + ch.len_utf8() == rendered.len() {
                    let open = open?;
                    let array = rendered[..open].trim();
                    let index = rendered[open + 1..cursor].trim();
                    return (!array.is_empty() && !index.is_empty()).then_some((array, index));
                }
            }
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.checked_sub(1)?,
            '<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth = angle_depth.checked_sub(1)?
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    None
}

fn rendered_method_call_target(function: &str) -> Option<(&str, &str)> {
    let dot = last_top_level_dot(function)?;
    let receiver = function[..dot].trim();
    let method = function[dot + 1..].trim();
    (!receiver.is_empty()
        && rust_method_token(method)
        && !raw_contains_top_level_binary_operator(receiver))
    .then_some((receiver, method))
}

fn rust_method_token(method: &str) -> bool {
    if rust_plain_identifier(method) {
        return true;
    }
    let Some((ident, generic_args)) = method.split_once("::<") else {
        return false;
    };
    rust_plain_identifier(ident)
        && generic_args.ends_with('>')
        && rust_method_generic_args_are_balanced(generic_args)
}

fn rust_method_generic_args_are_balanced(args: &str) -> bool {
    if args.len() <= 1 {
        return false;
    }
    let mut angle_depth = 1usize;
    for ch in args[..args.len() - 1].chars() {
        match ch {
            '<' => angle_depth = angle_depth.saturating_add(1),
            '>' => {
                let Some(depth) = angle_depth.checked_sub(1) else {
                    return false;
                };
                if depth == 0 {
                    return false;
                }
                angle_depth = depth;
            }
            ch if ch.is_ascii_alphanumeric()
                || matches!(
                    ch,
                    '_' | ':' | ',' | ' ' | '&' | '\'' | '[' | ']' | '(' | ')' | '*'
                ) => {}
            _ => return false,
        }
    }
    angle_depth == 1
}

fn last_top_level_dot(raw: &str) -> Option<usize> {
    let mut last_dot = None;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if let Some(end) = raw_string_literal_end_at(raw, idx) {
            while chars.peek().is_some_and(|(next, _)| *next < end) {
                chars.next();
            }
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '<' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                angle_depth = angle_depth.saturating_sub(1);
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
            '.' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0 =>
            {
                last_dot = Some(idx);
            }
            _ => {}
        }
    }
    last_dot
}

fn rendered_path_leaf_projection(rendered: &str) -> Option<RustExpr> {
    split_external_facade_rust_path(rendered).and_then(|segments| {
        (segments.len() > 1 || rendered.contains("::<")).then_some(RustExpr::Path(segments))
    })
}

fn rendered_call_parts(rendered: &str) -> Option<(&str, &str)> {
    if !rendered.ends_with(')') {
        return None;
    }
    let mut depth = 0usize;
    let mut open = None;
    let mut chars = rendered.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '(' => {
                if depth == 0 {
                    open = Some(idx);
                }
                depth += 1;
            }
            ')' => {
                depth = depth.checked_sub(1)?;
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
            'r' if rendered[idx..].starts_with("r#\"") => {
                for (_, inner) in chars.by_ref() {
                    if inner == '"' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    (depth == 0).then_some(())?;
    let open = open?;
    let function = rendered[..open].trim();
    if function.is_empty() {
        return None;
    }
    Some((function, &rendered[open + 1..rendered.len() - 1]))
}

fn borrowed_rendered_leaf_projection(rendered: &str) -> Option<RustExpr> {
    let (op, operand) = if let Some(operand) = rendered.strip_prefix("&mut ") {
        ("&mut ", operand)
    } else if let Some(operand) = rendered.strip_prefix('&') {
        ("&", operand.trim_start())
    } else {
        return None;
    };
    let recovered = rendered_projection_or_raw_leaf(operand);
    if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
        return Some(RustExpr::Unary {
            op: op.to_string(),
            expr: Box::new(recovered),
        });
    }
    None
}

fn borrowed_collect_clone_projection(
    iterator: RustExpr,
    collect_method: &str,
    policy: &CloneRecoveryPolicy,
) -> RustExpr {
    let cloned = clone_recovery_postfix_marker_expr(
        zero_arg_method_projection(parenthesize_raw_method_receiver(iterator), "cloned"),
        policy,
    );
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        zero_arg_method_projection(cloned, collect_method),
    )
}

fn borrowed_collect_clone_compat_projection(
    expr: &RustExpr,
    collect_method: &str,
    policy: &CloneRecoveryPolicy,
) -> Option<RustExpr> {
    let recovered = match expr.unshaped() {
        RustExpr::Raw(rendered) => {
            if let Some(projected) =
                raw_borrowed_collect_clone_compat_projection(rendered, collect_method, policy)
            {
                return Some(projected);
            }
            let recovered = rendered_projection_or_raw_leaf(rendered);
            if matches!(recovered.unshaped(), RustExpr::Raw(_))
                || emit::emit_expr(&recovered) != rendered.trim()
            {
                return None;
            }
            recovered
        }
        _ => expr.clone(),
    };
    if rust_expr_is_cloned_collect_call(&recovered, collect_method) {
        return Some(RustExpr::shaped(RustExprShape::OwnedRecovery, recovered));
    }
    rust_expr_collect_call_receiver(&recovered, collect_method)
        .map(|iterator| borrowed_collect_clone_projection(iterator, collect_method, policy))
}

fn raw_borrowed_collect_clone_compat_projection(
    rendered: &str,
    collect_method: &str,
    policy: &CloneRecoveryPolicy,
) -> Option<RustExpr> {
    let rendered = rendered.trim();
    let cloned_collect_suffix = format!(".cloned().{collect_method}()");
    if let Some(iterator_rendered) = rendered.strip_suffix(&cloned_collect_suffix) {
        let iterator = rendered_projection_or_raw_leaf(iterator_rendered.trim());
        return Some(RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            zero_arg_method_projection(
                zero_arg_method_projection(iterator, "cloned"),
                collect_method,
            ),
        ));
    }

    let collect_suffix = format!(".{collect_method}()");
    rendered
        .strip_suffix(&collect_suffix)
        .map(str::trim)
        .filter(|iterator_rendered| !iterator_rendered.is_empty())
        .map(|iterator_rendered| {
            borrowed_collect_clone_projection(
                rendered_projection_or_raw_leaf(iterator_rendered),
                collect_method,
                policy,
            )
        })
}

fn rust_expr_is_cloned_collect_call(expr: &RustExpr, collect_method: &str) -> bool {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if rust_method_matches_collect_method(method, collect_method) && args.is_empty() => {
            matches!(
                receiver.unshaped(),
                RustExpr::MethodCall { method, args, .. }
                    if method == "cloned" && args.is_empty()
            )
        }
        _ => false,
    }
}

fn rust_expr_collect_call_receiver(expr: &RustExpr, collect_method: &str) -> Option<RustExpr> {
    match expr.unshaped() {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if rust_method_matches_collect_method(method, collect_method) && args.is_empty() => {
            Some((**receiver).clone())
        }
        _ => None,
    }
}

fn rust_method_matches_collect_method(method: &str, expected: &str) -> bool {
    method == expected || method.split_whitespace().collect::<String>() == expected
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

fn index_to_usize_cast_projection(expr: RustExpr) -> RustExpr {
    if let Some(cast) = usize_cast_projection(expr.clone()) {
        return cast;
    }
    RustExpr::Cast {
        expr: Box::new(group_raw_cast_operand_if_needed(expr)),
        ty: "usize".to_string(),
    }
}

fn group_raw_cast_operand_if_needed(expr: RustExpr) -> RustExpr {
    raw_leaf_or_grouped(expr, |trimmed| {
        !(balanced_outer_parentheses_wrap_all(trimmed)
            || (!raw_contains_top_level_binary_operator(trimmed)
                && !raw_contains_top_level_as_cast(trimmed)))
    })
}

fn recover_as_str_receiver_projection(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => recover_parseable_rendered_operand_projection(&rendered),
        other => other,
    }
}

fn apply_one(expr: RustExpr, coercion: &Coercion) -> RustExpr {
    match coercion {
        Coercion::Identity => expr,
        Coercion::IndexToUsize => index_to_usize_cast_projection(expr),
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
            if let Some(view) = deref_string_view_projection(&expr) {
                view
            } else {
                let expr = recover_as_str_receiver_projection(expr);
                string_view_expr(zero_arg_method_projection(
                    parenthesize_raw_method_receiver(expr),
                    "as_str",
                ))
            }
        }
        Coercion::ToOwnedString => {
            if let Some(recovered) = owned_string_recovery_projection(expr.clone()) {
                RustExpr::shaped(RustExprShape::OwnedRecovery, recovered)
            } else {
                let expr = match expr {
                    RustExpr::Raw(rendered) => rendered_projection_or_raw_leaf(&rendered),
                    other => other,
                };
                RustExpr::shaped(
                    RustExprShape::OwnedRecovery,
                    zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "to_string"),
                )
            }
        }
        Coercion::AsSlice => {
            if expr_is_associated_path_without_call(&expr) {
                slice_view_expr(associated_path_view_projection(expr))
            } else {
                slice_view_expr(zero_arg_method_projection(
                    parenthesize_raw_method_receiver(expr),
                    "as_slice",
                ))
            }
        }
        Coercion::AsMutSlice => mutable_slice_view_expr(zero_arg_method_projection(
            parenthesize_raw_method_receiver(expr),
            "as_mut_slice",
        )),
        Coercion::WrapClosedEnum {
            enum_name,
            variant_name,
        } => {
            if java_type_param_variant_name(variant_name) {
                let root_variant = enum_name.strip_suffix("Value").unwrap_or(enum_name);
                RustExpr::Call {
                    function: Box::new(RustExpr::path([enum_name.as_str(), root_variant])),
                    args: vec![into_projection(expr)],
                }
            } else {
                RustExpr::Call {
                    function: Box::new(RustExpr::path([enum_name.as_str(), variant_name.as_str()])),
                    args: vec![expr],
                }
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
                    borrow_concrete_facade_block_expr(expr, target_type),
                )
            }
        }
        Coercion::ExternalInto { .. } => into_projection(expr),
        Coercion::BoxDynTrait { trait_name } => boxed_dyn_trait_projection(expr, trait_name),
        Coercion::CloneBoxDynTrait { trait_name: _ } => {
            if expr.recorded_shape() == Some(RustExprShape::AnonymousDynTrait) {
                expr
            } else if let Some(trait_name) = anonymous_dyn_trait_constructor_trait(&expr) {
                anonymous_dyn_trait_projection(trait_name)
            } else {
                RustExpr::shaped(
                    RustExprShape::OwnedRecovery,
                    zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "clone_box"),
                )
            }
        }
        Coercion::EraseObject => erased_object_box_projection(expr),
        Coercion::DowncastClosedEnum {
            enum_name,
            variant_name,
            invalid_cast,
        } => closed_enum_downcast_projection(expr, enum_name, variant_name, invalid_cast),
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
        Coercion::WrapOption => option_some_projection(expr),
        Coercion::UnwrapOption => zero_arg_method_projection(expr, "unwrap"),
        Coercion::UnwrapOptionBorrowed => RustExpr::shaped(
            RustExprShape::Borrowed,
            method_expect_projection(
                zero_arg_method_projection(strip_terminal_unwrap_projection(expr), "as_ref"),
                "Java null receiver",
            ),
        ),
        Coercion::UnwrapOptionMutBorrowed => RustExpr::shaped(
            RustExprShape::MutBorrowed,
            method_expect_projection(
                zero_arg_method_projection(strip_terminal_unwrap_projection(expr), "as_mut"),
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

fn borrowed_item_string_view_projection(borrowed_item: &str) -> RustExpr {
    let receiver = borrowed_item_projection_or_raw_leaf(borrowed_item);
    borrowed_item_string_view_projection_expr(receiver)
}

fn borrowed_item_string_view_projection_expr(receiver: RustExpr) -> RustExpr {
    string_view_expr(zero_arg_method_projection(receiver, "as_str"))
}

fn borrowed_item_projection_or_raw_leaf(borrowed_item: &str) -> RustExpr {
    rendered_plus_expr_projection_or_raw_leaf(borrowed_item)
}

fn java_type_param_variant_name(name: &str) -> bool {
    name.len() == 1 && name.chars().all(|ch| ch.is_ascii_uppercase())
}

fn option_some_projection(expr: RustExpr) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(expr);
    RustExpr::shaped(
        RustExprShape::OptionalConstructed,
        normalize_optional_constructed_string_view_expr(RustExpr::Call {
            function: Box::new(RustExpr::path(["Some"])),
            args: vec![expr],
        }),
    )
}

#[cfg(test)]
fn render_cast_operand_with_structural_parentheses(expr: &RustExpr) -> String {
    match expr.unshaped() {
        RustExpr::Binary { .. } => format!("({})", emit::emit_expr(expr)),
        RustExpr::Raw(rendered) => parenthesize_compound_cast_operand(rendered),
        _ => emit::emit_expr(expr).trim().to_string(),
    }
}

#[cfg(test)]
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

fn invalid_cast_policy_expr(policy: &InvalidCastPolicy) -> RustExpr {
    match policy {
        InvalidCastPolicy::Panic => RustExpr::MacroCall {
            macro_name: "panic".to_string(),
            args: vec![RustExpr::Literal(format!(
                "{:?}",
                crate::output_markers::INVALID_JAVA_CAST_MESSAGE
            ))],
        },
    }
}

fn apply_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    let target_type = normalize_emitted_java_type_name(target_type);
    match invalid_cast {
        InvalidCastPolicy::Panic => method_expect_projection(
            dyn_trait_downcast_ref_projection(expr, &target_type),
            "invalid Java cast",
        ),
    }
}

fn apply_boxed_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    let target_type = normalize_emitted_java_type_name(target_type);
    match invalid_cast {
        InvalidCastPolicy::Panic => method_expect_projection(
            boxed_dyn_trait_downcast_ref_projection(expr, &target_type),
            "invalid Java cast",
        ),
    }
}

pub fn erased_object_borrowed_downcast_ref_projection(
    expr: RustExpr,
    target_type: &str,
    optional_carrier: bool,
) -> RustExpr {
    let target_type = normalize_emitted_java_type_name(target_type);
    let handled_inner = shared_handle_inner_type(&target_type);
    if optional_carrier {
        let downcast_body =
            erased_object_downcast_ref_projection(RustExpr::Var("value".to_string()), &target_type);
        let closure_body = if handled_inner.is_some() {
            method_call_projection(downcast_body, "map", vec![value_as_ref_closure_expr()])
        } else {
            downcast_body
        };
        let downcast = method_call_projection(
            zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "as_ref"),
            "and_then",
            vec![single_param_closure_expr("value", closure_body)],
        );
        return method_expect_projection(
            downcast,
            crate::output_markers::INVALID_JAVA_CAST_MESSAGE,
        );
    }
    let downcast =
        erased_object_downcast_ref_projection(parenthesize_raw_method_receiver(expr), &target_type);
    let downcast = if handled_inner.is_some() {
        method_call_projection(downcast, "map", vec![value_as_ref_closure_expr()])
    } else {
        downcast
    };
    method_expect_projection(downcast, crate::output_markers::INVALID_JAVA_CAST_MESSAGE)
}

pub fn erased_object_is_type_projection(receiver: RustExpr, target_type: &str) -> RustExpr {
    typed_zero_arg_method_projection(receiver, "is", target_type)
}

pub fn erased_object_downcast_ref_projection(receiver: RustExpr, target_type: &str) -> RustExpr {
    typed_zero_arg_method_projection(receiver, "downcast_ref", target_type)
}

fn value_as_ref_closure_expr() -> RustExpr {
    single_param_closure_expr(
        "value",
        zero_arg_method_projection(RustExpr::Var("value".to_string()), "as_ref"),
    )
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
    match invalid_cast {
        // Parenthesised: `.` binds tighter than unary `*`, so an appended field
        // access turned `*x.downcast::<T>().expect(..).v` into a deref of the
        // *field* -- `error[E0614]: type i32 cannot be dereferenced`.
        InvalidCastPolicy::Panic => RustExpr::Grouped(Box::new(RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(method_expect_projection(
                typed_zero_arg_method_projection(
                    parenthesize_raw_method_receiver(expr),
                    "downcast",
                    &target_type,
                ),
                crate::output_markers::INVALID_JAVA_CAST_MESSAGE,
            )),
        })),
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

pub fn to_owned_sequence_projection(mut expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if rust_expr_is_to_vec_call(&expr) {
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
    let rendered = if let RustExpr::Raw(rendered) = expr.unshaped() {
        let rendered = rendered.clone();
        if let Some(recovered) = recover_rendered_to_owned_sequence_expr(&rendered) {
            return RustExpr::shaped(RustExprShape::OwnedRecovery, recovered);
        }
        let recovered = rendered_projection_or_raw_leaf(&rendered);
        if !matches!(recovered.unshaped(), RustExpr::Raw(_)) {
            expr = recovered;
            None
        } else {
            Some(rendered)
        }
    } else {
        None
    };
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        clone_recovery_postfix_marker_expr(
            zero_arg_method_projection(
                to_owned_sequence_method_receiver(expr, rendered.as_deref()),
                "to_vec",
            ),
            policy,
        ),
    )
}

fn recover_rendered_to_owned_sequence_expr(rendered: &str) -> Option<RustExpr> {
    let recovered = rendered_projection_or_raw_leaf(rendered);
    rust_expr_is_to_vec_call(&recovered).then_some(recovered)
}

fn rust_expr_is_to_vec_call(expr: &RustExpr) -> bool {
    matches!(
        expr.unshaped(),
        RustExpr::MethodCall { method, args, .. } if method == "to_vec" && args.is_empty()
    )
}

fn to_owned_sequence_method_receiver(expr: RustExpr, rendered: Option<&str>) -> RustExpr {
    if rust_owned_sequence_receiver_needs_parentheses(&expr, rendered) {
        RustExpr::Grouped(Box::new(expr))
    } else {
        parenthesize_raw_method_receiver(expr)
    }
}

fn rust_owned_sequence_receiver_needs_parentheses(expr: &RustExpr, rendered: Option<&str>) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => {
            rust_owned_sequence_receiver_needs_parentheses(expr, rendered)
        }
        RustExpr::Raw(_) => {
            rendered.is_some_and(|rendered| raw_contains_as_token_outside_literals(rendered.trim()))
        }
        RustExpr::Field(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MacroCall { .. }
        | RustExpr::MethodCall { .. }
        | RustExpr::Literal(_)
        | RustExpr::Var(_)
        | RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::Pattern(_)
        | RustExpr::Grouped(_)
        | RustExpr::MarkedBlock { .. }
        | RustExpr::Block { .. } => false,
        RustExpr::UnsupportedJavaArrayCreation { .. } => false,
        RustExpr::Try(_)
        | RustExpr::Index(_, _)
        | RustExpr::Tuple(_)
        | RustExpr::ArrayLiteral(_)
        | RustExpr::VecLiteral(_)
        | RustExpr::VecRepeat { .. }
        | RustExpr::Closure { .. }
        | RustExpr::Binary { .. }
        | RustExpr::Range { .. }
        | RustExpr::Unary { .. }
        | RustExpr::Cast { .. }
        | RustExpr::If { .. }
        | RustExpr::Match { .. }
        | RustExpr::StructLit { .. } => true,
        RustExpr::PostfixComment { expr, .. } => {
            rust_owned_sequence_receiver_needs_parentheses(expr, rendered)
        }
    }
}

fn apply_clone_recovery(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    if let Some(recovered) = clone_recovery_projection(expr.clone()) {
        return RustExpr::shaped(RustExprShape::OwnedRecovery, recovered);
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
    if let Some(inner) = unwrap_some_expr(&expr) {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            option_some_projection(apply_clone_recovery(inner, policy)),
        );
    }
    if !matches!(expr.unshaped(), RustExpr::Raw(_)) {
        return RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            clone_recovery_postfix_marker_expr(
                zero_arg_method_projection(parenthesize_raw_method_receiver(expr), "clone"),
                policy,
            ),
        );
    }
    let RustExpr::Raw(rendered) = expr.unshaped() else {
        unreachable!("structured clone recovery returns before raw fallback");
    };
    let rendered = rendered.clone();
    RustExpr::shaped(
        RustExprShape::OwnedRecovery,
        clone_recovery_postfix_marker_expr(
            zero_arg_method_projection(clone_recovery_method_receiver(expr, &rendered), "clone"),
            policy,
        ),
    )
}

fn clone_recovery_method_receiver(expr: RustExpr, rendered: &str) -> RustExpr {
    clone_method_receiver(expr, rendered, false, |trimmed| {
        rust_clone_recovery_receiver_needs_parentheses(trimmed)
    })
}

#[cfg(test)]
fn render_clone_recovery_expr(rendered: &str, policy: &CloneRecoveryPolicy) -> String {
    render_clone_projection_expr_with_receiver_policy(
        rendered,
        render_clone_recovery_policy(policy),
        rust_clone_recovery_receiver_needs_parentheses,
    )
}

#[cfg(test)]
pub fn render_clone_projection(rendered: &str) -> String {
    render_clone_projection_expr(rendered, "")
}

#[cfg(test)]
fn render_clone_projection_expr(rendered: &str, suffix: &str) -> String {
    render_clone_projection_expr_with_receiver_policy(
        rendered,
        suffix,
        rust_method_receiver_needs_parentheses,
    )
}

#[cfg(test)]
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

fn rendered_anonymous_dyn_trait_constructor_trait(rendered: &str) -> Option<&str> {
    let trimmed = rendered.trim();
    trimmed
        .strip_prefix("<dyn ")
        .and_then(|trait_name| trait_name.strip_suffix(">::anonymous()"))
}

fn anonymous_dyn_trait_constructor_trait(expr: &RustExpr) -> Option<&str> {
    match expr.unshaped() {
        RustExpr::Call { function, args } if args.is_empty() => {
            let RustExpr::Path(segments) = function.unshaped() else {
                return None;
            };
            let [trait_segment, method] = segments.as_slice() else {
                return None;
            };
            if method != "anonymous" {
                return None;
            }
            trait_segment
                .strip_prefix("<dyn ")
                .and_then(|trait_name| trait_name.strip_suffix('>'))
        }
        RustExpr::Raw(rendered) => rendered_anonymous_dyn_trait_constructor_trait(rendered),
        _ => None,
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
    use crate::facts::{FactProvenance, TypeCapabilityOverride};

    use super::*;

    fn value_clone_closure() -> RustExpr {
        single_param_closure_expr(
            "value",
            zero_arg_method_projection(
                RustExpr::Grouped(Box::new(RustExpr::Unary {
                    op: "*".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                })),
                "clone",
            ),
        )
    }

    fn sequence_index_position_closure(needle: RustExpr) -> RustExpr {
        single_param_closure_expr(
            "item",
            RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(RustExpr::Var("item".to_string())),
                right: Box::new(needle),
            },
        )
    }

    fn sequence_index_i32_closure() -> RustExpr {
        single_param_closure_expr(
            "index",
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("index".to_string())),
                ty: "i32".to_string(),
            },
        )
    }

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

        let erased_expr = RustExpr::Raw(format!(
            "maybe.unwrap().downcast::<String>(){}",
            crate::output_markers::invalid_java_cast_expect()
        ));
        let erased_projected = apply_required_borrowed_element_target_view(&erased_expr, &policy)
            .expect("erased string view");
        assert_eq!(
            emit::emit_expr(&erased_projected),
            "maybe.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str()"
        );
        assert!(matches!(
            erased_projected.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "expect"
                                && matches!(args.as_slice(), [RustExpr::Literal(message)] if message == "\"invalid Java cast\"")
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::MethodCall { receiver, method, args }
                                        if method == "and_then"
                                            && args.len() == 1
                                            && matches!(
                                                receiver.as_ref(),
                                                RustExpr::MethodCall { receiver, method, args }
                                                    if method == "as_ref"
                                                        && args.is_empty()
                                                        && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "maybe")
                                            )
                                )
                    )
        ));
        let borrowed_erased_expr = RustExpr::Raw(format!(
            "&(*maybe.unwrap().downcast::<String>(){})",
            crate::output_markers::invalid_java_cast_expect()
        ));
        let borrowed_erased_projected =
            apply_required_borrowed_element_target_view(&borrowed_erased_expr, &policy)
                .expect("borrowed erased string view");
        assert_eq!(
            emit::emit_expr(&borrowed_erased_projected),
            "maybe.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str()"
        );

        let dotted_borrowed_item = borrowed_item_string_view_projection("owner.value()");
        assert_eq!(
            emit::emit_expr(&dotted_borrowed_item),
            "owner.value().as_str()"
        );
        assert!(matches!(
            dotted_borrowed_item.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "value"
                                && args.is_empty()
                                && matches!(receiver.as_ref(), RustExpr::Var(owner) if owner == "owner")
                    )
        ));

        let complex_borrowed_item = borrowed_item_string_view_projection("left + right");
        assert_eq!(
            emit::emit_expr(&complex_borrowed_item),
            "(left + right).as_str()"
        );
        assert!(matches!(
            complex_borrowed_item.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Binary { op, .. } if op == "+")
        ));

        let structured_borrowed_item = borrowed_item_string_view_projection_expr(RustExpr::Field(
            Box::new(RustExpr::Var("owner".to_string())),
            "name".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&structured_borrowed_item),
            "owner.name.as_str()"
        );
        assert!(!matches!(
            structured_borrowed_item.unshaped(),
            RustExpr::Raw(_)
        ));
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
        assert_eq!(
            expr,
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
            }
        );

        let opaque = ensure_borrow_projection(
            RustExpr::Raw("left +".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&opaque), "&left +");
        assert!(matches!(
            opaque,
            RustExpr::Unary { op, expr }
                if op == "&"
                    && matches!(expr.as_ref(), RustExpr::Raw(raw) if raw == "left +")
        ));
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
        assert!(matches!(
            simple_index.clone().into_unshaped(),
            RustExpr::Unary { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Index(array, index)
                        if matches!(array.as_ref(), RustExpr::Var(name) if name == "values")
                            && matches!(index.as_ref(), RustExpr::Var(name) if name == "i")
                )
        ));

        let compound_index = ensure_borrow_projection(
            RustExpr::Raw("left + values[i]".to_string()),
            None,
            None,
            BorrowProjectionUse::Argument,
        );
        assert_eq!(emit::emit_expr(&compound_index), "&(left + values[i])");
        assert!(matches!(
            compound_index.clone().into_unshaped(),
            RustExpr::Unary { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Grouped(inner)
                        if matches!(
                            inner.as_ref(),
                            RustExpr::Binary { op, left, right }
                                if op == "+"
                                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                    && matches!(
                                        right.as_ref(),
                                        RustExpr::Index(array, index)
                                            if matches!(array.as_ref(), RustExpr::Var(name) if name == "values")
                                                && matches!(index.as_ref(), RustExpr::Var(name) if name == "i")
                                    )
                        )
                )
        ));

        let mutable_compound = apply_one(
            RustExpr::Raw("left + values[i]".to_string()),
            &Coercion::BorrowMut,
        );
        assert_eq!(
            emit::emit_expr(&mutable_compound),
            "&mut (left + values[i])"
        );
        assert!(matches!(
            mutable_compound.into_unshaped(),
            RustExpr::Unary { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Grouped(inner)
                        if matches!(
                            inner.as_ref(),
                            RustExpr::Binary { op, right, .. }
                                if op == "+"
                                    && matches!(right.as_ref(), RustExpr::Index(_, _))
                        )
                )
        ));
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
    fn as_str_deref_recovery_prefers_structured_unary_operands() {
        let local = apply_one(
            RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Var("partition".to_string())),
            },
            &Coercion::AsStr,
        );
        assert_eq!(emit::emit_expr(&local), "partition.as_str()");
        assert_eq!(local.recorded_shape(), Some(RustExprShape::StringView));
        assert!(matches!(
            local.clone().into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "partition")
        ));

        let compound = apply_one(
            RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }),
            },
            &Coercion::AsStr,
        );
        assert_eq!(emit::emit_expr(&compound), "(left + right).as_str()");
        assert!(matches!(
            compound.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Binary { .. })
        ));
    }

    #[test]
    fn borrow_projection_keeps_map_key_helper_idempotent() {
        let expr = ensure_borrow_projection(
            string_substring_projection(
                RustExpr::Var("name".to_string()),
                RustExpr::Literal("0".to_string()),
                RustExpr::Literal("1".to_string()),
            ),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(emit::emit_expr(&expr), "java_string_substring(&name, 0, 1)");

        let raw_compat = ensure_borrow_projection(
            RustExpr::Raw("java_string_substring(name.as_str(), 0, 1)".to_string()),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(
            emit::emit_expr(&raw_compat),
            "&java_string_substring(name.as_str(), 0, 1)"
        );

        let unknown_raw_helper = ensure_borrow_projection(
            RustExpr::Raw("java_string_unknown(name.as_str())".to_string()),
            None,
            None,
            BorrowProjectionUse::MapKey,
        );
        assert_eq!(
            emit::emit_expr(&unknown_raw_helper),
            "&java_string_unknown(name.as_str())"
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
    fn map_key_borrow_normalizes_raw_local_before_local_identity_checks() {
        let mut actual = RustSlotShape::conservative(JavaType::String);
        actual.ownership = OwnershipShape::SharedBorrow;
        actual.view = ViewShape::Native;

        let borrowed = normalize_map_key_borrow_projection(
            RustExpr::Raw("name".to_string()),
            Some("name"),
            Some(&actual),
        );

        assert_eq!(emit::emit_expr(&borrowed), "name.as_str()");
        assert!(matches!(
            borrowed.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "name")
        ));

        let collapsed = normalize_map_key_borrow_projection(
            RustExpr::Raw("&name".to_string()),
            Some("name"),
            Some(&actual),
        );
        assert_eq!(emit::emit_expr(&collapsed), "name.as_str()");
    }

    #[test]
    fn raw_field_path_does_not_satisfy_local_name_identity() {
        let mut actual = RustSlotShape::conservative(JavaType::String);
        actual.ownership = OwnershipShape::SharedBorrow;
        actual.view = ViewShape::Native;

        let borrowed = normalize_map_key_borrow_projection(
            RustExpr::Raw("name.field".to_string()),
            Some("name"),
            Some(&actual),
        );

        assert_eq!(emit::emit_expr(&borrowed), "&name.field");
        assert!(!matches!(
            borrowed.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "as_str"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "name")
        ));
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

        let forced_raw_simple = force_shared_borrow_projection(RustExpr::Raw("value".to_string()));
        assert_eq!(emit::emit_expr(&forced_raw_simple), "&value");
        assert_eq!(
            forced_raw_simple.clone().into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            }
        );
        assert_eq!(
            forced_raw_simple.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );

        let forced_raw_binary =
            force_shared_borrow_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(emit::emit_expr(&forced_raw_binary), "&(left + right)");
        assert!(matches!(
            forced_raw_binary.clone().into_unshaped(),
            RustExpr::Unary { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Binary { op, .. } if op == "+"
                ) || matches!(
                    expr.as_ref(),
                    RustExpr::Grouped(grouped)
                        if matches!(grouped.as_ref(), RustExpr::Binary { op, .. } if op == "+")
                )
        ));
        assert_eq!(
            forced_raw_binary.recorded_shape(),
            Some(RustExprShape::Borrowed)
        );

        let forced =
            force_shared_borrow_projection(RustExpr::Raw("custom_view(value)".to_string()));
        assert_eq!(emit::emit_expr(&forced), "&custom_view(value)");
        assert_eq!(
            forced.clone().into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(function_call_projection(
                    RustExpr::Var("custom_view".to_string()),
                    vec![RustExpr::Var("value".to_string())],
                )),
            }
        );
        assert_eq!(forced.recorded_shape(), Some(RustExprShape::Borrowed));
        assert_eq!(
            ensure_borrow_projection(forced.clone(), None, None, BorrowProjectionUse::Argument),
            forced
        );

        let dyn_trait = borrow_as_dyn_trait(RustExpr::Var("source".to_string()), "Readable");
        assert_eq!(emit::emit_expr(&dyn_trait), "(&source) as &dyn Readable");
        assert_eq!(dyn_trait.recorded_shape(), Some(RustExprShape::Borrowed));
        assert_eq!(
            dyn_trait.clone().into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("source".to_string())),
                }),
                ty: "&dyn Readable".to_string(),
            }
        );
        let raw_dyn_trait =
            borrow_as_dyn_trait(RustExpr::Raw("source.value()".to_string()), "Readable");
        assert_eq!(
            emit::emit_expr(&raw_dyn_trait),
            "(&source.value()) as &dyn Readable"
        );
        assert_eq!(
            raw_dyn_trait.clone().into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("source".to_string())),
                        method: "value".to_string(),
                        args: Vec::new(),
                    }),
                }),
                ty: "&dyn Readable".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&borrow_as_dyn_trait(
                RustExpr::Raw("left + right".to_string()),
                "Readable",
            )),
            "(&(left + right)) as &dyn Readable"
        );
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
            boxed_dyn_trait.clone().into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("boxed".to_string())),
                    method: "as_ref".to_string(),
                    args: Vec::new(),
                }),
                ty: "&dyn Readable".to_string(),
            }
        );
        let raw_boxed_dyn_trait =
            borrow_boxed_as_dyn_trait(RustExpr::Raw("factory()".to_string()), "Readable");
        assert_eq!(
            emit::emit_expr(&raw_boxed_dyn_trait),
            "factory().as_ref() as &dyn Readable"
        );
        assert!(matches!(
            raw_boxed_dyn_trait.into_unshaped(),
            RustExpr::Cast { expr, .. } if matches!(expr.as_ref(), RustExpr::MethodCall { .. })
        ));
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
        assert!(matches!(
            concrete_facade.clone().into_unshaped(),
            RustExpr::Block {
                stmts,
                tail: Some(tail),
            } if matches!(
                stmts.as_slice(),
                [RustStmt::Let {
                    name,
                    value: Some(RustExpr::Var(value)),
                    ..
                }] if name == "_java_value" && value == "source"
            ) && matches!(
                tail.as_ref(),
                RustExpr::Unary { op, expr }
                    if op == "&" && matches!(expr.as_ref(), RustExpr::Path(path) if path == &vec!["ReadableFacade".to_string()])
            )
        ));
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

        let raw_optional_borrow = apply_one(
            RustExpr::Raw("value.unwrap()".to_string()),
            &Coercion::UnwrapOptionBorrowed,
        );
        assert_eq!(
            emit::emit_expr(&raw_optional_borrow),
            "value.as_ref().expect(\"Java null receiver\")"
        );
        assert!(matches!(
            raw_optional_borrow.into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "expect"
                    && matches!(args.as_slice(), [RustExpr::Literal(message)]
                        if message == "\"Java null receiver\"")
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "as_ref"
                                && args.is_empty()
                                && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                    )
        ));

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
        let string_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    string_view_expr(RustExpr::Raw("custom_string_view(value)".to_string())),
                )],
            }
        );
        assert_eq!(
            string_projected.recorded_shape(),
            Some(RustExprShape::OptionalProjected)
        );
        assert_eq!(string_projected.recorded_slot_shape(), Some(&string_target));
        let raw_string_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    string_view_expr(RustExpr::Raw("custom_string_view(value)".to_string())),
                )],
            }
        );

        let builtin_string_projected = optional_element_projection(
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
        let slice_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    slice_view_expr(RustExpr::Raw("custom_slice_view(value)".to_string())),
                )],
            }
        );
        assert_eq!(slice_projected.recorded_slot_shape(), Some(&slice_target));

        let builtin_slice_projected = optional_element_projection(
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
        let builtin_mut_slice_projected = optional_element_projection(
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
        let custom_mut_slice_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    mutable_slice_view_expr(RustExpr::Raw(
                        "custom_mut_slice_view(value)".to_string()
                    )),
                )],
            }
        );
        assert_eq!(
            custom_mut_slice_projected.recorded_slot_shape(),
            Some(&mut_slice_target)
        );

        let owned_target = RustSlotShape::conservative(JavaType::String);
        let owned_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    RustExpr::Raw("custom_owned_string(value)".to_string()),
                )],
            }
        );
        assert_eq!(
            owned_projected.recorded_shape(),
            Some(RustExprShape::OptionalProjected)
        );
        assert_eq!(owned_projected.recorded_slot_shape(), Some(&owned_target));
        let raw_owned_projected = optional_element_projection(
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
                args: vec![single_param_closure_expr(
                    "value",
                    RustExpr::Raw("custom_owned_string(value)".to_string()),
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

        let raw_index_cast = apply_one(
            RustExpr::Raw("left + right".to_string()),
            &Coercion::IndexToUsize,
        );
        assert_eq!(emit::emit_expr(&raw_index_cast), "(left + right) as usize");
        assert_eq!(
            raw_index_cast,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
                ty: "usize".to_string(),
            }
        );

        let already_usize = apply_one(
            RustExpr::Raw("index as usize".to_string()),
            &Coercion::IndexToUsize,
        );
        assert_eq!(
            already_usize,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("index".to_string())),
                ty: "usize".to_string(),
            }
        );
        assert!(
            usize_cast_projection(RustExpr::Raw("opaque ? value as usize".to_string())).is_none(),
            "suffix-only raw text should not count as a structured usize cast"
        );

        let structured_usize = RustExpr::Cast {
            expr: Box::new(RustExpr::Var("index".to_string())),
            ty: "usize".to_string(),
        };
        let recovered_structured_usize =
            apply_one(structured_usize.clone(), &Coercion::IndexToUsize);
        assert_eq!(recovered_structured_usize, structured_usize);

        let decimal_index = index_to_usize_projection(
            RustExpr::Raw("12".to_string()),
            IndexToUsizeProjection::JavaStringCharAt,
        );
        assert_eq!(emit::emit_expr(&decimal_index), "12 as usize");
        assert!(matches!(
            decimal_index,
            RustExpr::Cast { expr, ty }
                if ty == "usize"
                    && matches!(expr.as_ref(), RustExpr::Literal(rendered) if rendered == "12")
        ));

        let raw_simple_index = index_to_usize_projection(
            RustExpr::Raw("index".to_string()),
            IndexToUsizeProjection::StreamLambdaArgument,
        );
        assert_eq!(emit::emit_expr(&raw_simple_index), "index as usize");
        assert!(matches!(
            raw_simple_index,
            RustExpr::Cast { expr, ty }
                if ty == "usize"
                    && matches!(expr.as_ref(), RustExpr::Var(name) if name == "index")
        ));

        let raw_compound_index = index_to_usize_projection(
            RustExpr::Raw("left + right".to_string()),
            IndexToUsizeProjection::StreamLambdaArgument,
        );
        assert_eq!(
            emit::emit_expr(&raw_compound_index),
            "(left + right) as usize"
        );
        assert!(matches!(
            raw_compound_index,
            RustExpr::Cast { expr, ty }
                if ty == "usize"
                    && matches!(expr.as_ref(), RustExpr::Grouped(grouped)
                        if matches!(grouped.as_ref(), RustExpr::Binary { op, left, right }
                            if op == "+"
                                && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")))
        ));
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
        assert!(matches!(
            expr.into_unshaped(),
            RustExpr::Call { function, .. } if matches!(function.as_ref(), RustExpr::Path(path) if path == &vec!["Some".to_string()])
        ));

        let opaque = RustExpr::shaped(
            RustExprShape::OptionalConstructed,
            RustExpr::Raw("custom_present_value()".to_string()),
        );
        assert!(rust_expr_is_constructed_optional_value(&opaque));

        let recovered_get = rendered_projection_or_raw_leaf("values.get(index).copied()");
        assert!(
            !matches!(recovered_get.unshaped(), RustExpr::Raw(_)),
            "parseable get/copied optional carriers should use structured evidence before raw text"
        );
        assert!(rust_expr_is_constructed_optional_value(&RustExpr::Raw(
            "values.get(index).copied()".to_string()
        )));
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
        assert_eq!(emit::emit_expr(&raw), "Some(&value.as_str())");
        assert_eq!(raw.recorded_shape(), None);
        assert!(
            matches!(raw.unshaped(), RustExpr::Raw(rendered) if rendered == "Some(&value.as_str())")
        );

        let dotted_raw = normalize_optional_constructed_string_view_expr(RustExpr::Raw(
            "Some(&owner.value().as_str())".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&dotted_raw),
            "Some(&owner.value().as_str())"
        );
        assert!(
            matches!(dotted_raw.unshaped(), RustExpr::Raw(rendered) if rendered == "Some(&owner.value().as_str())")
        );

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
        let keyword_field = constructor_self_field_projection("type");
        assert_eq!(emit::emit_expr(&keyword_field), "__java_self.r#type");
        assert_eq!(
            keyword_field,
            RustExpr::Field(
                Box::new(RustExpr::Var("__java_self".to_string())),
                "r#type".to_string(),
            )
        );
        let self_field = constructor_self_field_projection("self");
        assert_eq!(emit::emit_expr(&self_field), "__java_self.self__");
        let crate_field = constructor_self_field_projection("crate");
        assert_eq!(emit::emit_expr(&crate_field), "__java_self.crate__");
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

        let outer_field = outer_this_field_projection("Limit");
        assert_eq!(emit::emit_expr(&outer_field), "self.__outer.borrow().limit");
        assert_eq!(
            outer_field,
            RustExpr::Field(
                Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Field(
                        Box::new(RustExpr::Var("self".to_string())),
                        "__outer".to_string(),
                    )),
                    method: "borrow".to_string(),
                    args: Vec::new(),
                }),
                "limit".to_string(),
            )
        );

        let outer_mut_field = outer_this_field_mut_projection("Value");
        assert_eq!(
            emit::emit_expr(&outer_mut_field),
            "self.__outer.borrow_mut().value"
        );
        assert_eq!(
            outer_mut_field,
            RustExpr::Field(
                Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Field(
                        Box::new(RustExpr::Var("self".to_string())),
                        "__outer".to_string(),
                    )),
                    method: "borrow_mut".to_string(),
                    args: Vec::new(),
                }),
                "value".to_string(),
            )
        );
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
        assert!(matches!(
            expr.unshaped(),
            RustExpr::MethodCall { receiver, method, .. }
                if method == "as_str"
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "partition")
        ));

        let raw_method_chain = apply_coercions(
            RustExpr::Raw("name.as_ref().unwrap()".to_string()),
            &[Coercion::AsStr],
        );
        assert_eq!(
            emit::emit_expr(&raw_method_chain),
            "name.as_ref().unwrap().as_str()"
        );
        assert!(matches!(
            raw_method_chain.unshaped(),
            RustExpr::MethodCall { receiver, method, .. }
                if method == "as_str"
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { method, .. } if method == "unwrap"
                    )
        ));
    }

    #[test]
    fn parse_receiver_rendering_removes_redundant_shared_borrow() {
        assert_eq!(
            render_parse_receiver(RustExpr::Raw("&value".to_string())),
            "value"
        );
        assert_eq!(
            render_parse_receiver(RustExpr::Raw("&next.value()".to_string())),
            "next.value()"
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
        let collect = typed_collect_projection(RustExpr::Var("items".to_string()), "Vec<_>");
        assert_eq!(emit::emit_expr(&collect), "items.collect::<Vec<_>>()");
        assert!(matches!(
            collect,
            RustExpr::MethodCall { method, args, .. }
                if method == "collect::<Vec<_>>" && args.is_empty()
        ));

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

        let raw_borrow =
            primitive_parse_projection(RustExpr::Raw("&value".to_string()), PrimitiveType::Int);
        assert_eq!(
            emit::emit_expr(&raw_borrow),
            "value.parse::<i32>().unwrap()"
        );
        assert_eq!(
            raw_borrow,
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
        assert_eq!(
            emit::emit_expr(&primitive_parse_projection(
                RustExpr::Raw("&tokenizer.next_token()".to_string()),
                PrimitiveType::Float,
            )),
            "tokenizer.next_token().parse::<f32>().unwrap()"
        );
        assert_eq!(
            primitive_parse_projection(
                RustExpr::Raw("value.as_str()".to_string()),
                PrimitiveType::Double,
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("value".to_string())),
                        method: "as_str".to_string(),
                        args: Vec::new(),
                    }),
                    method: "parse::<f64>".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&primitive_parse_projection(
                RustExpr::Raw("value.as_str()".to_string()),
                PrimitiveType::Double,
            )),
            "value.as_str().parse::<f64>().unwrap()"
        );
        let raw_call =
            primitive_parse_projection(RustExpr::Raw("factory()".to_string()), PrimitiveType::Long);
        assert_eq!(
            emit::emit_expr(&raw_call),
            "factory().parse::<i64>().unwrap()"
        );
        assert!(matches!(
            raw_call,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::MethodCall { receiver, method, args }
                        if method == "parse::<i64>"
                            && args.is_empty()
                            && matches!(receiver.as_ref(), RustExpr::Call { function, args }
                                if args.is_empty()
                                    && matches!(function.as_ref(), RustExpr::Var(name) if name == "factory")))
        ));
        let raw_method_receiver = primitive_parse_projection(
            RustExpr::Raw("value.clone()".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&raw_method_receiver),
            "value.clone().parse::<i32>().unwrap()"
        );
        assert!(matches!(
            raw_method_receiver,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::MethodCall { receiver, method, args }
                        if method == "parse::<i32>"
                            && args.is_empty()
                            && matches!(receiver.as_ref(), RustExpr::MethodCall { method, args, .. }
                                if method == "clone" && args.is_empty()))
        ));
        let compound = primitive_parse_projection(
            RustExpr::Raw("left + right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&compound),
            "(left + right).parse::<i32>().unwrap()"
        );
        assert!(matches!(
            compound,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap"
                    && args.is_empty()
                            && matches!(receiver.as_ref(), RustExpr::MethodCall { receiver, method, args }
                                if method == "parse::<i32>"
                                    && args.is_empty()
                                    && matches!(receiver.as_ref(), RustExpr::Binary { op, left, right }
                                        if op == "+"
                                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right"))
                                    || matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                                        if matches!(
                                            grouped.as_ref(),
                                            RustExpr::Binary { op, left, right }
                                                if op == "+"
                                                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                                )))
        ));
        let borrowed_compound = primitive_parse_projection(
            RustExpr::Raw("&left + right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&borrowed_compound),
            "(left + right).parse::<i32>().unwrap()"
        );
        assert!(matches!(
            borrowed_compound,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), RustExpr::MethodCall { receiver, .. }
                    if matches!(receiver.as_ref(), RustExpr::Binary { op, left, right }
                        if op == "+"
                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right"))
                    || matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                        if matches!(
                            grouped.as_ref(),
                            RustExpr::Binary { op, left, right }
                                if op == "+"
                                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                    && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                        )))
        ));
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
        let raw_boolean_hash = primitive_hash_code_projection(
            RustExpr::Raw("left == right".to_string()),
            PrimitiveType::Boolean,
        );
        assert_eq!(
            emit::emit_expr(&raw_boolean_hash),
            "if left == right { 1231 } else { 1237 }"
        );
        assert!(matches!(raw_boolean_hash, RustExpr::If { .. }));

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
        let raw_int_hash = primitive_hash_code_projection(
            RustExpr::Raw("left + right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(emit::emit_expr(&raw_int_hash), "(left + right) as i32");
        assert_eq!(
            raw_int_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
                ty: "i32".to_string(),
            }
        );
        let structured_compound_int_hash = primitive_hash_code_projection(
            RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var("left".to_string())),
                right: Box::new(RustExpr::Var("right".to_string())),
            },
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&structured_compound_int_hash),
            "(left + right) as i32"
        );
        assert!(matches!(
            structured_compound_int_hash,
            RustExpr::Cast { expr, ty }
                if ty == "i32"
                    && matches!(expr.as_ref(), RustExpr::Binary { .. })
        ));
        let raw_float_hash = primitive_hash_code_projection(
            RustExpr::Raw("left + right".to_string()),
            PrimitiveType::Float,
        );
        assert_eq!(
            emit::emit_expr(&raw_float_hash),
            "(left + right).to_bits() as i32"
        );
        assert_eq!(
            raw_float_hash,
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                        op: "+".to_string(),
                        left: Box::new(RustExpr::Var("left".to_string())),
                        right: Box::new(RustExpr::Var("right".to_string())),
                    }))),
                    method: "to_bits".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        let raw_long_hash = primitive_hash_code_projection(
            RustExpr::Raw("left + right".to_string()),
            PrimitiveType::Long,
        );
        assert_eq!(
            emit::emit_expr(&raw_long_hash),
            "((left + right) as i64 ^ ((left + right) as i64) >> 32) as i32"
        );
        assert!(matches!(raw_long_hash, RustExpr::Cast { .. }));
        let raw_double_hash = primitive_hash_code_projection(
            RustExpr::Raw("left + right".to_string()),
            PrimitiveType::Double,
        );
        assert_eq!(
            emit::emit_expr(&raw_double_hash),
            "((left + right).to_bits() ^ (left + right).to_bits() >> 32) as i32"
        );
        assert!(matches!(raw_double_hash, RustExpr::Cast { .. }));
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
                    args: vec![RustExpr::Cast {
                        expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Var(
                            "index".to_string(),
                        )))),
                        ty: "usize".to_string(),
                    }],
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
                    args: vec![RustExpr::Cast {
                        expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Var(
                            "index".to_string(),
                        )))),
                        ty: "usize".to_string(),
                    }],
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
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("0".to_string()),
            )),
            "value.chars().nth(0 as usize).expect(\"Java string index out of bounds\")"
        );
        let raw_integer_index = string_char_at_projection(
            RustExpr::Var("value".to_string()),
            RustExpr::Raw("12".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&raw_integer_index),
            "value.chars().nth(12 as usize).expect(\"Java string index out of bounds\")"
        );
        assert!(matches!(
            raw_integer_index,
            RustExpr::MethodCall { receiver, method, args }
                if method == "expect"
                    && args.len() == 1
                    && matches!(receiver.as_ref(), RustExpr::MethodCall { args, .. }
                        if matches!(args.as_slice(), [RustExpr::Cast { expr, ty }]
                            if ty == "usize"
                                && matches!(expr.as_ref(), RustExpr::Literal(literal) if literal == "12")))
        ));
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("index".to_string())),
                    ty: "usize".to_string(),
                },
            )),
            "value.chars().nth(index as usize).expect(\"Java string index out of bounds\")"
        );
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Raw("index as usize".to_string()),
            )),
            "value.chars().nth(index as usize).expect(\"Java string index out of bounds\")"
        );
        let raw_usize_index = string_char_at_projection(
            RustExpr::Var("value".to_string()),
            RustExpr::Raw("index as usize".to_string()),
        );
        assert!(matches!(
            raw_usize_index,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), RustExpr::MethodCall { args, .. }
                    if matches!(args.as_slice(), [RustExpr::Cast { expr, ty }]
                        if ty == "usize"
                            && matches!(expr.as_ref(), RustExpr::Var(name) if name == "index")))
        ));
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
            string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("'='".to_string()),
                None,
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_string_index_of"])),
                args: vec![
                    RustExpr::Unary {
                        op: "&".to_string(),
                        expr: Box::new(RustExpr::Var("value".to_string())),
                    },
                    RustExpr::Literal("\"=\"".to_string()),
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
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "ex".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(RustExpr::MacroCall {
                        macro_name: "panic".to_string(),
                        args: vec![
                            RustExpr::Literal("\"uncaught Java exception: {:?}\"".to_string()),
                            RustExpr::Var("ex".to_string()),
                        ],
                    })),
                }],
            }
        );
        assert_eq!(
            unwrap_result_or_panic_uncaught_java_exception(RustExpr::Raw(
                "fallible()?".to_string()
            )),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::Var("fallible".to_string())),
                    args: Vec::new(),
                }),
                method: "unwrap_or_else".to_string(),
                args: vec![panic_uncaught_java_exception_closure_expr()],
            }
        );
        let nested_try = unwrap_result_or_panic_uncaught_java_exception(RustExpr::Raw(
            "outer(inner()?)?".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&nested_try),
            "outer(inner().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
        assert!(matches!(
            nested_try,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap_or_else"
                    && args.len() == 1
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::Call { function, args }
                            if matches!(function.as_ref(), RustExpr::Var(name) if name == "outer")
                                && matches!(
                                    args.as_slice(),
                                    [RustExpr::MethodCall { receiver, method, args }]
                                        if method == "unwrap_or_else"
                                            && args.len() == 1
                                            && matches!(
                                                receiver.as_ref(),
                                                RustExpr::Call { function, args }
                                                    if args.is_empty()
                                                        && matches!(function.as_ref(), RustExpr::Var(name) if name == "inner")
                                            )
                                )
                    )
        ));
        let raw_binary = unwrap_result_or_panic_uncaught_java_exception(RustExpr::Raw(
            "left + right".to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&raw_binary),
            "(left + right).unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
        assert!(matches!(
            raw_binary,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap_or_else"
                    && args.len() == 1
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::Binary { op, left, right }
                            if op == "+"
                                && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                    )
        ));
        let nested_raw_binary_child =
            unwrap_result_or_panic_uncaught_java_exception(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var("base".to_string())),
                right: Box::new(RustExpr::Raw("left + fallible()?".to_string())),
            });
        assert_eq!(
            emit::emit_expr(&nested_raw_binary_child),
            "base + (left + fallible().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex)))"
        );
        assert!(matches!(
            nested_raw_binary_child,
            RustExpr::Binary { op, left, right }
                if op == "+"
                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "base")
                    && matches!(
                        right.as_ref(),
                        RustExpr::Binary { op, right, .. }
                            if op == "+"
                                && matches!(
                                    right.as_ref(),
                                    RustExpr::MethodCall { receiver, method, args }
                                        if method == "unwrap_or_else"
                                            && args.len() == 1
                                            && matches!(
                                                receiver.as_ref(),
                                                RustExpr::Call { function, args }
                                                    if args.is_empty()
                                                        && matches!(function.as_ref(), RustExpr::Var(name) if name == "fallible")
                                            )
                                )
                    )
        ));
        let already_recovered = unwrap_result_or_panic_uncaught_java_exception(RustExpr::Raw(
            "fallible().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
                .to_string(),
        ));
        assert_eq!(
            emit::emit_expr(&already_recovered),
            "fallible().unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex))"
        );
        assert!(matches!(
            already_recovered,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap_or_else"
                    && args.len() == 1
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::Call { function, args }
                            if args.is_empty()
                                && matches!(function.as_ref(), RustExpr::Var(name) if name == "fallible")
                    )
        ));
        let panic_stmt =
            if_err_panic_uncaught_java_exception_stmt(RustExpr::Var("close_result".to_string()));
        assert_eq!(
            emit::emit_statements(&[panic_stmt.clone()], 1),
            "    if let Err(ex) = close_result {\n        panic!(\"uncaught Java exception: {:?}\", ex);\n    }\n"
        );
        assert_eq!(
            panic_stmt,
            RustStmt::IfLet {
                pattern: "Err(ex)".to_string(),
                expr: RustExpr::Var("close_result".to_string()),
                then_body: vec![RustStmt::Expr(RustExpr::MacroCall {
                    macro_name: "panic".to_string(),
                    args: vec![
                        RustExpr::Literal("\"uncaught Java exception: {:?}\"".to_string()),
                        RustExpr::Var("ex".to_string()),
                    ],
                })],
                else_body: Vec::new(),
            }
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
            "std::thread::sleep(std::time::Duration::from_millis((left + right) as u64))"
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
            RustExpr::MacroCall { macro_name, args } if macro_name == "println" && args == vec![
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
            RustExpr::MacroCall { macro_name, args } if macro_name == "println" && args.is_empty()
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
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "&&".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
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
                    args: vec![value_clone_closure()],
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
                    args: vec![value_clone_closure()],
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
                    args: vec![value_clone_closure()],
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
                    args: vec![value_clone_closure()],
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
            anonymous.clone().into_unshaped(),
            RustExpr::Call {
                function: Box::new(RustExpr::Path(vec![
                    "<dyn Callback>".to_string(),
                    "anonymous".to_string(),
                ])),
                args: Vec::new(),
            }
        );
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
        assert_eq!(
            typed_none.clone().into_unshaped(),
            RustExpr::GenericPath {
                base: vec!["None".to_string()],
                args: vec![RustType::Raw("Box<dyn std::any::Any>".to_string())],
                member: None,
            }
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
            RustExpr::GenericPath { .. }
        ));
        assert_eq!(
            emit::emit_expr(&java_class_for_name_path_projection("Owner")),
            "JavaClass::<Owner>::for_name"
        );
        assert_eq!(
            java_class_for_name_path_projection("Owner"),
            RustExpr::GenericPath {
                base: vec!["JavaClass".to_string()],
                args: vec![RustType::Raw("Owner".to_string())],
                member: Some("for_name".to_string()),
            }
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
            emit::emit_expr(&rust_type_associated_path_projection(
                RustType::Generic {
                    base: "std::collections::HashSet".to_string(),
                    args: vec![RustType::String],
                },
                "new",
            )),
            "std::collections::HashSet::<String>::new"
        );
        let ref_type_associated_path = rust_type_associated_path_projection(
            RustType::Ref {
                mutable: false,
                lifetime: None,
                inner: Box::new(RustType::String),
            },
            "new",
        );
        assert_eq!(emit::emit_expr(&ref_type_associated_path), "<&String>::new");
        assert!(matches!(
            ref_type_associated_path,
            RustExpr::TypeAssociatedPath { ty, member }
                if member == "new" && matches!(ty, RustType::Ref { .. })
        ));
        let subtype_ctor = subtype_owned_constructor_from_borrowed_projection(
            "pkg::Outer::<T>",
            RustExpr::Var("value".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&subtype_ctor),
            "pkg::Outer::<T>::new_1(value.clone())"
        );
        assert!(matches!(
            subtype_ctor,
            RustExpr::Call { function, args }
                if matches!(function.as_ref(), RustExpr::Path(path)
                    if path == &vec!["pkg".to_string(), "Outer::<T>".to_string(), "new_1".to_string()])
                    && matches!(args.as_slice(), [arg]
                        if matches!(arg.unshaped(), RustExpr::MethodCall { method, .. }
                            if method == "clone"))
        ));
        assert_eq!(
            emit::emit_expr(&static_storage_accessor_projection("VALUES")),
            "Self::__static_v_a_l_u_e_s()"
        );
        assert!(matches!(
            static_storage_accessor_projection("VALUES"),
            RustExpr::Call { function, args }
                if matches!(function.unshaped(), RustExpr::Path(_)) && args.is_empty()
        ));
        let static_storage_getter = static_storage_getter_body_projection(
            "VALUES_STORAGE",
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "new"])),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&static_storage_getter),
            "VALUES_STORAGE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone().unwrap_or_else(|| Vec::new())"
        );
        assert!(matches!(
            static_storage_getter,
            RustExpr::MethodCall { args, .. }
                if matches!(args.as_slice(), [RustExpr::Closure { .. }])
        ));
        assert_eq!(
            emit::emit_expr(&static_storage_assignment_value_projection(
                static_storage_setter_path_projection("VALUES"),
                RustExpr::Var("next".to_string()),
                static_storage_accessor_projection("VALUES"),
            )),
            "{ Self::__set_static_v_a_l_u_e_s(next); Self::__static_v_a_l_u_e_s() }"
        );
        assert!(matches!(
            static_storage_assignment_value_projection(
                static_storage_setter_path_projection("VALUES"),
                RustExpr::Var("next".to_string()),
                static_storage_accessor_projection("VALUES"),
            ),
            RustExpr::Block {
                stmts,
                tail: Some(_)
            } if matches!(stmts.as_slice(), [RustStmt::Expr(RustExpr::Call { .. })])
        ));
        let static_storage_setter = static_storage_setter_body_projection("VALUES_STORAGE");
        assert_eq!(
            emit::emit_statements(&[static_storage_setter.clone()], 0),
            "*VALUES_STORAGE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);\n"
        );
        assert!(matches!(
            static_storage_setter,
            RustStmt::Assign {
                target: RustExpr::Unary { ref op, .. },
                value: RustExpr::Call { .. },
            } if op == "*"
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
            render_function_call_projection(
                owner_associated_path_projection("Owner", "build"),
                vec!["left + right".to_string()],
            ),
            "Owner::build(left + right)"
        );
        assert_eq!(
            render_receiver_method_call_projection(
                "owner".to_string(),
                "getValue",
                vec!["left + right".to_string()],
            ),
            "owner.get_value(left + right)"
        );
        assert_eq!(
            render_current_receiver_method_call_projection("emit", vec!["arg".to_string()]),
            "self.emit(arg)"
        );
        assert_eq!(
            emit::emit_expr(&current_receiver_method_call_projection(
                "emit",
                vec![RustExpr::Raw("value".to_string())],
            )),
            "self.emit(value)"
        );
        assert_eq!(
            current_receiver_method_call_projection(
                "emit",
                vec![RustExpr::Raw("value".to_string())],
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("self".to_string())),
                method: "emit".to_string(),
                args: vec![RustExpr::Raw("value".to_string())],
            }
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
        let prefix_update =
            java_update_expression_projection(RustExpr::Var("i".to_string()), "+=", true);
        assert_eq!(emit::emit_expr(&prefix_update), "{ i += 1; i }");
        assert!(matches!(
            prefix_update,
            RustExpr::Block {
                stmts,
                tail: Some(tail),
            } if matches!(stmts.as_slice(), [RustStmt::AssignOp { op, .. }] if op == "+=")
                && matches!(*tail, RustExpr::Var(ref name) if name == "i")
        ));
        let postfix_update =
            java_update_expression_projection(RustExpr::Var("i".to_string()), "+=", false);
        assert_eq!(
            emit::emit_expr(&postfix_update),
            "{ let __java_old_value = i; i += 1; __java_old_value }"
        );
        assert!(matches!(
            postfix_update,
            RustExpr::Block {
                stmts,
                tail: Some(tail),
            } if matches!(stmts.as_slice(), [
                RustStmt::Let { name, .. },
                RustStmt::AssignOp { op, .. },
            ] if name == "__java_old_value" && op == "+=")
                && matches!(*tail, RustExpr::Var(ref name) if name == "__java_old_value")
        ));
        let primitive_prefix_update = java_primitive_update_expression_projection(
            RustExpr::Var("i".to_string()),
            true,
            PrimitiveType::Int,
            true,
        )
        .expect("int primitive update expression should lower");
        assert_eq!(
            emit::emit_expr(&primitive_prefix_update),
            "{ i = (i).wrapping_add(1); i }"
        );
        assert!(matches!(
            primitive_prefix_update,
            RustExpr::Block {
                stmts,
                tail: Some(tail),
            } if matches!(stmts.as_slice(), [RustStmt::Assign { .. }])
                && matches!(*tail, RustExpr::Var(ref name) if name == "i")
        ));
        let updated_value = java_primitive_updated_value_projection(
            RustExpr::Var("i".to_string()),
            true,
            PrimitiveType::Int,
        )
        .expect("int update value should lower");
        assert_eq!(emit::emit_expr(&updated_value), "(i).wrapping_add(1)");
        assert_eq!(
            updated_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Grouped(Box::new(RustExpr::Var("i".to_string(),)))),
                method: "wrapping_add".to_string(),
                args: vec![RustExpr::Literal("1".to_string())],
            }
        );
        let raw_updated_value = java_primitive_updated_value_projection(
            RustExpr::Raw("left + right".to_string()),
            false,
            PrimitiveType::Long,
        )
        .expect("long update value should lower");
        assert_eq!(
            emit::emit_expr(&raw_updated_value),
            "(left + right).wrapping_sub(1)"
        );
        assert_eq!(
            raw_updated_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
                method: "wrapping_sub".to_string(),
                args: vec![RustExpr::Literal("1".to_string())],
            }
        );
        let char_updated_value = java_primitive_updated_value_projection(
            RustExpr::Var("ch".to_string()),
            true,
            PrimitiveType::Char,
        )
        .expect("char update value should lower");
        assert_eq!(
            emit::emit_expr(&char_updated_value),
            "char::from_u32(((ch as u32) as i32).wrapping_add(1) as u32 & 0xffff).unwrap_or('\\0')"
        );
        assert!(matches!(
            char_updated_value,
            RustExpr::MethodCall { receiver, method, args }
                if method == "unwrap_or"
                    && matches!(args.as_slice(), [RustExpr::Literal(fallback)] if fallback == "'\\0'")
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::Call { function, args }
                            if matches!(function.as_ref(), RustExpr::Path(path)
                                if path == &vec!["char".to_string(), "from_u32".to_string()])
                                && args.len() == 1
                    )
        ));
        let raw_char_updated_value = java_primitive_updated_value_projection(
            RustExpr::Raw("left + right".to_string()),
            false,
            PrimitiveType::Char,
        )
        .expect("raw char update value should lower");
        assert_eq!(
            emit::emit_expr(&raw_char_updated_value),
            "char::from_u32((((left + right) as u32) as i32).wrapping_sub(1) as u32 & 0xffff).unwrap_or('\\0')"
        );
        assert!(!matches!(
            raw_char_updated_value.into_unshaped(),
            RustExpr::Raw(_)
        ));
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
        let arrays_equals_range = arrays_equals_range_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("left_from".to_string()),
            RustExpr::Var("left_to".to_string()),
            RustExpr::Var("right".to_string()),
            RustExpr::Var("right_from".to_string()),
            RustExpr::Var("right_to".to_string()),
        );
        let arrays_equals_range_rendered = emit::emit_expr(&arrays_equals_range);
        assert!(
            arrays_equals_range_rendered.contains("let __java_arrays_left = &left")
                && arrays_equals_range_rendered.contains("let __java_arrays_right = &right")
                && arrays_equals_range_rendered
                    .contains("Java IllegalArgumentException: fromIndex > toIndex")
                && arrays_equals_range_rendered.contains("Java ArrayIndexOutOfBoundsException")
                && arrays_equals_range_rendered.contains("java_arrays_equals(&__java_arrays_left")
                && arrays_equals_range_rendered.contains("&__java_arrays_right")
        );
        assert!(matches!(
            arrays_equals_range,
            RustExpr::Block {
                tail: Some(tail),
                ..
            } if matches!(*tail, RustExpr::Call { .. })
        ));
        let arrays_binary_search = arrays_binary_search_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("key".to_string()),
        );
        let arrays_binary_search_rendered = emit::emit_expr(&arrays_binary_search);
        assert!(
            arrays_binary_search_rendered.contains("let __java_arrays_source")
                && arrays_binary_search_rendered.contains("let __java_arrays_key")
                && arrays_binary_search_rendered.contains(".binary_search(&__java_arrays_key)")
                && arrays_binary_search_rendered.contains("Ok(__java_arrays_index)")
                && arrays_binary_search_rendered.contains("Err(__java_arrays_insert)")
        );
        assert!(matches!(
            arrays_binary_search,
            RustExpr::Block {
                tail: Some(tail),
                ..
            } if matches!(*tail, RustExpr::Match { .. })
        ));
        let arrays_binary_search_range = arrays_binary_search_range_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("from".to_string()),
            RustExpr::Var("to".to_string()),
            RustExpr::Var("key".to_string()),
        );
        let arrays_binary_search_range_rendered = emit::emit_expr(&arrays_binary_search_range);
        assert!(
            arrays_binary_search_range_rendered.contains("let __java_arrays_from = from")
                && arrays_binary_search_range_rendered
                    .contains("Java IllegalArgumentException: fromIndex > toIndex")
                && arrays_binary_search_range_rendered
                    .contains("Java ArrayIndexOutOfBoundsException")
                && arrays_binary_search_range_rendered
                    .contains("__java_arrays_source[__java_arrays_from..__java_arrays_to]")
                && arrays_binary_search_range_rendered.contains("Ok(__java_arrays_index)")
                && arrays_binary_search_range_rendered.contains("Err(__java_arrays_insert)")
        );
        assert!(matches!(
            arrays_binary_search_range,
            RustExpr::Block {
                tail: Some(tail),
                ..
            } if matches!(*tail, RustExpr::Match { .. })
        ));
        let arrays_copy_of = arrays_copy_of_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("len".to_string()),
        );
        let arrays_copy_of_rendered = emit::emit_expr(&arrays_copy_of);
        assert!(
            arrays_copy_of_rendered.contains("let __java_arrays_source = &values")
                && arrays_copy_of_rendered.contains("let __java_arrays_len = len")
                && arrays_copy_of_rendered.contains("Java NegativeArraySizeException")
                && arrays_copy_of_rendered
                    .contains(".iter().cloned().take(__java_arrays_len).collect::<Vec<_>>()")
                && arrays_copy_of_rendered
                    .contains("__java_arrays_out.resize(__java_arrays_len, Default::default())")
        );
        assert!(matches!(
            arrays_copy_of,
            RustExpr::Block {
                tail: Some(tail),
                ..
            } if matches!(*tail, RustExpr::Var(_))
        ));
        let arrays_copy_of_range = arrays_copy_of_range_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("from".to_string()),
            RustExpr::Var("to".to_string()),
        );
        let arrays_copy_of_range_rendered = emit::emit_expr(&arrays_copy_of_range);
        assert!(
            arrays_copy_of_range_rendered.contains("let __java_arrays_from = from")
                && arrays_copy_of_range_rendered
                    .contains("Java IllegalArgumentException: fromIndex > toIndex")
                && arrays_copy_of_range_rendered.contains("Java ArrayIndexOutOfBoundsException")
                && arrays_copy_of_range_rendered.contains(
                    "__java_arrays_source[__java_arrays_from..__java_arrays_source.len().min(__java_arrays_to)].to_vec()"
                )
                && arrays_copy_of_range_rendered
                    .contains("__java_arrays_out.resize(__java_arrays_len, Default::default())")
        );
        assert!(matches!(
            arrays_copy_of_range,
            RustExpr::Block {
                tail: Some(tail),
                ..
            } if matches!(*tail, RustExpr::Var(_))
        ));
        let arrays_compare_range = arrays_compare_range_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("left_from".to_string()),
            RustExpr::Var("left_to".to_string()),
            RustExpr::Var("right".to_string()),
            RustExpr::Var("right_from".to_string()),
            RustExpr::Var("right_to".to_string()),
        );
        assert!(
            emit::emit_expr(&arrays_compare_range).contains("java_arrays_compare(")
                && matches!(
                    arrays_compare_range,
                    RustExpr::Block {
                        tail: Some(tail),
                        ..
                    } if matches!(*tail, RustExpr::Call { .. })
                )
        );
        let arrays_mismatch_range = arrays_mismatch_range_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Var("left_from".to_string()),
            RustExpr::Var("left_to".to_string()),
            RustExpr::Var("right".to_string()),
            RustExpr::Var("right_from".to_string()),
            RustExpr::Var("right_to".to_string()),
        );
        assert!(
            emit::emit_expr(&arrays_mismatch_range).contains("java_arrays_mismatch(")
                && matches!(
                    arrays_mismatch_range,
                    RustExpr::Block {
                        tail: Some(tail),
                        ..
                    } if matches!(*tail, RustExpr::Call { .. })
                )
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
            emit::emit_expr(&collections_n_copies_projection(
                RustExpr::Raw("left + right".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "vec![value; (left + right) as usize]"
        );
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
            "Vec::with_capacity((left + right) as usize)"
        );
        assert_eq!(
            native_collection_with_capacity_projection(
                crate::stdlib::JavaCollectionKind::List,
                RustExpr::Raw("names.as_ref().unwrap().len()".to_string()),
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["Vec", "with_capacity"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::MethodCall {
                                receiver: Box::new(RustExpr::Var("names".to_string())),
                                method: "as_ref".to_string(),
                                args: Vec::new(),
                            }),
                            method: "unwrap".to_string(),
                            args: Vec::new(),
                        }),
                        method: "len".to_string(),
                        args: Vec::new(),
                    }),
                    ty: "usize".to_string(),
                }],
            }
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
        let borrowed_flat_map =
            borrowed_collection_flat_map_projection(RustExpr::Var("groups".to_string()), "group");
        assert_eq!(
            emit::emit_expr(&borrowed_flat_map),
            "groups.flat_map(|group| group.iter())"
        );
        assert!(matches!(
            borrowed_flat_map,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "groups")
                    && method == "flat_map"
                    && matches!(args.as_slice(), [RustExpr::Closure { params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "group")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::MethodCall { receiver, method, args }
                                    if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "group")
                                        && method == "iter"
                                        && args.is_empty())))
        ));
        assert_eq!(
            render_borrowed_collection_flat_map("group"),
            ".flat_map(|group| group.iter())"
        );
        let to_map = stream_collect_to_map_projection(
            RustExpr::Var("values".to_string()),
            "v",
            RustExpr::Var("key".to_string()),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("v".to_string())),
                method: "len".to_string(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&to_map),
            "values.map(|v| (key, v.len())).collect::<std::collections::HashMap<_, _>>()"
        );
        assert!(matches!(
            to_map,
            RustExpr::MethodCall { receiver, method, args }
                if method == "collect::<std::collections::HashMap<_, _>>"
                    && args.is_empty()
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { method, args, .. }
                            if method == "map"
                                && matches!(args.as_slice(), [RustExpr::Closure { body, .. }]
                                    if matches!(body, RustClosureBody::Expr(expr)
                                        if matches!(expr.as_ref(), RustExpr::Tuple(items)
                                            if items.len() == 2)))
                    )
        ));
        assert_eq!(
            render_stream_collect_to_map_terminal(
                "v",
                RustExpr::Var("key".to_string()),
                RustExpr::Var("value".to_string()),
            ),
            ".map(|v| (key, value)).collect::<std::collections::HashMap<_, _>>()"
        );
        let join_terminal = stream_join_terminal_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("item".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            },
            RustExpr::Literal("\"|\"".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&join_terminal),
            "values.map(|item| item.to_string()).collect::<Vec<_>>().join(\"|\")"
        );
        assert!(matches!(
            join_terminal,
            RustExpr::MethodCall { receiver, method, args }
                if method == "join"
                    && matches!(args.as_slice(), [RustExpr::Literal(delimiter)] if delimiter == "\"|\"")
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "collect::<Vec<_>>"
                                && args.is_empty()
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::MethodCall { method, args, .. }
                                        if method == "map"
                                            && matches!(args.as_slice(), [RustExpr::Closure { body, .. }]
                                                if matches!(body, RustClosureBody::Expr(expr)
                                                    if matches!(expr.as_ref(), RustExpr::MethodCall { method, .. }
                                                        if method == "to_string")))
                                )
                    )
        ));
        assert_eq!(
            render_stream_join_terminal(
                RustExpr::Var("display".to_string()),
                RustExpr::Literal("\"\"".to_string()),
            ),
            ".map(|item| display).collect::<Vec<_>>().join(\"\")"
        );
        let any_match = stream_match_terminal_projection(
            RustExpr::Var("values".to_string()),
            "any",
            "value",
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "is_empty".to_string(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&any_match),
            "values.any(|value| value.is_empty())"
        );
        assert!(matches!(
            any_match,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && method == "any"
                    && matches!(args.as_slice(), [RustExpr::Closure { params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "value")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::MethodCall { method, .. }
                                    if method == "is_empty")))
        ));
        assert_eq!(
            render_stream_match_terminal("all", "value", RustExpr::Var("predicate".to_string()),),
            ".all(|value| predicate)"
        );
        let reduce = stream_reduce_terminal_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("empty".to_string()),
            "left",
            "right",
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("left".to_string())),
                method: "union".to_string(),
                args: vec![RustExpr::Var("right".to_string())],
            },
        );
        assert_eq!(
            emit::emit_expr(&reduce),
            "values.fold(empty, |left, right| left.union(right))"
        );
        assert!(matches!(
            reduce,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && method == "fold"
                    && matches!(args.as_slice(), [
                        RustExpr::Var(identity),
                        RustExpr::Closure { params, body, .. }
                    ] if identity == "empty"
                        && matches!(params.as_slice(), [
                            RustClosureParam { pattern: left, ty: None },
                            RustClosureParam { pattern: right, ty: None },
                        ] if left == "left" && right == "right")
                        && matches!(body, RustClosureBody::Expr(expr)
                            if matches!(expr.as_ref(), RustExpr::MethodCall { method, .. }
                                if method == "union")))
        ));
        assert_eq!(
            render_stream_reduce_terminal(
                RustExpr::Var("empty".to_string()),
                "left",
                "right",
                RustExpr::Var("body".to_string()),
            ),
            ".fold(empty, |left, right| body)"
        );
        let filter_op = stream_filter_op_projection(
            RustExpr::Var("values".to_string()),
            "name",
            RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("name".to_string())),
                    method: "is_empty".to_string(),
                    args: Vec::new(),
                }),
            },
            true,
        );
        assert_eq!(
            emit::emit_expr(&filter_op),
            "values.filter(move |name| !name.is_empty())"
        );
        assert!(matches!(
            filter_op,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && method == "filter"
                    && matches!(args.as_slice(), [RustExpr::Closure { is_move: true, params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "name")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::Unary { op, .. } if op == "!")))
        ));
        assert_eq!(
            render_stream_filter_op("name", RustExpr::Var("predicate".to_string()), true),
            ".filter(move |name| predicate)"
        );
        let map_op = stream_map_op_projection(
            RustExpr::Var("values".to_string()),
            "value: &String",
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "trim".to_string(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&map_op),
            "values.map(|value: &String| value.trim())"
        );
        assert!(matches!(
            map_op,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && method == "map"
                    && matches!(args.as_slice(), [RustExpr::Closure { params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "value: &String")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::MethodCall { method, .. }
                                    if method == "trim")))
        ));
        assert_eq!(
            render_stream_map_op("value", RustExpr::Var("body".to_string())),
            ".map(|value| body)"
        );
        let flat_map_op = stream_flat_map_op_projection(
            RustExpr::Var("groups".to_string()),
            "group",
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("group".to_string())),
                method: "iter".to_string(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&flat_map_op),
            "groups.flat_map(|group| group.iter())"
        );
        assert!(matches!(
            flat_map_op,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "groups")
                    && method == "flat_map"
                    && matches!(args.as_slice(), [RustExpr::Closure { params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "group")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::MethodCall { method, .. }
                                    if method == "iter")))
        ));
        assert_eq!(
            render_stream_flat_map_op("group", RustExpr::Var("body".to_string())),
            ".flat_map(|group| body)"
        );
        let prefixed_flat_map = stream_prefixed_flat_map_op_projection(
            RustExpr::Var("groups".to_string()),
            "group",
            vec![RustStmt::Let {
                mutable: false,
                name: "names".to_string(),
                ty: None,
                value: Some(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("group".to_string())),
                    method: "names".to_string(),
                    args: Vec::new(),
                }),
            }],
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("names".to_string())),
                method: "into_iter".to_string(),
                args: Vec::new(),
            },
        );
        assert_eq!(
            emit::emit_expr(&prefixed_flat_map),
            "groups.flat_map(|group| { let names = group.names(); names.into_iter() })"
        );
        assert!(matches!(
            prefixed_flat_map,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "groups")
                    && method == "flat_map"
                    && matches!(args.as_slice(), [RustExpr::Closure { body, .. }]
                        if matches!(body, RustClosureBody::Expr(expr)
                            if matches!(expr.as_ref(), RustExpr::Block { stmts, tail: Some(tail) }
                                if matches!(stmts.as_slice(), [RustStmt::Let { name, value: Some(value), .. }]
                                    if name == "names"
                                        && matches!(value, RustExpr::MethodCall { method, .. }
                                            if method == "names"))
                                    && matches!(tail.as_ref(), RustExpr::MethodCall { method, .. }
                                        if method == "into_iter"))))
        ));
        assert_eq!(
            render_stream_prefixed_flat_map_op_from_prefix_string(
                "group",
                "let names = group.names();".to_string(),
                RustExpr::Var("body".to_string()),
            ),
            ".flat_map(|group| { let names = group.names(); body })"
        );
        let expr_prefixed_flat_map = stream_prefixed_flat_map_op_projection(
            RustExpr::Var("groups".to_string()),
            "group",
            vec![RustStmt::Expr(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("group".to_string())),
                method: "prepare".to_string(),
                args: Vec::new(),
            })],
            RustExpr::Var("body".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&expr_prefixed_flat_map),
            "groups.flat_map(|group| { group.prepare(); body })"
        );
        assert!(matches!(
            expr_prefixed_flat_map,
            RustExpr::MethodCall { args, .. }
                if matches!(args.as_slice(), [RustExpr::Closure { body, .. }]
                    if matches!(body, RustClosureBody::Expr(expr)
                        if matches!(expr.as_ref(), RustExpr::Block { stmts, tail: Some(tail) }
                            if matches!(stmts.as_slice(), [RustStmt::Expr(RustExpr::MethodCall { method, .. })]
                                if method == "prepare")
                                && matches!(tail.as_ref(), RustExpr::Var(name) if name == "body"))))
        ));
        let borrowed_non_null =
            optional_non_null_filter_map_projection(RustExpr::Var("values".to_string()), true);
        assert_eq!(
            emit::emit_expr(&borrowed_non_null),
            "values.filter_map(|item| item.as_ref())"
        );
        assert!(matches!(
            borrowed_non_null,
            RustExpr::MethodCall { receiver, method, args }
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && method == "filter_map"
                    && matches!(args.as_slice(), [RustExpr::Closure { params, body, .. }]
                        if matches!(params.as_slice(), [RustClosureParam { pattern, ty: None }]
                            if pattern == "item")
                            && matches!(body, RustClosureBody::Expr(expr)
                                if matches!(expr.as_ref(), RustExpr::MethodCall { receiver, method, args }
                                    if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "item")
                                        && method == "as_ref"
                                        && args.is_empty())))
        ));
        let owned_non_null =
            optional_non_null_filter_map_projection(RustExpr::Var("values".to_string()), false);
        assert_eq!(
            emit::emit_expr(&owned_non_null),
            "values.filter_map(|item| item)"
        );
        assert!(matches!(
            owned_non_null,
            RustExpr::MethodCall { method, args, .. }
                if method == "filter_map"
                    && matches!(args.as_slice(), [RustExpr::Closure { body, .. }]
                        if matches!(body, RustClosureBody::Expr(expr)
                            if matches!(expr.as_ref(), RustExpr::Var(name) if name == "item")))
        ));
        assert_eq!(
            render_optional_non_null_filter_map(true),
            ".filter_map(|item| item.as_ref())"
        );
        assert_eq!(
            render_optional_non_null_filter_map(false),
            ".filter_map(|item| item)"
        );
        let borrowed_iteration = sequence_iteration_source_projection(
            RustExpr::Var("values".to_string()),
            false,
            SequenceIterationProjection::Borrowed,
        );
        assert_eq!(emit::emit_expr(&borrowed_iteration), "values.iter()");
        assert!(matches!(
            borrowed_iteration,
            RustExpr::MethodCall { ref method, .. } if method == "iter"
        ));
        assert_eq!(
            emit::emit_expr(&sequence_iteration_source_projection(
                RustExpr::Var("maybe_values".to_string()),
                true,
                SequenceIterationProjection::Borrowed,
            )),
            "maybe_values.as_ref().expect(\"Java null receiver\").iter()"
        );
        assert_eq!(
            emit::emit_expr(&sequence_iteration_source_projection(
                RustExpr::Raw("left + right".to_string()),
                false,
                SequenceIterationProjection::OwnedValue,
            )),
            "(left + right).into_iter()"
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
            char_sequence_collect_string_projection(RustExpr::Raw("make_chars()".to_string()))
                .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("make_chars()".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "collect::<String>".to_string(),
                args: Vec::new(),
            }
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
        let erased_object_iter =
            erased_object_sequence_iter_cloned_projection(RustExpr::Var("values".to_string()));
        assert_eq!(
            emit::emit_expr(&erased_object_iter),
            "values.iter().map(|value| java_clone_erased_object_ref(value))"
        );
        assert_eq!(
            erased_object_iter.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![single_param_closure_expr(
                    "value",
                    RustExpr::Call {
                        function: Box::new(RustExpr::path(["java_clone_erased_object_ref"])),
                        args: vec![RustExpr::Var("value".to_string())],
                    },
                )],
            }
        );
        assert_eq!(
            erased_object_sequence_iter_cloned_projection(RustExpr::Raw(
                "make_values()".to_string()
            ))
            .into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("make_values()".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                }),
                method: "map".to_string(),
                args: vec![single_param_closure_expr(
                    "value",
                    RustExpr::Call {
                        function: Box::new(RustExpr::path(["java_clone_erased_object_ref"])),
                        args: vec![RustExpr::Var("value".to_string())],
                    },
                )],
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
        assert_eq!(
            iterator_next_expect_projection(RustExpr::Raw("factory()".to_string())).into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Raw("factory()".to_string())),
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
        assert!(matches!(
            discard_value_projection(RustExpr::Var("value".to_string())),
            RustExpr::Block {
                stmts,
                tail: None
            } if matches!(
                stmts.as_slice(),
                [RustStmt::Let {
                    mutable: false,
                    name,
                    value: Some(RustExpr::Var(value)),
                    ..
                }] if name == "_" && value == "value"
            )
        ));
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
        assert!(matches!(
            optional_source_borrowed_facade_return.into_unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args,
            } if method == "expect"
                && matches!(args.as_slice(), [RustExpr::Literal(message)] if message == "\"Java null receiver\"")
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall {
                        receiver,
                        method,
                        args,
                    } if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "maybe_path")
                        && method == "map"
                        && matches!(args.as_slice(), [RustExpr::Closure { body: RustClosureBody::Expr(body), .. }]
                            if matches!(body.as_ref(), RustExpr::Cast { ty, .. } if ty == "Box<dyn std::any::Any>"))
                )
        ));
        let optional_source_optional_target = apply_optional_rust_borrowed_facade_return(
            RustExpr::Var("maybe_path".to_string()),
            &optional_borrowed_facade_policy,
        );
        assert_eq!(
            emit::emit_expr(&optional_source_optional_target),
            "maybe_path.map(|value| Box::new(value.to_path_buf()) as Box<dyn std::any::Any>)"
        );
        assert!(matches!(
            optional_source_optional_target,
            RustExpr::MethodCall {
                receiver,
                method,
                args,
            } if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "maybe_path")
                && method == "map"
                && matches!(args.as_slice(), [RustExpr::Closure { body: RustClosureBody::Expr(body), .. }]
                    if matches!(body.as_ref(), RustExpr::Cast { ty, .. } if ty == "Box<dyn std::any::Any>"))
        ));
        assert_eq!(
            emit::emit_expr(&erased_object_box_projection(RustExpr::Raw(
                "(variant, None::<Void>)".to_string()
            ))),
            "Box::new((variant, None::<Void>)) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            erased_object_box_projection(RustExpr::Raw("(variant, None::<Void>)".to_string()))
                .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Tuple(vec![
                        RustExpr::Var("variant".to_string()),
                        RustExpr::path(["None::<Void>"]),
                    ])],
                }),
                ty: "Box<dyn std::any::Any>".to_string(),
            }
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
            boxed_dyn_trait_projection(RustExpr::Raw("left, right".to_string()), "Writer")
                .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Tuple(vec![
                        RustExpr::Var("left".to_string()),
                        RustExpr::Var("right".to_string()),
                    ])],
                }),
                ty: "Box<dyn Writer>".to_string(),
            }
        );
        assert_eq!(
            boxed_dyn_trait_projection(RustExpr::Raw("left + right, value".to_string()), "Writer")
                .into_unshaped(),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Box", "new"])),
                    args: vec![RustExpr::Tuple(vec![
                        RustExpr::Binary {
                            op: "+".to_string(),
                            left: Box::new(RustExpr::Var("left".to_string())),
                            right: Box::new(RustExpr::Var("right".to_string())),
                        },
                        RustExpr::Var("value".to_string()),
                    ])],
                }),
                ty: "Box<dyn Writer>".to_string(),
            }
        );
        assert_eq!(
            emit::emit_expr(&unsupported_deserialization_cast_handoff_projection(
                RustExpr::Raw("r#in.read_object()?".to_string()),
                "Localizable",
            )),
            "{ let _java_deserialized = r#in.read_object()?; panic!(\"unsupported Java deserialization cast to Localizable; provide a typed serialization facade\") }"
        );
        assert!(matches!(
            unsupported_deserialization_cast_handoff_projection(
                RustExpr::Raw("r#in.read_object()?".to_string()),
                "Localizable",
            ),
            RustExpr::Block {
                stmts,
                tail: Some(tail),
            } if matches!(
                stmts.as_slice(),
                [RustStmt::Let {
                    name,
                    value: Some(RustExpr::Raw(value)),
                    ..
                }] if name == "_java_deserialized" && value == "r#in.read_object()?"
            ) && matches!(tail.as_ref(), RustExpr::MacroCall { macro_name, .. } if macro_name == "panic")
        ));
        assert_eq!(
            emit::emit_expr(&abstract_method_stub_projection("Owner", "run")),
            "panic!(\"abstract method Owner.run has no implementation\")"
        );
        assert!(matches!(
            abstract_method_stub_projection("Owner", "run"),
            RustExpr::MacroCall { macro_name, args }
                if macro_name == "panic"
                    && matches!(args.as_slice(), [RustExpr::Literal(message)]
                        if message == "\"abstract method Owner.run has no implementation\"")
        ));
        assert_eq!(
            emit::emit_expr(&abstract_interface_method_stub_projection("Named", "name")),
            "panic!(\"abstract interface method Named.name has no implementation\")"
        );
        assert!(matches!(
            abstract_interface_method_stub_projection("Named", "name"),
            RustExpr::MacroCall { macro_name, args }
                if macro_name == "panic"
                    && matches!(args.as_slice(), [RustExpr::Literal(message)]
                        if message == "\"abstract interface method Named.name has no implementation\"")
        ));
        assert_eq!(
            emit::emit_expr(&unsupported_outer_capture_constructor_projection(
                "Inner", "Outer"
            )),
            "panic!(\"unsupported Java inner-class outer capture construction for Inner; pass an outer Outer handle\")"
        );
        assert!(matches!(
            unsupported_outer_capture_constructor_projection("Inner", "Outer"),
            RustExpr::MacroCall { macro_name, args }
                if macro_name == "panic"
                    && matches!(args.as_slice(), [RustExpr::Literal(message)]
                        if message == "\"unsupported Java inner-class outer capture construction for Inner; pass an outer Outer handle\"")
        ));
        assert_eq!(
            emit::emit_expr(&unsupported_constructor_call_handoff_projection(
                "unsupported constructor"
            )),
            "panic!(\"unsupported constructor\")/* TODO(translation): constructor call targets unsupported Java object graph mutation */"
        );
        assert!(matches!(
            unsupported_constructor_call_handoff_projection("unsupported constructor"),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): constructor call targets unsupported Java object graph mutation */"
                    && matches!(expr.as_ref(), RustExpr::MacroCall { macro_name, args }
                        if macro_name == "panic"
                            && matches!(args.as_slice(), [RustExpr::Literal(message)]
                                if message == "\"unsupported constructor\""))
        ));
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
        assert!(matches!(
            varargs_spread_extend_projection(
                RustExpr::Raw("vec![left]".to_string()),
                RustExpr::Var("tail".to_string()),
            ),
            RustExpr::Block {
                stmts,
                tail: Some(tail)
            } if matches!(
                stmts.as_slice(),
                [
                    RustStmt::Let {
                        mutable: true,
                        name,
                        value: Some(RustExpr::Raw(_)),
                        ..
                    },
                    RustStmt::Expr(RustExpr::MethodCall { method, .. })
                ] if name == "__java_varargs" && method == "extend"
            ) && matches!(tail.as_ref(), RustExpr::Var(name) if name == "__java_varargs")
        ));
        assert_eq!(
            emit::emit_expr(&collection_remove_if_projection(
                RustExpr::Var("lines".to_string()),
                "line",
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("line".to_string())),
                    method: "is_empty".to_string(),
                    args: Vec::new(),
                },
            )),
            "lines.retain(|line| !(line.is_empty()))"
        );
        assert_eq!(
            collection_remove_if_projection(
                RustExpr::Var("lines".to_string()),
                "line",
                RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("line".to_string())),
                    method: "is_empty".to_string(),
                    args: Vec::new(),
                },
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("lines".to_string())),
                method: "retain".to_string(),
                args: vec![single_param_closure_expr(
                    "line",
                    RustExpr::Unary {
                        op: "!".to_string(),
                        expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("line".to_string())),
                            method: "is_empty".to_string(),
                            args: Vec::new(),
                        }))),
                    },
                )],
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
        let sort_range = collection_sort_range_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("from".to_string()),
            RustExpr::Var("to".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&sort_range),
            "{ let __java_arrays_target = &mut values; let __java_arrays_from = from; let __java_arrays_to = to; if __java_arrays_from > __java_arrays_to { panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); } if __java_arrays_from < 0 || (__java_arrays_to as usize) > __java_arrays_target.len() { panic!(\"Java ArrayIndexOutOfBoundsException\"); } let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; __java_arrays_target[__java_arrays_from..__java_arrays_to].sort(); }"
        );
        assert!(matches!(
            sort_range,
            RustExpr::Block { stmts, tail: None }
                if stmts.iter().any(|stmt| matches!(stmt, RustStmt::If { .. }))
                    && matches!(stmts.last(), Some(RustStmt::Expr(RustExpr::MethodCall { method, .. })) if method == "sort")
        ));
        let sort_range = collection_sort_range_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("from".to_string()),
            RustExpr::Var("to".to_string()),
        );
        assert!(matches!(
            sort_range,
            RustExpr::Block { stmts, .. }
                if matches!(
                    stmts.get(4),
                    Some(RustStmt::If {
                        condition: RustExpr::Binary { op, right, .. },
                        ..
                    }) if op == "||"
                        && matches!(
                            right.as_ref(),
                            RustExpr::Binary {
                                op,
                                left,
                                right,
                            } if op == ">"
                                && matches!(left.as_ref(), RustExpr::Cast { ty, .. } if ty == "usize")
                                && matches!(
                                    right.as_ref(),
                                    RustExpr::MethodCall { method, args, .. }
                                        if method == "len" && args.is_empty()
                                )
                        )
                )
        ));
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
                        args: vec![sequence_index_position_closure(RustExpr::Var(
                            "needle".to_string()
                        ))],
                    }),
                    method: "map".to_string(),
                    args: vec![sequence_index_i32_closure()],
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
                        args: vec![sequence_index_position_closure(RustExpr::Var(
                            "needle".to_string()
                        ))],
                    }),
                    method: "map".to_string(),
                    args: vec![sequence_index_i32_closure()],
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
        assert!(matches!(
            collection_sort_by_comparator_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("comparator".to_string()),
            ),
            RustExpr::Block { stmts, tail: None }
                if matches!(
                    stmts.as_slice(),
                    [
                        RustStmt::Let {
                            mutable: false,
                            name,
                            value: Some(RustExpr::Var(comparator)),
                            ..
                        },
                        RustStmt::Expr(RustExpr::MethodCall { method, args, .. })
                    ] if name == "__java_comparator"
                        && comparator == "comparator"
                        && method == "sort_by"
                        && matches!(args.as_slice(), [RustExpr::Closure { .. }])
                )
        ));
        assert_eq!(
            emit::emit_expr(&collection_sort_by_callback_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("callback".to_string()),
            )),
            "{ let mut __java_comparator = callback; values.sort_by(|a, b| __java_comparator(a.clone(), b.clone()).cmp(&0)); }"
        );
        assert!(matches!(
            collection_sort_by_callback_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("callback".to_string()),
            ),
            RustExpr::Block { stmts, tail: None }
                if matches!(
                    stmts.as_slice(),
                    [
                        RustStmt::Let {
                            mutable: true,
                            name,
                            value: Some(RustExpr::Var(callback)),
                            ..
                        },
                        RustStmt::Expr(RustExpr::MethodCall { method, args, .. })
                    ] if name == "__java_comparator"
                        && callback == "callback"
                        && method == "sort_by"
                        && matches!(args.as_slice(), [RustExpr::Closure { .. }])
                )
        ));
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
        let fill_range = array_fill_range_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("from".to_string()),
            RustExpr::Var("to".to_string()),
            RustExpr::Var("value".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&fill_range),
            "{ let __java_arrays_target = &mut values; let __java_arrays_from = from; let __java_arrays_to = to; if __java_arrays_from > __java_arrays_to { panic!(\"Java IllegalArgumentException: fromIndex > toIndex\"); } if __java_arrays_from < 0 || (__java_arrays_to as usize) > __java_arrays_target.len() { panic!(\"Java ArrayIndexOutOfBoundsException\"); } let __java_arrays_from = __java_arrays_from as usize; let __java_arrays_to = __java_arrays_to as usize; __java_arrays_target[__java_arrays_from..__java_arrays_to].fill(value); }"
        );
        assert!(matches!(
            fill_range,
            RustExpr::Block { stmts, tail: None }
                if stmts.iter().any(|stmt| matches!(stmt, RustStmt::If { .. }))
                    && matches!(stmts.last(), Some(RustStmt::Expr(RustExpr::MethodCall { method, .. })) if method == "fill")
        ));
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
            copied_expect.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("values".to_string())),
                        method: "get".to_string(),
                        args: vec![RustExpr::Var("key".to_string())],
                    }),
                    method: "copied".to_string(),
                    args: Vec::new(),
                }),
                method: "expect".to_string(),
                args: vec![RustExpr::Literal("\"Java null receiver\"".to_string())],
            }
        );
        assert_eq!(
            copied_expect.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        let compound_copied_expect =
            optional_copied_expect_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(
            emit::emit_expr(&compound_copied_expect),
            "(left + right).copied().expect(\"Java null receiver\")"
        );
        assert!(matches!(
            compound_copied_expect.unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args
            } if method == "expect"
                && matches!(args.as_slice(), [RustExpr::Literal(message)] if message == "\"Java null receiver\"")
                && matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { receiver, method, args }
                        if method == "copied"
                            && args.is_empty()
                            && matches!(
                                receiver.as_ref(),
                                RustExpr::Binary { op, left, right }
                                    if op == "+"
                                        && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                        && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                            )
                )
        ));
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
        assert!(matches!(
            subtype_constructor,
            RustExpr::Call { function, args }
                if matches!(function.as_ref(), RustExpr::Path(path)
                    if path == &vec!["Locatable".to_string(), "new_1".to_string()])
                    && matches!(
                        args.as_slice(),
                        [arg] if arg.recorded_shape() == Some(RustExprShape::OwnedRecovery)
                            && matches!(
                                arg.unshaped(),
                                RustExpr::MethodCall { receiver, method, args }
                                    if method == "clone"
                                        && args.is_empty()
                                        && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                            )
                    )
        ));
        let generic_subtype_constructor = subtype_owned_constructor_from_borrowed_projection(
            "JavaClass::<Vec<T>>",
            RustExpr::Var("value".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&generic_subtype_constructor),
            "JavaClass::<Vec<T>>::new_1(value.clone())"
        );
        assert!(matches!(
            generic_subtype_constructor,
            RustExpr::Call { function, .. }
                if matches!(
                    function.as_ref(),
                    RustExpr::Path(path)
                        if path == &vec!["JavaClass::<Vec<T>>".to_string(), "new_1".to_string()]
                )
        ));
        let generic_owner_fallback = subtype_owned_constructor_from_borrowed_projection(
            "std::rc::Rc<FormatLine>",
            RustExpr::Var("value".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&generic_owner_fallback),
            "std::rc::Rc<FormatLine>::new_1(value.clone())"
        );
        assert!(matches!(
            generic_owner_fallback,
            RustExpr::Call { function, .. }
                if matches!(
                    function.as_ref(),
                    RustExpr::Path(path)
                        if path == &vec![
                            "std".to_string(),
                            "rc".to_string(),
                            "Rc<FormatLine>".to_string(),
                            "new_1".to_string(),
                        ]
                )
        ));
        let raw_subtype_constructor = subtype_owned_constructor_from_borrowed_projection(
            "Locatable",
            RustExpr::Raw("left + right".to_string()),
        );
        assert_eq!(
            emit::emit_expr(&raw_subtype_constructor),
            "Locatable::new_1((left + right).clone())"
        );
        assert!(matches!(
            raw_subtype_constructor,
            RustExpr::Call { function, args }
                if matches!(function.as_ref(), RustExpr::Path(path)
                    if path == &vec!["Locatable".to_string(), "new_1".to_string()])
                    && matches!(
                        args.as_slice(),
                        [arg] if arg.recorded_shape() == Some(RustExprShape::OwnedRecovery)
                            && matches!(
                                arg.unshaped(),
                                RustExpr::MethodCall { receiver, method, args }
                                    if method == "clone"
                                        && args.is_empty()
                                        && matches!(
                                            receiver.as_ref(),
                                            RustExpr::Grouped(grouped)
                                                if matches!(
                                                    grouped.as_ref(),
                                                    RustExpr::Binary { op, left, right }
                                                        if op == "+"
                                                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                                                )
                                        )
                            )
                    )
        ));
        assert_eq!(
            emit::emit_expr(&exception_self_as_supertype_facade_boundary_projection()),
            "Default::default()/* TODO(translation): Java exception self-as-supertype facade boundary */"
        );
        assert!(matches!(
            exception_self_as_supertype_facade_boundary_projection(),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): Java exception self-as-supertype facade boundary */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::Call { function, args }
                            if args.is_empty()
                                && matches!(
                                    function.as_ref(),
                                    RustExpr::Path(path)
                                        if path == &vec!["Default".to_string(), "default".to_string()]
                                )
                    )
        ));
        assert_eq!(
            emit::emit_expr(&explicit_java_facade_cast_placeholder_projection(
                RustExpr::Var("graphics".to_string()),
                "Graphics2D",
            )),
            "{ let _java_facade_cast_source = &(graphics); Graphics2D::default() }/* TODO(translation): validate explicit Java facade cast */"
        );
        assert!(matches!(
            explicit_java_facade_cast_placeholder_projection(
                RustExpr::Var("graphics".to_string()),
                "Graphics2D",
            ),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): validate explicit Java facade cast */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::Block {
                            stmts,
                            tail: Some(tail),
                        } if matches!(
                            stmts.as_slice(),
                            [RustStmt::Let {
                                name,
                                value: Some(RustExpr::Unary { op, expr }),
                                ..
                            }] if name == "_java_facade_cast_source"
                                && op == "&"
                                && matches!(expr.as_ref(), RustExpr::Grouped(grouped)
                                    if matches!(grouped.as_ref(), RustExpr::Var(name) if name == "graphics"))
                        ) && matches!(
                            tail.as_ref(),
                            RustExpr::Call { function, args }
                                if args.is_empty()
                                    && matches!(function.as_ref(), RustExpr::Path(path)
                                        if path == &vec!["Graphics2D".to_string(), "default".to_string()])
                        )
                    )
        ));
        assert_eq!(
            emit::emit_expr(&explicit_java_facade_cast_placeholder_projection(
                RustExpr::Raw("left + right".to_string()),
                "Graphics2D",
            )),
            "{ let _java_facade_cast_source = &(left + right); Graphics2D::default() }/* TODO(translation): validate explicit Java facade cast */"
        );
        let generic_facade_default = explicit_java_facade_cast_placeholder_projection(
            RustExpr::Var("reader".to_string()),
            "Box::<dyn Readable>",
        );
        assert_eq!(
            emit::emit_expr(&generic_facade_default),
            "{ let _java_facade_cast_source = &(reader); Box::<dyn Readable>::default() }/* TODO(translation): validate explicit Java facade cast */"
        );
        assert!(matches!(
            generic_facade_default,
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Block {
                        tail: Some(tail),
                        ..
                    } if matches!(
                        tail.as_ref(),
                        RustExpr::Call { function, args }
                            if args.is_empty()
                                && matches!(
                                    function.as_ref(),
                                    RustExpr::Path(path)
                                        if path == &vec![
                                            "Box::<dyn Readable>".to_string(),
                                            "default".to_string()
                                        ]
                                )
                    )
                )
        ));
        let raw_generic_facade_default = rust_type_default_call_expr("(Box<dyn Readable>)");
        assert_eq!(
            emit::emit_expr(&raw_generic_facade_default),
            "<(Box<dyn Readable>)>::default()"
        );
        assert!(matches!(
            raw_generic_facade_default,
            RustExpr::Call { function, args }
                if args.is_empty()
                    && matches!(
                        function.as_ref(),
                        RustExpr::TypeAssociatedPath {
                            ty: RustType::Raw(owner),
                            member,
                        } if owner == "(Box<dyn Readable>)" && member == "default"
                    )
        ));
        let angle_generic_facade_default = explicit_java_facade_cast_placeholder_projection(
            RustExpr::Var("line".to_string()),
            "std::rc::Rc<FormatLine>",
        );
        assert_eq!(
            emit::emit_expr(&angle_generic_facade_default),
            "{ let _java_facade_cast_source = &(line); std::rc::Rc<FormatLine>::default() }/* TODO(translation): validate explicit Java facade cast */"
        );
        assert!(matches!(
            angle_generic_facade_default,
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::Block {
                        tail: Some(tail),
                        ..
                    } if matches!(
                        tail.as_ref(),
                        RustExpr::Call { function, args }
                            if args.is_empty()
                                && matches!(
                                    function.as_ref(),
                                    RustExpr::Path(path)
                                        if path == &vec![
                                            "std".to_string(),
                                            "rc".to_string(),
                                            "Rc<FormatLine>".to_string(),
                                            "default".to_string(),
                                        ]
                                )
                    )
                )
        ));
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                false,
            )),
            "obj.downcast_ref::<Target>().expect(\"invalid Java cast\")"
        );
        assert!(matches!(
            erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                false,
            ),
            RustExpr::MethodCall { method, .. } if method == "expect"
        ));
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                true,
            )),
            "obj.as_ref().and_then(|value| value.downcast_ref::<Target>()).expect(\"invalid Java cast\")"
        );
        assert!(matches!(
            erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "Target",
                true,
            ),
            RustExpr::MethodCall { method, .. } if method == "expect"
        ));
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Raw("left + right".to_string()),
                "Target",
                false,
            )),
            "(left + right).downcast_ref::<Target>().expect(\"invalid Java cast\")"
        );
        assert_eq!(
            emit::emit_expr(&erased_object_borrowed_downcast_ref_projection(
                RustExpr::Var("obj".to_string()),
                "GuideTree.Node",
                false,
            )),
            "obj.downcast_ref::<GuideTreeNode>().expect(\"invalid Java cast\")"
        );
        let is_type = erased_object_is_type_projection(RustExpr::Var("obj".to_string()), "Target");
        assert_eq!(emit::emit_expr(&is_type), "obj.is::<Target>()");
        assert!(matches!(
            is_type.unshaped(),
            RustExpr::MethodCall { method, args, .. }
                if method == "is::<Target>" && args.is_empty()
        ));
        let downcast =
            erased_object_downcast_ref_projection(RustExpr::Var("obj".to_string()), "Target");
        assert_eq!(emit::emit_expr(&downcast), "obj.downcast_ref::<Target>()");
        assert!(matches!(
            downcast.unshaped(),
            RustExpr::MethodCall { method, args, .. }
                if method == "downcast_ref::<Target>" && args.is_empty()
        ));
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
            string_join_vararg_value_projection(RustExpr::Raw("extra.to_string()".to_string())),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("extra".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            terminal_raw_to_string_call_projection("cache[slot].to_string()"),
            Some(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Index(
                    Box::new(RustExpr::Var("cache".to_string())),
                    Box::new(RustExpr::Var("slot".to_string())),
                )),
                method: "to_string".to_string(),
                args: Vec::new(),
            })
        );
        assert_eq!(
            emit::emit_expr(
                &terminal_raw_to_string_call_projection("factory().build(value).to_string()")
                    .expect("method-call receiver should recover")
            ),
            "factory().build(value).to_string()"
        );
        assert_eq!(
            emit::emit_expr(
                &terminal_raw_to_string_call_projection("Owner::VALUE.to_string()")
                    .expect("associated path receiver should recover")
            ),
            "Owner::VALUE.to_string()"
        );
        assert_eq!(
            emit::emit_expr(
                &terminal_raw_to_string_call_projection("(left + right).to_string()")
                    .expect("grouped binary receiver should recover")
            ),
            "(left + right).to_string()"
        );
        assert!(terminal_raw_to_string_call_projection("left + right.to_string()").is_none());
        assert!(terminal_raw_to_string_call_projection("\"bin\".to_string()").is_none());
        assert!(terminal_raw_to_string_call_projection("(\"left + right\").to_string()").is_none());
        assert!(terminal_raw_to_string_call_projection("fallible()?.to_string()").is_none());
        assert!(terminal_raw_to_string_call_projection("(value as Target).to_string()").is_none());
        assert!(terminal_raw_to_string_call_projection("(&value).to_string()").is_none());
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
            emit::emit_expr(&string_join_delimiter_projection(
                structured_literal_to_string.clone()
            )),
            "\"bin\""
        );
        assert_eq!(
            emit::emit_expr(&string_join_delimiter_projection(RustExpr::Var(
                "sep".to_string()
            ))),
            "sep"
        );
        let join_collection = string_join_collection_projection(
            RustExpr::Literal("\",\"".to_string()),
            RustExpr::Var("values".to_string()),
        );
        assert_eq!(emit::emit_expr(&join_collection), "values.join(\",\")");
        assert!(matches!(
            join_collection.unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args
            } if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                && method == "join"
                && matches!(args.as_slice(), [RustExpr::Literal(delimiter)] if delimiter == "\",\"")
        ));
        let join_varargs = string_join_varargs_projection(
            RustExpr::Var("sep".to_string()),
            vec![
                RustExpr::Literal("\"bin\"".to_string()),
                RustExpr::Var("extra".to_string()),
                RustExpr::Raw("suffix.to_string()".to_string()),
            ],
        );
        assert_eq!(
            emit::emit_expr(&join_varargs),
            "vec![\"bin\".to_string(), (extra).to_string(), suffix.to_string()].join(sep)"
        );
        assert_eq!(
            join_varargs,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::VecLiteral(vec![
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Literal("\"bin\"".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    },
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Grouped(Box::new(RustExpr::Var(
                            "extra".to_string(),
                        )))),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    },
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("suffix".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    },
                ])),
                method: "join".to_string(),
                args: vec![RustExpr::Var("sep".to_string())],
            }
        );
    }

    #[test]
    fn erased_object_type_method_projections_are_structured() {
        let is_type = erased_object_is_type_projection(RustExpr::Var("obj".to_string()), "Target");
        assert_eq!(emit::emit_expr(&is_type), "obj.is::<Target>()");
        assert!(matches!(
            is_type.unshaped(),
            RustExpr::MethodCall { method, args, .. }
                if method == "is::<Target>" && args.is_empty()
        ));

        let downcast =
            erased_object_downcast_ref_projection(RustExpr::Var("obj".to_string()), "Target");
        assert_eq!(emit::emit_expr(&downcast), "obj.downcast_ref::<Target>()");
        assert!(matches!(
            downcast.unshaped(),
            RustExpr::MethodCall { method, args, .. }
                if method == "downcast_ref::<Target>" && args.is_empty()
        ));
    }

    #[test]
    fn string_format_arg_projection_owns_display_to_string_spelling() {
        fn grouped_plus_literal(left_name: &str, right_literal: &str) -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var(left_name.to_string())),
                right: Box::new(RustExpr::Literal(right_literal.to_string())),
            }))
        }

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
                receiver: Box::new(grouped_plus_literal("value", "1")),
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
            RustExpr::MacroCall { macro_name, args }
                if macro_name == "format" && args.len() == 3
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
            string_helper_receiver_projection(RustExpr::Raw("left.as_ref().unwrap()".to_string()))
                .into_unshaped(),
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
        let raw_receiver =
            string_helper_receiver_projection(RustExpr::Raw("opaque(value)".to_string()));
        assert_eq!(emit::emit_expr(&raw_receiver), "&opaque(value)");
        assert_eq!(
            raw_receiver.into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::Var("opaque".to_string())),
                    args: vec![RustExpr::Var("value".to_string())],
                }),
            }
        );
        let raw_compound_receiver =
            string_helper_receiver_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(emit::emit_expr(&raw_compound_receiver), "&(left + right)");
        assert_eq!(
            raw_compound_receiver.into_unshaped(),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }),
            }
        );
    }

    #[test]
    fn string_get_bytes_projection_owns_byte_view_spelling() {
        let bytes = string_get_bytes_projection(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&bytes),
            "value.as_bytes().iter().map(|byte| (*byte) as i8).collect::<Vec<i8>>()"
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
                    args: vec![single_param_closure_expr(
                        "byte",
                        RustExpr::Cast {
                            expr: Box::new(RustExpr::Unary {
                                op: "*".to_string(),
                                expr: Box::new(RustExpr::Var("byte".to_string())),
                            }),
                            ty: "i8".to_string(),
                        },
                    )],
                }),
                method: "collect::<Vec<i8>>".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            emit::emit_expr(&string_get_bytes_projection(RustExpr::Raw(
                "make_value()".to_string()
            ))),
            "make_value().as_bytes().iter().map(|byte| (*byte) as i8).collect::<Vec<i8>>()"
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
                    args: vec![single_param_closure_expr(
                        "byte",
                        RustExpr::Cast {
                            expr: Box::new(RustExpr::Unary {
                                op: "*".to_string(),
                                expr: Box::new(RustExpr::Var("byte".to_string())),
                            }),
                            ty: "i8".to_string(),
                        },
                    )],
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
        fn grouped_left_plus_right_binary() -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var("left".to_string())),
                right: Box::new(RustExpr::Var("right".to_string())),
            }))
        }

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
                expr: Box::new(grouped_left_plus_right_binary()),
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
        let raw_narrowing =
            apply_rust_scalar_slot_cast(RustExpr::Raw("left + right".to_string()), &scalar_policy);
        assert_eq!(
            emit::emit_expr(&raw_narrowing),
            "(left + right).try_into().unwrap()"
        );
        assert_eq!(
            raw_narrowing,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(grouped_left_plus_right_binary()),
                    method: "try_into".to_string(),
                    args: Vec::new(),
                }),
                method: "unwrap".to_string(),
                args: Vec::new(),
            }
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
        assert_eq!(
            facade_method_name("StringFacade::lookup::<TargetType>"),
            "lookup::<TargetType>"
        );
        assert_eq!(
            facade_method_name("pkg::StringFacade::<T>::display_name"),
            "display_name"
        );

        let generic_call = facade_receiver_method_call_projection(
            RustExpr::Var("value".to_string()),
            "StringFacade::lookup::<TargetType>",
            vec![RustExpr::Var("key".to_string())],
        );
        assert_eq!(
            emit::emit_expr(&generic_call),
            "value.lookup::<TargetType>(key)"
        );
        assert_eq!(
            generic_call,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "lookup::<TargetType>".to_string(),
                args: vec![RustExpr::Var("key".to_string())],
            }
        );
    }

    #[test]
    fn primitive_f64_method_projection_owns_math_cast_spelling() {
        fn grouped_plus_binary(left_name: &str, right_name: &str) -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var(left_name.to_string())),
                right: Box::new(RustExpr::Var(right_name.to_string())),
            }))
        }

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
            "((left + right) as f64).ln()"
        );
        assert_eq!(
            primitive_f64_method_projection(RustExpr::Raw("left + right".to_string()), "ln"),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(grouped_plus_binary("left", "right")),
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
            "((left + middle) as f64).atan2(right as f64)"
        );
        assert_eq!(
            primitive_f64_binary_method_projection(
                RustExpr::Raw("left + middle".to_string()),
                RustExpr::Var("right".to_string()),
                "atan2",
            ),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Cast {
                    expr: Box::new(grouped_plus_binary("left", "middle")),
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
            "(base as f64).powf((exponent + delta) as f64)"
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
                    expr: Box::new(grouped_plus_binary("exponent", "delta")),
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
            "java_math_round_f64((left + right) as f64)"
        );
        assert_eq!(
            math_round_projection(
                RustExpr::Raw("left + right".to_string()),
                PrimitiveType::Int
            ),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["java_math_round_f64"])),
                args: vec![RustExpr::Cast {
                    expr: Box::new(grouped_plus_binary("left", "right")),
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
                receiver: Box::new(grouped_plus_binary("left", "right")),
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
        let raw_narrowed_floor_mod = math_floor_mod_projection(
            RustExpr::Var("left".to_string()),
            RustExpr::Raw("right + delta".to_string()),
            PrimitiveType::Long,
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&raw_narrowed_floor_mod),
            "java_math_floor_mod_i64(left as i64, (right + delta) as i64) as i32"
        );
        assert!(matches!(
            raw_narrowed_floor_mod,
            RustExpr::Cast { expr, .. } if matches!(expr.as_ref(), RustExpr::Call { .. })
        ));
        let raw_floor_div = math_floor_div_projection(
            RustExpr::Raw("left + middle".to_string()),
            RustExpr::Var("right".to_string()),
            PrimitiveType::Int,
        );
        assert_eq!(
            emit::emit_expr(&raw_floor_div),
            "java_math_floor_div_i32((left + middle) as i32, right as i32)"
        );
        assert!(matches!(raw_floor_div, RustExpr::Call { .. }));
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
        assert_eq!(render_len_i32_projection("value"), "value.len() as i32");
        assert_eq!(
            render_len_i32_projection("left + right"),
            "(left + right).len() as i32"
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
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "v".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("v".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    })),
                }],
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

        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "ExternalHandle".to_string(),
                copy: Some(false),
                clone: Some(false),
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::UserOverride,
            });
        let unsupported = borrowed_sequence_extend_arg(
            RustExpr::Var("handles".to_string()),
            &RustSlotShape::conservative(JavaType::Named {
                name: "ExternalHandle".to_string(),
                args: Vec::new(),
            }),
            &facts,
        );
        assert_eq!(
            emit::emit_expr(&unsupported),
            "handles.iter().cloned() /* java2rust: unsupported borrowed element recovery for extend */"
        );
        assert!(matches!(
            unsupported,
            RustExpr::PostfixComment { expr, comment }
                if comment == " /* java2rust: unsupported borrowed element recovery for extend */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "cloned"
                                && args.is_empty()
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::MethodCall { receiver, method, args }
                                        if method == "iter"
                                            && args.is_empty()
                                            && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "handles")
                                )
                    )
        ));
    }

    #[test]
    fn java_display_plan_renders_optional_null_policy() {
        fn grouped_plus_binary(left_name: &str, right_name: &str) -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var(left_name.to_string())),
                right: Box::new(RustExpr::Var(right_name.to_string())),
            }))
        }

        let concat_value = apply_java_display_plan(
            RustExpr::Var("name".to_string()),
            JavaDisplayPlan::ConcatValue,
        );
        assert_eq!(emit::emit_expr(&concat_value), "(name).to_string()");
        assert_eq!(
            concat_value,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Grouped(Box::new(RustExpr::Var(
                    "name".to_string(),
                )))),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        let compound_concat = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::ConcatValue,
        );
        assert_eq!(
            emit::emit_expr(&compound_concat),
            "(left + right).to_string()"
        );
        assert!(matches!(
            compound_concat,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), RustExpr::Grouped(inner)
                    if matches!(inner.as_ref(), RustExpr::Binary { op, left, right }
                        if op == "+"
                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")))
        ));

        let format_value = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::FormatValue,
        );
        assert_eq!(emit::emit_expr(&format_value), "(left + right).to_string()");
        assert_eq!(
            format_value,
            RustExpr::MethodCall {
                receiver: Box::new(grouped_plus_binary("left", "right")),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        let structured_format_value = apply_java_display_plan(
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }),
                method: "to_string".to_string(),
                args: Vec::new(),
            },
            JavaDisplayPlan::FormatValue,
        );
        assert_eq!(
            emit::emit_expr(&structured_format_value),
            "(left + right).to_string()"
        );
        assert!(matches!(
            structured_format_value,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), RustExpr::Grouped(inner)
                    if matches!(inner.as_ref(), RustExpr::Binary { .. }))
        ));

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
        let raw_optional_string = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::OptionalString,
        );
        assert_eq!(
            emit::emit_expr(&raw_optional_string),
            "(left + right).as_deref().unwrap_or(\"null\")"
        );
        assert_eq!(
            raw_optional_string,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(grouped_plus_binary("left", "right")),
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
                    args: vec![RustExpr::Closure {
                        is_move: false,
                        return_type: None,
                        params: vec![RustClosureParam {
                            pattern: "value".to_string(),
                            ty: None,
                        }],
                        body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("value".to_string())),
                            method: "to_string".to_string(),
                            args: Vec::new(),
                        })),
                    }],
                }),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: Vec::new(),
                    body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Literal("\"null\"".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    })),
                }],
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
                    args: vec![RustExpr::Closure {
                        is_move: false,
                        return_type: None,
                        params: vec![RustClosureParam {
                            pattern: "value".to_string(),
                            ty: None,
                        }],
                        body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("value".to_string())),
                            method: "to_string".to_string(),
                            args: Vec::new(),
                        })),
                    }],
                }),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: Vec::new(),
                    body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Literal("\"null\"".to_string())),
                        method: "to_string".to_string(),
                        args: Vec::new(),
                    })),
                }],
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
        let raw_optional_value = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::OptionalValue,
        );
        assert_eq!(
            emit::emit_expr(&raw_optional_value),
            "(left + right).as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        assert!(matches!(
            raw_optional_value,
            RustExpr::MethodCall { method, receiver, .. }
                if method == "unwrap_or_else"
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { method, receiver, .. }
                            if method == "map"
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::MethodCall { method, receiver, .. }
                                        if method == "as_ref"
                                            && matches!(
                                                receiver.as_ref(),
                                                RustExpr::Binary { op, left, right }
                                                    if op == "+"
                                                        && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                                        && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                                            )
                                )
                    )
        ));
        assert_eq!(
            emit::emit_expr(&apply_java_display_plan(
                RustExpr::Raw("make_item()".to_string()),
                JavaDisplayPlan::OptionalBorrowedValue,
            )),
            "make_item().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        let raw_optional_borrowed_value = apply_java_display_plan(
            RustExpr::Raw("left + right".to_string()),
            JavaDisplayPlan::OptionalBorrowedValue,
        );
        assert_eq!(
            emit::emit_expr(&raw_optional_borrowed_value),
            "(left + right).map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())"
        );
        assert!(matches!(
            raw_optional_borrowed_value,
            RustExpr::MethodCall { method, receiver, .. }
                if method == "unwrap_or_else"
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { method, receiver, .. }
                            if method == "map"
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::Binary { op, left, right }
                                        if op == "+"
                                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                            && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
                                )
                    )
        ));
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
        assert_eq!(render_java_display_operand(&structured), "(left + right)");

        let expr = RustExpr::Raw("(value * 100).to_string()".to_string());
        assert_eq!(render_java_display_operand(&expr), "(value * 100)");

        let structured_raw_arithmetic = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Raw("(value * 100)".to_string())),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            render_java_display_operand(&structured_raw_arithmetic),
            "(value * 100)"
        );

        let grouped_arithmetic = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Grouped(Box::new(
                rendered_plus_expr_projection_or_raw_leaf("value * (100 as f64)"),
            ))),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        assert_eq!(
            render_java_display_operand(&grouped_arithmetic),
            "(value * (100 as f64))"
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
    fn snapshot_rendered_shared_borrow_argument_keeps_replacements_structured() {
        let cases = [
            (
                option_some_projection(RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                }),
                "Some(&value.clone()/* TODO(translation): validate same-call alias snapshot */)",
            ),
            (
                RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::Var("value".to_string())),
                },
                "&value.clone()/* TODO(translation): validate same-call alias snapshot */",
            ),
            (
                zero_arg_method_projection(RustExpr::Var("value".to_string()), "as_slice"),
                "value.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice()",
            ),
            (
                option_some_projection(zero_arg_method_projection(
                    RustExpr::Var("value".to_string()),
                    "as_slice",
                )),
                "Some(value.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice())",
            ),
            (
                zero_arg_method_projection(RustExpr::Var("value".to_string()), "as_str"),
                "value.clone()/* TODO(translation): validate same-call alias snapshot */.as_str()",
            ),
            (
                option_some_projection(zero_arg_method_projection(
                    RustExpr::Var("value".to_string()),
                    "as_str",
                )),
                "Some(value.clone()/* TODO(translation): validate same-call alias snapshot */.as_str())",
            ),
        ];

        for (lowered, expected) in cases {
            let snapshot = snapshot_rendered_shared_borrow_argument(lowered, "value");
            assert_eq!(emit::emit_expr(&snapshot), expected);
            assert!(!matches!(snapshot.into_unshaped(), RustExpr::Raw(_)));
        }

        let raw_cases = [
            (
                RustExpr::Raw("Some(&value)".to_string()),
                "Some(&value.clone()/* TODO(translation): validate same-call alias snapshot */)",
            ),
            (
                RustExpr::Raw("value.as_str()".to_string()),
                "value.clone()/* TODO(translation): validate same-call alias snapshot */.as_str()",
            ),
            (
                RustExpr::Raw("Some(value.as_slice())".to_string()),
                "Some(value.clone()/* TODO(translation): validate same-call alias snapshot */.as_slice())",
            ),
        ];
        for (lowered, expected) in raw_cases {
            let snapshot = snapshot_rendered_shared_borrow_argument(lowered, "value");
            assert_eq!(emit::emit_expr(&snapshot), expected);
            assert!(
                !matches!(snapshot.into_unshaped(), RustExpr::Raw(_)),
                "raw compatibility input should recover to structured snapshot replacement"
            );
        }

        let unrelated = zero_arg_method_projection(RustExpr::Var("other".to_string()), "as_str");
        let unchanged = snapshot_rendered_shared_borrow_argument(unrelated.clone(), "value");
        assert_eq!(unchanged, unrelated);
    }

    #[test]
    fn audited_clone_recovery_adds_translation_marker() {
        let expr = apply_audited_clone_recovery(RustExpr::Var("value".to_string()));
        assert_eq!(
            emit::emit_expr(&expr),
            "value.clone()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            expr.into_unshaped(),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): validate added clone */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "clone"
                                && args.is_empty()
                                && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                    )
        ));

        let raw_expr = apply_audited_clone_recovery(RustExpr::Raw("left + right".to_string()));
        assert_eq!(
            emit::emit_expr(&raw_expr),
            "(left + right).clone()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            raw_expr.into_unshaped(),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): validate added clone */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "clone"
                                && args.is_empty()
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::Grouped(grouped)
                                        if matches!(grouped.as_ref(), RustExpr::Binary { op, left, right }
                                            if op == "+"
                                                && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                                                && matches!(right.as_ref(), RustExpr::Var(name) if name == "right"))
                                )
                    )
        ));

        let recoverable_raw =
            apply_audited_clone_recovery(RustExpr::Raw("value.to_owned()".to_string()));
        assert_eq!(
            emit::emit_expr(&recoverable_raw),
            "value.to_owned().clone()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            recoverable_raw.into_unshaped(),
            RustExpr::PostfixComment { expr, comment }
                if comment == "/* TODO(translation): validate added clone */"
                    && matches!(
                        expr.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "clone"
                                && args.is_empty()
                                && matches!(
                                    receiver.as_ref(),
                                    RustExpr::MethodCall { receiver, method, args }
                                        if method == "to_owned"
                                            && args.is_empty()
                                            && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                                )
                    )
        ));

        let already_cloned_raw =
            apply_audited_clone_recovery(RustExpr::Raw("value.clone()".to_string()));
        assert_eq!(
            already_cloned_raw.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("value".to_string())),
                method: "clone".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            already_cloned_raw.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
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
        assert!(matches!(
            expr.clone().into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "clone"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
        ));
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
            render_clone_projection("value as Target"),
            "(value as Target).clone()"
        );
        assert_eq!(render_clone_projection("*value"), "(*value).clone()");
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "value as Target".to_string()
            ))),
            "(value as Target).clone()"
        );
        assert!(matches!(
            apply_clone_projection(RustExpr::Raw("value as Target".to_string())).into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "clone"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                        if matches!(
                            grouped.as_ref(),
                            RustExpr::Cast { expr, ty }
                                if ty == "Target"
                                    && matches!(expr.as_ref(), RustExpr::Var(name) if name == "value")
                        ))
        ));
        assert_eq!(
            emit::emit_expr(&apply_clone_projection(RustExpr::Raw(
                "format!(\" as \")".to_string()
            ))),
            "format!(\" as \").clone()"
        );
        assert!(matches!(
            apply_clone_projection(RustExpr::Raw("\" as \"".to_string())).into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "clone"
                    && args.is_empty()
                    && matches!(receiver.as_ref(), RustExpr::Literal(raw) if raw == "\" as \"")
        ));
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
        assert!(matches!(
            apply_clone_projection(RustExpr::Raw("value.to_owned()".to_string())).into_unshaped(),
            RustExpr::MethodCall { receiver, method, args }
                if method == "clone"
                    && args.is_empty()
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::MethodCall { receiver, method, args }
                            if method == "to_owned"
                                && args.is_empty()
                                && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                    )
        ));
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
                expr: Box::new(RustExpr::Var("value".to_string())),
            }
        );
        assert_eq!(emit::emit_expr(&simple), "*value");

        let compound = deref_borrowed_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(
            compound.clone().into_unshaped(),
            RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
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
        assert!(matches!(
            expr.clone().into_unshaped(),
            RustExpr::MethodCall { receiver, method, .. }
                if method == "expect"
                    && matches!(
                        receiver.as_ref(),
                        RustExpr::PostfixComment { expr, comment }
                            if comment == render_clone_recovery_policy(&CloneRecoveryPolicy::AuditRequired)
                                && matches!(
                                    expr.as_ref(),
                                    RustExpr::MethodCall { method, .. } if method == "cloned"
                                )
                    )
        ));

        let erased = optional_clone_recovery_owned_element_projection(
            RustExpr::Var("value".to_string()),
            true,
        );
        assert_eq!(
            emit::emit_expr(&erased),
            "value.as_ref().map(|value| java_clone_erased_object_ref(value))/* TODO(translation): validate added clone */.expect(\"Java null receiver\")"
        );
        assert!(matches!(
            erased.into_unshaped(),
            RustExpr::MethodCall { receiver, method, .. }
                if method == "expect"
                    && matches!(receiver.as_ref(), RustExpr::PostfixComment { .. })
        ));

        let borrowed = apply_borrowed_option_clone_adapter(
            RustExpr::Var("value".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(
            emit::emit_expr(&borrowed),
            "value.cloned()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            borrowed.into_unshaped(),
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { method, .. } if method == "cloned"
                )
        ));

        let owned = apply_owned_option_clone_adapter(
            RustExpr::Var("value".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(
            emit::emit_expr(&owned),
            "value.clone()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            owned.into_unshaped(),
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { method, .. } if method == "clone"
                )
        ));
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
        let structured = method_expect_projection(
            zero_arg_method_projection(RustExpr::Var("value".to_string()), "downcast::<Target>"),
            crate::output_markers::INVALID_JAVA_CAST_MESSAGE,
        );
        let structured_rewritten = rewrite_downcast_expect_message(structured, "replacement");
        assert_eq!(
            emit::emit_expr(&structured_rewritten),
            "value.downcast::<Target>().expect(\"replacement\")"
        );
        assert!(!matches!(structured_rewritten.unshaped(), RustExpr::Raw(_)));

        let real = RustExpr::Raw(format!(
            "value.downcast::<Target>(){}",
            crate::output_markers::invalid_java_cast_expect()
        ));
        let real_rewritten = rewrite_downcast_expect_message(real, "replacement");
        assert_eq!(
            emit::emit_expr(&real_rewritten),
            "value.downcast::<Target>().expect(\"replacement\")"
        );
        assert!(
            !matches!(real_rewritten.unshaped(), RustExpr::Raw(_)),
            "direct raw downcast expect rewrites should recover a structured projection"
        );

        let nested_binary = RustExpr::Raw(format!(
            "left + value.downcast::<Target>(){}",
            crate::output_markers::invalid_java_cast_expect()
        ));
        let nested_rewritten = rewrite_downcast_expect_message(nested_binary, "replacement");
        assert_eq!(
            emit::emit_expr(&nested_rewritten),
            "left + value.downcast::<Target>().expect(\"replacement\")"
        );
        assert!(matches!(
            nested_rewritten.unshaped(),
            RustExpr::Binary { op, right, .. }
                if op == "+"
                    && matches!(
                        right.as_ref(),
                        RustExpr::MethodCall { method, args, .. }
                            if method == "expect"
                                && matches!(
                                    args.as_slice(),
                                    [RustExpr::Literal(message)] if message == "\"replacement\""
                                )
                    )
        ));

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
        assert!(matches!(rewritten.unshaped(), RustExpr::Raw(_)));

        let call_rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Raw(format!("use_name({erased_string_box})")),
            &["name".to_string()],
            &[],
        );
        assert_eq!(
            emit::emit_expr(&call_rewritten),
            "use_name(name.as_ref().map(|value| value.as_str()))"
        );
        assert!(!matches!(call_rewritten.unshaped(), RustExpr::Raw(_)));
    }

    #[test]
    fn optional_erased_string_local_rewrite_preserves_structured_projection() {
        let boxed = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("name".to_string())),
            method: "map".to_string(),
            args: vec![single_param_closure_expr(
                "value",
                RustExpr::Cast {
                    expr: Box::new(function_call_projection(
                        RustExpr::path(["Box", "new"]),
                        vec![zero_arg_method_projection(
                            RustExpr::Var("value".to_string()),
                            "clone",
                        )],
                    )),
                    ty: "Box<dyn std::any::Any>".to_string(),
                },
            )],
        };
        let nested = function_call_projection(RustExpr::Var("use_name".to_string()), vec![boxed]);

        let rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            nested.clone(),
            &["name".to_string()],
            &[],
        );

        assert_eq!(
            emit::emit_expr(&rewritten),
            "use_name(name.as_ref().map(|value| value.as_str()))"
        );
        assert!(!matches!(rewritten, RustExpr::Raw(_)));

        let shaped = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::shaped(RustExprShape::OwnedRecovery, nested.clone()),
            &["name".to_string()],
            &[],
        );
        assert_eq!(
            emit::emit_expr(&shaped),
            "use_name(name.as_ref().map(|value| value.as_str()))"
        );
        assert_eq!(shaped.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(!matches!(shaped.unshaped(), RustExpr::Raw(_)));

        let grouped = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Grouped(Box::new(nested)),
            &["name".to_string()],
            &[],
        );
        assert_eq!(
            emit::emit_expr(&grouped),
            "(use_name(name.as_ref().map(|value| value.as_str())))"
        );
        assert!(!matches!(grouped.unshaped(), RustExpr::Raw(_)));
    }

    #[test]
    fn optional_erased_string_cast_local_rewrite_preserves_structured_projection() {
        let erased_view = zero_arg_method_projection(
            method_expect_projection(
                zero_arg_method_projection(
                    zero_arg_method_projection(RustExpr::Var("name".to_string()), "unwrap"),
                    "downcast::<String>",
                ),
                crate::output_markers::INVALID_JAVA_CAST_MESSAGE,
            ),
            "as_str",
        );

        let rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            erased_view,
            &[],
            &["name".to_string()],
        );

        assert_eq!(
            emit::emit_expr(&rewritten),
            "name.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str()"
        );
        assert!(!matches!(rewritten, RustExpr::Raw(_)));
    }

    #[test]
    fn optional_erased_string_raw_suffix_rewrite_recovers_structured_projection() {
        let suffix = format!(
            ".unwrap().downcast::<String>(){}.as_str()",
            crate::output_markers::invalid_java_cast_expect()
        );

        let rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Raw(format!("name{suffix}")),
            &[],
            &[],
        );

        assert_eq!(
            emit::emit_expr(&rewritten),
            "name.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str()"
        );
        assert!(!matches!(rewritten.unshaped(), RustExpr::Raw(_)));

        let nested = rewrite_optional_erased_borrowed_string_view_call_args(
            function_call_projection(
                RustExpr::Var("use_name".to_string()),
                vec![RustExpr::Raw(format!("name{suffix}"))],
            ),
            &[],
            &[],
        );

        assert_eq!(
            emit::emit_expr(&nested),
            "use_name(name.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str())"
        );
        assert!(!matches!(nested.unshaped(), RustExpr::Raw(_)));
        let raw_call = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Raw(format!("use_name(name{suffix})")),
            &[],
            &[],
        );
        assert_eq!(
            emit::emit_expr(&raw_call),
            "use_name(name.as_ref().and_then(|value| value.downcast_ref::<String>()).expect(\"invalid Java cast\").as_str())"
        );
        assert!(!matches!(raw_call.unshaped(), RustExpr::Raw(_)));
    }

    #[test]
    fn optional_erased_string_statement_like_raw_stays_leaf() {
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

        let rewritten = rewrite_optional_erased_borrowed_string_view_call_args(
            RustExpr::Raw(rendered.clone()),
            &[],
            &[],
        );

        assert_eq!(rewritten, RustExpr::Raw(rendered));
    }

    #[test]
    fn erased_object_clone_recovery_records_recovery_shape() {
        let expr = clone_erased_object_ref_once(RustExpr::Raw("&value".to_string()));
        assert_eq!(
            emit::emit_expr(&expr),
            "java_clone_erased_object_ref(&value)"
        );
        assert_eq!(expr.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(matches!(
            expr.clone().into_unshaped(),
            RustExpr::Call { .. }
        ));
        let compound = clone_erased_object_ref_once(RustExpr::Raw("left + right".to_string()));
        assert_eq!(
            emit::emit_expr(&compound),
            "java_clone_erased_object_ref(&(left + right))"
        );
        assert!(matches!(
            compound.clone().into_unshaped(),
            RustExpr::Call { .. }
        ));

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
            "java_clone_erased_object_ref(&(\"java_clone_erased_object_ref(\".to_string()))"
        );
        assert_eq!(literal.recorded_shape(), Some(RustExprShape::OwnedRecovery));
        assert!(matches!(
            literal.clone().into_unshaped(),
            RustExpr::Call { .. }
        ));

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
            "(*(Box::new(value) as Box<dyn std::any::Any>).downcast::<Target>().expect(\"invalid Java cast\"))"
        );
        assert!(matches!(
            expr,
            RustExpr::Grouped(grouped)
                if matches!(
                    grouped.as_ref(),
                    RustExpr::Unary { op, expr }
                        if op == "*"
                            && matches!(
                                expr.as_ref(),
                                RustExpr::MethodCall { method, .. } if method == "expect"
                            )
                )
        ));

        let nested = apply_erased_object_downcast(
            RustExpr::Var("value".to_string()),
            "GuideTree.Node",
            &InvalidCastPolicy::Panic,
        );
        assert_eq!(
            emit::emit_expr(&nested),
            "(*value.downcast::<GuideTreeNode>().expect(\"invalid Java cast\"))"
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
        assert_eq!(
            anonymous.recorded_shape(),
            Some(RustExprShape::AnonymousDynTrait)
        );
        assert!(matches!(
            anonymous.into_unshaped(),
            RustExpr::Call { function, args }
                if args.is_empty()
                    && matches!(function.as_ref(), RustExpr::Path(segments)
                        if segments == &vec![
                            "<dyn Callback>".to_string(),
                            "anonymous".to_string()
                        ])
        ));
        let other_anonymous = apply_one(
            RustExpr::Raw("<dyn Runnable>::anonymous()".to_string()),
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(
            emit::emit_expr(&other_anonymous),
            "<dyn Runnable>::anonymous()"
        );
        assert_eq!(
            other_anonymous.recorded_shape(),
            Some(RustExprShape::AnonymousDynTrait)
        );
        assert!(matches!(
            other_anonymous.into_unshaped(),
            RustExpr::Call { function, args }
                if args.is_empty()
                    && matches!(function.as_ref(), RustExpr::Path(segments)
                        if segments == &vec![
                            "<dyn Runnable>".to_string(),
                            "anonymous".to_string()
                        ])
        ));

        let tagged_anonymous = anonymous_dyn_trait_projection("Callback");
        let recloned_tagged_anonymous = apply_one(
            tagged_anonymous.clone(),
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(recloned_tagged_anonymous, tagged_anonymous);

        let structured_anonymous = RustExpr::Call {
            function: Box::new(RustExpr::Path(vec![
                "<dyn Callable>".to_string(),
                "anonymous".to_string(),
            ])),
            args: Vec::new(),
        };
        let recovered_structured_anonymous = apply_one(
            structured_anonymous,
            &Coercion::CloneBoxDynTrait {
                trait_name: "Callback".to_string(),
            },
        );
        assert_eq!(
            emit::emit_expr(&recovered_structured_anonymous),
            "<dyn Callable>::anonymous()"
        );
        assert_eq!(
            recovered_structured_anonymous.recorded_shape(),
            Some(RustExprShape::AnonymousDynTrait)
        );

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
        assert!(matches!(
            literal_mention.into_unshaped(),
            RustExpr::MethodCall { method, args, .. }
                if method == "clone_box" && args.is_empty()
        ));
    }

    #[test]
    fn rendered_projection_recovers_borrowed_leaf_expressions() {
        assert_eq!(
            rendered_projection_or_raw_leaf("12"),
            RustExpr::Literal("12".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("4499201580859392i64"),
            RustExpr::Literal("4499201580859392i64".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("1_024usize"),
            RustExpr::Literal("1_024usize".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("\"literal\""),
            RustExpr::Literal("\"literal\"".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("r#\"literal\"#"),
            RustExpr::Literal("r#\"literal\"#".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("'x'"),
            RustExpr::Literal("'x'".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("format!(\"{}\", name)"),
            RustExpr::MacroCall {
                macro_name: "format".to_string(),
                args: vec![
                    RustExpr::Literal("\"{}\"".to_string()),
                    RustExpr::Var("name".to_string()),
                ],
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("vec![1, 2, value]"),
            RustExpr::VecLiteral(vec![
                RustExpr::Literal("1".to_string()),
                RustExpr::Literal("2".to_string()),
                RustExpr::Var("value".to_string()),
            ])
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("vec![value; count as usize]"),
            RustExpr::VecRepeat {
                value: Box::new(RustExpr::Var("value".to_string())),
                len: Box::new(RustExpr::Cast {
                    expr: Box::new(RustExpr::Var("count".to_string())),
                    ty: "usize".to_string(),
                }),
            }
        );
        let borrowed_vec = rendered_projection_or_raw_leaf("&vec![2, 3, 6, 4]");
        assert_eq!(emit::emit_expr(&borrowed_vec), "&vec![2, 3, 6, 4]");
        assert!(matches!(
            borrowed_vec,
            RustExpr::Unary { op, expr }
                if op == "&" && matches!(expr.as_ref(), RustExpr::VecLiteral(items) if items.len() == 4)
        ));
        let borrowed_string = rendered_projection_or_raw_leaf("&\"actual\".to_string()");
        assert_eq!(emit::emit_expr(&borrowed_string), "&\"actual\".to_string()");
        assert!(matches!(
            borrowed_string,
            RustExpr::Unary { op, expr }
                if op == "&" && matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { receiver, method, args }
                        if method == "to_string"
                            && args.is_empty()
                            && matches!(
                                receiver.as_ref(),
                                RustExpr::Literal(literal) if literal == "\"actual\""
                            )
                )
        ));
        let optional_borrowed_string =
            rendered_projection_or_raw_leaf("Some(&\"actual\".to_string())");
        assert_eq!(
            emit::emit_expr(&optional_borrowed_string),
            "Some(&\"actual\".to_string())"
        );
        assert!(matches!(
            optional_borrowed_string.into_unshaped(),
            RustExpr::Call { function, args }
                if rust_expr_is_single_segment_name(function.as_ref(), "Some")
                    && matches!(
                        args.as_slice(),
                        [RustExpr::Unary { op, expr }]
                            if op == "&" && matches!(
                                expr.as_ref(),
                                RustExpr::MethodCall { method, .. } if method == "to_string"
                            )
                    )
        ));
        let optional_borrowed_name = rendered_projection_or_raw_leaf("Some(&value)");
        assert!(matches!(
            optional_borrowed_name.into_unshaped(),
            RustExpr::Call { function, args }
                if rust_expr_is_single_segment_name(function.as_ref(), "Some")
                    && matches!(
                        args.as_slice(),
                        [RustExpr::Unary { op, expr }]
                            if op == "&"
                                && matches!(expr.as_ref(), RustExpr::Var(name) if name == "value")
                    )
        ));
        let optional_slice_view = rendered_projection_or_raw_leaf("Some(value.as_slice())");
        assert!(matches!(
            optional_slice_view.into_unshaped(),
            RustExpr::Call { function, args }
                if rust_expr_is_single_segment_name(function.as_ref(), "Some")
                    && matches!(
                        args.as_slice(),
                        [RustExpr::MethodCall { receiver, method, args }]
                            if method == "as_slice"
                                && args.is_empty()
                                && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "value")
                    )
        ));
        assert_eq!(
            rendered_projection_or_raw_leaf("index as usize"),
            RustExpr::Cast {
                expr: Box::new(RustExpr::Var("index".to_string())),
                ty: "usize".to_string(),
            }
        );
        let indexed_static = rendered_projection_or_raw_leaf("Table::VALUES[index as usize]");
        assert_eq!(
            emit::emit_expr(&indexed_static),
            "Table::VALUES[index as usize]"
        );
        assert!(matches!(
            indexed_static,
            RustExpr::Index(array, index)
                if matches!(array.as_ref(), RustExpr::Path(segments) if segments == &["Table", "VALUES"])
                    && matches!(
                        index.as_ref(),
                        RustExpr::Cast { expr, ty }
                            if ty == "usize"
                                && matches!(expr.as_ref(), RustExpr::Var(name) if name == "index")
                    )
        ));
        assert_eq!(
            rendered_projection_or_raw_leaf("value.clone() as Target"),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "clone".to_string(),
                    args: Vec::new(),
                }),
                ty: "Target".to_string(),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("left + 12"),
            RustExpr::Raw("left + 12".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("left + right as usize"),
            RustExpr::Raw("left + right as usize".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("left + format!(\"{}\", name)"),
            RustExpr::Raw("left + format!(\"{}\", name)".to_string())
        );
        let plus = rendered_plus_expr_projection_or_raw_leaf("left + format!(\"+\") + right");
        assert_eq!(emit::emit_expr(&plus), "(left + format!(\"+\")) + right");
        assert!(matches!(
            plus,
            RustExpr::Binary { op, left, right }
                if op == "+"
                    && matches!(left.as_ref(), RustExpr::Binary { op, left, right }
                        if op == "+"
                            && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                            && matches!(right.as_ref(), RustExpr::MacroCall { macro_name, args }
                                if macro_name == "format"
                                    && matches!(args.as_slice(), [RustExpr::Literal(literal)]
                                        if literal == "\"+\"")))
                    && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
        ));
        assert_eq!(
            rendered_plus_expr_projection_or_raw_leaf("\"left + right\""),
            RustExpr::Literal("\"left + right\"".to_string())
        );
        assert_eq!(
            rendered_plus_expr_projection_or_raw_leaf("r#\"left + right\"#"),
            RustExpr::Literal("r#\"left + right\"#".to_string())
        );
        assert_eq!(
            rendered_plus_expr_projection_or_raw_leaf("self.value * scale"),
            RustExpr::Binary {
                op: "*".to_string(),
                left: Box::new(RustExpr::Field(
                    Box::new(RustExpr::Var("self".to_string())),
                    "value".to_string(),
                )),
                right: Box::new(RustExpr::Var("scale".to_string())),
            }
        );
        assert_eq!(
            rendered_plus_expr_projection_or_raw_leaf("self.value == other.value"),
            RustExpr::Binary {
                op: "==".to_string(),
                left: Box::new(RustExpr::Field(
                    Box::new(RustExpr::Var("self".to_string())),
                    "value".to_string(),
                )),
                right: Box::new(RustExpr::Field(
                    Box::new(RustExpr::Var("other".to_string())),
                    "value".to_string(),
                )),
            }
        );
        assert_eq!(
            rendered_plus_expr_projection_or_raw_leaf("\"left * right\""),
            RustExpr::Literal("\"left * right\"".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("factory(r#\"a \", b\"#, value)"),
            RustExpr::Call {
                function: Box::new(RustExpr::Var("factory".to_string())),
                args: vec![
                    RustExpr::Literal("r#\"a \", b\"#".to_string()),
                    RustExpr::Var("value".to_string()),
                ],
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("&value"),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Var("value".to_string())),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("&mut value.field"),
            RustExpr::Unary {
                op: "&mut ".to_string(),
                expr: Box::new(RustExpr::Field(
                    Box::new(RustExpr::Var("value".to_string())),
                    "field".to_string(),
                )),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("fallible()"),
            RustExpr::Call {
                function: Box::new(RustExpr::Var("fallible".to_string())),
                args: Vec::new(),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("fallible()?"),
            RustExpr::Try(Box::new(RustExpr::Call {
                function: Box::new(RustExpr::Var("fallible".to_string())),
                args: Vec::new(),
            }))
        );
        assert!(matches!(
            rendered_projection_or_raw_leaf("outer(inner()?)?"),
            RustExpr::Try(outer)
                if matches!(
                    outer.as_ref(),
                    RustExpr::Call { function, args }
                        if matches!(function.as_ref(), RustExpr::Var(name) if name == "outer")
                            && matches!(
                                args.as_slice(),
                                [RustExpr::Try(inner)]
                                    if matches!(
                                        inner.as_ref(),
                                        RustExpr::Call { function, args }
                                            if args.is_empty()
                                                && matches!(function.as_ref(), RustExpr::Var(name) if name == "inner")
                                    )
                            )
                )
        ));
        assert!(matches!(
            rendered_projection_or_raw_leaf("a ? b"),
            RustExpr::Raw(raw) if raw == "a ? b"
        ));
        assert_eq!(
            rendered_projection_or_raw_leaf("OuterB::new_1_with_outer(__outer.clone(), value)"),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["OuterB", "new_1_with_outer"])),
                args: vec![
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("__outer".to_string())),
                        method: "clone".to_string(),
                        args: Vec::new(),
                    },
                    RustExpr::Var("value".to_string()),
                ],
            }
        );
        let generic_constructor = rendered_projection_or_raw_leaf("OuterB::new_1::<T>(value)");
        assert_eq!(
            emit::emit_expr(&generic_constructor),
            "OuterB::new_1::<T>(value)"
        );
        assert!(matches!(
            generic_constructor,
            RustExpr::Call { function, args }
                if !matches!(function.as_ref(), RustExpr::Raw(_))
                    && matches!(args.as_slice(), [RustExpr::Var(value)] if value == "value")
        ));
        assert_eq!(
            rendered_projection_or_raw_leaf("factory().build(value, other.clone())"),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::Var("factory".to_string())),
                    args: Vec::new(),
                }),
                method: "build".to_string(),
                args: vec![
                    RustExpr::Var("value".to_string()),
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("other".to_string())),
                        method: "clone".to_string(),
                        args: Vec::new(),
                    },
                ],
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("left + right.method(value)"),
            RustExpr::Raw("left + right.method(value)".to_string())
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("factory().collect::<Vec<_>>()"),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::Var("factory".to_string())),
                    args: Vec::new(),
                }),
                method: "collect::<Vec<_>>".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("None::<Void>"),
            RustExpr::path(["None::<Void>"])
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("Owner::VALUE::<Vec<T>>"),
            RustExpr::path(["Owner", "VALUE::<Vec<T>>"])
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("&(left + right)"),
            RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Grouped(Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }))),
            }
        );
        assert_eq!(
            rendered_projection_or_raw_leaf("values[left + right]"),
            RustExpr::Index(
                Box::new(RustExpr::Var("values".to_string())),
                Box::new(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                }),
            )
        );
    }

    #[test]
    fn raw_comma_splitter_ignores_commas_inside_raw_string_literals() {
        assert_eq!(
            split_raw_top_level_commas("r#\"a \", b\"#, value"),
            vec![
                RustExpr::Literal("r#\"a \", b\"#".to_string()),
                RustExpr::Var("value".to_string()),
            ]
        );

        assert_eq!(
            split_raw_top_level_commas("r##\"a \", b\"##, value"),
            vec![
                RustExpr::Literal("r##\"a \", b\"##".to_string()),
                RustExpr::Var("value".to_string()),
            ]
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

        let recoverable_raw = to_owned_sequence_projection(
            RustExpr::Raw("values.to_vec()".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(emit::emit_expr(&recoverable_raw), "values.to_vec()");
        assert_eq!(
            recoverable_raw.clone().into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("values".to_string())),
                method: "to_vec".to_string(),
                args: Vec::new(),
            }
        );
        assert_eq!(
            recoverable_raw.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );

        let literal_like_raw = to_owned_sequence_projection(
            RustExpr::Raw("\"values.to_vec()\"".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(
            emit::emit_expr(&literal_like_raw),
            "\"values.to_vec()\".to_vec()/* TODO(translation): validate added clone */"
        );
        assert!(matches!(
            literal_like_raw.clone().into_unshaped(),
            RustExpr::PostfixComment { expr, .. }
                if matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { receiver, method, args }
                        if method == "to_vec"
                            && args.is_empty()
                            && matches!(receiver.as_ref(), RustExpr::Literal(literal)
                                if literal == "\"values.to_vec()\"")
                )
        ));

        let audited = to_owned_sequence_projection(
            RustExpr::Var("values".to_string()),
            &CloneRecoveryPolicy::AuditRequired,
        );
        assert_eq!(
            emit::emit_expr(&audited),
            "values.to_vec()/* TODO(translation): validate added clone */"
        );
        assert_eq!(
            audited.clone().into_unshaped(),
            RustExpr::PostfixComment {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "to_vec".to_string(),
                    args: Vec::new(),
                }),
                comment: "/* TODO(translation): validate added clone */".to_string(),
            }
        );
        assert_eq!(audited.recorded_shape(), Some(RustExprShape::OwnedRecovery));
    }

    #[test]
    fn raw_borrowed_collect_clone_compat_recovers_structured_collect_calls() {
        let policy = CloneRecoveryPolicy::AuditRequired;

        let recoverable = apply_borrowed_vec_clone_adapter(
            RustExpr::Raw("make_values().iter().collect::<Vec<_>>()".to_string()),
            &policy,
        );
        assert_eq!(
            emit::emit_expr(&recoverable),
            "make_values().iter().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()"
        );
        assert_eq!(
            recoverable.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert!(!matches!(recoverable.unshaped(), RustExpr::Raw(_)));

        let already_cloned = apply_borrowed_vec_clone_adapter(
            RustExpr::Raw("make_values().iter().cloned().collect::<Vec<_>>()".to_string()),
            &policy,
        );
        assert_eq!(
            emit::emit_expr(&already_cloned),
            "make_values().iter().cloned().collect::<Vec<_>>()"
        );
        assert_eq!(
            already_cloned.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );

        let structured_collect = apply_borrowed_vec_clone_adapter(
            zero_arg_method_projection(
                zero_arg_method_projection(RustExpr::Var("values".to_string()), "iter"),
                "collect::<Vec<_>>",
            ),
            &policy,
        );
        assert_eq!(
            emit::emit_expr(&structured_collect),
            "values.iter().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()"
        );
        assert_eq!(
            structured_collect.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert!(!matches!(structured_collect.unshaped(), RustExpr::Raw(_)));

        let opaque = apply_borrowed_vec_clone_adapter(
            RustExpr::Raw("opaque collect::<Vec<_>>()".to_string()),
            &policy,
        );
        assert_eq!(
            emit::emit_expr(&opaque),
            "(opaque collect::<Vec<_>>()).into_iter().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()"
        );

        let fallback_receiver =
            apply_borrowed_vec_clone_adapter(RustExpr::Raw("make_values()".to_string()), &policy);
        assert_eq!(
            emit::emit_expr(&fallback_receiver),
            "make_values().into_iter().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()"
        );
        assert!(matches!(
            fallback_receiver.unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args
            } if method == "collect::<Vec<_>>"
                && args.is_empty()
                && matches!(
                    receiver.as_ref(),
                    RustExpr::PostfixComment { expr, .. }
                        if matches!(
                            expr.as_ref(),
                            RustExpr::MethodCall { receiver, method, args }
                                if method == "cloned"
                                    && args.is_empty()
                                    && matches!(
                                        receiver.as_ref(),
                                        RustExpr::MethodCall { receiver, method, args }
                                            if method == "into_iter"
                                                && args.is_empty()
                                                && matches!(
                                                    receiver.as_ref(),
                                                    RustExpr::Call { function, args }
                                                        if args.is_empty()
                                                            && matches!(
                                                                function.as_ref(),
                                                                RustExpr::Var(name) if name == "make_values"
                                                            )
                                                )
                                    )
                        )
                )
        ));

        let compound_fallback_receiver =
            apply_borrowed_vec_clone_adapter(RustExpr::Raw("left + right".to_string()), &policy);
        assert_eq!(
            emit::emit_expr(&compound_fallback_receiver),
            "(left + right).into_iter().cloned()/* TODO(translation): validate added clone */.collect::<Vec<_>>()"
        );
        assert!(matches!(
            compound_fallback_receiver.unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args
            } if method == "collect::<Vec<_>>"
                && args.is_empty()
                && matches!(
                    receiver.as_ref(),
                    RustExpr::PostfixComment { expr, .. }
                        if matches!(
                            expr.as_ref(),
                            RustExpr::MethodCall { receiver, method, args }
                                if method == "cloned"
                                    && args.is_empty()
                                    && matches!(
                                        receiver.as_ref(),
                                        RustExpr::MethodCall { receiver, method, args }
                                            if method == "into_iter"
                                                && args.is_empty()
                                                && matches!(
                                                    receiver.as_ref(),
                                                    RustExpr::Grouped(grouped)
                                                        if matches!(
                                                            grouped.as_ref(),
                                                            RustExpr::Binary { op, left, right }
                                                                if op == "+"
                                                                    && matches!(
                                                                        left.as_ref(),
                                                                        RustExpr::Var(name) if name == "left"
                                                                    )
                                                                    && matches!(
                                                                        right.as_ref(),
                                                                        RustExpr::Var(name) if name == "right"
                                                                    )
                                                        )
                                                )
                                    )
                        )
                )
        ));
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

    #[test]
    fn owned_string_recovery_idempotence_prefers_structure_and_shape() {
        let structured = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var("left".to_string())),
                right: Box::new(RustExpr::Var("right".to_string())),
            }),
            method: "to_string".to_string(),
            args: Vec::new(),
        };
        let recovered = apply_one(structured.clone(), &Coercion::ToOwnedString);
        assert_eq!(
            recovered.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert_eq!(recovered.clone().into_unshaped(), structured);

        let tagged = RustExpr::shaped(
            RustExprShape::OwnedRecovery,
            RustExpr::Raw("custom_owned_string(value)".to_string()),
        );
        let recovered_tagged = apply_one(tagged.clone(), &Coercion::ToOwnedString);
        assert_eq!(recovered_tagged, tagged);

        let recoverable_raw = apply_one(
            RustExpr::Raw("make_name().to_string()".to_string()),
            &Coercion::ToOwnedString,
        );
        assert_eq!(
            recoverable_raw.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert!(matches!(
            recoverable_raw.into_unshaped(),
            RustExpr::MethodCall { method, args, .. } if method == "to_string" && args.is_empty()
        ));

        let terminal_raw = apply_one(
            RustExpr::Raw("(left + right).to_string()".to_string()),
            &Coercion::ToOwnedString,
        );
        assert_eq!(emit::emit_expr(&terminal_raw), "(left + right).to_string()");
        assert_eq!(
            terminal_raw.recorded_shape(),
            Some(RustExprShape::OwnedRecovery)
        );
        assert!(matches!(
            terminal_raw.unshaped(),
            RustExpr::MethodCall {
                receiver,
                method,
                args
            } if method == "to_string"
                && args.is_empty()
                && matches!(
                    receiver.as_ref(),
                    RustExpr::Grouped(grouped)
                        if matches!(grouped.as_ref(), RustExpr::Binary { op, .. } if op == "+")
                )
        ));
        assert_eq!(
            terminal_raw_to_string_call_projection("left + right.to_string()"),
            None
        );

        let fresh = apply_one(RustExpr::Var("name".to_string()), &Coercion::ToOwnedString);
        assert_eq!(
            fresh.into_unshaped(),
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("name".to_string())),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
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
            associated_path.clone().into_unshaped(),
            RustExpr::Path(vec!["Owner".to_string(), "VALUES".to_string()])
        );
        assert_eq!(
            associated_path.recorded_shape(),
            Some(RustExprShape::SliceView)
        );

        let structured_path = apply_one(
            RustExpr::Path(vec!["Owner".to_string(), "VALUES".to_string()]),
            &Coercion::AsSlice,
        );
        assert_eq!(emit::emit_expr(&structured_path), "Owner::VALUES");
        assert_eq!(
            structured_path.clone().into_unshaped(),
            RustExpr::Path(vec!["Owner".to_string(), "VALUES".to_string()])
        );
        assert_eq!(
            structured_path.recorded_shape(),
            Some(RustExprShape::SliceView)
        );
    }

    /// Mutable slice idempotence is also producer-recorded. A raw spelling that
    /// happens to end in `.as_mut_slice()` is not enough semantic evidence.
    #[test]
    fn mutable_slice_projection_uses_the_recorded_shape_not_the_suffix() {
        let structured = ensure_borrow_projection(
            RustExpr::Var("values".to_string()),
            None,
            None,
            BorrowProjectionUse::MutableSliceView,
        );
        assert_eq!(emit::emit_expr(&structured), "&mut values");
        assert_eq!(
            structured,
            RustExpr::Unary {
                op: "&mut ".to_string(),
                expr: Box::new(RustExpr::Var("values".to_string())),
            }
        );

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
        let borrowed =
            ensure_borrow_projection(raw, None, None, BorrowProjectionUse::MutableSliceView);
        assert_eq!(emit::emit_expr(&borrowed), "&mut values.as_mut_slice()");
        assert!(matches!(
            borrowed,
            RustExpr::Unary { op, expr } if op == "&mut "
                && matches!(
                    expr.as_ref(),
                    RustExpr::MethodCall { receiver, method, args }
                        if method == "as_mut_slice"
                            && args.is_empty()
                            && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                )
        ));

        let compound = ensure_borrow_projection(
            RustExpr::Raw("left + right".to_string()),
            None,
            None,
            BorrowProjectionUse::MutableSliceView,
        );
        assert_eq!(emit::emit_expr(&compound), "&mut (left + right)");
        assert!(matches!(
            compound,
            RustExpr::Unary { op, expr } if op == "&mut "
                && matches!(
                    expr.as_ref(),
                    RustExpr::Binary { op, left, right }
                        if op == "+"
                            && matches!(left.as_ref(), RustExpr::Var(left) if left == "left")
                            && matches!(right.as_ref(), RustExpr::Var(right) if right == "right")
                )
        ));
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
        assert!(matches!(
            once,
            RustExpr::Shaped { shape, expr, .. }
                if shape == RustExprShape::ResultPropagated
                    && matches!(expr.as_ref(), RustExpr::Try(inner)
                        if matches!(inner.as_ref(), RustExpr::Raw(raw) if raw == "call()"))
        ));
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
