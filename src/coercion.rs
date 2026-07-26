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
use crate::rust_ir::RustExpr;
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
        RustExpr::Raw(format!("compile_error!({:?})", self.message))
    }

    pub fn compile_error_expr_consuming(&self, expr: &RustExpr) -> RustExpr {
        RustExpr::Raw(format!(
            "{{ let _ = {}; compile_error!({:?}) }}",
            emit::emit_expr(expr),
            self.message
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coercion {
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
            SyntaxSlotCoercionPolicy::ErasedObjectToJavaDisplayString => {
                vec![Coercion::ErasedObjectToJavaDisplayString]
            }
            SyntaxSlotCoercionPolicy::OptionalToJavaDisplayString => {
                vec![Coercion::OptionalToJavaDisplayString]
            }
            SyntaxSlotCoercionPolicy::OptionalToJavaDisplayValue => {
                vec![Coercion::OptionalToJavaDisplayValue]
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
            let mapped_value =
                apply_shape_coercion_plan(RustExpr::Var("value".to_string()), element_plan);
            return render_optional_element_projection(expr, mapped_value);
        }
        OptionalShapeCoercionPolicy::NoChange => {}
    }
    apply_coercions(expr, &plan.coercions())
}

fn render_optional_element_projection(expr: RustExpr, mapped_value: RustExpr) -> RustExpr {
    let rendered_expr = emit::emit_expr(&expr);
    let rendered_mapped = emit::emit_expr(&mapped_value);
    match rendered_mapped.as_str() {
        "&value" => RustExpr::Raw(format!("{rendered_expr}.as_ref()")),
        "value" => expr,
        "value.as_str()" => RustExpr::Raw(format!(
            "{rendered_expr}.as_ref().map(|value| value.as_str())"
        )),
        "value.as_slice()" => RustExpr::Raw(format!("{rendered_expr}.as_deref()")),
        "value.as_mut_slice()" => RustExpr::Raw(format!("{rendered_expr}.as_deref_mut()")),
        _ => RustExpr::Raw(format!("{rendered_expr}.map(|value| {rendered_mapped})")),
    }
}

pub fn project_optional_element_borrow_carrier(
    option_expr: RustExpr,
    element_ownership: OwnershipShape,
) -> Option<RustExpr> {
    let rendered = emit::emit_expr(&option_expr);
    match element_ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::Raw(format!("{rendered}.as_ref()"))),
        OwnershipShape::MutableBorrow => Some(RustExpr::Raw(format!("{rendered}.as_mut()"))),
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
        SlotConversionPlan::RustUnsignedByteArrayReturn(policy) => {
            apply_rust_unsigned_byte_array_return(expr, policy)
        }
        SlotConversionPlan::Identity { .. } => expr,
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
    let rendered = emit::emit_expr(&expr);
    if let Some(inner) = rendered
        .strip_prefix('&')
        .and_then(|value| value.strip_suffix(".as_str()"))
    {
        return RustExpr::Raw(format!("{inner}.as_str()"));
    }
    expr
}

pub fn normalize_optional_constructed_string_view_expr(expr: RustExpr) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if let Some(inner) = rendered
        .strip_prefix("Some(&")
        .and_then(|value| value.strip_suffix(".as_str())"))
    {
        return RustExpr::Raw(format!("Some({inner}.as_str())"));
    }
    expr
}

pub fn unwrap_rendered_some_expr(expr: &RustExpr) -> Option<RustExpr> {
    let rendered = emit::emit_expr(expr);
    rendered
        .strip_prefix("Some(")
        .and_then(|inner| inner.strip_suffix(')'))
        .map(|inner| RustExpr::Raw(inner.to_string()))
}

pub fn parenthesize_borrowed_method_receiver(receiver: RustExpr) -> RustExpr {
    let rendered = emit::emit_expr(&receiver);
    if rendered.starts_with('&') && !(rendered.starts_with("(&") && rendered.ends_with(')')) {
        RustExpr::Raw(format!("({rendered})"))
    } else {
        receiver
    }
}

pub fn force_shared_borrow_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("&{}", emit::emit_expr(&expr)))
}

pub fn borrow_as_dyn_trait(expr: RustExpr, trait_name: &str) -> RustExpr {
    RustExpr::Raw(format!("&{} as &dyn {trait_name}", emit::emit_expr(&expr)))
}

pub fn anonymous_dyn_trait_projection(trait_name: &str) -> RustExpr {
    RustExpr::Raw(format!("<dyn {trait_name}>::anonymous()"))
}

pub fn unwrap_optional_shared_value(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.as_ref().unwrap()", emit::emit_expr(&expr)))
}

pub fn unwrap_java_null_receiver_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.expect(\"Java null receiver\")",
        emit::emit_expr(&expr)
    ))
}

pub fn typed_none_projection(rust_ty: &str) -> RustExpr {
    RustExpr::Raw(format!("None::<{rust_ty}>"))
}

pub fn optional_get_owned_value_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.as_ref().map(|value| (*value).clone()).expect(\"Java null receiver\")",
        emit::emit_expr(&receiver)
    ))
}

pub fn optional_or_else_owned_value_projection(receiver: RustExpr, fallback: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.as_ref().map(|value| (*value).clone()).unwrap_or({})",
        emit::emit_expr(&receiver),
        emit::emit_expr(&fallback)
    ))
}

pub fn rust_expr_mentions_constructor_self_temporary(expr: &RustExpr) -> bool {
    match expr {
        RustExpr::Var(name) => name == "__java_self",
        RustExpr::Raw(rendered) => rendered.contains("__java_self"),
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

pub fn constructor_self_field_replacement(replacement: &str) -> String {
    replacement
        .strip_suffix(".clone()")
        .unwrap_or(replacement)
        .to_string()
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
        LiteralSlotPlan::BorrowedStringView { value } => RustExpr::Literal(format!("{value:?}")),
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
    let rendered = emit::emit_expr(&expr);
    if rendered == format!("&{base}") {
        RustExpr::Var(base.to_string())
    } else {
        expr
    }
}

pub fn normalize_map_key_borrow_projection(
    expr: RustExpr,
    local_name: Option<&str>,
    actual: Option<&RustSlotShape>,
) -> RustExpr {
    let expr = if actual.is_some_and(|shape| shape.ownership == OwnershipShape::SharedBorrow) {
        if let Some(name) = local_name {
            collapse_redundant_shared_borrow_projection(expr, name)
        } else {
            expr
        }
    } else {
        expr
    };
    let actual_for_projection = local_name
        .filter(|name| emit::emit_expr(&expr) == *name)
        .and(actual);
    ensure_borrow_projection(
        expr,
        actual_for_projection,
        None,
        BorrowProjectionUse::MapKey,
    )
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

pub fn rendered_expr_is_mutable_alias(arg: &RustExpr, base: &str) -> bool {
    let rendered = emit::emit_expr(arg);
    rendered == format!("&mut {base}")
        || rendered == format!("{base}.as_mut_slice()")
        || (rendered.starts_with(&format!("{base}."))
            && (rendered.contains(".as_mut().expect(\"Java null receiver\")")
                || rendered.contains(".as_deref_mut()")
                || rendered.ends_with(".as_mut_slice()")))
}

pub fn reborrow_optional_mut_projection(expr: &RustExpr) -> Option<RustExpr> {
    let rendered = emit::emit_expr(expr);
    if rendered.ends_with(".as_deref_mut()") {
        return None;
    }
    Some(RustExpr::Raw(format!("{rendered}.as_deref_mut()")))
}

pub fn clone_erased_object_ref_once(expr: RustExpr) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if rendered.contains("java_clone_erased_object_ref(")
        || rendered.contains("java_clone_erased_object_vec_ref(")
    {
        return expr;
    }
    let borrow_operand = rendered.strip_prefix('&').unwrap_or(&rendered);
    RustExpr::Raw(format!("java_clone_erased_object_ref(&{borrow_operand})"))
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
            return RustExpr::Raw(
                "compile_error!(\"Java Character parsing is not a primitive parse projection\")"
                    .to_string(),
            );
        }
    };
    RustExpr::Raw(format!(
        "{}.parse::<{rust_ty}>().unwrap()",
        render_parse_receiver(arg)
    ))
}

pub fn string_literal_source(expr: &RustExpr) -> Option<String> {
    let raw = emit::emit_expr(expr);
    let raw = raw.strip_suffix(".to_string()").unwrap_or(&raw);
    if !(raw.starts_with('"') && raw.ends_with('"')) {
        return None;
    }
    Some(raw[1..raw.len() - 1].to_string())
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
        RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&delimiter)))
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

pub fn string_helper_receiver_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&receiver)))
}

pub fn string_hash_code_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_hash_code".to_string())),
        args: vec![string_helper_receiver_projection(receiver)],
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
        function: Box::new(RustExpr::Raw(function.to_string())),
        args: vec![
            string_helper_receiver_projection(receiver),
            regex,
            replacement,
        ],
    }
}

pub fn string_matches_projection(receiver: RustExpr, pattern: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_matches".to_string())),
        args: vec![string_helper_receiver_projection(receiver), pattern],
    }
}

pub fn string_split_projection(receiver: RustExpr, regex: RustExpr, limit: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_split".to_string())),
        args: vec![string_helper_receiver_projection(receiver), regex, limit],
    }
}

pub fn string_from_bytes_projection(bytes: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_from_bytes".to_string())),
        args: vec![force_shared_borrow_projection(bytes)],
    }
}

pub fn string_from_byte_range_projection(
    bytes: RustExpr,
    offset: RustExpr,
    len: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_from_byte_range".to_string())),
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
        function: Box::new(RustExpr::Raw(function.to_string())),
        args: vec![RustExpr::Raw(render_string_view_argument(receiver)), other],
    }
}

pub fn string_get_bytes_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()",
        emit::emit_expr(&receiver)
    ))
}

pub fn string_to_char_array_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.chars().collect::<Vec<char>>()",
        emit::emit_expr(&receiver)
    ))
}

pub fn string_char_at_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.chars().nth({}).expect(\"Java string index out of bounds\")",
        emit::emit_expr(&receiver),
        render_index_to_usize_projection(
            &emit::emit_expr(&index),
            IndexToUsizeProjection::JavaStringCharAt,
        )
    ))
}

pub fn string_index_of_projection(
    receiver: RustExpr,
    needle: RustExpr,
    from_index: Option<RustExpr>,
) -> RustExpr {
    let receiver = render_string_view_argument(receiver);
    let needle = render_string_view_argument(needle);
    if let Some(from_index) = from_index {
        return RustExpr::Call {
            function: Box::new(RustExpr::Raw("java_string_index_of_from".to_string())),
            args: vec![RustExpr::Raw(receiver), RustExpr::Raw(needle), from_index],
        };
    }
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_index_of".to_string())),
        args: vec![RustExpr::Raw(receiver), RustExpr::Raw(needle)],
    }
}

pub fn string_substring_projection(receiver: RustExpr, begin: RustExpr, end: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("java_string_substring".to_string())),
        args: vec![
            RustExpr::Raw(render_string_view_argument(receiver)),
            begin,
            end,
        ],
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
    RustExpr::Raw(format!("{} as {rust_ty}", emit::emit_expr(&receiver)))
}

pub fn primitive_hash_code_projection(receiver: RustExpr, source: PrimitiveType) -> RustExpr {
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
    RustExpr::Raw(format!(
        "java_primitive_compare_to({}, {})",
        emit::emit_expr(&receiver),
        emit::emit_expr(&other)
    ))
}

pub fn primitive_f64_method_projection(arg: RustExpr, method: &str) -> RustExpr {
    RustExpr::Raw(format!("({} as f64).{method}()", emit::emit_expr(&arg)))
}

pub fn len_i32_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.len() as i32", emit::emit_expr(&expr)))
}

pub fn array2_rows_i32_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.nrows() as i32", emit::emit_expr(&expr)))
}

pub fn array2_cols_i32_projection(expr: RustExpr) -> RustExpr {
    len_i32_projection(expr)
}

pub fn owned_map_keys_vec_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.keys().cloned().collect::<Vec<_>>()",
        emit::emit_expr(&receiver)
    ))
}

pub fn owned_map_keys_set_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.keys().cloned().collect::<std::collections::HashSet<_>>()",
        emit::emit_expr(&receiver)
    ))
}

pub fn owned_collection_to_vec_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.iter().cloned().collect::<Vec<_>>()",
        emit::emit_expr(&receiver)
    ))
}

pub fn java_iterator_from_collection_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "JavaIterator::new({}.iter().cloned())",
        emit::emit_expr(&receiver)
    ))
}

pub fn sequence_iter_cloned_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.iter().cloned()", emit::emit_expr(&receiver)))
}

pub fn sequence_iter_copied_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.iter().copied()", emit::emit_expr(&receiver)))
}

pub fn iterator_next_expect_projection(iterator: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.next().expect(\"Java iterator exhausted\")",
        emit::emit_expr(&iterator)
    ))
}

pub fn tuple_field_projection(tuple: RustExpr, field_index: usize) -> RustExpr {
    RustExpr::Raw(format!("{}.{}", emit::emit_expr(&tuple), field_index))
}

pub fn subtype_owned_constructor_from_borrowed_projection(
    target_ty: &str,
    borrowed: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{target_ty}::new_1({}.clone())",
        emit::emit_expr(&borrowed)
    ))
}

pub fn primitive_is_nan_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.is_nan()", emit::emit_expr(&expr)))
}

pub fn file_exists_projection(receiver: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "std::path::Path::new({}.get_path().as_str()).exists()",
        emit::emit_expr(&receiver)
    ))
}

pub fn system_exit_projection(status: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "std::process::exit({})",
        emit::emit_expr(&primitive_number_value_projection(
            status,
            PrimitiveType::Int
        ))
    ))
}

pub fn thread_sleep_millis_projection(millis: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "std::thread::sleep(std::time::Duration::from_millis({} as u64))",
        emit::emit_expr(&millis)
    ))
}

pub fn math_max_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("std::cmp::max".to_string())),
        args: vec![left, right],
    }
}

pub fn math_min_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("std::cmp::min".to_string())),
        args: vec![left, right],
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
    RustExpr::Raw(format!(
        "vec![{}]",
        args.iter()
            .map(emit::emit_expr)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

pub fn arrays_equals_projection(left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Binary {
        op: "==".to_string(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn collections_empty_list_projection() -> RustExpr {
    RustExpr::Raw("Vec::new()".to_string())
}

pub fn collections_empty_sorted_set_projection() -> RustExpr {
    RustExpr::Raw("std::collections::BTreeSet::new()".to_string())
}

pub fn collections_singleton_projection(value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "std::collections::HashSet::from([{}])",
        emit::emit_expr(&value)
    ))
}

pub fn collections_singleton_list_projection(value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("vec![{}]", emit::emit_expr(&value)))
}

pub fn collections_singleton_map_projection(key: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "std::collections::HashMap::from([({}, {})])",
        emit::emit_expr(&key),
        emit::emit_expr(&value)
    ))
}

pub fn collections_n_copies_projection(count: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "vec![{}; {} as usize]",
        emit::emit_expr(&value),
        emit::emit_expr(&count)
    ))
}

pub fn native_collection_empty_new_projection(kind: crate::stdlib::JavaCollectionKind) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!(
            "{}::new",
            native_collection_type_name(kind)
        ))),
        args: Vec::new(),
    }
}

pub fn native_collection_with_capacity_projection(
    kind: crate::stdlib::JavaCollectionKind,
    capacity: RustExpr,
) -> RustExpr {
    RustExpr::Raw(format!(
        "{}::with_capacity({} as usize)",
        native_collection_type_name(kind),
        emit::emit_expr(&capacity)
    ))
}

pub fn native_collection_default_constructor_projection(
    kind: crate::stdlib::JavaCollectionKind,
) -> RustExpr {
    RustExpr::Raw(format!("{}::new()", native_collection_type_name(kind)))
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
        RustExpr::Raw("Vec::new()".to_string())
    } else {
        RustExpr::Raw(format!(
            "vec![{}]",
            values
                .iter()
                .map(emit::emit_expr)
                .collect::<Vec<_>>()
                .join(", ")
        ))
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
    RustExpr::Raw(format!(
        "{}.retain(|{}| !({}))",
        emit::emit_expr(&receiver),
        param_name,
        predicate
    ))
}

pub fn collection_sort_projection(target: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.sort()", emit::emit_expr(&target)))
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

pub fn array_fill_projection(target: RustExpr, value: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.fill({})",
        emit::emit_expr(&target),
        emit::emit_expr(&value)
    ))
}

pub fn sequence_get_expect_projection(receiver: RustExpr, index: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java index out of bounds\")",
        emit::emit_expr(&receiver),
        emit::emit_expr(&index)
    ))
}

pub fn map_get_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.get({})",
        emit::emit_expr(&receiver),
        emit::emit_expr(&key)
    ))
}

pub fn map_get_expect_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.get({}).expect(\"Java null receiver\")",
        emit::emit_expr(&receiver),
        emit::emit_expr(&key)
    ))
}

pub fn map_get_mut_expect_projection(receiver: RustExpr, key: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.get_mut({}).expect(\"Java null receiver\")",
        emit::emit_expr(&receiver),
        emit::emit_expr(&key)
    ))
}

pub fn optional_copied_expect_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.copied().expect(\"Java null receiver\")",
        emit::emit_expr(&expr)
    ))
}

pub fn render_string_format_arg_value(arg: RustExpr) -> String {
    format!("{}.to_string()", emit::emit_expr(&arg))
}

pub fn string_format_projection(mut args: Vec<RustExpr>) -> RustExpr {
    if args.is_empty() {
        return RustExpr::Raw("String::new()".to_string());
    }
    let format_arg = args.remove(0);
    let Some(java_format) = string_literal_source(&format_arg) else {
        return RustExpr::Call {
            function: Box::new(RustExpr::Raw("java_string_format".to_string())),
            args: vec![
                format_arg,
                RustExpr::Raw(format!(
                    "vec![{}]",
                    args.into_iter()
                        .map(render_string_format_arg_value)
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            ],
        };
    };
    let rust_format = java_format_to_rust_format(&java_format);
    let mut rendered_args = vec![RustExpr::Literal(format!("{rust_format:?}"))];
    rendered_args.extend(args);
    RustExpr::Raw(format!(
        "format!({})",
        rendered_args
            .iter()
            .map(emit::emit_expr)
            .collect::<Vec<_>>()
            .join(", ")
    ))
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
    let arg = emit::emit_expr(&arg);
    match borrowed_element_recovery_policy(element_shape, facts) {
        BorrowedElementRecoveryPolicy::BorrowElement => RustExpr::Raw(format!("{arg}.iter()")),
        BorrowedElementRecoveryPolicy::StringViewToOwned => {
            RustExpr::Raw(format!("{arg}.iter().map(|v| v.to_string())"))
        }
        BorrowedElementRecoveryPolicy::CopyElements => {
            RustExpr::Raw(format!("{arg}.iter().copied()"))
        }
        BorrowedElementRecoveryPolicy::CloneElements { .. } => {
            RustExpr::Raw(format!("{arg}.iter().cloned()"))
        }
        BorrowedElementRecoveryPolicy::Unsupported { .. } => RustExpr::Raw(format!(
            "{arg}.iter().cloned() /* java2rust: unsupported borrowed element recovery for extend */"
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
    let rendered = emit::emit_expr(&expr);
    let receiver = rendered.strip_suffix(".unwrap()").unwrap_or(&rendered);
    match projection {
        OptionalReceiverProjection::Shared => RustExpr::Raw(format!(
            "{receiver}.as_ref().expect(\"Java null receiver\")"
        )),
        OptionalReceiverProjection::Mutable => RustExpr::Raw(format!(
            "{receiver}.as_mut().expect(\"Java null receiver\")"
        )),
        OptionalReceiverProjection::Borrowed => {
            RustExpr::Raw(format!("{receiver}.expect(\"Java null receiver\")"))
        }
        OptionalReceiverProjection::SharedUnchecked => {
            RustExpr::Raw(format!("{receiver}.as_ref().unwrap()"))
        }
        OptionalReceiverProjection::MutableUnchecked => {
            RustExpr::Raw(format!("{receiver}.as_mut().unwrap()"))
        }
    }
}

pub fn rewrite_downcast_expect_message(
    expr: RustExpr,
    replacement_message: &'static str,
) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if rendered.contains(".downcast::<") && rendered.contains(".expect(\"invalid Java cast\")") {
        RustExpr::Raw(rendered.replace(
            ".expect(\"invalid Java cast\")",
            &format!(".expect(\"{replacement_message}\")"),
        ))
    } else {
        expr
    }
}

pub fn deref_borrowed_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("*{}", emit::emit_expr(&expr)))
}

pub fn unwrap_result_or_panic_uncaught_java_exception(expr: RustExpr) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    let rendered = rendered.strip_suffix('?').unwrap_or(&rendered);
    RustExpr::Raw(format!(
        "{}.unwrap_or_else(|ex| panic!(\"uncaught Java exception: {{:?}}\", ex))",
        rendered
    ))
}

pub fn normalize_reused_clone_for_borrowed_argument(
    lowered: &RustExpr,
    name: &str,
    target_shape: &RustSlotShape,
) -> Option<RustExpr> {
    let rendered = emit::emit_expr(lowered);
    if !rendered_expr_has_clone_recovery(&rendered) {
        return None;
    }
    match (
        &target_shape.java_ty,
        &target_shape.view,
        target_shape.ownership,
    ) {
        (JavaType::String, ViewShape::Str, OwnershipShape::SharedBorrow) => {
            Some(RustExpr::Raw(format!("{name}.as_str()")))
        }
        (JavaType::Array(_), ViewShape::Slice, OwnershipShape::MutableBorrow) => {
            Some(RustExpr::Raw(format!("{name}.as_mut_slice()")))
        }
        (JavaType::Array(_), ViewShape::Slice, OwnershipShape::SharedBorrow) => {
            Some(RustExpr::Raw(format!("{name}.as_slice()")))
        }
        (_, _, OwnershipShape::SharedBorrow) => Some(RustExpr::Raw(format!("&{name}"))),
        _ => None,
    }
}

pub fn borrowed_string_literal_temporary_plan(
    raw: &str,
    temporary_name: &str,
) -> Option<BorrowedStringLiteralTemporaryPlan> {
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
    let value = emit::emit_expr(&value);
    let value = if raw_contains_top_level_comma(&value) {
        format!("({value})")
    } else {
        value
    };
    RustExpr::Raw(format!("Box::new({value}) as Box<dyn std::any::Any>"))
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
            RustExpr::Raw(format!("{}.to_path_buf()", emit::emit_expr(&expr)))
        }
        RustBorrowedFacadeReturnRecovery::Clone => {
            RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&expr)))
        }
    };
    let erased = erased_object_box_projection(recovered);
    if policy.target.optional_element_type().is_some() {
        RustExpr::Raw(format!("Some({})", emit::emit_expr(&erased)))
    } else {
        erased
    }
}

fn apply_rust_scalar_slot_cast(expr: RustExpr, policy: &RustScalarSlotCastPolicy) -> RustExpr {
    if rust_scalar_type_name_for_primitive(policy.target_primitive)
        == Some(policy.rust_type_name.as_str())
    {
        return expr;
    }
    if policy.target_primitive == PrimitiveType::Char {
        return RustExpr::Raw(format!(
            "char::from_u32((({}) as u32) & 0xffff).unwrap_or('\\0')",
            emit::emit_expr(&expr)
        ));
    }
    if policy.rust_type_name == "char" {
        return RustExpr::Raw(format!(
            "(({}) as u32) as {}",
            emit::emit_expr(&expr),
            rust_primitive_type_name_policy(policy.target_primitive).rust_name
        ));
    }
    if rust_scalar_is_integral(&policy.rust_type_name)
        && java_primitive_is_integral(policy.target_primitive)
    {
        return RustExpr::Raw(format!("{}.try_into().unwrap()", emit::emit_expr(&expr)));
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

pub fn apply_java_display_plan(expr: RustExpr, plan: JavaDisplayPlan) -> RustExpr {
    match plan {
        JavaDisplayPlan::ConcatValue => {
            RustExpr::Raw(format!("({}).to_string()", emit::emit_expr(&expr)))
        }
        JavaDisplayPlan::FormatValue => RustExpr::Raw(format!(
            "{}.to_string()",
            render_java_display_operand(&expr)
        )),
        JavaDisplayPlan::ErasedObject => RustExpr::Raw(format!(
            "java_erased_object_to_string(&{})",
            emit::emit_expr(&expr)
        )),
        JavaDisplayPlan::OptionalString => RustExpr::Raw(format!(
            "{}.as_deref().unwrap_or(\"null\")",
            emit::emit_expr(&expr)
        )),
        JavaDisplayPlan::OptionalValue => RustExpr::Raw(format!(
            "{}.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
            emit::emit_expr(&expr)
        )),
        JavaDisplayPlan::OptionalBorrowedValue => RustExpr::Raw(format!(
            "{}.map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
            emit::emit_expr(&expr)
        )),
    }
}

pub fn render_java_display_operand(expr: &RustExpr) -> String {
    strip_arithmetic_display_conversion(&emit::emit_expr(expr))
}

pub fn rendered_expr_has_clone_recovery(rendered: &str) -> bool {
    rendered.ends_with(".clone()")
        || rendered.ends_with(".clone_box()")
        || rendered.ends_with(".cloned()")
        || rendered.ends_with(".cloned().expect(\"Java null receiver\")")
        || rendered.ends_with(".cloned()/* TODO(translation): validate added clone */")
        || rendered.ends_with(
            ".cloned()/* TODO(translation): validate added clone */.expect(\"Java null receiver\")",
        )
        || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
        || rendered.contains(".clone()/* TODO(translation): validate added clone */.into()")
}

pub fn rendered_expr_has_direct_clone_recovery(rendered: &str) -> bool {
    rendered.ends_with(".clone()")
        || rendered.ends_with(".clone_box()")
        || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
}

pub fn rust_expr_has_clone_recovery(expr: &RustExpr) -> bool {
    rendered_expr_has_clone_recovery(&emit::emit_expr(expr))
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
    RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&expr)))
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

fn strip_arithmetic_display_conversion(rendered: &str) -> String {
    if !(rendered.contains(" * ")
        || rendered.contains(" / ")
        || rendered.contains(" - ")
        || rendered.contains(" + "))
    {
        return rendered.to_string();
    }
    rendered.replace(".to_string()", "")
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
            rendered_expr_is_already_borrowed_map_key(&rendered)
                || actual.is_some_and(|shape| shape.ownership == OwnershipShape::SharedBorrow)
        }
        BorrowProjectionUse::StringView => rendered_expr_is_already_string_view(&rendered),
        BorrowProjectionUse::SliceView => rendered.ends_with(".as_slice()"),
        BorrowProjectionUse::MutableSliceView => {
            rendered.ends_with(".as_mut_slice()")
                || target.is_some_and(|target| {
                    target.ownership == OwnershipShape::MutableBorrow
                        && actual
                            .is_some_and(|actual| actual.ownership == OwnershipShape::MutableBorrow)
                })
        }
        BorrowProjectionUse::Argument
        | BorrowProjectionUse::Display
        | BorrowProjectionUse::MethodReceiver => target.is_some_and(|target| {
            actual.is_some_and(|actual| actual.ownership == target.ownership)
                && matches!(
                    target.ownership,
                    OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                )
        }),
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
    rendered
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.'))
        || rendered.starts_with("self.")
        || rendered.starts_with('"')
        || rendered.starts_with("r#\"")
        || rendered.starts_with("r\"")
        || rendered.contains('[')
        || rendered.ends_with(')')
}

fn rendered_expr_is_already_borrowed_map_key(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()")
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || trimmed.starts_with("java_string_")
}

pub fn rendered_expr_is_already_string_view(rendered: &str) -> bool {
    let trimmed = rendered.trim();
    trimmed.ends_with(".as_str()")
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || trimmed.starts_with("r#\"")
        || trimmed.starts_with("r\"")
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
        InstanceOfCheck::JavaSerializableMarker => {
            RustExpr::Raw(format!("java_is_serializable(&{})", emit::emit_expr(&expr)))
        }
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
        InstanceOfCheck::AlwaysTrue => RustExpr::Literal("true".to_string()),
        InstanceOfCheck::Unsupported {
            actual_dispatch,
            target_dispatch,
            policy,
        } => render_instanceof_fallback_policy(policy, actual_dispatch, target_dispatch),
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
            return RustExpr::Raw(format!(
                "{}.expect(\"Java null receiver\")",
                emit::emit_expr(&expr)
            ));
        }
        if matches!(
            adapter.recovery,
            BorrowedElementRecoveryPolicy::BorrowElement
        ) {
            let adapted = apply_borrowed_element_borrow_adapter(expr, adapter, policy);
            return apply_borrowed_element_target_view(adapted, policy);
        }
        let adapted = apply_borrowed_element_adapter(expr, adapter);
        adapted
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
            RustExpr::Raw(format!("{rendered}.expect(\"Java null receiver\")"))
        }
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow)
            if adapter.element_shape.ownership == OwnershipShape::MutableBorrow =>
        {
            RustExpr::Raw(format!("{rendered}.expect(\"Java null receiver\")"))
        }
        (BorrowedElementCarrier::Option, OwnershipShape::SharedBorrow) => RustExpr::Raw(format!(
            "{rendered}.as_ref().expect(\"Java null receiver\")"
        )),
        (BorrowedElementCarrier::Option, OwnershipShape::MutableBorrow) => RustExpr::Raw(format!(
            "{rendered}.as_mut().expect(\"Java null receiver\")"
        )),
        _ => RustExpr::Raw(format!("{rendered}.expect(\"Java null receiver\")")),
    }
}

fn apply_borrowed_element_adapter(
    expr: RustExpr,
    policy: &BorrowedElementAdapterPolicy,
) -> RustExpr {
    let adapted = match (&policy.carrier, &policy.recovery) {
        (_, BorrowedElementRecoveryPolicy::BorrowElement) => expr,
        (BorrowedElementCarrier::Option, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "map".to_string(),
                args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
            }
        }
        (BorrowedElementCarrier::Option, BorrowedElementRecoveryPolicy::CopyElements) => {
            RustExpr::MethodCall {
                receiver: Box::new(expr),
                method: "copied".to_string(),
                args: Vec::new(),
            }
        }
        (
            BorrowedElementCarrier::Option,
            BorrowedElementRecoveryPolicy::CloneElements { policy },
        ) => apply_borrowed_option_clone_adapter(expr, policy),
        (BorrowedElementCarrier::Vec, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            RustExpr::Raw(format!(
                "{}.into_iter().map(|v| v.to_string()).collect::<Vec<_>>()",
                emit::emit_expr(&expr)
            ))
        }
        (BorrowedElementCarrier::Set, BorrowedElementRecoveryPolicy::StringViewToOwned) => {
            RustExpr::Raw(format!(
                "{}.into_iter().map(|v| v.to_string()).collect::<std::collections::HashSet<_>>()",
                emit::emit_expr(&expr)
            ))
        }
        (BorrowedElementCarrier::Vec, BorrowedElementRecoveryPolicy::CopyElements) => {
            RustExpr::Raw(format!(
                "{}.into_iter().copied().collect::<Vec<_>>()",
                emit::emit_expr(&expr)
            ))
        }
        (BorrowedElementCarrier::Set, BorrowedElementRecoveryPolicy::CopyElements) => {
            RustExpr::Raw(format!(
                "{}.into_iter().copied().collect::<std::collections::HashSet<_>>()",
                emit::emit_expr(&expr)
            ))
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
        RustExpr::Raw(format!(
            "{}.expect(\"Java null receiver\")",
            emit::emit_expr(&adapted)
        ))
    } else {
        adapted
    }
}

fn apply_borrowed_element_target_view(
    expr: RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> RustExpr {
    if let Some(projected) = apply_optional_borrowed_element_target_view(&expr, policy) {
        return projected;
    }
    if let Some(projected) = apply_required_borrowed_element_target_view(&expr, policy) {
        return projected;
    }
    match &policy.target.dispatch {
        DispatchShape::DynTrait { trait_name } => {
            RustExpr::Raw(format!("{} as &dyn {trait_name}", emit::emit_expr(&expr)))
        }
        DispatchShape::Concrete if policy.target.java_ty != policy.element_shape.java_ty => {
            match &policy.target.java_ty {
                crate::java_ir::JavaType::Named { name, .. }
                | crate::java_ir::JavaType::Unknown(name) => RustExpr::Raw(format!(
                    "{{ let _java_value = {}; &{} }}",
                    emit::emit_expr(&expr),
                    crate::slot_render::rust_name_for_java_named_type(name),
                )),
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
    let rendered = emit::emit_expr(expr);
    match (target_element.ownership, &target_element.view) {
        (OwnershipShape::SharedBorrow, ViewShape::Slice)
            if matches!(target_element.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            (!rendered.ends_with(".as_deref()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_deref()")))
        }
        (OwnershipShape::MutableBorrow, ViewShape::Slice)
            if matches!(target_element.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            (!rendered.ends_with(".as_deref_mut()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_deref_mut()")))
        }
        (OwnershipShape::SharedBorrow, ViewShape::Str)
            if target_element.java_ty == crate::java_ir::JavaType::String =>
        {
            (!rendered.ends_with(".as_deref()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_deref()")))
        }
        (OwnershipShape::SharedBorrow, ViewShape::Native) => (!rendered.ends_with(".as_ref()"))
            .then(|| RustExpr::Raw(format!("{rendered}.as_ref()"))),
        (OwnershipShape::MutableBorrow, ViewShape::Native) => (!rendered.ends_with(".as_mut()"))
            .then(|| RustExpr::Raw(format!("{rendered}.as_mut()"))),
        _ => None,
    }
}

fn apply_required_borrowed_element_target_view(
    expr: &RustExpr,
    policy: &BorrowedElementSlotAdapterPolicy,
) -> Option<RustExpr> {
    if policy.target.optional_element_type().is_some() {
        return None;
    }
    let rendered = emit::emit_expr(expr);
    match (policy.target.ownership, &policy.target.view) {
        (OwnershipShape::SharedBorrow, ViewShape::Slice)
            if matches!(policy.target.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            (!rendered.ends_with(".as_slice()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_slice()")))
        }
        (OwnershipShape::MutableBorrow, ViewShape::Slice)
            if matches!(policy.target.java_ty, crate::java_ir::JavaType::Array(_)) =>
        {
            (!rendered.ends_with(".as_mut_slice()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_mut_slice()")))
        }
        (OwnershipShape::SharedBorrow, ViewShape::Str)
            if policy.target.java_ty == crate::java_ir::JavaType::String =>
        {
            (!rendered.ends_with(".as_str()"))
                .then(|| RustExpr::Raw(format!("{rendered}.as_str()")))
        }
        _ => None,
    }
}

fn apply_borrowed_option_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    match policy {
        CloneRecoveryPolicy::ProvenCloneCapability { .. } => RustExpr::MethodCall {
            receiver: Box::new(expr),
            method: "cloned".to_string(),
            args: Vec::new(),
        },
        CloneRecoveryPolicy::AuditRequired => RustExpr::Raw(format!(
            "{}.cloned(){}",
            emit::emit_expr(&expr),
            render_clone_recovery_policy(policy)
        )),
    }
}

fn apply_borrowed_vec_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.into_iter().cloned(){}.collect::<Vec<_>>()",
        emit::emit_expr(&expr),
        render_clone_recovery_policy(policy)
    ))
}

fn apply_borrowed_set_clone_adapter(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    RustExpr::Raw(format!(
        "{}.into_iter().cloned(){}.collect::<std::collections::HashSet<_>>()",
        emit::emit_expr(&expr),
        render_clone_recovery_policy(policy)
    ))
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
        Coercion::IndexToUsize => RustExpr::Raw(format!(
            "{} as usize",
            parenthesize_compound_cast_operand(&emit::emit_expr(&expr))
        )),
        Coercion::ToStringForConcat => apply_java_display_plan(expr, JavaDisplayPlan::ConcatValue),
        Coercion::ErasedObjectToJavaDisplayString => {
            apply_java_display_plan(expr, JavaDisplayPlan::ErasedObject)
        }
        Coercion::OptionalToJavaDisplayString => {
            apply_java_display_plan(expr, JavaDisplayPlan::OptionalString)
        }
        Coercion::OptionalToJavaDisplayValue => {
            apply_java_display_plan(expr, JavaDisplayPlan::OptionalValue)
        }
        Coercion::AsStr => RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&expr))),
        Coercion::ToOwnedString => {
            let rendered = emit::emit_expr(&expr);
            if rendered.ends_with(".to_string()") {
                expr
            } else {
                RustExpr::Raw(format!("{rendered}.to_string()"))
            }
        }
        Coercion::AsSlice => {
            let rendered = emit::emit_expr(&expr);
            if rendered.contains("::") && !rendered.contains('(') {
                RustExpr::Raw(rendered)
            } else {
                RustExpr::Raw(format!("{rendered}.as_slice()"))
            }
        }
        Coercion::AsMutSlice => RustExpr::Raw(format!("{}.as_mut_slice()", emit::emit_expr(&expr))),
        Coercion::WrapClosedEnum {
            enum_name,
            variant_name,
        } => RustExpr::Raw(format!(
            "{enum_name}::{variant_name}({})",
            emit::emit_expr(&expr)
        )),
        Coercion::BorrowDynTrait { trait_name } => borrow_as_dyn_trait(expr, trait_name),
        Coercion::BorrowBoxedDynTrait { trait_name } => RustExpr::Raw(format!(
            "{}.as_ref() as &dyn {trait_name}",
            emit::emit_expr(&expr)
        )),
        Coercion::BorrowConcreteFacade { target_type } => RustExpr::Raw(format!(
            "{{ let _java_value = {}; &{target_type} }}",
            emit::emit_expr(&expr)
        )),
        Coercion::ExternalInto { .. } => {
            RustExpr::Raw(format!("{}.into()", emit::emit_expr(&expr)))
        }
        Coercion::BoxDynTrait { trait_name } => RustExpr::Raw(format!(
            "Box::new({}) as Box<dyn {trait_name}>",
            emit::emit_expr(&expr)
        )),
        Coercion::CloneBoxDynTrait { .. } => {
            let rendered = emit::emit_expr(&expr);
            if rendered.starts_with("<dyn ") && rendered.contains(">::anonymous()") {
                RustExpr::Raw(rendered)
            } else {
                RustExpr::Raw(format!("{rendered}.clone_box()"))
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
            let rendered = parenthesize_compound_cast_operand(&emit::emit_expr(&expr));
            if *to == PrimitiveType::Char {
                return RustExpr::Raw(format!(
                    "char::from_u32(({rendered} as u32) & 0xffff).unwrap_or('\\0')"
                ));
            }
            if *from == PrimitiveType::Char {
                return RustExpr::Raw(format!(
                    "({rendered} as u32) as {}",
                    rust_primitive_type_name_policy(*to).rust_name
                ));
            }
            RustExpr::Raw(format!(
                "{rendered} as {}",
                rust_primitive_type_name_policy(*to).rust_name
            ))
        }
        Coercion::WrapOption => render_raw_option_some(expr),
        Coercion::UnwrapOption => RustExpr::Raw(format!("{}.unwrap()", emit::emit_expr(&expr))),
        Coercion::UnwrapOptionBorrowed => RustExpr::Raw(format!(
            "{}.as_ref().expect(\"Java null receiver\")",
            emit::emit_expr(&expr)
        )),
        Coercion::UnwrapOptionMutBorrowed => RustExpr::Raw(format!(
            "{}.as_mut().expect(\"Java null receiver\")",
            emit::emit_expr(&expr)
        )),
        Coercion::Borrow => RustExpr::Raw(format!("&{}", emit::emit_expr(&expr))),
        Coercion::BorrowMut => RustExpr::Raw(format!("&mut {}", emit::emit_expr(&expr))),
        Coercion::CopyBorrowedValue => RustExpr::Raw(format!("*{}", emit::emit_expr(&expr))),
        Coercion::CloneErasedObject { sequence } => {
            let helper = if *sequence {
                "java_clone_erased_object_vec_ref"
            } else {
                "java_clone_erased_object_ref"
            };
            RustExpr::Raw(format!("{helper}(&{})", emit::emit_expr(&expr)))
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

fn render_option_some(expr: RustExpr) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(expr);
    normalize_optional_constructed_string_view_expr(RustExpr::Call {
        function: Box::new(RustExpr::Var("Some".to_string())),
        args: vec![expr],
    })
}

fn render_raw_option_some(expr: RustExpr) -> RustExpr {
    let expr = normalize_optional_constructed_element_expr(expr);
    normalize_optional_constructed_string_view_expr(RustExpr::Raw(format!(
        "Some({})",
        emit::emit_expr(&expr)
    )))
}

fn parenthesize_compound_cast_operand(rendered: &str) -> String {
    let trimmed = rendered.trim();
    if balanced_outer_parentheses_wrap_all(trimmed) {
        return trimmed.to_string();
    }
    if trimmed.contains(" + ")
        || trimmed.contains(" - ")
        || trimmed.contains(" * ")
        || trimmed.contains(" / ")
        || trimmed.contains(" % ")
        || trimmed.contains(" & ")
        || trimmed.contains(" | ")
        || trimmed.contains(" ^ ")
        || trimmed.contains(" << ")
        || trimmed.contains(" >> ")
        || trimmed.contains(" && ")
        || trimmed.contains(" || ")
    {
        format!("({trimmed})")
    } else {
        trimmed.to_string()
    }
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

fn apply_unsupported_dispatch_coercion(
    expr: RustExpr,
    actual_dispatch: &DispatchShape,
    target_dispatch: &DispatchShape,
    policy: &DispatchCoercionFallbackPolicy,
) -> RustExpr {
    dispatch_coercion_fallback_message_policy(policy, actual_dispatch, target_dispatch)
        .compile_error_expr_consuming(&expr)
}

fn render_invalid_cast_policy(policy: &InvalidCastPolicy) -> &'static str {
    match policy {
        InvalidCastPolicy::Panic => "panic!(\"invalid Java cast\")",
    }
}

fn apply_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    match invalid_cast {
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "{}.as_any().downcast_ref::<{target_type}>().expect(\"invalid Java cast\")",
            emit::emit_expr(&expr)
        )),
    }
}

fn apply_boxed_dyn_trait_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    match invalid_cast {
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "{}.as_ref().as_any().downcast_ref::<{target_type}>().expect(\"invalid Java cast\")",
            emit::emit_expr(&expr)
        )),
    }
}

pub fn erased_object_borrowed_downcast_ref_projection(
    expr: RustExpr,
    target_type: &str,
    optional_carrier: bool,
) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if optional_carrier {
        RustExpr::Raw(format!(
            "{rendered}.as_ref().and_then(|value| value.downcast_ref::<{target_type}>()).expect(\"invalid Java cast\")"
        ))
    } else {
        RustExpr::Raw(format!(
            "{rendered}.downcast_ref::<{target_type}>().expect(\"invalid Java cast\")"
        ))
    }
}

fn apply_erased_object_downcast(
    expr: RustExpr,
    target_type: &str,
    invalid_cast: &InvalidCastPolicy,
) -> RustExpr {
    if !plausible_rust_nominal_type_name(target_type) {
        return expr;
    }
    match invalid_cast {
        InvalidCastPolicy::Panic => RustExpr::Raw(format!(
            "*{}.downcast::<{target_type}>().expect(\"invalid Java cast\")",
            emit::emit_expr(&expr)
        )),
    }
}

fn plausible_rust_nominal_type_name(target_type: &str) -> bool {
    target_type
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_uppercase())
}

fn apply_to_owned_sequence(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    to_owned_sequence_projection(expr, policy)
}

pub fn to_owned_sequence_projection(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if rendered.ends_with(".to_vec()") {
        return expr;
    }
    let receiver = if rust_receiver_expr_needs_parentheses(&expr, &rendered) {
        format!("({rendered})")
    } else {
        rendered.to_string()
    };
    RustExpr::Raw(format!(
        "{receiver}.to_vec(){}",
        render_clone_recovery_policy(policy)
    ))
}

fn apply_clone_recovery(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if rendered_expr_has_clone_recovery(&rendered) {
        return expr;
    }
    RustExpr::Raw(render_clone_recovery_expr(&rendered, policy))
}

fn render_clone_recovery_expr(rendered: &str, policy: &CloneRecoveryPolicy) -> String {
    if let Some(receiver) = rendered.strip_suffix(".into()") {
        return format!(
            "{receiver}.clone(){}.into()",
            render_clone_recovery_policy(policy)
        );
    }
    let receiver = if rust_method_receiver_needs_parentheses(&rendered) {
        format!("({rendered})")
    } else {
        rendered.to_string()
    };
    format!("{receiver}.clone(){}", render_clone_recovery_policy(policy))
}

fn rust_receiver_expr_needs_parentheses(expr: &RustExpr, rendered: &str) -> bool {
    match expr {
        RustExpr::Field(_, _)
        | RustExpr::Call { .. }
        | RustExpr::MethodCall { .. }
        | RustExpr::Literal(_)
        | RustExpr::Var(_) => false,
        RustExpr::Index(_, _) => rust_method_receiver_needs_parentheses(rendered),
        RustExpr::Raw(_) => rust_method_receiver_needs_parentheses(rendered),
        RustExpr::Binary { .. }
        | RustExpr::Unary { .. }
        | RustExpr::Cast { .. }
        | RustExpr::If { .. } => true,
        RustExpr::StructLit { .. } => true,
    }
}

fn rust_method_receiver_needs_parentheses(rendered: &str) -> bool {
    rendered.contains(" as ")
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
mod tests {
    use super::*;

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
        assert_eq!(
            emit::emit_expr(&primitive_hash_code_projection(
                RustExpr::Var("flag".to_string()),
                PrimitiveType::Boolean,
            )),
            "if flag { 1231 } else { 1237 }"
        );
        assert_eq!(
            emit::emit_expr(&primitive_hash_code_projection(
                RustExpr::Var("wide".to_string()),
                PrimitiveType::Long,
            )),
            "((wide as i64) ^ ((wide as i64) >> 32)) as i32"
        );
        assert_eq!(
            emit::emit_expr(&primitive_hash_code_projection(
                RustExpr::Var("score".to_string()),
                PrimitiveType::Double,
            )),
            "((score).to_bits() ^ ((score).to_bits() >> 32)) as i32"
        );
        assert_eq!(
            emit::emit_expr(&primitive_compare_to_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "java_primitive_compare_to(left, right)"
        );
    }

    #[test]
    fn string_char_at_projection_owns_index_and_bounds_spelling() {
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Var("index".to_string()),
            )),
            "value.chars().nth((index) as usize).expect(\"Java string index out of bounds\")"
        );
        assert_eq!(
            emit::emit_expr(&string_char_at_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Raw("left + right".to_string()),
            )),
            "value.chars().nth((left + right) as usize).expect(\"Java string index out of bounds\")"
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
            emit::emit_expr(&string_index_of_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Var("needle".to_string()),
                Some(RustExpr::Var("start".to_string())),
            )),
            "java_string_index_of_from(&value, &needle, start)"
        );
        assert_eq!(
            emit::emit_expr(&string_substring_projection(
                RustExpr::Raw("value.as_str()".to_string()),
                RustExpr::Literal("0".to_string()),
                RustExpr::Literal("2".to_string()),
            )),
            "java_string_substring(value.as_str(), 0, 2)"
        );
    }

    #[test]
    fn string_runtime_helper_projections_own_helper_call_shapes() {
        assert_eq!(
            emit::emit_expr(&string_hash_code_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "java_string_hash_code(value.as_str())"
        );
        assert_eq!(
            emit::emit_expr(&string_replace_first_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"-\"".to_string()),
                RustExpr::Literal("\"\"".to_string()),
            )),
            "java_string_replace_first(value.as_str(), \"-\", \"\")"
        );
        assert_eq!(
            emit::emit_expr(&string_matches_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"[0-9]\"".to_string()),
            )),
            "java_string_matches(value.as_str(), \"[0-9]\")"
        );
        assert_eq!(
            emit::emit_expr(&string_split_projection(
                RustExpr::Var("value".to_string()),
                RustExpr::Literal("\"_\"".to_string()),
                RustExpr::Literal("0".to_string()),
            )),
            "java_string_split(value.as_str(), \"_\", 0)"
        );
        assert_eq!(
            emit::emit_expr(&string_compare_to_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Raw("right.as_str()".to_string()),
            )),
            "java_string_compare_to(&left, right.as_str())"
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
        assert_eq!(
            emit::emit_expr(&file_exists_projection(RustExpr::Var("file".to_string()))),
            "std::path::Path::new(file.get_path().as_str()).exists()"
        );
        assert_eq!(
            emit::emit_expr(&system_exit_projection(RustExpr::Var("status".to_string()))),
            "std::process::exit(status as i32)"
        );
        assert_eq!(
            emit::emit_expr(&thread_sleep_millis_projection(RustExpr::Var(
                "millis".to_string()
            ))),
            "std::thread::sleep(std::time::Duration::from_millis(millis as u64))"
        );
    }

    #[test]
    fn jdk_math_and_objects_projections_own_expression_shapes() {
        assert_eq!(
            emit::emit_expr(&math_max_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "std::cmp::max(left, right)"
        );
        assert_eq!(
            emit::emit_expr(&math_min_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "std::cmp::min(left, right)"
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
        assert_eq!(
            emit::emit_expr(&anonymous_dyn_trait_projection("Callback")),
            "<dyn Callback>::anonymous()"
        );
        assert_eq!(
            emit::emit_expr(&unwrap_java_null_receiver_projection(RustExpr::Var(
                "stream".to_string()
            ))),
            "stream.expect(\"Java null receiver\")"
        );
        assert_eq!(
            emit::emit_expr(&typed_none_projection("Box<dyn std::any::Any>")),
            "None::<Box<dyn std::any::Any>>"
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
        assert_eq!(
            emit::emit_expr(&arrays_equals_projection(
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            )),
            "left == right"
        );
        assert_eq!(
            emit::emit_expr(&collections_empty_list_projection()),
            "Vec::new()"
        );
        assert_eq!(
            emit::emit_expr(&collections_empty_sorted_set_projection()),
            "std::collections::BTreeSet::new()"
        );
        assert_eq!(
            emit::emit_expr(&collections_singleton_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "std::collections::HashSet::from([value])"
        );
        assert_eq!(
            emit::emit_expr(&collections_singleton_list_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "vec![value]"
        );
        assert_eq!(
            emit::emit_expr(&collections_singleton_map_projection(
                RustExpr::Var("key".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "std::collections::HashMap::from([(key, value)])"
        );
        assert_eq!(
            emit::emit_expr(&collections_n_copies_projection(
                RustExpr::Var("count".to_string()),
                RustExpr::Var("value".to_string()),
            )),
            "vec![value; count as usize]"
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
            emit::emit_expr(&native_collection_default_constructor_projection(
                crate::stdlib::JavaCollectionKind::Set,
            )),
            "std::collections::HashSet::new()"
        );
        assert_eq!(
            emit::emit_expr(&sequence_iter_cloned_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.iter().cloned()"
        );
        assert_eq!(
            emit::emit_expr(&sequence_iter_copied_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.iter().copied()"
        );
        assert_eq!(
            emit::emit_expr(&iterator_next_expect_projection(RustExpr::Var(
                "entries".to_string()
            ))),
            "entries.next().expect(\"Java iterator exhausted\")"
        );
        assert_eq!(
            emit::emit_expr(&tuple_field_projection(
                RustExpr::Var("entry".to_string()),
                1,
            )),
            "entry.1"
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
            emit::emit_expr(&erased_object_box_projection(RustExpr::Raw(
                "(variant, None::<Void>)".to_string()
            ))),
            "Box::new((variant, None::<Void>)) as Box<dyn std::any::Any>"
        );
        assert_eq!(
            emit::emit_expr(&varargs_packed_vec_projection(Vec::new())),
            "Vec::new()"
        );
        assert_eq!(
            emit::emit_expr(&varargs_packed_vec_projection(vec![
                RustExpr::Var("left".to_string()),
                RustExpr::Var("right".to_string()),
            ])),
            "vec![left, right]"
        );
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
            emit::emit_expr(&collection_sort_projection(RustExpr::Var(
                "values".to_string()
            ))),
            "values.sort()"
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
            emit::emit_expr(&sequence_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Raw("index as usize".to_string()),
            )),
            "values.get(index as usize).expect(\"Java index out of bounds\")"
        );
        assert_eq!(
            emit::emit_expr(&map_get_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get(key)"
        );
        assert_eq!(
            emit::emit_expr(&map_get_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get(key).expect(\"Java null receiver\")"
        );
        assert_eq!(
            emit::emit_expr(&map_get_mut_expect_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Var("key".to_string()),
            )),
            "values.get_mut(key).expect(\"Java null receiver\")"
        );
        assert_eq!(
            emit::emit_expr(&optional_copied_expect_projection(RustExpr::Raw(
                "values.get(key)".to_string()
            ))),
            "values.get(key).copied().expect(\"Java null receiver\")"
        );
        assert_eq!(
            emit::emit_expr(&subtype_owned_constructor_from_borrowed_projection(
                "Locatable",
                RustExpr::Var("value".to_string()),
            )),
            "Locatable::new_1(value.clone())"
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
    }

    #[test]
    fn string_join_projection_helpers_own_literal_and_to_string_idempotence() {
        assert_eq!(
            render_string_join_vararg_value(RustExpr::Literal("\"bin\"".to_string())),
            "\"bin\".to_string()"
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
            "sep.as_str()"
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
            "vec![\"bin\".to_string(), (extra).to_string(), suffix.to_string()].join(sep.as_str())"
        );
    }

    #[test]
    fn string_format_arg_projection_owns_display_to_string_spelling() {
        assert_eq!(
            render_string_format_arg_value(RustExpr::Var("count".to_string())),
            "count.to_string()"
        );
        assert_eq!(
            render_string_format_arg_value(RustExpr::Raw("value + 1".to_string())),
            "value + 1.to_string()"
        );
        assert_eq!(
            emit::emit_expr(&string_format_projection(Vec::new())),
            "String::new()"
        );
        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::Literal("\"%s:%04.2f\"".to_string()),
                RustExpr::Var("name".to_string()),
                RustExpr::Var("score".to_string()),
            ])),
            "format!(\"{}:{:04.2}\", name, score)"
        );
        assert_eq!(
            emit::emit_expr(&string_format_projection(vec![
                RustExpr::Var("template".to_string()),
                RustExpr::Var("count".to_string()),
            ])),
            "java_string_format(template, vec![count.to_string()])"
        );
    }

    #[test]
    fn string_helper_receiver_projection_owns_runtime_helper_receiver_view() {
        assert_eq!(
            emit::emit_expr(&string_helper_receiver_projection(RustExpr::Raw(
                "left.as_ref().unwrap()".to_string()
            ))),
            "left.as_ref().unwrap().as_str()"
        );
    }

    #[test]
    fn string_get_bytes_projection_owns_byte_view_spelling() {
        assert_eq!(
            emit::emit_expr(&string_get_bytes_projection(RustExpr::Var(
                "value".to_string()
            ))),
            "value.as_bytes().iter().map(|byte| *byte as i8).collect::<Vec<i8>>()"
        );
    }

    #[test]
    fn primitive_number_value_projection_owns_boxed_value_cast_spelling() {
        assert_eq!(
            emit::emit_expr(&primitive_number_value_projection(
                RustExpr::Var("value".to_string()),
                PrimitiveType::Double
            )),
            "value as f64"
        );
        assert_eq!(
            emit::emit_expr(&primitive_number_value_projection(
                RustExpr::Var("flag".to_string()),
                PrimitiveType::Boolean
            )),
            "flag"
        );
    }

    #[test]
    fn primitive_f64_method_projection_owns_math_cast_spelling() {
        assert_eq!(
            emit::emit_expr(&primitive_f64_method_projection(
                RustExpr::Var("value".to_string()),
                "ln"
            )),
            "(value as f64).ln()"
        );
    }

    #[test]
    fn len_i32_projection_owns_java_length_spelling() {
        assert_eq!(
            emit::emit_expr(&len_i32_projection(RustExpr::Var("value".to_string()))),
            "value.len() as i32"
        );
        assert_eq!(
            emit::emit_expr(&array2_rows_i32_projection(RustExpr::Var(
                "matrix".to_string()
            ))),
            "matrix.nrows() as i32"
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
    }

    #[test]
    fn java_display_plan_renders_optional_null_policy() {
        let expr = apply_java_display_plan(
            RustExpr::Var("name".to_string()),
            JavaDisplayPlan::OptionalString,
        );
        assert_eq!(
            emit::emit_expr(&expr),
            "name.as_deref().unwrap_or(\"null\")"
        );
    }

    #[test]
    fn java_display_operand_removes_redundant_arithmetic_to_string() {
        let expr = RustExpr::Raw("(value * 100).to_string()".to_string());
        assert_eq!(render_java_display_operand(&expr), "(value * 100)");
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
    fn audited_clone_recovery_preserves_into_suffix() {
        assert_eq!(
            render_audited_clone_recovery("value.into()"),
            "value.clone()/* TODO(translation): validate added clone */.into()"
        );
    }
}
