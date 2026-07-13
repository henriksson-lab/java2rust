use std::collections::{HashMap, HashSet};

use crate::effective_facts::EffectiveFacts;
use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::facts::{
    CaptureOrigin, CaptureSource, DispatchRequirementKind, FactSet, NullabilityState,
    OwnedRecoveryCapability, ReceiverMutabilityFact,
};
use crate::java_ir::{
    Block, ConstructorDecl, Expr, FieldDecl, JavaType, MethodDecl, Modifier, Param, Program, Stmt,
    TypeDecl, UnaryOp,
};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    DispatchRequirementProjectionRole, apply_required_dispatch, apply_required_ownership,
    apply_required_view, dispatch_requirement_projection_policy, param_shape_for_java_type,
    return_shape_for_java_type, storage_shape_for_java_type,
};
use crate::slot_policy::{
    BorrowedContentKind, BorrowedContentPolicy,
    borrowed_return_content_policy_from_param_with_facts,
    constructor_param_borrowed_content_policy_with_facts,
    method_param_borrowed_content_policy_with_facts,
};
use crate::slot_shape::{
    DispatchShape, OwnershipShape, RustSlotShape, ViewShape, conservative_ownership,
};
use crate::types::{optional_element_type, optional_java_type};
use crate::usage::{
    BindingUse, BindingUseFacts, CallArgumentBoundary, FieldUse, MethodUsageFacts,
    UsageConstructorKey, UsageFacts, UsageMethodKey,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SignaturePlan {
    methods: HashMap<MethodKey, MethodSignaturePlan>,
    constructors: HashMap<ConstructorKey, ConstructorSignaturePlan>,
    fields: HashMap<FieldKey, RustSlotShape>,
    solve_iterations: usize,
    diagnostics: Vec<SignatureDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstructorKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldKey {
    pub type_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignaturePlan {
    pub params: Vec<ParamPlan>,
    pub return_shape: Option<RustSlotShape>,
    pub locals: Vec<LocalPlan>,
    pub receiver: ReceiverPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorSignaturePlan {
    pub params: Vec<ParamPlan>,
    pub locals: Vec<LocalPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamPlan {
    pub name: String,
    pub java_ty: JavaType,
    pub is_varargs: bool,
    pub shape: RustSlotShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlan {
    pub name: String,
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub binding_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverPlan {
    None,
    Shared,
    Mutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureConstraint {
    RequiredOwnership(OwnershipShape),
    RequiredView(ViewShape),
    RequiredDispatch(DispatchShape),
    RequiredStorageNullability(NullabilityState),
    MutableBinding,
    StorageSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSlotRole {
    Param { is_varargs: bool },
    Return,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureBoundaryKind {
    PublicApi,
    UnknownApi,
    EscapingStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureBoundaryFallback {
    ConservativeNative,
    PreserveObjectRepresentation,
    PreserveObjectRepresentationEscapingStorage,
    EscapingNativeStorage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBoundaryPolicy {
    pub kind: SignatureBoundaryKind,
    pub java_ty: JavaType,
    pub fallback: SignatureBoundaryFallback,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBoundaryRepresentationPolicy {
    pub java_ty: JavaType,
    pub preserve_object_representation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBoundaryRepresentationFactPolicy {
    pub java_ty: JavaType,
    pub object_representation_facts: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureNativeBoundaryPolicy {
    pub java_ty: JavaType,
    pub required_ownership: OwnershipShape,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureCapabilityPolicy {
    pub java_ty: JavaType,
    pub capability_ownership: Option<OwnershipShape>,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSlotBaselinePolicy {
    pub java_ty: JavaType,
    pub boundary_kind: Option<SignatureBoundaryKind>,
    pub capability_constraints: Vec<SignatureConstraint>,
    pub boundary_constraints: Vec<SignatureConstraint>,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSlotDispatchRequirementPolicy {
    pub java_ty: JavaType,
    pub role: SignatureSlotRole,
    pub source_dispatch: DispatchShape,
    pub requirements: Vec<DispatchRequirementKind>,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldNullabilityShapePolicy {
    pub type_name: String,
    pub field_name: String,
    pub nullability: Option<NullabilityState>,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureConstraintPropagationKind {
    ShapeOnly,
    SlotShapeAndStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureConstraintPropagationPolicy {
    pub kind: SignatureConstraintPropagationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureShapeRequest {
    pub java_ty: JavaType,
    pub role: SignatureSlotRole,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergedSignatureConstraints {
    required_owner: Option<OwnershipShape>,
    slot_view_req: Option<ViewShape>,
    slot_dispatch_req: Option<DispatchShape>,
    storage_nullability_req: Option<NullabilityState>,
    mutable_binding: bool,
    storage_slot: bool,
    conflicts: Vec<SignatureConstraintConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureConstraintConflict {
    pub axis: SignatureConstraintAxis,
    pub kept: SignatureConstraint,
    pub rejected: SignatureConstraint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureRequirementMerge<T> {
    pub kept: T,
    pub rejected: Option<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureOwnershipRequirementPriorityPolicy {
    pub requirement: OwnershipShape,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureViewRequirementPriorityPolicy {
    pub requirement: ViewShape,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureDispatchRequirementPriorityPolicy {
    pub requirement: DispatchShape,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBoundaryKindMerge {
    pub kept: Option<SignatureBoundaryKind>,
    pub rejected: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureMemberApiBoundaryPolicy {
    pub public_api: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingBoundaryPolicy {
    pub binding_name: String,
    pub kind: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodParamBoundaryPolicy {
    pub type_name: String,
    pub method_name: String,
    pub param_name: String,
    pub kind: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorParamBoundaryPolicy {
    pub type_name: String,
    pub constructor_name: String,
    pub param_name: String,
    pub kind: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldBoundaryPolicy {
    pub type_name: String,
    pub field_name: String,
    pub kind: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodReturnBoundaryPolicy {
    pub type_name: String,
    pub method_name: String,
    pub kind: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureOwnedRecoveryPolicy {
    NotRequired,
    ProvenByCopy,
    ProvenByExplicitClone,
    AuditConservativeClone,
    UnsupportedNonCloneable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureOwnedRecoveryCapabilityPolicy {
    pub java_ty: JavaType,
    pub recovery: OwnedRecoveryCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureConstraintAxis {
    Owner,
    View,
    Dispatch,
    Nullability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureFixpointResult {
    pub constraints: SignatureConstraintState,
    pub iterations: usize,
    pub diagnostics: Vec<SignatureDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureConstraintState {
    pub methods: HashMap<MethodKey, MethodConstraintSet>,
    pub constructors: HashMap<ConstructorKey, ConstructorConstraintSet>,
    pub fields: HashMap<FieldKey, Vec<SignatureConstraint>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodConstraintSet {
    pub public_api: bool,
    pub param_names: Vec<String>,
    pub params: Vec<Vec<SignatureConstraint>>,
    pub return_constraints: Vec<SignatureConstraint>,
    pub locals: Vec<LocalConstraintSet>,
    pub receiver: Vec<ReceiverConstraint>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstructorConstraintSet {
    pub public_api: bool,
    pub param_names: Vec<String>,
    pub params: Vec<Vec<SignatureConstraint>>,
    pub locals: Vec<LocalConstraintSet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalConstraintSet {
    pub name: String,
    pub java_ty: JavaType,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverConstraint {
    StaticReceiver,
    MutableReceiver,
}

pub fn solve_signatures(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> SignaturePlan {
    solve_signatures_with_facts(program, representations, usage, &FactSet::default())
}

pub fn solve_signatures_with_facts(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
) -> SignaturePlan {
    let constraints = collect_signature_constraints(program, usage, facts);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, usage);
    build_signature_plan_from_constraints(
        program,
        representations,
        &result.constraints,
        result.iterations,
        result.diagnostics,
        facts,
    )
}

pub fn collect_signature_constraints(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
) -> SignatureConstraintState {
    let mut constraints = SignatureConstraintState::default();
    for ty in &program.types {
        collect_type_signature_constraints(ty, usage, facts, &mut constraints);
    }
    constraints
}

pub fn solve_signature_constraints_to_fixpoint(
    constraints: SignatureConstraintState,
    usage: &UsageFacts,
) -> (SignatureConstraintState, usize) {
    let result = solve_signature_constraints_to_fixpoint_result(constraints, usage);
    (result.constraints, result.iterations)
}

pub fn solve_signature_constraints_to_fixpoint_result(
    constraints: SignatureConstraintState,
    usage: &UsageFacts,
) -> SignatureFixpointResult {
    let saturation_policy = signature_fixpoint_saturation_policy(&constraints);
    let result = saturation_policy.saturate(
        constraints,
        |constraints| {
            let changed = propagate_call_site_constraints(constraints, usage)
                | propagate_constructor_call_site_constraints(constraints, usage)
                | propagate_constructor_delegation_constraints(constraints, usage)
                | propagate_call_argument_constraints_to_callees(constraints, usage)
                | propagate_constructor_call_argument_constraints_to_callees(constraints, usage)
                | propagate_constructor_delegation_argument_constraints_to_callees(
                    constraints,
                    usage,
                )
                | propagate_call_result_constraints_to_bindings(constraints, usage)
                | propagate_constructor_call_result_constraints_to_bindings(constraints, usage)
                | propagate_call_result_constraints_to_fields(constraints, usage)
                | propagate_constructor_call_result_constraints_to_fields(constraints, usage)
                | propagate_method_local_alias_constraints(constraints, usage)
                | propagate_method_local_alias_source_constraints(constraints, usage)
                | propagate_constructor_local_alias_constraints(constraints, usage)
                | propagate_constructor_local_alias_source_constraints(constraints, usage)
                | propagate_method_field_storage_constraints(constraints, usage)
                | propagate_constructor_field_storage_constraints(constraints, usage)
                | propagate_method_return_binding_constraints(constraints, usage)
                | propagate_method_binding_return_constraints(constraints, usage)
                | propagate_method_return_field_constraints(constraints, usage)
                | propagate_method_field_return_constraints(constraints, usage);
            SaturationStep::from(changed)
        },
        |max_iterations| SignatureDiagnostic {
            code: "signature_fixpoint_budget_exceeded",
            message: format!(
                "signature constraint fixpoint exceeded finite saturation budget of {max_iterations} iterations"
            ),
        },
    );
    SignatureFixpointResult {
        constraints: result.state,
        iterations: result.iterations,
        diagnostics: result.diagnostics,
    }
}

pub fn signature_fixpoint_saturation_policy(
    constraints: &SignatureConstraintState,
) -> FiniteSaturationPolicy {
    let slot_count = signature_constraint_slot_count(constraints);
    let unique_constraint_count = unique_signature_constraints(constraints).len();
    FiniteSaturationPolicy::new(slot_count, unique_constraint_count)
}

pub fn signature_fixpoint_iteration_budget(constraints: &SignatureConstraintState) -> usize {
    signature_fixpoint_saturation_policy(constraints).iteration_budget()
}

fn signature_constraint_slot_count(constraints: &SignatureConstraintState) -> usize {
    let method_slots = constraints
        .methods
        .values()
        .map(|method| method.params.len() + method.locals.len() + 1)
        .sum::<usize>();
    let constructor_slots = constraints
        .constructors
        .values()
        .map(|constructor| constructor.params.len() + constructor.locals.len())
        .sum::<usize>();
    method_slots + constructor_slots + constraints.fields.len()
}

fn unique_signature_constraints(
    constraints: &SignatureConstraintState,
) -> Vec<SignatureConstraint> {
    let mut unique = Vec::new();
    for method in constraints.methods.values() {
        for param in &method.params {
            push_unique_signature_constraints(&mut unique, param);
        }
        push_unique_signature_constraints(&mut unique, &method.return_constraints);
        for local in &method.locals {
            push_unique_signature_constraints(&mut unique, &local.constraints);
        }
    }
    for constructor in constraints.constructors.values() {
        for param in &constructor.params {
            push_unique_signature_constraints(&mut unique, param);
        }
        for local in &constructor.locals {
            push_unique_signature_constraints(&mut unique, &local.constraints);
        }
    }
    for field in constraints.fields.values() {
        push_unique_signature_constraints(&mut unique, field);
    }
    unique
}

fn push_unique_signature_constraints(
    unique: &mut Vec<SignatureConstraint>,
    constraints: &[SignatureConstraint],
) {
    for constraint in constraints {
        if !unique.contains(constraint) {
            unique.push(constraint.clone());
        }
    }
}

fn propagate_call_site_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(caller_constraints) = constraints.methods.get(&caller_key) else {
            continue;
        };
        if caller_constraints.public_api {
            continue;
        }
        for call_site in &method_usage.call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            let Some(callee_constraints) = constraints.methods.get(&callee_key) else {
                continue;
            };
            if call_site.result_returned {
                for constraint in &callee_constraints.return_constraints {
                    if signature_shape_constraint_propagation_policy().allows(constraint) {
                        pending.push((
                            caller_key.clone(),
                            String::new(),
                            PendingSignatureConstraint::Return(constraint.clone()),
                        ));
                    }
                }
            }
            for binding in method_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name
                        || *boundary != call_site.boundary
                        || *arg_index >= callee_constraints.params.len()
                    {
                        continue;
                    }
                    for constraint in &callee_constraints.params[*arg_index] {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((
                                caller_key.clone(),
                                binding.name.clone(),
                                PendingSignatureConstraint::Binding(constraint.clone()),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, param_name, constraint) in pending {
        changed |= match constraint {
            PendingSignatureConstraint::Binding(constraint) => {
                add_method_binding_constraint(constraints, &method_key, &param_name, constraint)
            }
            PendingSignatureConstraint::Return(constraint) => {
                add_method_return_constraint(constraints, &method_key, constraint)
            }
        };
    }
    changed
}

fn propagate_constructor_call_site_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, constructor_usage) in &usage.constructors {
        let constructor_key = ConstructorKey {
            type_name: usage_key.type_name.clone(),
            rust_name: usage_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&constructor_key) else {
            continue;
        };
        if constructor_constraints.public_api {
            continue;
        }
        for call_site in &constructor_usage.call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            let Some(callee_constraints) = constraints.methods.get(&callee_key) else {
                continue;
            };
            for binding in constructor_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name
                        || *boundary != call_site.boundary
                        || *arg_index >= callee_constraints.params.len()
                    {
                        continue;
                    }
                    for constraint in &callee_constraints.params[*arg_index] {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((
                                constructor_key.clone(),
                                binding.name.clone(),
                                constraint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding_name, constraint) in pending {
        changed |= add_constructor_binding_constraint(
            constraints,
            &constructor_key,
            &binding_name,
            constraint,
        );
    }
    changed
}

fn propagate_constructor_delegation_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, constructor_usage) in &usage.constructors {
        let constructor_key = ConstructorKey {
            type_name: usage_key.type_name.clone(),
            rust_name: usage_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&constructor_key) else {
            continue;
        };
        if constructor_constraints.public_api {
            continue;
        }
        for call_site in &constructor_usage.constructor_call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = ConstructorKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            let Some(callee_constraints) = constraints.constructors.get(&callee_key) else {
                continue;
            };
            for binding in constructor_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name
                        || *boundary != call_site.boundary
                        || *arg_index >= callee_constraints.params.len()
                    {
                        continue;
                    }
                    for constraint in &callee_constraints.params[*arg_index] {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((
                                constructor_key.clone(),
                                binding.name.clone(),
                                constraint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding_name, constraint) in pending {
        changed |= add_constructor_binding_constraint(
            constraints,
            &constructor_key,
            &binding_name,
            constraint,
        );
    }
    changed
}

fn propagate_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(caller_constraints) = constraints.methods.get(&caller_key) else {
            continue;
        };
        if caller_constraints.public_api {
            continue;
        }
        for call_site in &method_usage.call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            if !constraints.methods.contains_key(&callee_key) {
                continue;
            }
            for binding in method_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name || *boundary != call_site.boundary {
                        continue;
                    }
                    for constraint in binding_slot_requirements(caller_constraints, &binding.name) {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((callee_key.clone(), *arg_index, constraint.clone()));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (callee_key, arg_index, constraint) in pending {
        let param_name = constraints
            .methods
            .get(&callee_key)
            .and_then(|method_constraints| method_constraints.param_names.get(arg_index))
            .cloned();
        if let Some(param_name) = param_name {
            changed |=
                add_method_param_constraint(constraints, &callee_key, &param_name, constraint);
        }
    }
    changed
}

fn propagate_constructor_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, constructor_usage) in &usage.constructors {
        let constructor_key = ConstructorKey {
            type_name: usage_key.type_name.clone(),
            rust_name: usage_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&constructor_key) else {
            continue;
        };
        if constructor_constraints.public_api {
            continue;
        }
        for call_site in &constructor_usage.call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            if !constraints.methods.contains_key(&callee_key) {
                continue;
            }
            for binding in constructor_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name || *boundary != call_site.boundary {
                        continue;
                    }
                    for constraint in constructor_binding_slot_requirements(
                        constructor_constraints,
                        &binding.name,
                    ) {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((callee_key.clone(), *arg_index, constraint.clone()));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (callee_key, arg_index, constraint) in pending {
        let param_name = constraints
            .methods
            .get(&callee_key)
            .and_then(|method_constraints| method_constraints.param_names.get(arg_index))
            .cloned();
        if let Some(param_name) = param_name {
            changed |=
                add_method_param_constraint(constraints, &callee_key, &param_name, constraint);
        }
    }
    changed
}

fn propagate_constructor_delegation_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, constructor_usage) in &usage.constructors {
        let constructor_key = ConstructorKey {
            type_name: usage_key.type_name.clone(),
            rust_name: usage_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&constructor_key) else {
            continue;
        };
        if constructor_constraints.public_api {
            continue;
        }
        for call_site in &constructor_usage.constructor_call_sites {
            if call_site.boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = ConstructorKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            if !constraints.constructors.contains_key(&callee_key) {
                continue;
            }
            for binding in constructor_usage.bindings.values() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name || *boundary != call_site.boundary {
                        continue;
                    }
                    for constraint in constructor_binding_slot_requirements(
                        constructor_constraints,
                        &binding.name,
                    ) {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((callee_key.clone(), *arg_index, constraint.clone()));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (callee_key, arg_index, constraint) in pending {
        let param_name = constraints
            .constructors
            .get(&callee_key)
            .and_then(|constructor_constraints| constructor_constraints.param_names.get(arg_index))
            .cloned();
        if let Some(param_name) = param_name {
            changed |=
                add_constructor_param_constraint(constraints, &callee_key, &param_name, constraint);
        }
    }
    changed
}

fn propagate_call_result_constraints_to_bindings(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedFromCall { name, boundary } = use_site else {
                    continue;
                };
                if *boundary != CallArgumentBoundary::InternalSameType {
                    continue;
                }
                for constraint in
                    internal_call_return_constraints(constraints, &method_usage.call_sites, name)
                {
                    pending.push((method_key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding_name, constraint) in pending {
        changed |=
            add_method_binding_constraint(constraints, &method_key, &binding_name, constraint);
    }
    changed
}

fn propagate_constructor_call_result_constraints_to_bindings(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        for binding in constructor_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedFromCall { name, boundary } = use_site else {
                    continue;
                };
                if *boundary != CallArgumentBoundary::InternalSameType {
                    continue;
                }
                for constraint in internal_call_return_constraints(
                    constraints,
                    &constructor_usage.call_sites,
                    name,
                ) {
                    pending.push((key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding_name, constraint) in pending {
        changed |= add_constructor_binding_constraint(
            constraints,
            &constructor_key,
            &binding_name,
            constraint,
        );
    }
    changed
}

fn propagate_call_result_constraints_to_fields(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for field_use in &method_usage.field_uses {
            let FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary,
            } = field_use
            else {
                continue;
            };
            if *boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            for constraint in
                internal_call_return_constraints(constraints, &method_usage.call_sites, name)
            {
                pending.push((
                    method_key.type_name.clone(),
                    field_name.clone(),
                    constraint.clone(),
                ));
            }
        }
    }
    let mut changed = false;
    for (type_name, field_name, constraint) in pending {
        changed |= add_field_constraint(constraints, &type_name, &field_name, constraint);
    }
    changed
}

fn propagate_constructor_call_result_constraints_to_fields(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        for field_use in &constructor_usage.field_uses {
            let FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary,
            } = field_use
            else {
                continue;
            };
            if *boundary != CallArgumentBoundary::InternalSameType {
                continue;
            }
            for constraint in
                internal_call_return_constraints(constraints, &constructor_usage.call_sites, name)
            {
                pending.push((
                    constructor_key.type_name.clone(),
                    field_name.clone(),
                    constraint.clone(),
                ));
            }
        }
    }
    let mut changed = false;
    for (type_name, field_name, constraint) in pending {
        changed |= add_field_constraint(constraints, &type_name, &field_name, constraint);
    }
    changed
}

fn internal_call_return_constraints<'a>(
    constraints: &'a SignatureConstraintState,
    call_sites: &'a [crate::usage::CallSiteUse],
    name: &str,
) -> Vec<&'a SignatureConstraint> {
    call_sites
        .iter()
        .filter(|call_site| {
            call_site.name == name && call_site.boundary == CallArgumentBoundary::InternalSameType
        })
        .filter_map(|call_site| call_site.resolved.as_ref())
        .filter_map(|resolved| {
            constraints.methods.get(&MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            })
        })
        .flat_map(|callee_constraints| callee_constraints.return_constraints.iter())
        .filter(|constraint| signature_shape_constraint_propagation_policy().allows(constraint))
        .collect()
}

enum PendingSignatureConstraint {
    Binding(SignatureConstraint),
    Return(SignatureConstraint),
}

fn propagate_method_local_alias_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal { local_name } = use_site else {
                    continue;
                };
                for constraint in local_slot_requirements(&method_constraints.locals, local_name) {
                    pending.push((method_key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding_name, constraint) in pending {
        changed |=
            add_method_binding_constraint(constraints, &method_key, &binding_name, constraint);
    }
    changed
}

fn propagate_constructor_local_alias_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&key) else {
            continue;
        };
        for binding in constructor_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal { local_name } = use_site else {
                    continue;
                };
                for constraint in
                    local_slot_requirements(&constructor_constraints.locals, local_name)
                {
                    pending.push((key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding_name, constraint) in pending {
        changed |= add_constructor_binding_constraint(
            constraints,
            &constructor_key,
            &binding_name,
            constraint,
        );
    }
    changed
}

fn propagate_method_local_alias_source_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal { local_name } = use_site else {
                    continue;
                };
                for constraint in binding_slot_requirements(method_constraints, &binding.name) {
                    pending.push((method_key.clone(), local_name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, local_name, constraint) in pending {
        changed |= add_method_local_constraint(constraints, &method_key, &local_name, constraint);
    }
    changed
}

fn propagate_constructor_local_alias_source_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&key) else {
            continue;
        };
        for binding in constructor_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal { local_name } = use_site else {
                    continue;
                };
                for constraint in
                    constructor_binding_slot_requirements(constructor_constraints, &binding.name)
                {
                    pending.push((key.clone(), local_name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, local_name, constraint) in pending {
        changed |= add_constructor_local_constraint(
            constraints,
            &constructor_key,
            &local_name,
            constraint,
        );
    }
    changed
}

fn propagate_method_field_storage_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::StoredInField { field_name } = use_site else {
                    continue;
                };
                for constraint in
                    field_shape_requirements(constraints, &method_key.type_name, field_name)
                {
                    pending.push((method_key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding_name, constraint) in pending {
        changed |=
            add_method_binding_constraint(constraints, &method_key, &binding_name, constraint);
    }
    changed
}

fn propagate_constructor_field_storage_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        for binding in constructor_usage.bindings.values() {
            for use_site in &binding.uses {
                let BindingUse::StoredInField { field_name } = use_site else {
                    continue;
                };
                for constraint in
                    field_shape_requirements(constraints, &constructor_key.type_name, field_name)
                {
                    pending.push((key.clone(), binding.name.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding_name, constraint) in pending {
        changed |= add_constructor_binding_constraint(
            constraints,
            &constructor_key,
            &binding_name,
            constraint,
        );
    }
    changed
}

fn propagate_method_return_binding_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            if !binding
                .uses
                .iter()
                .any(|use_site| matches!(use_site, BindingUse::Returned))
            {
                continue;
            }
            for constraint in method_constraints
                .return_constraints
                .iter()
                .filter(|constraint| {
                    signature_shape_constraint_propagation_policy().allows(constraint)
                })
            {
                pending.push((method_key.clone(), binding.name.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, binding_name, constraint) in pending {
        changed |=
            add_method_binding_constraint(constraints, &method_key, &binding_name, constraint);
    }
    changed
}

fn propagate_method_binding_return_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.bindings.values() {
            if !binding
                .uses
                .iter()
                .any(|use_site| matches!(use_site, BindingUse::Returned))
            {
                continue;
            }
            for constraint in binding_slot_requirements(method_constraints, &binding.name) {
                pending.push((method_key.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, constraint) in pending {
        changed |= add_method_return_constraint(constraints, &method_key, constraint);
    }
    changed
}

fn propagate_method_return_field_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for field_use in &method_usage.field_uses {
            let FieldUse::Returned { field_name } = field_use else {
                continue;
            };
            for constraint in method_constraints
                .return_constraints
                .iter()
                .filter(|constraint| {
                    signature_shape_constraint_propagation_policy().allows(constraint)
                })
            {
                pending.push((
                    method_key.type_name.clone(),
                    field_name.clone(),
                    constraint.clone(),
                ));
            }
        }
    }
    let mut changed = false;
    for (type_name, field_name, constraint) in pending {
        changed |= add_field_constraint(constraints, &type_name, &field_name, constraint);
    }
    changed
}

fn propagate_method_field_return_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for field_use in &method_usage.field_uses {
            let FieldUse::Returned { field_name } = field_use else {
                continue;
            };
            for constraint in
                field_shape_requirements(constraints, &method_key.type_name, field_name).filter(
                    |constraint| signature_shape_constraint_propagation_policy().allows(constraint),
                )
            {
                pending.push((method_key.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, constraint) in pending {
        changed |= add_method_return_constraint(constraints, &method_key, constraint);
    }
    changed
}

fn field_shape_requirements<'a>(
    constraints: &'a SignatureConstraintState,
    type_name: &str,
    field_name: &str,
) -> impl Iterator<Item = &'a SignatureConstraint> {
    constraints
        .fields
        .get(&FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        })
        .into_iter()
        .flat_map(|constraints| constraints.iter())
        .filter(|constraint| signature_slot_constraint_propagation_policy().allows(constraint))
}

fn local_slot_requirements<'a>(
    locals: &'a [LocalConstraintSet],
    local_name: &str,
) -> impl Iterator<Item = &'a SignatureConstraint> {
    locals
        .iter()
        .filter(move |local| local.name == local_name)
        .flat_map(|local| local.constraints.iter())
        .filter(|constraint| signature_slot_constraint_propagation_policy().allows(constraint))
}

fn binding_slot_requirements<'a>(
    method_constraints: &'a MethodConstraintSet,
    binding_name: &str,
) -> Vec<&'a SignatureConstraint> {
    let mut requirements = Vec::new();
    if let Some(param_index) = method_constraints
        .param_names
        .iter()
        .position(|name| name == binding_name)
    {
        requirements.extend(
            method_constraints.params[param_index]
                .iter()
                .filter(|constraint| {
                    signature_slot_constraint_propagation_policy().allows(constraint)
                }),
        );
    }
    requirements.extend(local_slot_requirements(
        &method_constraints.locals,
        binding_name,
    ));
    requirements
}

fn constructor_binding_slot_requirements<'a>(
    constructor_constraints: &'a ConstructorConstraintSet,
    binding_name: &str,
) -> Vec<&'a SignatureConstraint> {
    let mut requirements = Vec::new();
    if let Some(param_index) = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == binding_name)
    {
        requirements.extend(constructor_constraints.params[param_index].iter().filter(
            |constraint| signature_slot_constraint_propagation_policy().allows(constraint),
        ));
    }
    requirements.extend(local_slot_requirements(
        &constructor_constraints.locals,
        binding_name,
    ));
    requirements
}

fn method_key_for_usage(
    constraints: &SignatureConstraintState,
    usage_key: &UsageMethodKey,
) -> Option<MethodKey> {
    let exact = MethodKey {
        type_name: usage_key.type_name.clone(),
        rust_name: usage_key.method_name.clone(),
    };
    if constraints.methods.contains_key(&exact) {
        return Some(exact);
    }
    constraints
        .methods
        .keys()
        .find(|method_key| {
            method_key.type_name == usage_key.type_name
                && method_key.rust_name.starts_with(&usage_key.method_name)
        })
        .cloned()
}

fn add_field_constraint(
    constraints: &mut SignatureConstraintState,
    type_name: &str,
    field_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    let Some(field_constraints) = constraints.fields.get_mut(&FieldKey {
        type_name: type_name.to_string(),
        field_name: field_name.to_string(),
    }) else {
        return false;
    };
    if field_constraints.contains(&constraint) {
        false
    } else {
        field_constraints.push(constraint);
        true
    }
}

fn add_method_param_constraint(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    param_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    let Some(method_constraints) = constraints.methods.get_mut(method_key) else {
        return false;
    };
    let Some(param_index) = method_constraints
        .param_names
        .iter()
        .position(|name| name == param_name)
    else {
        return false;
    };
    let param_constraints = &mut method_constraints.params[param_index];
    if param_constraints.contains(&constraint) {
        false
    } else {
        param_constraints.push(constraint);
        true
    }
}

fn add_method_return_constraint(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    constraint: SignatureConstraint,
) -> bool {
    let Some(method_constraints) = constraints.methods.get_mut(method_key) else {
        return false;
    };
    if method_constraints.return_constraints.contains(&constraint) {
        false
    } else {
        method_constraints.return_constraints.push(constraint);
        true
    }
}

fn add_method_binding_constraint(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    binding_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    add_method_param_constraint(constraints, method_key, binding_name, constraint.clone())
        | add_method_local_constraint(constraints, method_key, binding_name, constraint)
}

fn add_method_local_constraint(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    local_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    let Some(method_constraints) = constraints.methods.get_mut(method_key) else {
        return false;
    };
    let Some(local_constraints) = method_constraints
        .locals
        .iter_mut()
        .find(|local| local.name == local_name)
    else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn add_constructor_binding_constraint(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    binding_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    add_constructor_param_constraint(
        constraints,
        constructor_key,
        binding_name,
        constraint.clone(),
    ) | add_constructor_local_constraint(constraints, constructor_key, binding_name, constraint)
}

fn add_constructor_param_constraint(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    param_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    let Some(constructor_constraints) = constraints.constructors.get_mut(constructor_key) else {
        return false;
    };
    let Some(param_index) = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == param_name)
    else {
        return false;
    };
    let param_constraints = &mut constructor_constraints.params[param_index];
    if param_constraints.contains(&constraint) {
        false
    } else {
        param_constraints.push(constraint);
        true
    }
}

fn add_constructor_local_constraint(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    local_name: &str,
    constraint: SignatureConstraint,
) -> bool {
    let Some(constructor_constraints) = constraints.constructors.get_mut(constructor_key) else {
        return false;
    };
    let Some(local_constraints) = constructor_constraints
        .locals
        .iter_mut()
        .find(|local| local.name == local_name)
    else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn signature_diagnostics_for_constraints(
    program: &Program,
    constraints: &SignatureConstraintState,
    facts: &FactSet,
) -> Vec<SignatureDiagnostic> {
    let mut diagnostics = Vec::new();
    for ty in &program.types {
        collect_type_signature_diagnostics(ty, constraints, facts, &mut diagnostics);
    }
    diagnostics
}

fn collect_type_signature_diagnostics(
    ty: &TypeDecl,
    constraints: &SignatureConstraintState,
    facts: &FactSet,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) {
    for member in &ty.members {
        match member {
            crate::java_ir::Member::Field(field) => {
                let field_key = FieldKey {
                    type_name: ty.name.clone(),
                    field_name: field.name.clone(),
                };
                if let Some(field_constraints) = constraints.fields.get(&field_key) {
                    push_field_signature_diagnostics(
                        field,
                        &field_key,
                        field_constraints,
                        facts,
                        diagnostics,
                    );
                }
            }
            crate::java_ir::Member::Method(method) => {
                let method_key = MethodKey {
                    type_name: ty.name.clone(),
                    rust_name: method_rust_name(method),
                };
                if let Some(method_constraints) = constraints.methods.get(&method_key) {
                    push_method_signature_diagnostics(
                        method,
                        &method_key,
                        method_constraints,
                        facts,
                        diagnostics,
                    );
                }
            }
            crate::java_ir::Member::Constructor(ctor) => {
                let constructor_key = ConstructorKey {
                    type_name: ty.name.clone(),
                    rust_name: constructor_rust_name(ctor),
                };
                if let Some(constructor_constraints) =
                    constraints.constructors.get(&constructor_key)
                {
                    push_constructor_signature_diagnostics(
                        ctor,
                        &constructor_key,
                        constructor_constraints,
                        facts,
                        diagnostics,
                    );
                }
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signature_diagnostics(nested, constraints, facts, diagnostics);
            }
            _ => {}
        }
    }
}

fn push_field_signature_diagnostics(
    field: &FieldDecl,
    field_key: &FieldKey,
    field_constraints: &[SignatureConstraint],
    facts: &FactSet,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) {
    let slot = format!("{} field {}", field_key.type_name, field_key.field_name);
    push_constraint_conflict_diagnostics(diagnostics, slot.clone(), field_constraints);
    push_capability_diagnostics_for_signature_constraints(
        diagnostics,
        slot,
        &field.ty,
        field_constraints,
        facts,
    );
}

fn push_method_signature_diagnostics(
    method: &MethodDecl,
    method_key: &MethodKey,
    method_constraints: &MethodConstraintSet,
    facts: &FactSet,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) {
    for (index, param_constraints) in method_constraints.params.iter().enumerate() {
        let param_name = method_constraints
            .param_names
            .get(index)
            .map(String::as_str)
            .unwrap_or("<unknown>");
        push_constraint_conflict_diagnostics(
            diagnostics,
            format!(
                "{}::{} parameter {}",
                method_key.type_name, method_key.rust_name, param_name
            ),
            param_constraints,
        );
        if let Some(param) = method.params.get(index) {
            push_capability_diagnostics_for_signature_constraints(
                diagnostics,
                format!(
                    "{}::{} parameter {}",
                    method_key.type_name, method_key.rust_name, param.name
                ),
                &param.ty,
                param_constraints,
                facts,
            );
        }
    }
    push_constraint_conflict_diagnostics(
        diagnostics,
        format!("{}::{} return", method_key.type_name, method_key.rust_name),
        &method_constraints.return_constraints,
    );
    push_capability_diagnostics_for_signature_constraints(
        diagnostics,
        format!("{}::{} return", method_key.type_name, method_key.rust_name),
        &method.return_type,
        &method_constraints.return_constraints,
        facts,
    );
    for local in &method_constraints.locals {
        push_constraint_conflict_diagnostics(
            diagnostics,
            format!(
                "{}::{} local {}",
                method_key.type_name, method_key.rust_name, local.name
            ),
            &local.constraints,
        );
        push_capability_diagnostics_for_signature_constraints(
            diagnostics,
            format!(
                "{}::{} local {}",
                method_key.type_name, method_key.rust_name, local.name
            ),
            &local.java_ty,
            &local.constraints,
            facts,
        );
    }
}

fn push_constructor_signature_diagnostics(
    ctor: &ConstructorDecl,
    constructor_key: &ConstructorKey,
    constructor_constraints: &ConstructorConstraintSet,
    facts: &FactSet,
    diagnostics: &mut Vec<SignatureDiagnostic>,
) {
    for (index, param_constraints) in constructor_constraints.params.iter().enumerate() {
        let param_name = constructor_constraints
            .param_names
            .get(index)
            .map(String::as_str)
            .unwrap_or("<unknown>");
        push_constraint_conflict_diagnostics(
            diagnostics,
            format!(
                "{}::{} parameter {}",
                constructor_key.type_name, constructor_key.rust_name, param_name
            ),
            param_constraints,
        );
        if let Some(param) = ctor.params.get(index) {
            push_capability_diagnostics_for_signature_constraints(
                diagnostics,
                format!(
                    "{}::{} parameter {}",
                    constructor_key.type_name, constructor_key.rust_name, param.name
                ),
                &param.ty,
                param_constraints,
                facts,
            );
        }
    }
    for local in &constructor_constraints.locals {
        push_constraint_conflict_diagnostics(
            diagnostics,
            format!(
                "{}::{} local {}",
                constructor_key.type_name, constructor_key.rust_name, local.name
            ),
            &local.constraints,
        );
        push_capability_diagnostics_for_signature_constraints(
            diagnostics,
            format!(
                "{}::{} local {}",
                constructor_key.type_name, constructor_key.rust_name, local.name
            ),
            &local.java_ty,
            &local.constraints,
            facts,
        );
    }
}

fn push_constraint_conflict_diagnostics(
    diagnostics: &mut Vec<SignatureDiagnostic>,
    slot: String,
    constraints: &[SignatureConstraint],
) {
    let merged = MergedSignatureConstraints::from_constraints(constraints);
    for conflict in merged.conflicts() {
        diagnostics.push(SignatureDiagnostic {
            code: "signature_constraint_conflict",
            message: format!(
                "{slot} has conflicting {:?} constraints; kept {:?}, rejected {:?}",
                conflict.axis, conflict.kept, conflict.rejected
            ),
        });
    }
}

pub fn capability_diagnostics_for_signature_constraints(
    slot: String,
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> Vec<SignatureDiagnostic> {
    let mut diagnostics = Vec::new();
    push_capability_diagnostics_for_signature_constraints(
        &mut diagnostics,
        slot,
        java_ty,
        constraints,
        facts,
    );
    diagnostics
}

fn push_capability_diagnostics_for_signature_constraints(
    diagnostics: &mut Vec<SignatureDiagnostic>,
    slot: String,
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) {
    match signature_owned_recovery_policy(java_ty, constraints, facts) {
        SignatureOwnedRecoveryPolicy::NotRequired
        | SignatureOwnedRecoveryPolicy::ProvenByCopy
        | SignatureOwnedRecoveryPolicy::ProvenByExplicitClone => {}
        SignatureOwnedRecoveryPolicy::AuditConservativeClone => {
            diagnostics.push(SignatureDiagnostic {
                code: "signature_clone_recovery_requires_audit",
                message: format!(
                    "{slot} needs owned recovery from a borrowed view; {java_ty:?} is Clone only by conservative/default capability, so the inserted clone requires audit"
                ),
            });
        }
        SignatureOwnedRecoveryPolicy::UnsupportedNonCloneable => {
            diagnostics.push(SignatureDiagnostic {
                code: "signature_owned_recovery_requires_clone",
                message: format!(
                    "{slot} needs owned recovery from a borrowed view, but {java_ty:?} is neither Copy nor Clone by type capability facts"
                ),
            });
        }
    }
}

pub fn signature_owned_recovery_policy(
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> SignatureOwnedRecoveryPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_owned_recovery_policy_with_effective_facts(java_ty, constraints, &effective_facts)
}

pub fn signature_owned_recovery_policy_with_effective_facts(
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
    facts: &EffectiveFacts<'_>,
) -> SignatureOwnedRecoveryPolicy {
    let merged = MergedSignatureConstraints::from_constraints(constraints);
    if !merged.conflicts().iter().any(|conflict| {
        conflict.axis == SignatureConstraintAxis::Owner
            && signature_constraint_pair_contains_ownership(
                conflict,
                OwnershipShape::Owned,
                OwnershipShape::SharedBorrow,
            )
    }) {
        return SignatureOwnedRecoveryPolicy::NotRequired;
    }
    let capability =
        signature_owned_recovery_capability_policy_with_effective_facts(java_ty, facts);
    signature_owned_recovery_policy_from_capability(capability.recovery)
}

pub fn signature_owned_recovery_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureOwnedRecoveryCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_owned_recovery_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn signature_owned_recovery_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureOwnedRecoveryCapabilityPolicy {
    SignatureOwnedRecoveryCapabilityPolicy {
        java_ty: java_ty.clone(),
        recovery: facts.owned_recovery_capability_for(java_ty),
    }
}

pub fn signature_owned_recovery_policy_from_capability(
    capability: OwnedRecoveryCapability,
) -> SignatureOwnedRecoveryPolicy {
    match capability {
        OwnedRecoveryCapability::Copy => SignatureOwnedRecoveryPolicy::ProvenByCopy,
        OwnedRecoveryCapability::ExplicitClone { .. } => {
            SignatureOwnedRecoveryPolicy::ProvenByExplicitClone
        }
        OwnedRecoveryCapability::ConservativeClone => {
            SignatureOwnedRecoveryPolicy::AuditConservativeClone
        }
        OwnedRecoveryCapability::NonCloneable => {
            SignatureOwnedRecoveryPolicy::UnsupportedNonCloneable
        }
    }
}

fn signature_constraint_pair_contains_ownership(
    conflict: &SignatureConstraintConflict,
    first: OwnershipShape,
    second: OwnershipShape,
) -> bool {
    let first = SignatureConstraint::RequiredOwnership(first);
    let second = SignatureConstraint::RequiredOwnership(second);
    (conflict.kept == first && conflict.rejected == second)
        || (conflict.kept == second && conflict.rejected == first)
}

fn build_signature_plan_from_constraints(
    program: &Program,
    representations: &RepresentationPlan,
    constraints: &SignatureConstraintState,
    solve_iterations: usize,
    fixpoint_diagnostics: Vec<SignatureDiagnostic>,
    facts: &FactSet,
) -> SignaturePlan {
    let mut plan = SignaturePlan {
        solve_iterations,
        diagnostics: {
            let mut diagnostics = fixpoint_diagnostics;
            diagnostics.extend(signature_diagnostics_for_constraints(
                program,
                constraints,
                facts,
            ));
            diagnostics
        },
        ..SignaturePlan::default()
    };
    for ty in &program.types {
        collect_type_signatures(ty, representations, constraints, facts, &mut plan);
    }
    plan
}

impl SignaturePlan {
    pub fn solve_iterations(&self) -> usize {
        self.solve_iterations
    }

    pub fn diagnostics(&self) -> &[SignatureDiagnostic] {
        &self.diagnostics
    }

    pub fn method_signature(
        &self,
        ty: &TypeDecl,
        method: &MethodDecl,
    ) -> Option<&MethodSignaturePlan> {
        self.methods.get(&MethodKey {
            type_name: ty.name.clone(),
            rust_name: method_rust_name(method),
        })
    }

    pub fn required_method_signature(
        &self,
        ty: &TypeDecl,
        method: &MethodDecl,
    ) -> &MethodSignaturePlan {
        self.method_signature(ty, method)
            .expect("signature solver must plan every lowered method")
    }

    pub fn method_signature_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&MethodSignaturePlan> {
        self.methods.get(&MethodKey {
            type_name: type_name.to_string(),
            rust_name: rust_name.to_string(),
        })
    }

    pub fn constructor_signature(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> Option<&ConstructorSignaturePlan> {
        self.constructors.get(&ConstructorKey {
            type_name: ty.name.clone(),
            rust_name: constructor_rust_name(ctor),
        })
    }

    pub fn required_constructor_signature(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> &ConstructorSignaturePlan {
        self.constructor_signature(ty, ctor)
            .expect("signature solver must plan every lowered constructor")
    }

    pub fn constructor_signature_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&ConstructorSignaturePlan> {
        self.constructors.get(&ConstructorKey {
            type_name: type_name.to_string(),
            rust_name: rust_name.to_string(),
        })
    }

    pub fn method_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> Option<RustSlotShape> {
        self.method_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.params.get(index))
            .map(|param| param.shape.clone())
    }

    pub fn required_method_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> RustSlotShape {
        self.method_arg_shape_by_rust_name(type_name, rust_name, index)
            .expect("signature solver must plan every resolved method argument")
    }

    pub fn constructor_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> Option<RustSlotShape> {
        self.constructor_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.params.get(index))
            .map(|param| param.shape.clone())
    }

    pub fn required_constructor_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> RustSlotShape {
        self.constructor_arg_shape_by_rust_name(type_name, rust_name, index)
            .expect("signature solver must plan every resolved constructor argument")
    }

    pub fn method_return_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<RustSlotShape> {
        self.method_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn field_shape(&self, type_name: &str, field_name: &str) -> Option<&RustSlotShape> {
        self.fields.get(&FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        })
    }

    pub fn required_field_shape(&self, type_name: &str, field_name: &str) -> RustSlotShape {
        self.field_shape(type_name, field_name)
            .cloned()
            .expect("signature solver must plan every lowered field")
    }

    pub fn constructor_param_plans(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> Option<&[ParamPlan]> {
        self.constructor_signature(ty, ctor)
            .map(|signature| signature.params.as_slice())
    }

    pub fn method_param_plans(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<&[ParamPlan]> {
        self.method_signature(ty, method)
            .map(|signature| signature.params.as_slice())
    }

    pub fn method_return_shape(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<RustSlotShape> {
        self.method_signature(ty, method)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn method_receiver(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<ReceiverPlan> {
        self.method_signature(ty, method)
            .map(|signature| signature.receiver)
    }

    pub fn method_local_shapes_or_empty(&self, ty: &TypeDecl, method: &MethodDecl) -> &[LocalPlan] {
        self.method_signature(ty, method)
            .map(|signature| signature.locals.as_slice())
            .unwrap_or(&[])
    }

    pub fn constructor_local_shapes_or_empty(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> &[LocalPlan] {
        self.constructor_signature(ty, ctor)
            .map(|signature| signature.locals.as_slice())
            .unwrap_or(&[])
    }

    pub fn local_shape<'a>(
        &self,
        locals: &'a [LocalPlan],
        local_name: &str,
    ) -> Option<&'a RustSlotShape> {
        locals
            .iter()
            .find(|local| local.name == local_name)
            .map(|local| &local.shape)
    }

    pub fn required_local_plan<'a>(
        &self,
        locals: &'a [LocalPlan],
        local_name: &str,
    ) -> &'a LocalPlan {
        locals
            .iter()
            .find(|local| local.name == local_name)
            .expect("signature solver must plan every lowered local")
    }

    pub fn required_local_shape(&self, locals: &[LocalPlan], local_name: &str) -> RustSlotShape {
        self.local_shape(locals, local_name)
            .cloned()
            .expect("signature solver must plan every lowered local")
    }

    pub fn local_binding_mutability(&self, locals: &[LocalPlan], local_name: &str) -> Option<bool> {
        locals
            .iter()
            .find(|local| local.name == local_name)
            .map(|local| local.binding_mutable)
    }
}

fn collect_type_signatures(
    ty: &TypeDecl,
    representations: &RepresentationPlan,
    constraints: &SignatureConstraintState,
    facts: &FactSet,
    plan: &mut SignaturePlan,
) {
    for member in &ty.members {
        match member {
            crate::java_ir::Member::Field(field) => {
                let field_key = FieldKey {
                    type_name: ty.name.clone(),
                    field_name: field.name.clone(),
                };
                let field_constraints = constraints
                    .fields
                    .get(&field_key)
                    .cloned()
                    .unwrap_or_else(field_shape_constraints);
                plan.fields.insert(
                    field_key,
                    constrained_field_shape(&field.ty, representations, &field_constraints, facts),
                );
            }
            crate::java_ir::Member::Method(method) => {
                let method_key = MethodKey {
                    type_name: ty.name.clone(),
                    rust_name: method_rust_name(method),
                };
                let method_constraints = constraints.methods.get(&method_key);
                plan.methods.insert(
                    method_key,
                    MethodSignaturePlan {
                        params: method
                            .params
                            .iter()
                            .enumerate()
                            .map(|(index, param)| {
                                let constraints = method_constraints
                                    .and_then(|constraints| constraints.params.get(index))
                                    .cloned()
                                    .unwrap_or_default();
                                constrained_param_plan(param, representations, &constraints, facts)
                            })
                            .collect(),
                        return_shape: {
                            let constraints = method_constraints
                                .map(|constraints| constraints.return_constraints.clone())
                                .unwrap_or_default();
                            constrained_return_shape(method, representations, &constraints, facts)
                        },
                        locals: method
                            .body
                            .as_ref()
                            .map(|body| {
                                collect_local_plans_from_constraints(
                                    body,
                                    representations,
                                    method_constraints
                                        .map(|constraints| constraints.locals.as_slice())
                                        .unwrap_or(&[]),
                                    facts,
                                )
                            })
                            .unwrap_or_default(),
                        receiver: constrained_receiver_plan(
                            method_constraints
                                .map(|constraints| constraints.receiver.as_slice())
                                .unwrap_or(&[]),
                        ),
                    },
                );
            }
            crate::java_ir::Member::Constructor(ctor) => {
                let constructor_key = ConstructorKey {
                    type_name: ty.name.clone(),
                    rust_name: constructor_rust_name(ctor),
                };
                let constructor_constraints = constraints.constructors.get(&constructor_key);
                plan.constructors.insert(
                    constructor_key,
                    ConstructorSignaturePlan {
                        params: ctor
                            .params
                            .iter()
                            .enumerate()
                            .map(|(index, param)| {
                                let constraints = constructor_constraints
                                    .and_then(|constraints| constraints.params.get(index))
                                    .cloned()
                                    .unwrap_or_default();
                                constrained_param_plan(param, representations, &constraints, facts)
                            })
                            .collect(),
                        locals: ctor
                            .body
                            .as_ref()
                            .map(|body| {
                                collect_local_plans_from_constraints(
                                    body,
                                    representations,
                                    constructor_constraints
                                        .map(|constraints| constraints.locals.as_slice())
                                        .unwrap_or(&[]),
                                    facts,
                                )
                            })
                            .unwrap_or_default(),
                    },
                );
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signatures(nested, representations, constraints, facts, plan);
            }
            crate::java_ir::Member::Unsupported { .. } => {}
        }
    }
}

pub fn method_param_shape_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let boundary_kind = method_param_boundary_policy(ty, method, param, usage).kind;
    let mut constraints =
        signature_slot_baseline_policy(&param.ty, boundary_kind, facts).constraints;
    if let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) && let Some(binding) = method_usage.bindings.get(&param.name)
    {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts),
        );
    }
    if boundary_kind.is_none() {
        constraints.extend(
            method_param_borrowed_content_policy_with_facts(ty, method, param, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
    }
    constraints
}

pub fn conservative_boundary_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    conservative_boundary_constraints_with_effective_facts(java_ty, &effective_facts)
}

pub fn conservative_boundary_constraints_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    signature_boundary_policy_with_effective_facts(
        SignatureBoundaryKind::UnknownApi,
        java_ty,
        facts,
    )
    .constraints
}

pub fn signature_boundary_policy(
    kind: SignatureBoundaryKind,
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureBoundaryPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_boundary_policy_with_effective_facts(kind, java_ty, &effective_facts)
}

pub fn signature_boundary_policy_with_effective_facts(
    kind: SignatureBoundaryKind,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureBoundaryPolicy {
    let fallback = signature_boundary_fallback_with_effective_facts(kind, java_ty, facts);
    SignatureBoundaryPolicy {
        kind,
        java_ty: java_ty.clone(),
        constraints: signature_boundary_constraints_with_effective_facts(fallback, java_ty, facts),
        fallback,
    }
}

fn signature_boundary_fallback_with_effective_facts(
    kind: SignatureBoundaryKind,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureBoundaryFallback {
    let representation_policy =
        signature_boundary_representation_policy_with_effective_facts(java_ty, facts);
    match kind {
        SignatureBoundaryKind::PublicApi | SignatureBoundaryKind::UnknownApi => {
            if representation_policy.preserve_object_representation {
                SignatureBoundaryFallback::PreserveObjectRepresentation
            } else {
                SignatureBoundaryFallback::ConservativeNative
            }
        }
        SignatureBoundaryKind::EscapingStorage => {
            if representation_policy.preserve_object_representation {
                SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage
            } else {
                SignatureBoundaryFallback::EscapingNativeStorage
            }
        }
    }
}

pub fn signature_boundary_representation_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureBoundaryRepresentationPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_boundary_representation_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn signature_boundary_representation_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureBoundaryRepresentationPolicy {
    let fact_policy =
        signature_boundary_representation_fact_policy_with_effective_facts(java_ty, facts);
    SignatureBoundaryRepresentationPolicy {
        java_ty: java_ty.clone(),
        preserve_object_representation: matches!(
            java_ty,
            JavaType::Unknown(_) | JavaType::Wildcard(_)
        ) || fact_policy.object_representation_facts,
    }
}

pub fn signature_boundary_representation_fact_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureBoundaryRepresentationFactPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_boundary_representation_fact_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn signature_boundary_representation_fact_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureBoundaryRepresentationFactPolicy {
    SignatureBoundaryRepresentationFactPolicy {
        java_ty: java_ty.clone(),
        object_representation_facts: facts.has_object_representation_facts(java_ty),
    }
}

pub fn signature_boundary_constraints(
    fallback: SignatureBoundaryFallback,
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    signature_boundary_constraints_with_effective_facts(fallback, java_ty, &effective_facts)
}

pub fn signature_boundary_constraints_with_effective_facts(
    fallback: SignatureBoundaryFallback,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    match fallback {
        SignatureBoundaryFallback::ConservativeNative => {
            signature_native_boundary_policy_with_effective_facts(java_ty, facts).constraints
        }
        SignatureBoundaryFallback::PreserveObjectRepresentation => Vec::new(),
        SignatureBoundaryFallback::PreserveObjectRepresentationEscapingStorage => {
            vec![SignatureConstraint::StorageSlot]
        }
        SignatureBoundaryFallback::EscapingNativeStorage => {
            signature_native_boundary_policy_with_effective_facts(java_ty, facts).constraints
        }
    }
}

pub fn signature_native_boundary_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureNativeBoundaryPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_native_boundary_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn signature_native_boundary_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureNativeBoundaryPolicy {
    let capability = signature_capability_policy_with_effective_facts(java_ty, facts);
    let owner_requirement = capability
        .capability_ownership
        .unwrap_or_else(|| conservative_ownership(java_ty));
    let constraints = vec![
        SignatureConstraint::RequiredOwnership(owner_requirement),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ];
    SignatureNativeBoundaryPolicy {
        java_ty: java_ty.clone(),
        required_ownership: owner_requirement,
        constraints,
    }
}

pub fn signature_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SignatureCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn signature_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureCapabilityPolicy {
    let capability_ownership = facts
        .is_copy_type(java_ty)
        .then_some(OwnershipShape::CopyValue);
    SignatureCapabilityPolicy {
        java_ty: java_ty.clone(),
        constraints: signature_capability_constraints(capability_ownership),
        capability_ownership,
    }
}

pub fn signature_slot_baseline_policy(
    java_ty: &JavaType,
    boundary_kind: Option<SignatureBoundaryKind>,
    facts: &FactSet,
) -> SignatureSlotBaselinePolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_slot_baseline_policy_with_effective_facts(java_ty, boundary_kind, &effective_facts)
}

pub fn signature_slot_baseline_policy_with_effective_facts(
    java_ty: &JavaType,
    boundary_kind: Option<SignatureBoundaryKind>,
    facts: &EffectiveFacts<'_>,
) -> SignatureSlotBaselinePolicy {
    let capability_constraints =
        signature_capability_policy_with_effective_facts(java_ty, facts).constraints;
    let boundary_constraints = boundary_kind
        .map(|kind| {
            signature_boundary_policy_with_effective_facts(kind, java_ty, facts).constraints
        })
        .unwrap_or_default();
    let mut constraints = capability_constraints.clone();
    push_unique_signature_constraints(&mut constraints, &boundary_constraints);
    SignatureSlotBaselinePolicy {
        java_ty: java_ty.clone(),
        boundary_kind,
        capability_constraints,
        boundary_constraints,
        constraints,
    }
}

pub fn signature_slot_dispatch_requirement_policy(
    java_ty: &JavaType,
    role: SignatureSlotRole,
    source_dispatch: DispatchShape,
    facts: &FactSet,
) -> SignatureSlotDispatchRequirementPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_slot_dispatch_requirement_policy_with_effective_facts(
        java_ty,
        role,
        source_dispatch,
        &effective_facts,
    )
}

pub fn signature_slot_dispatch_requirement_policy_with_effective_facts(
    java_ty: &JavaType,
    role: SignatureSlotRole,
    source_dispatch: DispatchShape,
    facts: &EffectiveFacts<'_>,
) -> SignatureSlotDispatchRequirementPolicy {
    let requirements = java_type_name_for_signature_dispatch(java_ty)
        .map(|type_name| facts.dispatch_requirements_for(type_name))
        .unwrap_or_default();
    let constraints = signature_slot_dispatch_requirement_constraints(
        role,
        source_dispatch.clone(),
        &requirements,
    );
    SignatureSlotDispatchRequirementPolicy {
        java_ty: java_ty.clone(),
        role,
        source_dispatch,
        requirements,
        constraints,
    }
}

fn signature_slot_dispatch_requirement_constraints(
    role: SignatureSlotRole,
    source_dispatch: DispatchShape,
    requirements: &[DispatchRequirementKind],
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    let projection_role = dispatch_requirement_projection_role(role);
    for requirement in requirements {
        if let Some(dispatch) = dispatch_requirement_projection_policy(
            projection_role,
            source_dispatch.clone(),
            *requirement,
        )
        .projected_dispatch
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredDispatch(dispatch)],
            );
        }
    }
    constraints
}

fn dispatch_requirement_projection_role(
    role: SignatureSlotRole,
) -> DispatchRequirementProjectionRole {
    match role {
        SignatureSlotRole::Param { .. } => DispatchRequirementProjectionRole::Param,
        SignatureSlotRole::Return => DispatchRequirementProjectionRole::Return,
        SignatureSlotRole::Storage => DispatchRequirementProjectionRole::Storage,
    }
}

fn java_type_name_for_signature_dispatch(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::String => Some("String"),
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => {
            None
        }
    }
}

fn signature_capability_constraints(
    capability_ownership: Option<OwnershipShape>,
) -> Vec<SignatureConstraint> {
    capability_ownership
        .map(SignatureConstraint::RequiredOwnership)
        .into_iter()
        .collect()
}

pub fn signature_member_api_boundary_policy(
    modifiers: &[Modifier],
) -> SignatureMemberApiBoundaryPolicy {
    SignatureMemberApiBoundaryPolicy {
        public_api: modifiers
            .iter()
            .any(|modifier| matches!(modifier, Modifier::Public | Modifier::Protected)),
    }
}

pub fn signature_constraints_for_borrowed_content_policy(
    policy: BorrowedContentPolicy,
) -> Vec<SignatureConstraint> {
    match policy {
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::StringLike,
        } => vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
            SignatureConstraint::RequiredView(ViewShape::Str),
        ],
        BorrowedContentPolicy::ContentView {
            kind: BorrowedContentKind::SequenceLike,
        } => vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
            SignatureConstraint::RequiredView(ViewShape::Slice),
        ],
    }
}

pub fn signature_shape_constraint_propagation_policy() -> SignatureConstraintPropagationPolicy {
    SignatureConstraintPropagationPolicy {
        kind: SignatureConstraintPropagationKind::ShapeOnly,
    }
}

pub fn signature_slot_constraint_propagation_policy() -> SignatureConstraintPropagationPolicy {
    SignatureConstraintPropagationPolicy {
        kind: SignatureConstraintPropagationKind::SlotShapeAndStorage,
    }
}

impl SignatureConstraintPropagationPolicy {
    pub fn allows(self, constraint: &SignatureConstraint) -> bool {
        match self.kind {
            SignatureConstraintPropagationKind::ShapeOnly => constraint.is_shape_requirement(),
            SignatureConstraintPropagationKind::SlotShapeAndStorage => {
                constraint.is_shape_requirement() || constraint.is_storage_slot()
            }
        }
    }
}

impl SignatureConstraint {
    fn is_shape_requirement(&self) -> bool {
        matches!(
            self,
            SignatureConstraint::RequiredOwnership(_)
                | SignatureConstraint::RequiredView(_)
                | SignatureConstraint::RequiredDispatch(_)
        )
    }

    fn is_storage_slot(&self) -> bool {
        matches!(self, SignatureConstraint::StorageSlot)
    }
}

pub fn method_param_boundary_policy(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
) -> MethodParamBoundaryPolicy {
    let kind = if signature_member_api_boundary_policy(&method.modifiers).public_api {
        Some(SignatureBoundaryKind::PublicApi)
    } else {
        usage
            .methods
            .get(&UsageMethodKey {
                type_name: ty.name.clone(),
                method_name: method.name.clone(),
            })
            .and_then(|method_usage| method_usage.bindings.get(&param.name))
            .and_then(|binding| binding_boundary_policy(binding).kind)
    };
    MethodParamBoundaryPolicy {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
        param_name: param.name.clone(),
        kind,
    }
}

pub fn constructor_param_boundary_policy(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
) -> ConstructorParamBoundaryPolicy {
    let constructor_name = constructor_rust_name(ctor);
    let kind = if signature_member_api_boundary_policy(&ctor.modifiers).public_api {
        Some(SignatureBoundaryKind::PublicApi)
    } else {
        usage
            .constructors
            .get(&crate::usage::UsageConstructorKey {
                type_name: ty.name.clone(),
                rust_name: constructor_name.clone(),
            })
            .and_then(|ctor_usage| ctor_usage.bindings.get(&param.name))
            .and_then(|binding| binding_boundary_policy(binding).kind)
    };
    ConstructorParamBoundaryPolicy {
        type_name: ty.name.clone(),
        constructor_name,
        param_name: param.name.clone(),
        kind,
    }
}

pub fn binding_boundary_policy(binding: &BindingUseFacts) -> BindingBoundaryPolicy {
    let mut kind = None;
    for usage in &binding.uses {
        let next = match usage {
            BindingUse::PassedToCall {
                boundary: CallArgumentBoundary::Unknown,
                ..
            }
            | BindingUse::AssignedFromCall {
                boundary: CallArgumentBoundary::Unknown,
                ..
            } => Some(SignatureBoundaryKind::UnknownApi),
            BindingUse::StoredInField { .. } => Some(SignatureBoundaryKind::EscapingStorage),
            _ => None,
        };
        kind = merge_signature_boundary_kind_policy(kind, next).kept;
    }
    BindingBoundaryPolicy {
        binding_name: binding.name.clone(),
        kind,
    }
}

pub fn merge_signature_boundary_kind_policy(
    current: Option<SignatureBoundaryKind>,
    next: Option<SignatureBoundaryKind>,
) -> SignatureBoundaryKindMerge {
    match (current, next) {
        (Some(SignatureBoundaryKind::EscapingStorage), rejected) => SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::EscapingStorage),
            rejected,
        },
        (rejected, Some(SignatureBoundaryKind::EscapingStorage)) => SignatureBoundaryKindMerge {
            kept: Some(SignatureBoundaryKind::EscapingStorage),
            rejected,
        },
        (Some(current), None) => SignatureBoundaryKindMerge {
            kept: Some(current),
            rejected: None,
        },
        (None, next) => SignatureBoundaryKindMerge {
            kept: next,
            rejected: None,
        },
        (Some(SignatureBoundaryKind::PublicApi), Some(SignatureBoundaryKind::UnknownApi))
        | (Some(SignatureBoundaryKind::UnknownApi), Some(SignatureBoundaryKind::PublicApi))
        | (Some(SignatureBoundaryKind::PublicApi), Some(SignatureBoundaryKind::PublicApi))
        | (Some(SignatureBoundaryKind::UnknownApi), Some(SignatureBoundaryKind::UnknownApi)) => {
            SignatureBoundaryKindMerge {
                kept: current,
                rejected: next.filter(|next| Some(*next) != current),
            }
        }
    }
}

pub fn constructor_param_shape_constraints(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let boundary_kind = constructor_param_boundary_policy(ty, ctor, param, usage).kind;
    let mut constraints =
        signature_slot_baseline_policy(&param.ty, boundary_kind, facts).constraints;
    if let Some(ctor_usage) = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name: constructor_rust_name(ctor),
    }) && let Some(binding) = ctor_usage.bindings.get(&param.name)
    {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts),
        );
    }
    if boundary_kind.is_none() {
        constraints.extend(
            constructor_param_borrowed_content_policy_with_facts(ty, ctor, param, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
    }
    constraints
}

fn constrained_param_plan(
    param: &Param,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> ParamPlan {
    let shape = solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: param.ty.clone(),
            role: SignatureSlotRole::Param {
                is_varargs: param.is_varargs,
            },
            constraints: constraints.to_vec(),
        },
        facts,
    )
    .expect("parameter signature shape is always present");
    param_plan_with_shape(param, shape)
}

pub fn solve_signature_shape(
    representations: &RepresentationPlan,
    request: SignatureShapeRequest,
) -> Option<RustSlotShape> {
    solve_signature_shape_with_extra_constraints(representations, request, Vec::new())
}

pub fn solve_signature_shape_with_facts(
    representations: &RepresentationPlan,
    request: SignatureShapeRequest,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let source_dispatch = representations.dispatch_for(&request.java_ty);
    let dispatch_policy = signature_slot_dispatch_requirement_policy(
        &request.java_ty,
        request.role,
        source_dispatch,
        facts,
    );
    solve_signature_shape_with_extra_constraints(
        representations,
        request,
        dispatch_policy.constraints,
    )
}

fn solve_signature_shape_with_extra_constraints(
    representations: &RepresentationPlan,
    request: SignatureShapeRequest,
    extra_constraints: Vec<SignatureConstraint>,
) -> Option<RustSlotShape> {
    let mut all_constraints = request.constraints.clone();
    push_unique_signature_constraints(&mut all_constraints, &extra_constraints);
    let constraints = MergedSignatureConstraints::from_constraints(&request.constraints);
    let base_shape = if constraints.requires_storage_slot() {
        Some(storage_shape_for_java_type(
            representations,
            request.java_ty.clone(),
        ))
    } else {
        match request.role {
            SignatureSlotRole::Param { is_varargs } => Some(param_shape_for_java_type(
                representations,
                request.java_ty.clone(),
                is_varargs,
            )),
            SignatureSlotRole::Return => {
                return_shape_for_java_type(representations, &request.java_ty)
            }
            SignatureSlotRole::Storage => Some(storage_shape_for_java_type(
                representations,
                request.java_ty.clone(),
            )),
        }
    };
    let constraints = MergedSignatureConstraints::from_constraints(&all_constraints);
    base_shape.map(|shape| constraints.apply_to_shape(shape))
}

pub fn apply_signature_constraints(
    shape: RustSlotShape,
    constraints: &[SignatureConstraint],
) -> RustSlotShape {
    MergedSignatureConstraints::from_constraints(constraints).apply_to_shape(shape)
}

impl MergedSignatureConstraints {
    pub fn from_constraints(constraints: &[SignatureConstraint]) -> Self {
        let mut merged = Self::default();
        for constraint in constraints {
            merged.add_constraint(constraint);
        }
        merged
    }

    pub fn apply_to_shape(&self, mut shape: RustSlotShape) -> RustSlotShape {
        if let Some(nullability) = self.storage_nullability_req {
            apply_required_storage_nullability(&mut shape, nullability);
        }
        if let Some(requirement) = &self.slot_dispatch_req {
            apply_required_dispatch(&mut shape, requirement.clone());
        }
        if let Some(requirement) = self.required_owner {
            apply_required_ownership(&mut shape, requirement);
        }
        if let Some(requirement) = &self.slot_view_req {
            apply_required_view(&mut shape, requirement.clone());
        }
        shape
    }

    pub fn requires_mutable_binding(&self) -> bool {
        self.mutable_binding
    }

    pub fn requires_storage_slot(&self) -> bool {
        self.storage_slot
    }

    pub fn conflicts(&self) -> &[SignatureConstraintConflict] {
        &self.conflicts
    }

    fn add_constraint(&mut self, constraint: &SignatureConstraint) {
        match constraint {
            SignatureConstraint::RequiredOwnership(requirement) => {
                let merge = merge_ownership_requirement_policy(self.required_owner, *requirement);
                self.required_owner = Some(merge.kept);
                if let Some(rejected) = merge.rejected {
                    self.conflicts.push(SignatureConstraintConflict {
                        axis: SignatureConstraintAxis::Owner,
                        kept: SignatureConstraint::RequiredOwnership(merge.kept),
                        rejected: SignatureConstraint::RequiredOwnership(rejected),
                    });
                }
            }
            SignatureConstraint::RequiredView(requirement) => {
                let merge =
                    merge_view_requirement_policy(self.slot_view_req.take(), requirement.clone());
                self.slot_view_req = Some(merge.kept.clone());
                if let Some(rejected) = merge.rejected {
                    self.conflicts.push(SignatureConstraintConflict {
                        axis: SignatureConstraintAxis::View,
                        kept: SignatureConstraint::RequiredView(merge.kept),
                        rejected: SignatureConstraint::RequiredView(rejected),
                    });
                }
            }
            SignatureConstraint::RequiredDispatch(requirement) => {
                let merge = merge_dispatch_requirement_policy(
                    self.slot_dispatch_req.take(),
                    requirement.clone(),
                );
                self.slot_dispatch_req = Some(merge.kept.clone());
                if let Some(rejected) = merge.rejected {
                    self.conflicts.push(SignatureConstraintConflict {
                        axis: SignatureConstraintAxis::Dispatch,
                        kept: SignatureConstraint::RequiredDispatch(merge.kept),
                        rejected: SignatureConstraint::RequiredDispatch(rejected),
                    });
                }
            }
            SignatureConstraint::RequiredStorageNullability(requirement) => {
                let merge = merge_storage_nullability_requirement_policy(
                    self.storage_nullability_req,
                    *requirement,
                );
                self.storage_nullability_req = Some(merge.kept);
                if let Some(rejected) = merge.rejected {
                    self.conflicts.push(SignatureConstraintConflict {
                        axis: SignatureConstraintAxis::Nullability,
                        kept: SignatureConstraint::RequiredStorageNullability(merge.kept),
                        rejected: SignatureConstraint::RequiredStorageNullability(rejected),
                    });
                }
            }
            SignatureConstraint::MutableBinding => {
                self.mutable_binding = true;
            }
            SignatureConstraint::StorageSlot => {
                self.storage_slot = true;
            }
        }
    }
}

pub fn merge_ownership_requirement_policy(
    current: Option<OwnershipShape>,
    next: OwnershipShape,
) -> SignatureRequirementMerge<OwnershipShape> {
    let Some(current) = current else {
        return SignatureRequirementMerge {
            kept: next,
            rejected: None,
        };
    };
    if current == next {
        return SignatureRequirementMerge {
            kept: current,
            rejected: None,
        };
    }
    if signature_ownership_requirement_priority_policy(current).rank
        >= signature_ownership_requirement_priority_policy(next).rank
    {
        SignatureRequirementMerge {
            kept: current,
            rejected: Some(next),
        }
    } else {
        SignatureRequirementMerge {
            kept: next,
            rejected: Some(current),
        }
    }
}

pub fn signature_ownership_requirement_priority_policy(
    requirement: OwnershipShape,
) -> SignatureOwnershipRequirementPriorityPolicy {
    let rank = match requirement {
        OwnershipShape::Owned => 40,
        OwnershipShape::CopyValue => 30,
        OwnershipShape::MutableBorrow => 20,
        OwnershipShape::SharedBorrow => 10,
    };
    SignatureOwnershipRequirementPriorityPolicy { requirement, rank }
}

pub fn merge_view_requirement_policy(
    current: Option<ViewShape>,
    next: ViewShape,
) -> SignatureRequirementMerge<ViewShape> {
    let Some(current) = current else {
        return SignatureRequirementMerge {
            kept: next,
            rejected: None,
        };
    };
    if current == next {
        return SignatureRequirementMerge {
            kept: current,
            rejected: None,
        };
    }
    if signature_view_requirement_priority_policy(current.clone()).rank
        >= signature_view_requirement_priority_policy(next.clone()).rank
    {
        SignatureRequirementMerge {
            kept: current,
            rejected: Some(next),
        }
    } else {
        SignatureRequirementMerge {
            kept: next,
            rejected: Some(current),
        }
    }
}

pub fn signature_view_requirement_priority_policy(
    requirement: ViewShape,
) -> SignatureViewRequirementPriorityPolicy {
    let rank = match &requirement {
        ViewShape::Native => 50,
        ViewShape::ErasedObjectView => 40,
        ViewShape::EnumView | ViewShape::DynTraitView => 30,
        ViewShape::Slice | ViewShape::Str => 10,
    };
    SignatureViewRequirementPriorityPolicy { requirement, rank }
}

pub fn merge_dispatch_requirement_policy(
    current: Option<DispatchShape>,
    next: DispatchShape,
) -> SignatureRequirementMerge<DispatchShape> {
    let Some(current) = current else {
        return SignatureRequirementMerge {
            kept: next,
            rejected: None,
        };
    };
    if current == next {
        return SignatureRequirementMerge {
            kept: current,
            rejected: None,
        };
    }
    if signature_dispatch_requirement_priority_policy(current.clone()).rank
        >= signature_dispatch_requirement_priority_policy(next.clone()).rank
    {
        SignatureRequirementMerge {
            kept: current,
            rejected: Some(next),
        }
    } else {
        SignatureRequirementMerge {
            kept: next,
            rejected: Some(current),
        }
    }
}

pub fn signature_dispatch_requirement_priority_policy(
    requirement: DispatchShape,
) -> SignatureDispatchRequirementPriorityPolicy {
    let rank = match &requirement {
        DispatchShape::Concrete => 50,
        DispatchShape::ErasedObject => 40,
        DispatchShape::BoxedDynTrait { .. } => 30,
        DispatchShape::ClosedEnum { .. } | DispatchShape::DynTrait { .. } => 20,
    };
    SignatureDispatchRequirementPriorityPolicy { requirement, rank }
}

pub fn merge_storage_nullability_requirement_policy(
    current: Option<NullabilityState>,
    next: NullabilityState,
) -> SignatureRequirementMerge<NullabilityState> {
    let Some(current) = current else {
        return SignatureRequirementMerge {
            kept: next,
            rejected: None,
        };
    };
    if current == next {
        return SignatureRequirementMerge {
            kept: current,
            rejected: None,
        };
    }
    if storage_nullability_requirement_rank(current) >= storage_nullability_requirement_rank(next) {
        SignatureRequirementMerge {
            kept: current,
            rejected: Some(next),
        }
    } else {
        SignatureRequirementMerge {
            kept: next,
            rejected: Some(current),
        }
    }
}

fn storage_nullability_requirement_rank(requirement: NullabilityState) -> usize {
    match requirement {
        NullabilityState::MaybeNull => 40,
        NullabilityState::DefinitelyNull => 30,
        NullabilityState::Unknown => 20,
        NullabilityState::DefinitelyNonNull => 10,
    }
}

fn apply_required_storage_nullability(shape: &mut RustSlotShape, nullability: NullabilityState) {
    if matches!(nullability, NullabilityState::DefinitelyNonNull)
        || !java_type_can_be_null(&shape.java_ty)
        || optional_element_type(&shape.java_ty).is_some()
    {
        return;
    }
    shape.java_ty = optional_java_type(shape.java_ty.clone());
    shape.type_identity = crate::slot_shape::TypeIdentity::from_java_type(shape.java_ty.clone());
}

fn java_type_can_be_null(java_ty: &JavaType) -> bool {
    !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
}

fn signature_constraints_require_mutable_binding(constraints: &[SignatureConstraint]) -> bool {
    MergedSignatureConstraints::from_constraints(constraints).requires_mutable_binding()
}

pub fn field_shape_constraints() -> Vec<SignatureConstraint> {
    vec![SignatureConstraint::StorageSlot]
}

pub fn field_shape_constraints_with_facts(
    ty: &TypeDecl,
    field: &FieldDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = field_shape_constraints();
    push_unique_signature_constraints(
        &mut constraints,
        &field_nullability_shape_policy(ty, field, facts).constraints,
    );
    push_unique_signature_constraints(
        &mut constraints,
        &signature_slot_baseline_policy(
            &field.ty,
            signature_member_api_boundary_policy(&field.modifiers)
                .public_api
                .then_some(SignatureBoundaryKind::PublicApi),
            facts,
        )
        .constraints,
    );
    constraints
}

pub fn field_nullability_shape_policy(
    ty: &TypeDecl,
    field: &FieldDecl,
    facts: &FactSet,
) -> FieldNullabilityShapePolicy {
    let nullability = facts.field_nullability_for(&ty.name, &field.name);
    FieldNullabilityShapePolicy {
        type_name: ty.name.clone(),
        field_name: field.name.clone(),
        nullability,
        constraints: nullability
            .map(SignatureConstraint::RequiredStorageNullability)
            .into_iter()
            .collect(),
    }
}

pub fn field_shape_constraints_with_usage(
    ty: &TypeDecl,
    field: &FieldDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = field_shape_constraints_with_facts(ty, field, facts);
    if let Some(kind) = field_boundary_policy(ty, field, usage).kind {
        push_unique_signature_constraints(
            &mut constraints,
            &signature_boundary_policy(kind, &field.ty, facts).constraints,
        );
    }
    constraints
}

pub fn field_boundary_policy(
    ty: &TypeDecl,
    field: &FieldDecl,
    usage: &UsageFacts,
) -> FieldBoundaryPolicy {
    let method_field_use = usage
        .methods
        .iter()
        .filter(|(key, _)| key.type_name == ty.name)
        .flat_map(|(_, method_usage)| method_usage.field_uses.iter());
    let constructor_field_use = usage
        .constructors
        .iter()
        .filter(|(key, _)| key.type_name == ty.name)
        .flat_map(|(_, constructor_usage)| constructor_usage.field_uses.iter());

    let mut kind = None;
    for field_use in method_field_use.chain(constructor_field_use) {
        let next = match field_use {
            FieldUse::AssignedFromCall {
                field_name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            }
            | FieldUse::PassedToCall {
                field_name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } if field_name == &field.name => Some(SignatureBoundaryKind::UnknownApi),
            _ => None,
        };
        kind = merge_signature_boundary_kind_policy(kind, next).kept;
    }
    FieldBoundaryPolicy {
        type_name: ty.name.clone(),
        field_name: field.name.clone(),
        kind,
    }
}

fn constrained_field_shape(
    java_ty: &JavaType,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> RustSlotShape {
    solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: java_ty.clone(),
            role: SignatureSlotRole::Storage,
            constraints: constraints.to_vec(),
        },
        facts,
    )
    .expect("field signature shape is always present")
}

fn param_plan_with_shape(param: &Param, shape: RustSlotShape) -> ParamPlan {
    ParamPlan {
        name: param.name.clone(),
        java_ty: param.ty.clone(),
        is_varargs: param.is_varargs,
        shape,
    }
}

fn collect_type_signature_constraints(
    ty: &TypeDecl,
    usage: &UsageFacts,
    facts: &FactSet,
    constraints: &mut SignatureConstraintState,
) {
    for member in &ty.members {
        match member {
            crate::java_ir::Member::Field(field) => {
                constraints.fields.insert(
                    FieldKey {
                        type_name: ty.name.clone(),
                        field_name: field.name.clone(),
                    },
                    field_shape_constraints_with_usage(ty, field, usage, facts),
                );
            }
            crate::java_ir::Member::Method(method) => {
                let captured = captured_binding_names_for_method(facts, &ty.name, &method.name);
                let method_usage = usage.methods.get(&UsageMethodKey {
                    type_name: ty.name.clone(),
                    method_name: method.name.clone(),
                });
                constraints.methods.insert(
                    MethodKey {
                        type_name: ty.name.clone(),
                        rust_name: method_rust_name(method),
                    },
                    MethodConstraintSet {
                        public_api: signature_member_api_boundary_policy(&method.modifiers)
                            .public_api,
                        param_names: method
                            .params
                            .iter()
                            .map(|param| param.name.clone())
                            .collect(),
                        params: method
                            .params
                            .iter()
                            .map(|param| {
                                let mut constraints =
                                    method_param_shape_constraints(ty, method, param, usage, facts);
                                apply_capture_constraints(
                                    &mut constraints,
                                    &captured,
                                    &param.name,
                                    &param.ty,
                                    facts,
                                );
                                constraints
                            })
                            .collect(),
                        return_constraints: method_return_shape_constraints(
                            ty, method, usage, facts,
                        ),
                        locals: {
                            method
                                .body
                                .as_ref()
                                .map(|body| {
                                    collect_local_constraint_sets(
                                        body,
                                        method_usage,
                                        facts,
                                        &captured,
                                    )
                                })
                                .unwrap_or_default()
                        },
                        receiver: receiver_constraints_with_facts_or_usage(
                            ty,
                            method,
                            method_usage,
                            facts,
                        ),
                    },
                );
            }
            crate::java_ir::Member::Constructor(ctor) => {
                let captured = captured_binding_names_for_constructor(facts, &ty.name);
                constraints.constructors.insert(
                    ConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: constructor_rust_name(ctor),
                    },
                    ConstructorConstraintSet {
                        public_api: signature_member_api_boundary_policy(&ctor.modifiers)
                            .public_api,
                        param_names: ctor.params.iter().map(|param| param.name.clone()).collect(),
                        params: ctor
                            .params
                            .iter()
                            .map(|param| {
                                let mut constraints = constructor_param_shape_constraints(
                                    ty, ctor, param, usage, facts,
                                );
                                apply_capture_constraints(
                                    &mut constraints,
                                    &captured,
                                    &param.name,
                                    &param.ty,
                                    facts,
                                );
                                constraints
                            })
                            .collect(),
                        locals: {
                            let constructor_usage = usage.constructors.get(&UsageConstructorKey {
                                type_name: ty.name.clone(),
                                rust_name: constructor_rust_name(ctor),
                            });
                            ctor.body
                                .as_ref()
                                .map(|body| {
                                    collect_local_constraint_sets(
                                        body,
                                        constructor_usage,
                                        facts,
                                        &captured,
                                    )
                                })
                                .unwrap_or_default()
                        },
                    },
                );
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signature_constraints(nested, usage, facts, constraints);
            }
            crate::java_ir::Member::Unsupported { .. } => {}
        }
    }
}

fn captured_binding_names_for_method(
    facts: &FactSet,
    type_name: &str,
    method_name: &str,
) -> HashSet<String> {
    facts
        .captures
        .owners
        .iter()
        .filter(|owner| {
            matches!(
                &owner.origin,
                CaptureOrigin::Method {
                    type_name: owner_type,
                    method_name: owner_method,
                } if owner_type == type_name && owner_method == method_name
            )
        })
        .flat_map(|owner| owner.captures.iter())
        .filter(|capture| {
            matches!(
                capture.source,
                CaptureSource::Local | CaptureSource::Parameter
            )
        })
        .map(|capture| capture.name.clone())
        .collect()
}

fn captured_binding_names_for_constructor(facts: &FactSet, type_name: &str) -> HashSet<String> {
    facts
        .captures
        .owners
        .iter()
        .filter(|owner| {
            matches!(
                &owner.origin,
                CaptureOrigin::Constructor { type_name: owner_type } if owner_type == type_name
            )
        })
        .flat_map(|owner| owner.captures.iter())
        .filter(|capture| {
            matches!(
                capture.source,
                CaptureSource::Local | CaptureSource::Parameter
            )
        })
        .map(|capture| capture.name.clone())
        .collect()
}

fn apply_capture_constraints(
    constraints: &mut Vec<SignatureConstraint>,
    captured: &HashSet<String>,
    name: &str,
    java_ty: &JavaType,
    facts: &FactSet,
) {
    if !captured.contains(name) {
        return;
    }
    extend_signature_constraints_unique(
        constraints,
        signature_boundary_policy(SignatureBoundaryKind::EscapingStorage, java_ty, facts)
            .constraints,
    );
}

fn extend_signature_constraints_unique(
    constraints: &mut Vec<SignatureConstraint>,
    additions: Vec<SignatureConstraint>,
) {
    for constraint in additions {
        if !constraints.contains(&constraint) {
            constraints.push(constraint);
        }
    }
}

fn collect_local_constraint_sets(
    block: &crate::java_ir::Block,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
) -> Vec<LocalConstraintSet> {
    let mut locals = Vec::new();
    let mutated = collect_mutated_local_names(block);
    collect_local_constraint_sets_from_stmts(
        &block.statements,
        &mutated,
        usage,
        facts,
        captured,
        &mut locals,
    );
    locals
}

fn collect_local_constraint_sets_from_stmts(
    statements: &[crate::java_ir::Stmt],
    mutated: &HashSet<String>,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
    locals: &mut Vec<LocalConstraintSet>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                let mut constraints =
                    local_shape_constraints_with_usage(name, ty, mutated, usage, facts);
                apply_capture_constraints(&mut constraints, captured, name, ty, facts);
                locals.push(LocalConstraintSet {
                    name: name.clone(),
                    java_ty: ty.clone(),
                    constraints,
                });
            }
            Stmt::For { init, body, .. } => {
                collect_local_constraint_sets_from_stmts(
                    init, mutated, usage, facts, captured, locals,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    locals,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                let mut constraints =
                    local_shape_constraints_with_usage(var_name, var_type, mutated, usage, facts);
                apply_capture_constraints(&mut constraints, captured, var_name, var_type, facts);
                locals.push(LocalConstraintSet {
                    name: var_name.clone(),
                    java_ty: var_type.clone(),
                    constraints,
                });
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    locals,
                );
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_local_constraint_sets_from_stmts(
                    &then_block.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    locals,
                );
                if let Some(else_block) = else_block {
                    collect_local_constraint_sets_from_stmts(
                        &else_block.statements,
                        mutated,
                        usage,
                        facts,
                        captured,
                        locals,
                    );
                }
            }
            Stmt::While { body, .. } | Stmt::Block(body) => {
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    locals,
                );
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_local_constraint_sets_from_stmts(
                        &case.statements,
                        mutated,
                        usage,
                        facts,
                        captured,
                        locals,
                    );
                }
            }
            Stmt::Try {
                resources, body, ..
            } => {
                collect_local_constraint_sets_from_stmts(
                    resources, mutated, usage, facts, captured, locals,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    locals,
                );
            }
            Stmt::LocalType(_)
            | Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_local_plans_from_constraints(
    block: &crate::java_ir::Block,
    representations: &RepresentationPlan,
    constraints: &[LocalConstraintSet],
    facts: &FactSet,
) -> Vec<LocalPlan> {
    let mut locals = Vec::new();
    collect_local_plans_from_stmts(
        &block.statements,
        representations,
        constraints,
        facts,
        &mut locals,
    );
    locals
}

fn collect_local_plans_from_stmts(
    statements: &[crate::java_ir::Stmt],
    representations: &RepresentationPlan,
    constraints: &[LocalConstraintSet],
    facts: &FactSet,
    locals: &mut Vec<LocalPlan>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                locals.push(local_plan(
                    name,
                    ty,
                    representations,
                    local_constraints_for(constraints, name),
                    facts,
                ));
            }
            Stmt::For { init, body, .. } => {
                collect_local_plans_from_stmts(init, representations, constraints, facts, locals);
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                locals.push(local_plan(
                    var_name,
                    var_type,
                    representations,
                    local_constraints_for(constraints, var_name),
                    facts,
                ));
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_local_plans_from_stmts(
                    &then_block.statements,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
                if let Some(else_block) = else_block {
                    collect_local_plans_from_stmts(
                        &else_block.statements,
                        representations,
                        constraints,
                        facts,
                        locals,
                    );
                }
            }
            Stmt::While { body, .. } | Stmt::Block(body) => {
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_local_plans_from_stmts(
                        &case.statements,
                        representations,
                        constraints,
                        facts,
                        locals,
                    );
                }
            }
            Stmt::Try {
                resources, body, ..
            } => {
                collect_local_plans_from_stmts(
                    resources,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    locals,
                );
            }
            Stmt::LocalType(_)
            | Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn local_constraints_for<'a>(
    constraints: &'a [LocalConstraintSet],
    name: &str,
) -> &'a [SignatureConstraint] {
    constraints
        .iter()
        .find(|constraints| constraints.name == name)
        .map(|constraints| constraints.constraints.as_slice())
        .unwrap_or(&[])
}

pub fn local_shape_constraints(name: &str, mutated: &HashSet<String>) -> Vec<SignatureConstraint> {
    if mutated.contains(name) {
        vec![SignatureConstraint::MutableBinding]
    } else {
        Vec::new()
    }
}

fn local_shape_constraints_with_usage(
    name: &str,
    java_ty: &JavaType,
    mutated: &HashSet<String>,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    local_shape_constraints_with_usage_and_effective_facts(
        name,
        java_ty,
        mutated,
        usage,
        facts,
        &effective_facts,
    )
}

fn local_shape_constraints_with_usage_and_effective_facts(
    name: &str,
    java_ty: &JavaType,
    mutated: &HashSet<String>,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    effective_facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    let mut constraints = local_shape_constraints(name, mutated);
    constraints.extend(
        signature_capability_policy_with_effective_facts(java_ty, effective_facts).constraints,
    );
    if let Some(binding) = usage.and_then(|usage| usage.bindings.get(name)) {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts),
        );
    }
    if let Some(kind) = usage
        .and_then(|usage| usage.bindings.get(name))
        .and_then(|binding| binding_boundary_policy(binding).kind)
    {
        constraints.extend(
            signature_boundary_policy_with_effective_facts(kind, java_ty, effective_facts)
                .constraints,
        );
    }
    constraints
}

pub fn external_receiver_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    for usage in &binding.uses {
        let BindingUse::ReceiverForCall {
            name,
            boundary: CallArgumentBoundary::Unknown,
        } = usage
        else {
            continue;
        };
        let receiver_type = external_api_receiver_type_key(&binding.java_ty);
        if facts
            .external_api_effect_for_receiver(receiver_type, name)
            .is_some_and(|effect| {
                effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
            })
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredOwnership(
                    OwnershipShape::MutableBorrow,
                )],
            );
        }
    }
    constraints
}

fn external_api_receiver_type_key(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } => Some(name.as_str()),
        JavaType::String => Some("String"),
        JavaType::Array(_) => Some("Array"),
        JavaType::Unknown(name) => Some(name.as_str()),
        JavaType::Void | JavaType::Primitive(_) | JavaType::Wildcard(_) => None,
    }
}

fn local_plan(
    name: &str,
    java_ty: &JavaType,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> LocalPlan {
    LocalPlan {
        name: name.to_string(),
        java_ty: java_ty.clone(),
        shape: solve_signature_shape_with_facts(
            representations,
            SignatureShapeRequest {
                java_ty: java_ty.clone(),
                role: SignatureSlotRole::Storage,
                constraints: constraints.to_vec(),
            },
            facts,
        )
        .expect("local signature shape is always present"),
        binding_mutable: signature_constraints_require_mutable_binding(constraints),
    }
}

fn collect_mutated_local_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_mutated_local_names_from_stmts(&block.statements, &mut mutated);
    mutated
}

fn collect_mutated_local_names_from_stmts(statements: &[Stmt], mutated: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    collect_mutated_local_names_from_expr(expr, mutated);
                }
            }
            Stmt::Assign { target, value } => {
                collect_assignment_target_name(target, mutated);
                collect_mutated_local_names_from_expr(value, mutated);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_mutated_local_names_from_expr(expr, mutated);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_mutated_local_names_from_expr(condition, mutated);
                collect_mutated_local_names_from_stmts(&then_block.statements, mutated);
                if let Some(else_block) = else_block {
                    collect_mutated_local_names_from_stmts(&else_block.statements, mutated);
                }
            }
            Stmt::While { condition, body } => {
                collect_mutated_local_names_from_expr(condition, mutated);
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_mutated_local_names_from_stmts(init, mutated);
                if let Some(condition) = condition {
                    collect_mutated_local_names_from_expr(condition, mutated);
                }
                for expr in update {
                    collect_mutated_local_names_from_expr(expr, mutated);
                }
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_mutated_local_names_from_expr(iterable, mutated);
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::Switch { selector, cases } => {
                collect_mutated_local_names_from_expr(selector, mutated);
                for case in cases {
                    collect_mutated_local_names_from_stmts(&case.statements, mutated);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_mutated_local_names_from_stmts(resources, mutated);
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
                for catch in catches {
                    collect_mutated_local_names_from_stmts(&catch.body.statements, mutated);
                }
                if let Some(finally_block) = finally_block {
                    collect_mutated_local_names_from_stmts(&finally_block.statements, mutated);
                }
            }
            Stmt::Block(block) => {
                collect_mutated_local_names_from_stmts(&block.statements, mutated);
            }
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_mutated_local_names_from_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_assignment_target_name(expr, mutated),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_mutated_local_names_from_expr(expr, mutated);
        }
        Expr::Binary { left, right, .. } => {
            collect_mutated_local_names_from_expr(left, mutated);
            collect_mutated_local_names_from_expr(right, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutated_local_names_from_expr(condition, mutated);
            collect_mutated_local_names_from_expr(then_expr, mutated);
            collect_mutated_local_names_from_expr(else_expr, mutated);
        }
        Expr::FieldAccess(receiver, _) => collect_mutated_local_names_from_expr(receiver, mutated),
        Expr::ArrayAccess(array, index) => {
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_mutated_local_names_from_expr(expr, mutated);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutated_local_names_from_expr(receiver, mutated);
            }
            for arg in args {
                collect_mutated_local_names_from_expr(arg, mutated);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_mutated_local_names_from_expr(arg, mutated);
            }
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_assignment_target_name(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            mutated.insert(name.clone());
        }
        Expr::ArrayAccess(array, index) => {
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::FieldAccess(receiver, _) => collect_mutated_local_names_from_expr(receiver, mutated),
        _ => collect_mutated_local_names_from_expr(expr, mutated),
    }
}

pub fn default_receiver_plan(method: &MethodDecl) -> ReceiverPlan {
    constrained_receiver_plan(&receiver_constraints(method))
}

pub fn receiver_constraints(method: &MethodDecl) -> Vec<ReceiverConstraint> {
    receiver_constraints_with_usage(method, None)
}

pub fn receiver_constraints_with_usage(
    method: &MethodDecl,
    usage: Option<&MethodUsageFacts>,
) -> Vec<ReceiverConstraint> {
    let mut constraints = Vec::new();
    if method.modifiers.contains(&Modifier::Static) {
        constraints.push(ReceiverConstraint::StaticReceiver);
    }
    if usage.is_some_and(|usage| usage.mutates_self) {
        constraints.push(ReceiverConstraint::MutableReceiver);
    }
    constraints
}

pub fn receiver_constraints_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> Vec<ReceiverConstraint> {
    receiver_constraints_with_facts_or_usage(ty, method, None, facts)
}

fn receiver_constraints_with_facts_or_usage(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
) -> Vec<ReceiverConstraint> {
    let mut constraints = Vec::new();
    if method.modifiers.contains(&Modifier::Static) {
        constraints.push(ReceiverConstraint::StaticReceiver);
    }
    if let Some(effect) = facts.method_effect_for(&ty.name, &method.name) {
        if effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver {
            constraints.push(ReceiverConstraint::MutableReceiver);
        }
    } else if usage.is_some_and(|usage| usage.mutates_self) {
        constraints.push(ReceiverConstraint::MutableReceiver);
    }
    constraints
}

pub fn constrained_receiver_plan(constraints: &[ReceiverConstraint]) -> ReceiverPlan {
    if constraints.contains(&ReceiverConstraint::StaticReceiver) {
        ReceiverPlan::None
    } else if constraints.contains(&ReceiverConstraint::MutableReceiver) {
        ReceiverPlan::Mutable
    } else {
        ReceiverPlan::Shared
    }
}

pub fn method_return_shape_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let boundary_kind = method_return_boundary_policy(ty, method, usage).kind;
    let mut constraints =
        signature_slot_baseline_policy(&method.return_type, boundary_kind, facts).constraints;
    if !matches!(method.return_type, JavaType::Void)
        && let Some(effect) = facts.method_effect_for(&ty.name, &method.name)
        && effect.return_nullability != NullabilityState::Unknown
    {
        push_unique_signature_constraints(
            &mut constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                effect.return_nullability,
            )],
        );
    }
    if boundary_kind.is_none() {
        constraints.extend(
            borrowed_return_content_policy_from_param_with_facts(ty, method, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
    }
    constraints
}

pub fn method_return_boundary_policy(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
) -> MethodReturnBoundaryPolicy {
    let mut kind = None;
    if signature_member_api_boundary_policy(&method.modifiers).public_api {
        kind =
            merge_signature_boundary_kind_policy(kind, Some(SignatureBoundaryKind::PublicApi)).kept;
    } else if let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) {
        let returns_unknown_call_result = method_usage.call_sites.iter().any(|call_site| {
            call_site.result_returned && call_site.boundary == CallArgumentBoundary::Unknown
        }) || method_usage
            .returned_call_results
            .iter()
            .any(|returned| returned.boundary == CallArgumentBoundary::Unknown);
        kind = merge_signature_boundary_kind_policy(
            kind,
            returns_unknown_call_result.then_some(SignatureBoundaryKind::UnknownApi),
        )
        .kept;
    }
    MethodReturnBoundaryPolicy {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
        kind,
    }
}

fn constrained_return_shape(
    method: &MethodDecl,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> Option<RustSlotShape> {
    solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: method.return_type.clone(),
            role: SignatureSlotRole::Return,
            constraints: constraints.to_vec(),
        },
        facts,
    )
}

fn method_rust_name(method: &MethodDecl) -> String {
    method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}

fn constructor_rust_name(ctor: &ConstructorDecl) -> String {
    ctor.rust_name.clone().unwrap_or_else(|| "new".to_string())
}
