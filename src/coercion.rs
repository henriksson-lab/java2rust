use crate::conversion_plan::{
    BorrowedDynToOwnedBoxedDynRecoveryPolicy, BorrowedElementAdapterPolicy, BorrowedElementCarrier,
    BorrowedElementRecoveryFallbackPolicy, BorrowedElementRecoveryPolicy,
    BorrowedElementSlotAdapterPolicy, CloneRecoveryPolicy, DispatchCoercionFallbackPolicy,
    DispatchCoercionPolicy, DynCloneBoxAdapterGenerationPolicy, InstanceOfCheck,
    InstanceOfFallbackPolicy, InvalidCastPolicy, LoweredType, OptionalShapeCoercionPolicy,
    OwnershipCoercionFallbackPolicy, OwnershipCoercionPolicy, RustBorrowedFacadeReturnPolicy,
    RustScalarSlotCastPolicy, RustUnsignedByteArrayReturnPolicy, ShapeCoercionPlan, Slot,
    SlotConversionPlan, SyntaxCoercionPlan, SyntaxSlotCoercionPolicy, ViewCoercionPolicy,
    dyn_clone_box_adapter_deferral_message, dyn_clone_box_adapter_generation_policy,
    dyn_clone_box_adapter_requirement,
};
use crate::emit;
use crate::facts::FactSet;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::RustExpr;
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape};

#[derive(Debug, Clone, PartialEq)]
pub struct LoweredExpr {
    pub expr: RustExpr,
    pub actual: LoweredType,
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
        RustExpr::Raw(format!("compile_error!(\"{}\")", self.message))
    }

    pub fn compile_error_expr_consuming(&self, expr: &RustExpr) -> RustExpr {
        RustExpr::Raw(format!(
            "{{ let _ = {}; compile_error!(\"{}\") }}",
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
    OptionalToJavaDisplayString,
    OptionalToJavaDisplayValue,
    AsStr,
    ToOwnedString,
    AsSlice,
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
    Borrow,
    CopyBorrowedValue,
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
        plan.extend(self.ownership.coercions());
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
                coercions.extend(non_identity_coercions(element_plan.coercions()));
                coercions
            }
            OptionalShapeCoercionPolicy::MapElement { .. } => Vec::new(),
            OptionalShapeCoercionPolicy::NoChange => Vec::new(),
        }
    }
}

fn optional_unwrap_coercion_for_element_plan(element_plan: &ShapeCoercionPlan) -> Coercion {
    if element_plan.target.ownership == OwnershipShape::SharedBorrow {
        Coercion::UnwrapOptionBorrowed
    } else {
        Coercion::UnwrapOption
    }
}

impl ViewCoercionPolicy {
    pub fn coercions(&self) -> Vec<Coercion> {
        match self {
            ViewCoercionPolicy::NoViewChange => Vec::new(),
            ViewCoercionPolicy::BorrowStringView => vec![Coercion::AsStr],
            ViewCoercionPolicy::OwnStringFromView => vec![Coercion::ToOwnedString],
            ViewCoercionPolicy::BorrowSequenceSliceView => vec![Coercion::AsSlice],
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
            OwnershipCoercionPolicy::RecoverOwnedWithClone { policy } => {
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
            return apply_shape_coercion_plan(expr, element_plan);
        }
        OptionalShapeCoercionPolicy::MapElement { element_plan, .. } => {
            let mapped_value =
                apply_shape_coercion_plan(RustExpr::Var("value".to_string()), element_plan);
            if emit::emit_expr(&mapped_value) == "&value" {
                return RustExpr::Raw(format!("{}.as_ref()", emit::emit_expr(&expr)));
            }
            if emit::emit_expr(&mapped_value) == "value" {
                return expr;
            }
            if emit::emit_expr(&mapped_value) == "value.as_str()" {
                return RustExpr::Raw(format!(
                    "{}.as_ref().map(|value| value.as_str())",
                    emit::emit_expr(&expr)
                ));
            }
            return RustExpr::Raw(format!(
                "{}.map(|value| {})",
                emit::emit_expr(&expr),
                emit::emit_expr(&mapped_value)
            ));
        }
        OptionalShapeCoercionPolicy::NoChange => {}
    }
    apply_coercions(expr, &plan.coercions())
}

pub fn apply_syntax_coercion_plan(expr: RustExpr, plan: &SyntaxCoercionPlan) -> RustExpr {
    apply_coercions(expr, &plan.coercions())
}

pub fn apply_slot_conversion_plan(expr: RustExpr, plan: &SlotConversionPlan) -> RustExpr {
    match plan {
        SlotConversionPlan::Shape(plan) => apply_shape_coercion_plan(expr, plan),
        SlotConversionPlan::Syntax(plan) => apply_syntax_coercion_plan(expr, plan),
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
    let recovered = if rust_borrowed_return_is_path_like(&policy.rust_type_name) {
        RustExpr::Raw(format!("{}.to_path_buf()", emit::emit_expr(&expr)))
    } else {
        RustExpr::Raw(format!("{}.clone()", emit::emit_expr(&expr)))
    };
    let erased = RustExpr::Raw(format!(
        "Box::new({}) as Box<dyn std::any::Any>",
        emit::emit_expr(&recovered)
    ));
    if policy.target.optional_element_type().is_some() {
        RustExpr::Raw(format!("Some({})", emit::emit_expr(&erased)))
    } else {
        erased
    }
}

fn rust_borrowed_return_is_path_like(rust_type_name: &str) -> bool {
    matches!(
        rust_type_name.trim_start_matches('&').trim(),
        "Path" | "std::path::Path" | "std::path::PathBuf" | "PathBuf"
    )
}

fn apply_rust_scalar_slot_cast(expr: RustExpr, policy: &RustScalarSlotCastPolicy) -> RustExpr {
    if rust_scalar_type_name_for_primitive(policy.target_primitive)
        == Some(policy.rust_type_name.as_str())
    {
        return expr;
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
        let adapted = apply_borrowed_element_adapter(expr, adapter);
        if matches!(
            adapter.recovery,
            BorrowedElementRecoveryPolicy::BorrowElement
        ) {
            apply_borrowed_element_target_view(adapted, policy)
        } else {
            adapted
        }
    } else {
        expr
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
        Coercion::IndexToUsize => RustExpr::Raw(format!("{} as usize", emit::emit_expr(&expr))),
        Coercion::ToStringForConcat => {
            RustExpr::Raw(format!("({}).to_string()", emit::emit_expr(&expr)))
        }
        Coercion::OptionalToJavaDisplayString => RustExpr::Raw(format!(
            "{}.as_deref().unwrap_or(\"null\")",
            emit::emit_expr(&expr)
        )),
        Coercion::OptionalToJavaDisplayValue => RustExpr::Raw(format!(
            "{}.as_ref().map(|value| value.to_string()).unwrap_or_else(|| \"null\".to_string())",
            emit::emit_expr(&expr)
        )),
        Coercion::AsStr => RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&expr))),
        Coercion::ToOwnedString => {
            let rendered = emit::emit_expr(&expr);
            if rendered.ends_with(".to_string()") {
                expr
            } else {
                RustExpr::Raw(format!("{rendered}.to_string()"))
            }
        }
        Coercion::AsSlice => RustExpr::Raw(format!("{}.as_slice()", emit::emit_expr(&expr))),
        Coercion::WrapClosedEnum {
            enum_name,
            variant_name,
        } => RustExpr::Raw(format!(
            "{enum_name}::{variant_name}({})",
            emit::emit_expr(&expr)
        )),
        Coercion::BorrowDynTrait { trait_name } => {
            RustExpr::Raw(format!("&{} as &dyn {trait_name}", emit::emit_expr(&expr)))
        }
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
        Coercion::WidenPrimitive { to, .. } => RustExpr::Cast {
            expr: Box::new(expr),
            ty: rust_primitive_type_name_policy(*to).rust_name.to_string(),
        },
        Coercion::WrapOption => RustExpr::Raw(format!("Some({})", emit::emit_expr(&expr))),
        Coercion::UnwrapOption => RustExpr::Raw(format!("{}.unwrap()", emit::emit_expr(&expr))),
        Coercion::UnwrapOptionBorrowed => RustExpr::Raw(format!(
            "{}.as_ref().expect(\"Java null receiver\")",
            emit::emit_expr(&expr)
        )),
        Coercion::Borrow => RustExpr::Raw(format!("&{}", emit::emit_expr(&expr))),
        Coercion::BorrowMut => RustExpr::Raw(format!("&mut {}", emit::emit_expr(&expr))),
        Coercion::CopyBorrowedValue => RustExpr::Raw(format!("*{}", emit::emit_expr(&expr))),
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

fn apply_clone_recovery(expr: RustExpr, policy: &CloneRecoveryPolicy) -> RustExpr {
    let rendered = emit::emit_expr(&expr);
    if rendered.ends_with(".clone()")
        || rendered.ends_with(".clone()/* TODO(translation): validate added clone */")
    {
        return expr;
    }
    let receiver = if rust_method_receiver_needs_parentheses(&rendered) {
        format!("({rendered})")
    } else {
        rendered
    };
    RustExpr::Raw(format!(
        "{receiver}.clone(){}",
        render_clone_recovery_policy(policy)
    ))
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
