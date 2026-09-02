use std::collections::{BTreeMap, HashMap, HashSet};

use crate::binding_id::{BindingId, BindingKind};
use crate::call_plan::resolved_call_arg_target_type;
use crate::effective_facts::EffectiveFacts;
use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::facts::{
    CaptureOrigin, CaptureSource, DispatchRequirementKind, ExternalApiArgumentEffectFact,
    ExternalArgumentAliasPolicy, FacadeAbiFact, FactProvenance, FactSet, HierarchyOverride,
    HierarchyStrategy, NullabilityState, OwnedRecoveryCapability, ReceiverAbi,
    ReceiverMutabilityFact, external_parameter_nullability_policy,
};
use crate::graph::strongly_connected_components;
use crate::java_ir::{
    BinaryOp, Block, ConstructorDecl, Expr, FieldDecl, JavaType, LambdaBody, Literal, MethodDecl,
    Modifier, Param, Program, Stmt, SwitchLabel, TypeDecl, TypeKind, UnaryOp,
};
use crate::java_signature::JavaCallableKey;
use crate::lattice::{
    dispatch_shape_lattice_rank, join_ranked_lattice, ownership_lattice_rank,
    storage_nullability_lattice_rank, view_lattice_rank,
};
use crate::loop_plan::{PostLoopRefinementPlan, plan_while_stmt, post_loop_refinement_for_while};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    DispatchRequirementProjectionRole, apply_required_dispatch, apply_required_ownership,
    apply_required_view, dispatch_requirement_projection_policy, param_shape_for_java_type,
    return_shape_for_java_type, storage_shape_for_java_type,
};
use crate::slot_policy::{
    BorrowedContentKind, BorrowedContentPolicy, borrowed_content_type_policy,
    borrowed_return_content_policy_from_param_with_facts,
    constructor_param_borrowed_content_policy_with_facts, legacy_idless_method_assigns_to_binding,
    method_param_borrowed_content_policy_with_facts,
};
use crate::slot_shape::{
    DispatchShape, OwnershipShape, RustSlotShape, ViewShape, conservative_ownership,
};
use crate::types::{TypeEnv, optional_element_type, optional_java_type};
use crate::usage::{
    BindingUse, BindingUseFacts, CallArgumentBoundary, FieldUse, MethodUsageFacts,
    ReturnedCallResultUse, UsageConstructorKey, UsageFacts, UsageMethodKey,
};

pub type SolvedSlotShape = RustSlotShape;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SignaturePlan {
    methods: HashMap<MethodKey, MethodSignaturePlan>,
    methods_by_java_key: HashMap<JavaCallableKey, MethodSignaturePlan>,
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
    pub binding_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlan {
    pub binding: BindingId,
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
    NullableSequenceElement,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureBoundaryRole {
    MethodParam,
    MethodReturn,
    ConstructorParam,
    MethodLocal,
    ConstructorLocal,
    Field,
    MethodReceiver,
    CapturedValue,
    CallArgument,
    CallResult,
    ConstructorInvocation,
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
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSlotDispatchRequirementPolicy {
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
    /// Type parameters in scope where this slot appears. Empty means none are,
    /// which is the conservative reading: the slot is treated as a class.
    pub type_params: Vec<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureConstraintProvenance {
    OwnedRequirement,
    BorrowedRequirement,
    MutableBorrowRequirement,
    CopyRequirement,
    NativeViewRequirement,
    StringViewRequirement,
    SliceViewRequirement,
    DynTraitViewRequirement,
    EnumViewRequirement,
    ErasedObjectRequirement,
    ConcreteDispatchRequirement,
    DynTraitRequirement,
    BoxedDynTraitRequirement,
    ClosedEnumRequirement,
    OptionalStorageRequirement,
    NonNullStorageRequirement,
    UnknownStorageRequirement,
    MutableBindingRequirement,
    StorageSlotRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureConstraintConflictDiagnostic {
    pub axis: SignatureConstraintAxis,
    pub kept: SignatureConstraint,
    pub rejected: SignatureConstraint,
    pub kept_provenance: SignatureConstraintProvenance,
    pub rejected_provenance: SignatureConstraintProvenance,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureRequirementMerge<T> {
    pub kept: T,
    pub rejected: Option<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureBoundaryKindPriorityPolicy {
    pub requirement: SignatureBoundaryKind,
    pub rank: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureBoundaryKindMerge {
    pub kept: Option<SignatureBoundaryKind>,
    pub rejected: Option<SignatureBoundaryKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureFlagMerge {
    pub kept: bool,
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
    ProvenByCloneBoxAdapter,
    ProvenByFactoryCopy,
    ProvenByImmutableSharedHandle,
    AuditConservativeClone,
    UnsupportedFacadeOwnedExternalHandle,
    UnsupportedFfiOwnedHandle,
    UnsupportedNonCloneable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureOwnedRecoveryCapabilityPolicy {
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
    pub dependency_sccs: SignatureDependencySccAnalysis,
    pub pass_plan: SignatureSolverPassPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureSolverProfile {
    pub collect_constraints_ms: u128,
    pub dependency_graph_ms: u128,
    pub scc_analysis_ms: u128,
    pub pass_plan_ms: u128,
    pub fixpoint_ms: u128,
    pub plan_build_ms: u128,
    pub slot_count: usize,
    pub unique_constraint_count: usize,
    pub iteration_budget: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
    pub component_count: usize,
    pub cyclic_component_count: usize,
    pub max_component_size: usize,
    pub planned_components: usize,
    pub planned_mutable_components: usize,
    pub fixpoint_iterations: usize,
    pub diagnostics: usize,
    pub component_sweeps: usize,
    pub changed_component_sweeps: usize,
    pub state_clones: usize,
    pub state_clone_ms: u128,
    pub propagation_sweep_ms: u128,
    pub component_rollback_ms: u128,
    pub component_change_check_ms: u128,
    pub propagation_arms: BTreeMap<String, SignaturePropagationArmProfile>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignaturePropagationArmProfile {
    pub sweeps: usize,
    pub changed_sweeps: usize,
    pub elapsed_ns: u128,
}

#[derive(Debug, Clone)]
pub struct ProfiledSignaturePlan {
    pub plan: SignaturePlan,
    pub profile: SignatureSolverProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSolverProgressEvent {
    pub phase: &'static str,
    pub status: &'static str,
    pub elapsed_ms: u128,
    pub detail: String,
}

pub type SignatureSolverProgressSink<'a> = &'a (dyn Fn(SignatureSolverProgressEvent) + Send + Sync);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureSolverExecutionStats {
    pub component_sweeps: usize,
    pub changed_component_sweeps: usize,
    pub state_clones: usize,
    pub state_clone_ms: u128,
    pub propagation_sweep_ms: u128,
    pub component_rollback_ms: u128,
    pub component_change_check_ms: u128,
    pub propagation_arms: BTreeMap<String, SignaturePropagationArmProfile>,
}

impl SignatureSolverExecutionStats {
    fn add_propagation_arm_stats(
        &mut self,
        other: &BTreeMap<String, SignaturePropagationArmProfile>,
    ) {
        for (label, arm) in other {
            let existing = self.propagation_arms.entry(label.clone()).or_default();
            existing.sweeps += arm.sweeps;
            existing.changed_sweeps += arm.changed_sweeps;
            existing.elapsed_ns += arm.elapsed_ns;
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureFixpointDomainStats {
    pub slot_count: usize,
    pub unique_constraint_count: usize,
    pub iteration_budget: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureSaturationReport {
    pub domain_stats: SignatureFixpointDomainStats,
    pub iterations: usize,
    pub diagnostics: usize,
    pub dependency_stats: SignatureDependencySccStats,
    pub pass_stats: SignatureSolverPassStats,
    pub join_conflicts: usize,
    pub top_fallback_reasons: Vec<SignatureReportCount>,
    pub top_conflict_axes: Vec<SignatureReportCount>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureReportCount {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureConstraintState {
    pub methods: HashMap<MethodKey, MethodConstraintSet>,
    pub constructors: HashMap<ConstructorKey, ConstructorConstraintSet>,
    pub fields: HashMap<FieldKey, Vec<SignatureConstraint>>,
}

#[derive(Debug, Clone, Default)]
pub struct MethodConstraintSet {
    pub public_api: bool,
    pub param_names: Vec<String>,
    pub param_index_by_name: HashMap<String, usize>,
    pub param_java_types: Vec<JavaType>,
    pub params: Vec<Vec<SignatureConstraint>>,
    pub return_constraints: Vec<SignatureConstraint>,
    pub locals: Vec<LocalConstraintSet>,
    pub local_index_by_name: HashMap<String, usize>,
    pub local_index_by_binding: HashMap<BindingId, usize>,
    pub receiver: Vec<ReceiverConstraint>,
}

#[derive(Debug, Clone, Default)]
pub struct ConstructorConstraintSet {
    pub public_api: bool,
    pub param_names: Vec<String>,
    pub param_index_by_name: HashMap<String, usize>,
    pub params: Vec<Vec<SignatureConstraint>>,
    pub locals: Vec<LocalConstraintSet>,
    pub local_index_by_name: HashMap<String, usize>,
    pub local_index_by_binding: HashMap<BindingId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalConstraintSet {
    pub binding: BindingId,
    pub name: String,
    pub java_ty: JavaType,
    pub constraints: Vec<SignatureConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverConstraint {
    StaticReceiver,
    MutableReceiver,
}

impl MethodConstraintSet {
    fn param_index(&self, name: &str) -> Option<usize> {
        self.param_index_by_name
            .get(name)
            .copied()
            .or_else(|| self.param_names.iter().position(|param| param == name))
    }

    fn local(&self, name: &str) -> Option<&LocalConstraintSet> {
        self.local_index(name)
            .and_then(|index| self.locals.get(index))
    }

    fn local_mut(&mut self, name: &str) -> Option<&mut LocalConstraintSet> {
        let index = self.local_index(name)?;
        self.locals.get_mut(index)
    }

    fn local_by_binding(&self, binding: &BindingId) -> Option<&LocalConstraintSet> {
        self.local_index_by_binding
            .get(binding)
            .and_then(|index| self.locals.get(*index))
            .or_else(|| self.locals.iter().find(|local| local.binding == *binding))
    }

    fn local_mut_by_binding(&mut self, binding: &BindingId) -> Option<&mut LocalConstraintSet> {
        let index = self
            .local_index_by_binding
            .get(binding)
            .copied()
            .or_else(|| {
                self.locals
                    .iter()
                    .position(|local| local.binding == *binding)
            })?;
        self.locals.get_mut(index)
    }

    fn local_index(&self, name: &str) -> Option<usize> {
        if self
            .locals
            .iter()
            .filter(|local| local.name == name)
            .count()
            > 1
        {
            return None;
        }
        self.local_index_by_name
            .get(name)
            .copied()
            .or_else(|| self.locals.iter().position(|local| local.name == name))
    }
}

impl PartialEq for MethodConstraintSet {
    fn eq(&self, other: &Self) -> bool {
        self.public_api == other.public_api
            && self.param_names == other.param_names
            && self.param_java_types == other.param_java_types
            && self.params == other.params
            && self.return_constraints == other.return_constraints
            && self.locals == other.locals
            && self.receiver == other.receiver
    }
}

impl Eq for MethodConstraintSet {}

impl ConstructorConstraintSet {
    fn param_index(&self, name: &str) -> Option<usize> {
        self.param_index_by_name
            .get(name)
            .copied()
            .or_else(|| self.param_names.iter().position(|param| param == name))
    }

    fn local(&self, name: &str) -> Option<&LocalConstraintSet> {
        self.local_index(name)
            .and_then(|index| self.locals.get(index))
    }

    fn local_mut(&mut self, name: &str) -> Option<&mut LocalConstraintSet> {
        let index = self.local_index(name)?;
        self.locals.get_mut(index)
    }

    fn local_by_binding(&self, binding: &BindingId) -> Option<&LocalConstraintSet> {
        self.local_index_by_binding
            .get(binding)
            .and_then(|index| self.locals.get(*index))
            .or_else(|| self.locals.iter().find(|local| local.binding == *binding))
    }

    fn local_mut_by_binding(&mut self, binding: &BindingId) -> Option<&mut LocalConstraintSet> {
        let index = self
            .local_index_by_binding
            .get(binding)
            .copied()
            .or_else(|| {
                self.locals
                    .iter()
                    .position(|local| local.binding == *binding)
            })?;
        self.locals.get_mut(index)
    }

    fn local_index(&self, name: &str) -> Option<usize> {
        if self
            .locals
            .iter()
            .filter(|local| local.name == name)
            .count()
            > 1
        {
            return None;
        }
        self.local_index_by_name
            .get(name)
            .copied()
            .or_else(|| self.locals.iter().position(|local| local.name == name))
    }
}

impl PartialEq for ConstructorConstraintSet {
    fn eq(&self, other: &Self) -> bool {
        self.public_api == other.public_api
            && self.param_names == other.param_names
            && self.params == other.params
            && self.locals == other.locals
    }
}

impl Eq for ConstructorConstraintSet {}

fn index_names(names: &[String]) -> HashMap<String, usize> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect()
}

fn index_local_constraints(locals: &[LocalConstraintSet]) -> HashMap<String, usize> {
    let mut index_by_name = HashMap::new();
    let mut ambiguous_names = HashSet::new();
    for (index, local) in locals.iter().enumerate() {
        if index_by_name.insert(local.name.clone(), index).is_some() {
            ambiguous_names.insert(local.name.clone());
        }
    }
    for name in ambiguous_names {
        index_by_name.remove(&name);
    }
    index_by_name
}

fn index_local_constraints_by_binding(locals: &[LocalConstraintSet]) -> HashMap<BindingId, usize> {
    locals
        .iter()
        .enumerate()
        .map(|(index, local)| (local.binding.clone(), index))
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureDependencyGraph {
    pub nodes: HashSet<SignatureDependencyNode>,
    pub edges: Vec<SignatureDependencyEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureDependencySccAnalysis {
    pub components: Vec<Vec<SignatureDependencyNode>>,
    pub component_successors: Vec<Vec<usize>>,
    pub component_by_node: HashMap<SignatureDependencyNode, usize>,
    pub stats: SignatureDependencySccStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureDependencySccStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub component_count: usize,
    pub cyclic_component_count: usize,
    pub max_component_size: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureSolverPassPlan {
    pub component_order: Vec<usize>,
    pub cyclic_components: HashSet<usize>,
    pub mutable_components: HashSet<usize>,
    pub stats: SignatureSolverPassStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureSolverPassStats {
    pub planned_components: usize,
    pub planned_mutable_components: usize,
    pub cyclic_components: usize,
    pub max_component_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignatureCallableKey {
    Method(MethodKey),
    Constructor(ConstructorKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SignatureDependencyNode {
    MethodParam {
        method: MethodKey,
        name: String,
        index: usize,
    },
    MethodReturn {
        method: MethodKey,
    },
    MethodLocal {
        method: MethodKey,
        name: String,
        binding: Option<BindingId>,
    },
    MethodReceiver {
        method: MethodKey,
    },
    ConstructorParam {
        constructor: ConstructorKey,
        name: String,
        index: usize,
    },
    ConstructorLocal {
        constructor: ConstructorKey,
        name: String,
        binding: Option<BindingId>,
    },
    Field {
        field: FieldKey,
    },
    CapturedValue {
        owner: SignatureCallableKey,
        name: String,
    },
    CallArg {
        caller: SignatureCallableKey,
        call_index: usize,
        arg_index: usize,
    },
    CallResult {
        caller: SignatureCallableKey,
        call_index: usize,
    },
    UnknownBoundaryArg {
        caller: SignatureCallableKey,
        call_index: usize,
        arg_index: usize,
    },
    UnknownBoundaryResult {
        caller: SignatureCallableKey,
        call_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureDependencyEdge {
    pub from: SignatureDependencyNode,
    pub to: SignatureDependencyNode,
    pub kind: SignatureDependencyEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignatureDependencyEdgeKind {
    LocalAlias,
    FieldStorage,
    MethodReturn,
    CallArgument,
    CallResult,
    ConstructorDelegationArgument,
    UnknownBoundary,
    Receiver,
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
    solve_signatures_with_facts_and_support_context(program, representations, usage, facts, program)
}

pub fn solve_signatures_with_facts_and_support_context(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
    support_program: &Program,
) -> SignaturePlan {
    solve_signatures_with_facts_and_support_context_profiled(
        program,
        representations,
        usage,
        facts,
        support_program,
    )
    .plan
}

pub fn solve_signatures_with_facts_and_support_context_profiled(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
    support_program: &Program,
) -> ProfiledSignaturePlan {
    solve_signatures_with_facts_and_support_context_profiled_with_progress(
        program,
        representations,
        usage,
        facts,
        support_program,
        None,
    )
}

pub fn solve_signatures_with_facts_and_support_context_profiled_with_progress(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    facts: &FactSet,
    support_program: &Program,
    progress_sink: Option<SignatureSolverProgressSink<'_>>,
) -> ProfiledSignaturePlan {
    emit_signature_solver_progress(
        progress_sink,
        "collect_constraints",
        "running",
        0,
        "collect_signature_constraints",
    );
    let collect_start = std::time::Instant::now();
    let constraints = collect_signature_constraints(program, usage, facts);
    let collect_constraints_ms = collect_start.elapsed().as_millis();
    emit_signature_solver_progress(
        progress_sink,
        "collect_constraints",
        "ok",
        collect_constraints_ms,
        "collect_signature_constraints",
    );

    let domain_stats = signature_fixpoint_domain_stats(&constraints);

    emit_signature_solver_progress(
        progress_sink,
        "dependency_graph",
        "running",
        0,
        "build_signature_dependency_graph",
    );
    let graph_start = std::time::Instant::now();
    let dependency_graph = build_signature_dependency_graph(&constraints, usage);
    let dependency_graph_ms = graph_start.elapsed().as_millis();
    emit_signature_solver_progress(
        progress_sink,
        "dependency_graph",
        "ok",
        dependency_graph_ms,
        &format!(
            "nodes={} edges={}",
            dependency_graph.nodes.len(),
            dependency_graph.edges.len()
        ),
    );

    emit_signature_solver_progress(
        progress_sink,
        "scc_analysis",
        "running",
        0,
        "analyze_signature_dependency_sccs",
    );
    let scc_start = std::time::Instant::now();
    let dependency_sccs = analyze_signature_dependency_sccs(&dependency_graph);
    let scc_analysis_ms = scc_start.elapsed().as_millis();
    emit_signature_solver_progress(
        progress_sink,
        "scc_analysis",
        "ok",
        scc_analysis_ms,
        &format!(
            "components={} cyclic={} max_component_size={}",
            dependency_sccs.stats.component_count,
            dependency_sccs.stats.cyclic_component_count,
            dependency_sccs.stats.max_component_size
        ),
    );

    emit_signature_solver_progress(
        progress_sink,
        "pass_plan",
        "running",
        0,
        "plan_signature_solver_passes",
    );
    let pass_plan_start = std::time::Instant::now();
    let pass_plan = plan_signature_solver_passes(&dependency_sccs);
    let pass_plan_ms = pass_plan_start.elapsed().as_millis();
    emit_signature_solver_progress(
        progress_sink,
        "pass_plan",
        "ok",
        pass_plan_ms,
        &format!(
            "components={} mutable={}",
            pass_plan.component_order.len(),
            pass_plan.mutable_components.len()
        ),
    );

    emit_signature_solver_progress(
        progress_sink,
        "fixpoint",
        "running",
        0,
        &format!(
            "slots={} constraints={} budget={}",
            domain_stats.slot_count,
            domain_stats.unique_constraint_count,
            domain_stats.iteration_budget
        ),
    );
    let fixpoint_start = std::time::Instant::now();
    let saturation_policy = signature_fixpoint_saturation_policy(&constraints);
    let mut execution_stats = SignatureSolverExecutionStats::default();
    let mut current_iteration = 0usize;
    let result = saturation_policy.saturate(
        constraints,
        |constraints| {
            current_iteration += 1;
            let (changed, stats) = execute_signature_solver_pass_plan_with_stats(
                constraints,
                usage,
                &dependency_sccs,
                &pass_plan,
                Some(SignatureSolverPassProgress {
                    progress_sink,
                    fixpoint_start,
                    iteration: current_iteration,
                }),
            );
            execution_stats.component_sweeps += stats.component_sweeps;
            execution_stats.changed_component_sweeps += stats.changed_component_sweeps;
            execution_stats.state_clones += stats.state_clones;
            execution_stats.state_clone_ms += stats.state_clone_ms;
            execution_stats.propagation_sweep_ms += stats.propagation_sweep_ms;
            execution_stats.component_rollback_ms += stats.component_rollback_ms;
            execution_stats.component_change_check_ms += stats.component_change_check_ms;
            execution_stats.add_propagation_arm_stats(&stats.propagation_arms);
            emit_signature_solver_progress(
                progress_sink,
                "fixpoint_iteration",
                if changed { "changed" } else { "stable" },
                fixpoint_start.elapsed().as_millis(),
                &format!(
                    "iteration={} component_sweeps={} changed_component_sweeps={}",
                    current_iteration,
                    execution_stats.component_sweeps,
                    execution_stats.changed_component_sweeps
                ),
            );
            SaturationStep::from(changed)
        },
        |max_iterations| SignatureDiagnostic {
            code: "signature_fixpoint_budget_exceeded",
            message: format!(
                "signature constraint fixpoint exceeded finite saturation budget of {max_iterations} iterations"
            ),
        },
    );
    let fixpoint_ms = fixpoint_start.elapsed().as_millis();
    let fixpoint_iterations = result.iterations;
    let fixpoint_diagnostics = result.diagnostics.len();
    emit_signature_solver_progress(
        progress_sink,
        "fixpoint",
        "ok",
        fixpoint_ms,
        &format!("iterations={fixpoint_iterations} diagnostics={fixpoint_diagnostics}"),
    );

    emit_signature_solver_progress(
        progress_sink,
        "plan_build",
        "running",
        0,
        "build_signature_plan_from_constraints",
    );
    let plan_build_start = std::time::Instant::now();
    let plan = build_signature_plan_from_constraints(
        program,
        support_program,
        representations,
        &result.state,
        fixpoint_iterations,
        result.diagnostics,
        facts,
    );
    let plan_build_ms = plan_build_start.elapsed().as_millis();
    emit_signature_solver_progress(
        progress_sink,
        "plan_build",
        "ok",
        plan_build_ms,
        "build_signature_plan_from_constraints",
    );

    let profile = SignatureSolverProfile {
        collect_constraints_ms,
        dependency_graph_ms,
        scc_analysis_ms,
        pass_plan_ms,
        fixpoint_ms,
        plan_build_ms,
        slot_count: domain_stats.slot_count,
        unique_constraint_count: domain_stats.unique_constraint_count,
        iteration_budget: domain_stats.iteration_budget,
        graph_node_count: dependency_sccs.stats.node_count,
        graph_edge_count: dependency_sccs.stats.edge_count,
        component_count: dependency_sccs.stats.component_count,
        cyclic_component_count: dependency_sccs.stats.cyclic_component_count,
        max_component_size: dependency_sccs.stats.max_component_size,
        planned_components: pass_plan.stats.planned_components,
        planned_mutable_components: pass_plan.stats.planned_mutable_components,
        fixpoint_iterations,
        diagnostics: fixpoint_diagnostics,
        component_sweeps: execution_stats.component_sweeps,
        changed_component_sweeps: execution_stats.changed_component_sweeps,
        state_clones: execution_stats.state_clones,
        state_clone_ms: execution_stats.state_clone_ms,
        propagation_sweep_ms: execution_stats.propagation_sweep_ms,
        component_rollback_ms: execution_stats.component_rollback_ms,
        component_change_check_ms: execution_stats.component_change_check_ms,
        propagation_arms: execution_stats.propagation_arms,
    };
    ProfiledSignaturePlan { plan, profile }
}

fn emit_signature_solver_progress(
    progress_sink: Option<SignatureSolverProgressSink<'_>>,
    phase: &'static str,
    status: &'static str,
    elapsed_ms: u128,
    detail: &str,
) {
    if let Some(progress_sink) = progress_sink {
        progress_sink(SignatureSolverProgressEvent {
            phase,
            status,
            elapsed_ms,
            detail: detail.to_string(),
        });
    }
}

pub fn collect_signature_constraints(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
) -> SignatureConstraintState {
    let mut constraints = SignatureConstraintState::default();
    let declared_source_types = program_declared_source_type_names(program);
    for ty in &program.types {
        collect_type_signature_constraints(
            ty,
            usage,
            facts,
            &declared_source_types,
            &mut constraints,
        );
    }
    constraints
}

fn program_declared_source_type_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for ty in &program.types {
        collect_declared_source_type_names(ty, &mut names);
    }
    names
}

fn collect_declared_source_type_names(ty: &TypeDecl, names: &mut HashSet<String>) {
    names.insert(ty.name.clone());
    names.insert(type_name_simple_segment(&ty.name).to_string());
    for member in &ty.members {
        collect_member_declared_source_type_names(member, names);
    }
}

fn collect_member_declared_source_type_names(
    member: &crate::java_ir::Member,
    names: &mut HashSet<String>,
) {
    match member {
        crate::java_ir::Member::Type(nested) => collect_declared_source_type_names(nested, names),
        crate::java_ir::Member::Method(method) => {
            if let Some(body) = &method.body {
                collect_declared_source_type_names_from_block(body, names);
            }
        }
        crate::java_ir::Member::Constructor(ctor) => {
            if let Some(body) = &ctor.body {
                collect_declared_source_type_names_from_block(body, names);
            }
        }
        crate::java_ir::Member::Field(_) | crate::java_ir::Member::Unsupported { .. } => {}
    }
}

fn collect_declared_source_type_names_from_block(block: &Block, names: &mut HashSet<String>) {
    for stmt in &block.statements {
        match stmt {
            Stmt::LocalType(local) => collect_declared_source_type_names(local, names),
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_declared_source_type_names_from_block(block, names);
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_declared_source_type_names_from_block(then_block, names);
                if let Some(else_block) = else_block {
                    collect_declared_source_type_names_from_block(else_block, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::EnhancedFor { body, .. } => {
                collect_declared_source_type_names_from_block(body, names);
            }
            Stmt::For { init, body, .. } => {
                for stmt in init {
                    if let Stmt::LocalType(local) = stmt {
                        collect_declared_source_type_names(local, names);
                    }
                }
                collect_declared_source_type_names_from_block(body, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_declared_source_type_names_from_block(
                        &Block {
                            statements: case.statements.clone(),
                        },
                        names,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                for stmt in resources {
                    if let Stmt::LocalType(local) = stmt {
                        collect_declared_source_type_names(local, names);
                    }
                }
                collect_declared_source_type_names_from_block(body, names);
                for catch in catches {
                    collect_declared_source_type_names_from_block(&catch.body, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_declared_source_type_names_from_block(finally_block, names);
                }
            }
            Stmt::VarDecl { .. }
            | Stmt::Expr(_)
            | Stmt::Assign { .. }
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

pub fn build_signature_dependency_graph(
    constraints: &SignatureConstraintState,
    usage: &UsageFacts,
) -> SignatureDependencyGraph {
    build_signature_dependency_graph_with_facts(constraints, usage, &FactSet::default())
}

pub fn build_signature_dependency_graph_with_facts(
    constraints: &SignatureConstraintState,
    usage: &UsageFacts,
    facts: &FactSet,
) -> SignatureDependencyGraph {
    let mut graph = SignatureDependencyGraph::default();
    add_signature_slot_nodes(constraints, &mut graph);
    add_method_usage_dependency_edges(constraints, usage, &mut graph);
    add_constructor_usage_dependency_edges(constraints, usage, &mut graph);
    add_capture_dependency_nodes(constraints, facts, &mut graph);
    graph
}

pub fn analyze_signature_dependency_sccs(
    graph: &SignatureDependencyGraph,
) -> SignatureDependencySccAnalysis {
    let mut successors = HashMap::<SignatureDependencyNode, Vec<SignatureDependencyNode>>::new();
    for edge in &graph.edges {
        successors
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let scc = strongly_connected_components(graph.nodes.iter().cloned(), |node| {
        successors.get(node).cloned().unwrap_or_default()
    });
    let stats = SignatureDependencySccStats {
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        component_count: scc.components.len(),
        cyclic_component_count: scc
            .components
            .iter()
            .filter(|component| {
                component.len() > 1
                    || component.first().is_some_and(|node| {
                        successors.get(node).is_some_and(|next| next.contains(node))
                    })
            })
            .count(),
        max_component_size: scc.components.iter().map(Vec::len).max().unwrap_or(0),
    };
    SignatureDependencySccAnalysis {
        components: scc.components,
        component_successors: scc.component_successors,
        component_by_node: scc.component_by_node,
        stats,
    }
}

pub fn plan_signature_solver_passes(
    analysis: &SignatureDependencySccAnalysis,
) -> SignatureSolverPassPlan {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    for component_index in 0..analysis.components.len() {
        push_signature_component_postorder(
            component_index,
            &analysis.component_successors,
            &mut visited,
            &mut order,
        );
    }
    order.reverse();

    let cyclic_components = analysis
        .components
        .iter()
        .enumerate()
        .filter_map(|(index, component)| {
            if component.len() > 1
                || component.first().is_some_and(|node| {
                    analysis
                        .component_by_node
                        .get(node)
                        .is_some_and(|component| *component == index)
                        && analysis.component_successors[index].contains(&index)
                })
            {
                Some(index)
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();
    let mutable_components = analysis
        .components
        .iter()
        .enumerate()
        .filter_map(|(index, component)| {
            component
                .iter()
                .any(signature_dependency_node_stores_constraints)
                .then_some(index)
        })
        .collect::<HashSet<_>>();
    let stats = SignatureSolverPassStats {
        planned_components: order.len(),
        planned_mutable_components: mutable_components.len(),
        cyclic_components: analysis.stats.cyclic_component_count,
        max_component_size: analysis.stats.max_component_size,
    };

    SignatureSolverPassPlan {
        component_order: order,
        cyclic_components,
        mutable_components,
        stats,
    }
}

fn signature_dependency_node_stores_constraints(node: &SignatureDependencyNode) -> bool {
    matches!(
        node,
        SignatureDependencyNode::MethodReceiver { .. }
            | SignatureDependencyNode::MethodParam { .. }
            | SignatureDependencyNode::MethodReturn { .. }
            | SignatureDependencyNode::MethodLocal { .. }
            | SignatureDependencyNode::ConstructorParam { .. }
            | SignatureDependencyNode::ConstructorLocal { .. }
            | SignatureDependencyNode::Field { .. }
    )
}

fn push_signature_component_postorder(
    component_index: usize,
    successors: &[Vec<usize>],
    visited: &mut HashSet<usize>,
    order: &mut Vec<usize>,
) {
    if !visited.insert(component_index) {
        return;
    }
    for successor in successors.get(component_index).into_iter().flatten() {
        push_signature_component_postorder(*successor, successors, visited, order);
    }
    order.push(component_index);
}

fn add_signature_slot_nodes(
    constraints: &SignatureConstraintState,
    graph: &mut SignatureDependencyGraph,
) {
    for (method, method_constraints) in &constraints.methods {
        add_signature_dependency_node(
            graph,
            SignatureDependencyNode::MethodReceiver {
                method: method.clone(),
            },
        );
        add_signature_dependency_node(
            graph,
            SignatureDependencyNode::MethodReturn {
                method: method.clone(),
            },
        );
        for (index, name) in method_constraints.param_names.iter().enumerate() {
            add_signature_dependency_node(
                graph,
                SignatureDependencyNode::MethodParam {
                    method: method.clone(),
                    name: name.clone(),
                    index,
                },
            );
        }
        for local in &method_constraints.locals {
            let precise = SignatureDependencyNode::MethodLocal {
                method: method.clone(),
                name: local.name.clone(),
                binding: Some(local.binding.clone()),
            };
            let legacy = SignatureDependencyNode::MethodLocal {
                method: method.clone(),
                name: local.name.clone(),
                binding: None,
            };
            add_signature_dependency_node(graph, precise.clone());
            add_signature_dependency_node(graph, legacy.clone());
            add_bidirectional_signature_dependency_edge(
                graph,
                precise,
                legacy,
                SignatureDependencyEdgeKind::LocalAlias,
            );
        }
    }
    for (constructor, constructor_constraints) in &constraints.constructors {
        for (index, name) in constructor_constraints.param_names.iter().enumerate() {
            add_signature_dependency_node(
                graph,
                SignatureDependencyNode::ConstructorParam {
                    constructor: constructor.clone(),
                    name: name.clone(),
                    index,
                },
            );
        }
        for local in &constructor_constraints.locals {
            let precise = SignatureDependencyNode::ConstructorLocal {
                constructor: constructor.clone(),
                name: local.name.clone(),
                binding: Some(local.binding.clone()),
            };
            let legacy = SignatureDependencyNode::ConstructorLocal {
                constructor: constructor.clone(),
                name: local.name.clone(),
                binding: None,
            };
            add_signature_dependency_node(graph, precise.clone());
            add_signature_dependency_node(graph, legacy.clone());
            add_bidirectional_signature_dependency_edge(
                graph,
                precise,
                legacy,
                SignatureDependencyEdgeKind::LocalAlias,
            );
        }
    }
    for field in constraints.fields.keys() {
        add_signature_dependency_node(
            graph,
            SignatureDependencyNode::Field {
                field: field.clone(),
            },
        );
    }
}

fn add_method_usage_dependency_edges(
    constraints: &SignatureConstraintState,
    usage: &UsageFacts,
    graph: &mut SignatureDependencyGraph,
) {
    for (usage_key, method_usage) in &usage.methods {
        let Some(method) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let caller = SignatureCallableKey::Method(method.clone());
        for (call_index, call_site) in method_usage.call_sites.iter().enumerate() {
            let result = SignatureDependencyNode::CallResult {
                caller: caller.clone(),
                call_index,
            };
            add_signature_dependency_node(graph, result.clone());
            if call_site.boundary == CallArgumentBoundary::Unknown {
                add_signature_dependency_edge(
                    graph,
                    SignatureDependencyNode::UnknownBoundaryResult {
                        caller: caller.clone(),
                        call_index,
                    },
                    result.clone(),
                    SignatureDependencyEdgeKind::UnknownBoundary,
                );
            }
            if let Some(resolved) = &call_site.resolved {
                add_signature_dependency_edge(
                    graph,
                    SignatureDependencyNode::MethodReturn {
                        method: MethodKey {
                            type_name: resolved.declaring_type.clone(),
                            rust_name: resolved.rust_name.clone(),
                        },
                    },
                    result,
                    SignatureDependencyEdgeKind::CallResult,
                );
            }
        }
        add_method_binding_dependency_edges(constraints, method_usage, &method, &caller, graph);
        add_method_field_dependency_edges(method_usage, &method, &caller, graph);
        for returned in &method_usage.returned_call_results {
            for (call_index, call_site) in method_usage.call_sites.iter().enumerate() {
                if call_site.name == returned.name && call_site.boundary == returned.boundary {
                    add_signature_dependency_edge(
                        graph,
                        SignatureDependencyNode::CallResult {
                            caller: caller.clone(),
                            call_index,
                        },
                        SignatureDependencyNode::MethodReturn {
                            method: method.clone(),
                        },
                        SignatureDependencyEdgeKind::CallResult,
                    );
                }
            }
        }
    }
}

fn add_constructor_usage_dependency_edges(
    constraints: &SignatureConstraintState,
    usage: &UsageFacts,
    graph: &mut SignatureDependencyGraph,
) {
    for (usage_key, constructor_usage) in &usage.constructors {
        let constructor = ConstructorKey {
            type_name: usage_key.type_name.clone(),
            rust_name: usage_key.rust_name.clone(),
        };
        if !constraints.constructors.contains_key(&constructor) {
            continue;
        }
        let caller = SignatureCallableKey::Constructor(constructor.clone());
        for (call_index, call_site) in constructor_usage.call_sites.iter().enumerate() {
            let result = SignatureDependencyNode::CallResult {
                caller: caller.clone(),
                call_index,
            };
            add_signature_dependency_node(graph, result.clone());
            if call_site.boundary == CallArgumentBoundary::Unknown {
                add_signature_dependency_edge(
                    graph,
                    SignatureDependencyNode::UnknownBoundaryResult {
                        caller: caller.clone(),
                        call_index,
                    },
                    result.clone(),
                    SignatureDependencyEdgeKind::UnknownBoundary,
                );
            }
            if let Some(resolved) = &call_site.resolved {
                add_signature_dependency_edge(
                    graph,
                    SignatureDependencyNode::MethodReturn {
                        method: MethodKey {
                            type_name: resolved.declaring_type.clone(),
                            rust_name: resolved.rust_name.clone(),
                        },
                    },
                    result,
                    SignatureDependencyEdgeKind::CallResult,
                );
            }
        }
        for (call_index, call_site) in constructor_usage.constructor_call_sites.iter().enumerate() {
            let call_index = constructor_call_dependency_index(constructor_usage, call_index);
            if let Some(resolved) = &call_site.resolved {
                for arg_index in 0..call_site.arg_count {
                    add_signature_dependency_edge(
                        graph,
                        SignatureDependencyNode::CallArg {
                            caller: caller.clone(),
                            call_index,
                            arg_index,
                        },
                        SignatureDependencyNode::ConstructorParam {
                            constructor: ConstructorKey {
                                type_name: resolved.declaring_type.clone(),
                                rust_name: resolved.rust_name.clone(),
                            },
                            name: constructor_param_name_for_resolved(
                                constraints,
                                resolved,
                                arg_index,
                            ),
                            index: arg_index,
                        },
                        SignatureDependencyEdgeKind::ConstructorDelegationArgument,
                    );
                }
            }
        }
        add_constructor_binding_dependency_edges(
            constraints,
            constructor_usage,
            &constructor,
            &caller,
            graph,
        );
        add_constructor_field_dependency_edges(constructor_usage, &constructor, &caller, graph);
    }
}

fn add_capture_dependency_nodes(
    constraints: &SignatureConstraintState,
    facts: &FactSet,
    graph: &mut SignatureDependencyGraph,
) {
    for owner in &facts.captures.owners {
        let Some(callable) = capture_owner_callable(constraints, &owner.origin) else {
            continue;
        };
        for capture in &owner.captures {
            let captured = SignatureDependencyNode::CapturedValue {
                owner: callable.clone(),
                name: capture.name.clone(),
            };
            add_signature_dependency_node(graph, captured.clone());
            if let Some(source) = capture_source_dependency_node(
                constraints,
                &callable,
                &capture.name,
                capture.source,
            ) {
                add_bidirectional_signature_dependency_edge(
                    graph,
                    source,
                    captured,
                    SignatureDependencyEdgeKind::LocalAlias,
                );
            }
        }
    }
}

fn capture_owner_callable(
    constraints: &SignatureConstraintState,
    origin: &CaptureOrigin,
) -> Option<SignatureCallableKey> {
    match origin {
        CaptureOrigin::Method {
            type_name,
            method_name,
        } => method_key_for_usage(
            constraints,
            &UsageMethodKey {
                type_name: type_name.clone(),
                method_name: method_name.clone(),
            },
        )
        .map(SignatureCallableKey::Method),
        CaptureOrigin::Constructor { type_name } => constraints
            .constructors
            .keys()
            .find(|constructor| constructor.type_name == *type_name)
            .cloned()
            .map(SignatureCallableKey::Constructor),
        CaptureOrigin::Type { .. } => None,
    }
}

fn capture_source_dependency_node(
    constraints: &SignatureConstraintState,
    callable: &SignatureCallableKey,
    name: &str,
    source: CaptureSource,
) -> Option<SignatureDependencyNode> {
    match (callable, source) {
        (SignatureCallableKey::Method(method), CaptureSource::Parameter | CaptureSource::Local) => {
            method_binding_dependency_node(constraints, method, name)
        }
        (SignatureCallableKey::Method(method), CaptureSource::ThisField) => {
            Some(SignatureDependencyNode::Field {
                field: FieldKey {
                    type_name: method.type_name.clone(),
                    field_name: name.to_string(),
                },
            })
        }
        (
            SignatureCallableKey::Constructor(constructor),
            CaptureSource::Parameter | CaptureSource::Local,
        ) => constructor_binding_dependency_node(constraints, constructor, name),
        (SignatureCallableKey::Constructor(constructor), CaptureSource::ThisField) => {
            Some(SignatureDependencyNode::Field {
                field: FieldKey {
                    type_name: constructor.type_name.clone(),
                    field_name: name.to_string(),
                },
            })
        }
        (_, CaptureSource::OuterThis) => None,
    }
}

fn add_method_binding_dependency_edges(
    constraints: &SignatureConstraintState,
    method_usage: &MethodUsageFacts,
    method: &MethodKey,
    caller: &SignatureCallableKey,
    graph: &mut SignatureDependencyGraph,
) {
    for binding in method_usage.all_binding_facts() {
        let Some(binding_node) = method_binding_fact_dependency_node(constraints, method, binding)
        else {
            continue;
        };
        for use_site in &binding.uses {
            match use_site {
                BindingUse::AssignedToLocal {
                    local_name,
                    local_binding,
                } => {
                    let local = SignatureDependencyNode::MethodLocal {
                        method: method.clone(),
                        name: local_name.clone(),
                        binding: local_binding.clone(),
                    };
                    add_bidirectional_signature_dependency_edge(
                        graph,
                        binding_node.clone(),
                        local,
                        SignatureDependencyEdgeKind::LocalAlias,
                    );
                }
                BindingUse::PassedToCall {
                    name,
                    arg_index,
                    boundary,
                    ..
                } => {
                    for call_index in matching_method_call_indexes(method_usage, name, *boundary) {
                        add_call_argument_dependency_edges(
                            graph,
                            caller,
                            call_index,
                            *arg_index,
                            *boundary,
                            binding_node.clone(),
                            method_call_param_target(
                                constraints,
                                method_usage,
                                call_index,
                                *arg_index,
                            ),
                            SignatureDependencyEdgeKind::CallArgument,
                        );
                    }
                }
                BindingUse::ReceiverForCall { name, boundary, .. } => {
                    for call_index in matching_method_call_indexes(method_usage, name, *boundary) {
                        add_signature_dependency_edge(
                            graph,
                            binding_node.clone(),
                            SignatureDependencyNode::CallArg {
                                caller: caller.clone(),
                                call_index,
                                arg_index: usize::MAX,
                            },
                            SignatureDependencyEdgeKind::Receiver,
                        );
                    }
                }
                BindingUse::AssignedFromCall { name, boundary, .. } => {
                    for call_index in matching_method_call_indexes(method_usage, name, *boundary) {
                        add_signature_dependency_edge(
                            graph,
                            SignatureDependencyNode::CallResult {
                                caller: caller.clone(),
                                call_index,
                            },
                            binding_node.clone(),
                            SignatureDependencyEdgeKind::CallResult,
                        );
                    }
                }
                BindingUse::StoredInField { field_name } => {
                    add_bidirectional_signature_dependency_edge(
                        graph,
                        binding_node.clone(),
                        SignatureDependencyNode::Field {
                            field: FieldKey {
                                type_name: method.type_name.clone(),
                                field_name: field_name.clone(),
                            },
                        },
                        SignatureDependencyEdgeKind::FieldStorage,
                    );
                }
                BindingUse::Returned => {
                    add_bidirectional_signature_dependency_edge(
                        graph,
                        binding_node.clone(),
                        SignatureDependencyNode::MethodReturn {
                            method: method.clone(),
                        },
                        SignatureDependencyEdgeKind::MethodReturn,
                    );
                }
                BindingUse::ContentRead
                | BindingUse::InstanceOfTarget { .. }
                | BindingUse::Mutated => {}
            }
        }
    }
}

fn add_constructor_binding_dependency_edges(
    constraints: &SignatureConstraintState,
    constructor_usage: &MethodUsageFacts,
    constructor: &ConstructorKey,
    caller: &SignatureCallableKey,
    graph: &mut SignatureDependencyGraph,
) {
    for binding in constructor_usage.all_binding_facts() {
        let Some(binding_node) =
            constructor_binding_fact_dependency_node(constraints, constructor, binding)
        else {
            continue;
        };
        for use_site in &binding.uses {
            match use_site {
                BindingUse::AssignedToLocal {
                    local_name,
                    local_binding,
                } => {
                    let local = SignatureDependencyNode::ConstructorLocal {
                        constructor: constructor.clone(),
                        name: local_name.clone(),
                        binding: local_binding.clone(),
                    };
                    add_bidirectional_signature_dependency_edge(
                        graph,
                        binding_node.clone(),
                        local,
                        SignatureDependencyEdgeKind::LocalAlias,
                    );
                }
                BindingUse::PassedToCall {
                    name,
                    arg_index,
                    boundary,
                    ..
                } => {
                    for call_index in
                        matching_method_call_indexes(constructor_usage, name, *boundary)
                    {
                        add_call_argument_dependency_edges(
                            graph,
                            caller,
                            call_index,
                            *arg_index,
                            *boundary,
                            binding_node.clone(),
                            method_call_param_target(
                                constraints,
                                constructor_usage,
                                call_index,
                                *arg_index,
                            ),
                            SignatureDependencyEdgeKind::CallArgument,
                        );
                    }
                    for call_index in
                        matching_constructor_call_indexes(constructor_usage, name, *boundary)
                    {
                        add_call_argument_dependency_edges(
                            graph,
                            caller,
                            call_index,
                            *arg_index,
                            *boundary,
                            binding_node.clone(),
                            None,
                            SignatureDependencyEdgeKind::ConstructorDelegationArgument,
                        );
                    }
                }
                BindingUse::AssignedFromCall { name, boundary, .. } => {
                    for call_index in
                        matching_method_call_indexes(constructor_usage, name, *boundary)
                    {
                        add_signature_dependency_edge(
                            graph,
                            SignatureDependencyNode::CallResult {
                                caller: caller.clone(),
                                call_index,
                            },
                            binding_node.clone(),
                            SignatureDependencyEdgeKind::CallResult,
                        );
                    }
                }
                BindingUse::StoredInField { field_name } => {
                    add_bidirectional_signature_dependency_edge(
                        graph,
                        binding_node.clone(),
                        SignatureDependencyNode::Field {
                            field: FieldKey {
                                type_name: constructor.type_name.clone(),
                                field_name: field_name.clone(),
                            },
                        },
                        SignatureDependencyEdgeKind::FieldStorage,
                    );
                }
                BindingUse::ContentRead
                | BindingUse::InstanceOfTarget { .. }
                | BindingUse::ReceiverForCall { .. }
                | BindingUse::Returned
                | BindingUse::Mutated => {}
            }
        }
    }
}

fn add_method_field_dependency_edges(
    method_usage: &MethodUsageFacts,
    method: &MethodKey,
    caller: &SignatureCallableKey,
    graph: &mut SignatureDependencyGraph,
) {
    for field_use in &method_usage.field_uses {
        match field_use {
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary,
                ..
            } => {
                for call_index in matching_method_call_indexes(method_usage, name, *boundary) {
                    add_signature_dependency_edge(
                        graph,
                        SignatureDependencyNode::CallResult {
                            caller: caller.clone(),
                            call_index,
                        },
                        SignatureDependencyNode::Field {
                            field: FieldKey {
                                type_name: method.type_name.clone(),
                                field_name: field_name.clone(),
                            },
                        },
                        SignatureDependencyEdgeKind::CallResult,
                    );
                }
            }
            FieldUse::PassedToCall {
                field_name,
                name,
                arg_index,
                boundary,
                ..
            } => {
                let field = SignatureDependencyNode::Field {
                    field: FieldKey {
                        type_name: method.type_name.clone(),
                        field_name: field_name.clone(),
                    },
                };
                for call_index in matching_method_call_indexes(method_usage, name, *boundary) {
                    add_call_argument_dependency_edges(
                        graph,
                        caller,
                        call_index,
                        *arg_index,
                        *boundary,
                        field.clone(),
                        None,
                        SignatureDependencyEdgeKind::CallArgument,
                    );
                }
            }
            FieldUse::Returned { field_name } => {
                add_bidirectional_signature_dependency_edge(
                    graph,
                    SignatureDependencyNode::Field {
                        field: FieldKey {
                            type_name: method.type_name.clone(),
                            field_name: field_name.clone(),
                        },
                    },
                    SignatureDependencyNode::MethodReturn {
                        method: method.clone(),
                    },
                    SignatureDependencyEdgeKind::MethodReturn,
                );
            }
        }
    }
}

fn add_constructor_field_dependency_edges(
    constructor_usage: &MethodUsageFacts,
    constructor: &ConstructorKey,
    caller: &SignatureCallableKey,
    graph: &mut SignatureDependencyGraph,
) {
    for field_use in &constructor_usage.field_uses {
        match field_use {
            FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary,
                ..
            } => {
                for call_index in matching_method_call_indexes(constructor_usage, name, *boundary) {
                    add_signature_dependency_edge(
                        graph,
                        SignatureDependencyNode::CallResult {
                            caller: caller.clone(),
                            call_index,
                        },
                        SignatureDependencyNode::Field {
                            field: FieldKey {
                                type_name: constructor.type_name.clone(),
                                field_name: field_name.clone(),
                            },
                        },
                        SignatureDependencyEdgeKind::CallResult,
                    );
                }
            }
            FieldUse::PassedToCall {
                field_name,
                name,
                arg_index,
                boundary,
                ..
            } => {
                let field = SignatureDependencyNode::Field {
                    field: FieldKey {
                        type_name: constructor.type_name.clone(),
                        field_name: field_name.clone(),
                    },
                };
                for call_index in matching_method_call_indexes(constructor_usage, name, *boundary) {
                    add_call_argument_dependency_edges(
                        graph,
                        caller,
                        call_index,
                        *arg_index,
                        *boundary,
                        field.clone(),
                        None,
                        SignatureDependencyEdgeKind::CallArgument,
                    );
                }
            }
            FieldUse::Returned { .. } => {}
        }
    }
}

fn add_call_argument_dependency_edges(
    graph: &mut SignatureDependencyGraph,
    caller: &SignatureCallableKey,
    call_index: usize,
    arg_index: usize,
    boundary: CallArgumentBoundary,
    source: SignatureDependencyNode,
    callee_param: Option<SignatureDependencyNode>,
    kind: SignatureDependencyEdgeKind,
) {
    let call_arg = SignatureDependencyNode::CallArg {
        caller: caller.clone(),
        call_index,
        arg_index,
    };
    add_signature_dependency_edge(graph, source, call_arg.clone(), kind);
    if boundary == CallArgumentBoundary::Unknown {
        add_signature_dependency_edge(
            graph,
            call_arg,
            SignatureDependencyNode::UnknownBoundaryArg {
                caller: caller.clone(),
                call_index,
                arg_index,
            },
            SignatureDependencyEdgeKind::UnknownBoundary,
        );
        if let Some(callee_param) = callee_param
            && resolved_source_call_propagates_signature_constraints(boundary)
        {
            add_signature_dependency_edge(
                graph,
                SignatureDependencyNode::CallArg {
                    caller: caller.clone(),
                    call_index,
                    arg_index,
                },
                callee_param,
                kind,
            );
        }
    } else if let Some(callee_param) = callee_param {
        add_signature_dependency_edge(graph, call_arg, callee_param, kind);
    }
}

fn method_binding_dependency_node(
    constraints: &SignatureConstraintState,
    method: &MethodKey,
    binding_name: &str,
) -> Option<SignatureDependencyNode> {
    let method_constraints = constraints.methods.get(method)?;
    if let Some(index) = method_constraints.param_index(binding_name) {
        return Some(SignatureDependencyNode::MethodParam {
            method: method.clone(),
            name: binding_name.to_string(),
            index,
        });
    }
    if method_constraints.local(binding_name).is_some() {
        return Some(SignatureDependencyNode::MethodLocal {
            method: method.clone(),
            name: binding_name.to_string(),
            binding: None,
        });
    }
    None
}

fn method_binding_fact_dependency_node(
    constraints: &SignatureConstraintState,
    method: &MethodKey,
    binding: &BindingUseFacts,
) -> Option<SignatureDependencyNode> {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => {
            let method_constraints = constraints.methods.get(method)?;
            let index = method_constraints.param_index(&binding.name)?;
            Some(SignatureDependencyNode::MethodParam {
                method: method.clone(),
                name: binding.name.clone(),
                index,
            })
        }
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => {
            let local_binding = binding.binding.as_ref()?;
            let method_constraints = constraints.methods.get(method)?;
            method_constraints.local_by_binding(local_binding)?;
            Some(SignatureDependencyNode::MethodLocal {
                method: method.clone(),
                name: binding.name.clone(),
                binding: Some(local_binding.clone()),
            })
        }
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            method_binding_dependency_node(constraints, method, &binding.name)
        }
    }
}

fn constructor_binding_dependency_node(
    constraints: &SignatureConstraintState,
    constructor: &ConstructorKey,
    binding_name: &str,
) -> Option<SignatureDependencyNode> {
    let constructor_constraints = constraints.constructors.get(constructor)?;
    if let Some(index) = constructor_constraints.param_index(binding_name) {
        return Some(SignatureDependencyNode::ConstructorParam {
            constructor: constructor.clone(),
            name: binding_name.to_string(),
            index,
        });
    }
    if constructor_constraints.local(binding_name).is_some() {
        return Some(SignatureDependencyNode::ConstructorLocal {
            constructor: constructor.clone(),
            name: binding_name.to_string(),
            binding: None,
        });
    }
    None
}

fn constructor_binding_fact_dependency_node(
    constraints: &SignatureConstraintState,
    constructor: &ConstructorKey,
    binding: &BindingUseFacts,
) -> Option<SignatureDependencyNode> {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => {
            let constructor_constraints = constraints.constructors.get(constructor)?;
            let index = constructor_constraints.param_index(&binding.name)?;
            Some(SignatureDependencyNode::ConstructorParam {
                constructor: constructor.clone(),
                name: binding.name.clone(),
                index,
            })
        }
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => {
            let local_binding = binding.binding.as_ref()?;
            let constructor_constraints = constraints.constructors.get(constructor)?;
            constructor_constraints.local_by_binding(local_binding)?;
            Some(SignatureDependencyNode::ConstructorLocal {
                constructor: constructor.clone(),
                name: binding.name.clone(),
                binding: Some(local_binding.clone()),
            })
        }
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            constructor_binding_dependency_node(constraints, constructor, &binding.name)
        }
    }
}

fn method_call_param_target(
    constraints: &SignatureConstraintState,
    usage: &MethodUsageFacts,
    call_index: usize,
    arg_index: usize,
) -> Option<SignatureDependencyNode> {
    let resolved = usage.call_sites.get(call_index)?.resolved.as_ref()?;
    let method = MethodKey {
        type_name: resolved.declaring_type.clone(),
        rust_name: resolved.rust_name.clone(),
    };
    let name = constraints
        .methods
        .get(&method)?
        .param_names
        .get(arg_index)?
        .clone();
    Some(SignatureDependencyNode::MethodParam {
        method,
        name,
        index: arg_index,
    })
}

fn constructor_param_name_for_resolved(
    constraints: &SignatureConstraintState,
    resolved: &crate::java_ir::ResolvedConstructor,
    arg_index: usize,
) -> String {
    constraints
        .constructors
        .get(&ConstructorKey {
            type_name: resolved.declaring_type.clone(),
            rust_name: resolved.rust_name.clone(),
        })
        .and_then(|constructor| constructor.param_names.get(arg_index))
        .cloned()
        .unwrap_or_else(|| format!("arg{arg_index}"))
}

fn matching_method_call_indexes(
    usage: &MethodUsageFacts,
    name: &str,
    boundary: CallArgumentBoundary,
) -> Vec<usize> {
    usage
        .call_sites
        .iter()
        .enumerate()
        .filter(|(_, call_site)| call_site.name == name && call_site.boundary == boundary)
        .map(|(index, _)| index)
        .collect()
}

fn matching_constructor_call_indexes(
    usage: &MethodUsageFacts,
    name: &str,
    boundary: CallArgumentBoundary,
) -> Vec<usize> {
    usage
        .constructor_call_sites
        .iter()
        .enumerate()
        .filter(|(_, call_site)| call_site.name == name && call_site.boundary == boundary)
        .map(|(index, _)| constructor_call_dependency_index(usage, index))
        .collect()
}

fn constructor_call_dependency_index(usage: &MethodUsageFacts, index: usize) -> usize {
    usage.call_sites.len() + index
}

fn add_signature_dependency_node(
    graph: &mut SignatureDependencyGraph,
    node: SignatureDependencyNode,
) {
    graph.nodes.insert(node);
}

fn add_signature_dependency_edge(
    graph: &mut SignatureDependencyGraph,
    from: SignatureDependencyNode,
    to: SignatureDependencyNode,
    kind: SignatureDependencyEdgeKind,
) {
    add_signature_dependency_node(graph, from.clone());
    add_signature_dependency_node(graph, to.clone());
    let edge = SignatureDependencyEdge { from, to, kind };
    if !graph.edges.contains(&edge) {
        graph.edges.push(edge);
    }
}

fn add_bidirectional_signature_dependency_edge(
    graph: &mut SignatureDependencyGraph,
    left: SignatureDependencyNode,
    right: SignatureDependencyNode,
    kind: SignatureDependencyEdgeKind,
) {
    add_signature_dependency_edge(graph, left.clone(), right.clone(), kind);
    add_signature_dependency_edge(graph, right, left, kind);
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
    let dependency_graph = build_signature_dependency_graph(&constraints, usage);
    let dependency_sccs = analyze_signature_dependency_sccs(&dependency_graph);
    let pass_plan = plan_signature_solver_passes(&dependency_sccs);
    let saturation_policy = signature_fixpoint_saturation_policy(&constraints);
    let result = saturation_policy.saturate(
        constraints,
        |constraints| {
            let changed =
                execute_signature_solver_pass_plan(constraints, usage, &dependency_sccs, &pass_plan);
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
        dependency_sccs,
        pass_plan,
    }
}

pub fn signature_saturation_report(
    program: &Program,
    usage: &UsageFacts,
    facts: &FactSet,
) -> SignatureSaturationReport {
    let constraints = collect_signature_constraints(program, usage, facts);
    signature_saturation_report_for_constraints(constraints, usage)
}

pub fn signature_saturation_report_for_constraints(
    constraints: SignatureConstraintState,
    usage: &UsageFacts,
) -> SignatureSaturationReport {
    let domain_stats = signature_fixpoint_domain_stats(&constraints);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, usage);
    let conflict_diagnostics = signature_constraint_state_conflict_diagnostics(&result.constraints);
    SignatureSaturationReport {
        domain_stats,
        iterations: result.iterations,
        diagnostics: result.diagnostics.len(),
        dependency_stats: result.dependency_sccs.stats,
        pass_stats: result.pass_plan.stats,
        join_conflicts: conflict_diagnostics.len(),
        top_fallback_reasons: top_signature_constraint_provenance_counts(&result.constraints, 10),
        top_conflict_axes: top_signature_conflict_axis_counts(&conflict_diagnostics, 10),
    }
}

pub fn signature_constraint_state_conflict_diagnostics(
    constraints: &SignatureConstraintState,
) -> Vec<SignatureConstraintConflictDiagnostic> {
    let mut diagnostics = Vec::new();
    for slot_constraints in signature_constraint_slots(constraints) {
        diagnostics.extend(
            MergedSignatureConstraints::from_constraints(slot_constraints).conflict_diagnostics(),
        );
    }
    diagnostics
}

fn top_signature_constraint_provenance_counts(
    constraints: &SignatureConstraintState,
    limit: usize,
) -> Vec<SignatureReportCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for slot_constraints in signature_constraint_slots(constraints) {
        for constraint in slot_constraints {
            let label =
                signature_constraint_provenance_label(signature_constraint_provenance(constraint));
            *counts.entry(label.to_string()).or_default() += 1;
        }
    }
    top_signature_report_counts(counts, limit)
}

fn top_signature_conflict_axis_counts(
    diagnostics: &[SignatureConstraintConflictDiagnostic],
    limit: usize,
) -> Vec<SignatureReportCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for diagnostic in diagnostics {
        *counts.entry(format!("{:?}", diagnostic.axis)).or_default() += 1;
    }
    top_signature_report_counts(counts, limit)
}

fn top_signature_report_counts(
    counts: BTreeMap<String, usize>,
    limit: usize,
) -> Vec<SignatureReportCount> {
    let mut counts = counts
        .into_iter()
        .map(|(label, count)| SignatureReportCount { label, count })
        .collect::<Vec<_>>();
    counts.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    counts.truncate(limit);
    counts
}

fn execute_signature_solver_pass_plan(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    analysis: &SignatureDependencySccAnalysis,
    pass_plan: &SignatureSolverPassPlan,
) -> bool {
    execute_signature_solver_pass_plan_with_stats(constraints, usage, analysis, pass_plan, None).0
}

#[derive(Clone, Copy)]
struct SignatureSolverPassProgress<'a> {
    progress_sink: Option<SignatureSolverProgressSink<'a>>,
    fixpoint_start: std::time::Instant,
    iteration: usize,
}

fn execute_signature_solver_pass_plan_with_stats(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    analysis: &SignatureDependencySccAnalysis,
    pass_plan: &SignatureSolverPassPlan,
    progress: Option<SignatureSolverPassProgress<'_>>,
) -> (bool, SignatureSolverExecutionStats) {
    let mut changed = false;
    let mut stats = SignatureSolverExecutionStats::default();
    let mut mutable_component_count = 0usize;
    let total_mutable_components = pass_plan.mutable_components.len();
    for component_index in &pass_plan.component_order {
        if !pass_plan.mutable_components.contains(component_index) {
            continue;
        }
        mutable_component_count += 1;
        if let Some(progress) = progress
            && (mutable_component_count == 1
                || mutable_component_count % 250 == 0
                || mutable_component_count == total_mutable_components)
        {
            emit_signature_solver_progress(
                progress.progress_sink,
                "fixpoint_component",
                "running",
                progress.fixpoint_start.elapsed().as_millis(),
                &format!(
                    "iteration={} component={}/{} scc={}",
                    progress.iteration,
                    mutable_component_count,
                    total_mutable_components,
                    component_index
                ),
            );
        }
        let mut component_changed = true;
        while component_changed {
            stats.component_sweeps += 1;
            if let Some(progress) = progress
                && (stats.component_sweeps == 1 || stats.component_sweeps % 500 == 0)
            {
                emit_signature_solver_progress(
                    progress.progress_sink,
                    "fixpoint_component_sweep",
                    "running",
                    progress.fixpoint_start.elapsed().as_millis(),
                    &format!(
                        "iteration={} component_sweeps={} current_scc={}",
                        progress.iteration, stats.component_sweeps, component_index
                    ),
                );
            }
            let propagation_start = std::time::Instant::now();
            let scope = SignatureMutationScope {
                analysis,
                component_index: *component_index,
            };
            let (scoped_changed, propagation_stats) =
                apply_signature_propagation_sweep_with_stats(constraints, usage, Some(scope));
            stats.propagation_sweep_ms += propagation_start.elapsed().as_millis();
            stats.add_propagation_arm_stats(&propagation_stats);
            component_changed = scoped_changed;
            if component_changed {
                stats.changed_component_sweeps += 1;
            }
            changed |= component_changed;
            if !pass_plan.cyclic_components.contains(component_index) {
                break;
            }
        }
    }
    (changed, stats)
}

#[derive(Debug, Clone, Copy)]
struct SignatureMutationScope<'a> {
    analysis: &'a SignatureDependencySccAnalysis,
    component_index: usize,
}

impl SignatureMutationScope<'_> {
    fn permits(&self, node: &SignatureDependencyNode) -> bool {
        self.analysis
            .component_by_node
            .get(node)
            .is_some_and(|actual| *actual == self.component_index)
    }

    fn has_method_binding_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| {
                matches!(
                    node,
                    SignatureDependencyNode::MethodParam { .. }
                        | SignatureDependencyNode::MethodLocal { .. }
                )
            })
    }

    fn has_constructor_binding_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| {
                matches!(
                    node,
                    SignatureDependencyNode::ConstructorParam { .. }
                        | SignatureDependencyNode::ConstructorLocal { .. }
                )
            })
    }

    fn has_method_local_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| matches!(node, SignatureDependencyNode::MethodLocal { .. }))
    }

    fn has_constructor_local_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| matches!(node, SignatureDependencyNode::ConstructorLocal { .. }))
    }

    fn has_method_return_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| matches!(node, SignatureDependencyNode::MethodReturn { .. }))
    }

    fn has_field_target(&self) -> bool {
        self.analysis.components[self.component_index]
            .iter()
            .any(|node| matches!(node, SignatureDependencyNode::Field { .. }))
    }
}

fn scope_permits_method_local_target(
    constraints: &SignatureConstraintState,
    scope: Option<SignatureMutationScope<'_>>,
    method_key: &MethodKey,
    local_name: &str,
) -> bool {
    let Some(scope) = scope else {
        return true;
    };
    if constraints
        .methods
        .get(method_key)
        .and_then(|method_constraints| method_constraints.local(local_name))
        .is_none()
    {
        return false;
    }
    scope.permits(&SignatureDependencyNode::MethodLocal {
        method: method_key.clone(),
        name: local_name.to_string(),
        binding: None,
    })
}

fn scope_permits_method_return_target(
    scope: Option<SignatureMutationScope<'_>>,
    method_key: &MethodKey,
) -> bool {
    scope.is_none_or(|scope| {
        scope.permits(&SignatureDependencyNode::MethodReturn {
            method: method_key.clone(),
        })
    })
}

fn scope_permits_constructor_local_target(
    constraints: &SignatureConstraintState,
    scope: Option<SignatureMutationScope<'_>>,
    constructor_key: &ConstructorKey,
    local_name: &str,
) -> bool {
    let Some(scope) = scope else {
        return true;
    };
    if constraints
        .constructors
        .get(constructor_key)
        .and_then(|constructor_constraints| constructor_constraints.local(local_name))
        .is_none()
    {
        return false;
    }
    scope.permits(&SignatureDependencyNode::ConstructorLocal {
        constructor: constructor_key.clone(),
        name: local_name.to_string(),
        binding: None,
    })
}

fn scope_permits_field_target(
    scope: Option<SignatureMutationScope<'_>>,
    type_name: &str,
    field_name: &str,
) -> bool {
    scope.is_none_or(|scope| {
        scope.permits(&SignatureDependencyNode::Field {
            field: FieldKey {
                type_name: type_name.to_string(),
                field_name: field_name.to_string(),
            },
        })
    })
}

fn apply_signature_propagation_sweep_with_stats(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> (bool, BTreeMap<String, SignaturePropagationArmProfile>) {
    let mut stats = BTreeMap::new();
    let changed =
        record_signature_propagation_arm(&mut stats, "call_site", || {
            propagate_call_site_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "constructor_call_site", || {
            propagate_constructor_call_site_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(
            &mut stats,
            "resolved_source_mutable_argument_binding",
            || {
                propagate_resolved_source_mutable_argument_binding_constraints(
                    constraints,
                    usage,
                    scope,
                )
            },
        ) | record_signature_propagation_arm(
            &mut stats,
            "resolved_source_mutable_receiver_binding",
            || {
                propagate_resolved_source_mutable_receiver_binding_constraints(
                    constraints,
                    usage,
                    scope,
                )
            },
        ) | record_signature_propagation_arm(&mut stats, "constructor_delegation", || {
            propagate_constructor_delegation_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "call_argument_to_callee", || {
            propagate_call_argument_constraints_to_callees(constraints, usage, scope)
        }) | record_signature_propagation_arm(
            &mut stats,
            "call_argument_collection_carrier_to_callee",
            || {
                propagate_call_argument_collection_carrier_java_types_to_callees(
                    constraints,
                    usage,
                    scope,
                )
            },
        ) | record_signature_propagation_arm(
            &mut stats,
            "constructor_call_argument_to_callee",
            || {
                propagate_constructor_call_argument_constraints_to_callees(
                    constraints,
                    usage,
                    scope,
                )
            },
        ) | record_signature_propagation_arm(
            &mut stats,
            "constructor_delegation_argument_to_callee",
            || {
                propagate_constructor_delegation_argument_constraints_to_callees(
                    constraints,
                    usage,
                    scope,
                )
            },
        ) | record_signature_propagation_arm(&mut stats, "null_call_argument_to_callee", || {
            propagate_null_call_argument_constraints_to_callees(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "call_result_to_binding", || {
            propagate_call_result_constraints_to_bindings(constraints, usage, scope)
        }) | record_signature_propagation_arm(
            &mut stats,
            "constructor_call_result_to_binding",
            || propagate_constructor_call_result_constraints_to_bindings(constraints, usage, scope),
        ) | record_signature_propagation_arm(&mut stats, "call_result_to_field", || {
            propagate_call_result_constraints_to_fields(constraints, usage, scope)
        }) | record_signature_propagation_arm(
            &mut stats,
            "constructor_call_result_to_field",
            || propagate_constructor_call_result_constraints_to_fields(constraints, usage, scope),
        ) | record_signature_propagation_arm(&mut stats, "call_result_to_return", || {
            propagate_call_result_constraints_to_returns(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_field_storage", || {
            propagate_method_field_storage_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "constructor_field_storage", || {
            propagate_constructor_field_storage_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_local_alias", || {
            propagate_method_local_alias_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_local_alias_source", || {
            propagate_method_local_alias_source_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "constructor_local_alias", || {
            propagate_constructor_local_alias_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "constructor_local_alias_source", || {
            propagate_constructor_local_alias_source_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(
            &mut stats,
            "constructor_binding_field_nullability",
            || propagate_constructor_binding_field_nullability(constraints, usage, scope),
        ) | record_signature_propagation_arm(&mut stats, "method_return_binding", || {
            propagate_method_return_binding_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_binding_return", || {
            propagate_method_binding_return_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_return_field", || {
            propagate_method_return_field_constraints(constraints, usage, scope)
        }) | record_signature_propagation_arm(&mut stats, "method_field_return", || {
            propagate_method_field_return_constraints(constraints, usage, scope)
        });
    (changed, stats)
}

fn record_signature_propagation_arm(
    stats: &mut BTreeMap<String, SignaturePropagationArmProfile>,
    label: &str,
    propagate: impl FnOnce() -> bool,
) -> bool {
    let start = std::time::Instant::now();
    let changed = propagate();
    let arm = stats.entry(label.to_string()).or_default();
    arm.sweeps += 1;
    if changed {
        arm.changed_sweeps += 1;
    }
    arm.elapsed_ns += start.elapsed().as_nanos();
    changed
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

pub fn signature_fixpoint_domain_stats(
    constraints: &SignatureConstraintState,
) -> SignatureFixpointDomainStats {
    let slot_count = signature_constraint_slot_count(constraints);
    let unique_constraint_count = unique_signature_constraints(constraints).len();
    let iteration_budget =
        FiniteSaturationPolicy::new(slot_count, unique_constraint_count).iteration_budget();
    SignatureFixpointDomainStats {
        slot_count,
        unique_constraint_count,
        iteration_budget,
    }
}

fn signature_constraint_slot_count(constraints: &SignatureConstraintState) -> usize {
    signature_constraint_slots(constraints).len()
}

fn unique_signature_constraints(
    constraints: &SignatureConstraintState,
) -> Vec<SignatureConstraint> {
    let mut unique = Vec::new();
    for slot_constraints in signature_constraint_slots(constraints) {
        push_unique_signature_constraints(&mut unique, slot_constraints);
    }
    unique
}

fn signature_constraint_slots(
    constraints: &SignatureConstraintState,
) -> Vec<&Vec<SignatureConstraint>> {
    let mut slots = Vec::new();
    for method in constraints.methods.values() {
        slots.extend(method.params.iter());
        slots.push(&method.return_constraints);
        slots.extend(method.locals.iter().map(|local| &local.constraints));
    }
    for constructor in constraints.constructors.values() {
        slots.extend(constructor.params.iter());
        slots.extend(constructor.locals.iter().map(|local| &local.constraints));
    }
    slots.extend(constraints.fields.values());
    slots
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
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        if !constraints.methods.contains_key(&caller_key) {
            continue;
        }
        for call_site in &method_usage.call_sites {
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            let callee_constraints = constraints.methods.get(&callee_key);
            if let Some(callee_constraints) = callee_constraints {
                if call_site.result_returned {
                    for constraint in &callee_constraints.return_constraints {
                        if signature_shape_constraint_propagation_policy().allows(constraint) {
                            pending.push((
                                caller_key.clone(),
                                PendingSignatureConstraint::Return(constraint.clone()),
                            ));
                        }
                    }
                }
                for binding in method_usage.all_binding_facts() {
                    for usage in &binding.uses {
                        let BindingUse::PassedToCall {
                            name,
                            arg_index,
                            boundary,
                            ..
                        } = usage
                        else {
                            continue;
                        };
                        if name != &call_site.name || *boundary != call_site.boundary {
                            continue;
                        }
                        if *arg_index < callee_constraints.params.len() {
                            for constraint in propagated_callee_param_constraints_for_binding(
                                &callee_constraints.params[*arg_index],
                                &binding.java_ty,
                            ) {
                                if signature_shape_constraint_propagation_policy()
                                    .allows(constraint)
                                    || matches!(constraint, SignatureConstraint::MutableBinding)
                                {
                                    pending.push((
                                        caller_key.clone(),
                                        PendingSignatureConstraint::Binding {
                                            binding: binding.clone(),
                                            constraint: constraint.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                        for constraint in resolved_source_summary_arg_constraints(
                            &call_site.resolved,
                            *arg_index,
                            &binding.java_ty,
                        ) {
                            if signature_shape_constraint_propagation_policy().allows(&constraint) {
                                pending.push((
                                    caller_key.clone(),
                                    PendingSignatureConstraint::Binding {
                                        binding: binding.clone(),
                                        constraint,
                                    },
                                ));
                            }
                        }
                    }
                }
            } else {
                for binding in method_usage.all_binding_facts() {
                    for usage in &binding.uses {
                        let BindingUse::PassedToCall {
                            name,
                            arg_index,
                            boundary,
                            ..
                        } = usage
                        else {
                            continue;
                        };
                        if name != &call_site.name || *boundary != call_site.boundary {
                            continue;
                        }
                        for constraint in resolved_source_summary_arg_constraints(
                            &call_site.resolved,
                            *arg_index,
                            &binding.java_ty,
                        ) {
                            pending.push((
                                caller_key.clone(),
                                PendingSignatureConstraint::Binding {
                                    binding: binding.clone(),
                                    constraint,
                                },
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, constraint) in pending {
        changed |= match constraint {
            PendingSignatureConstraint::Binding {
                binding,
                constraint,
            } => add_method_binding_fact_constraint_scoped(
                constraints,
                &method_key,
                &binding,
                constraint,
                scope,
            ),
            PendingSignatureConstraint::Return(constraint) => {
                add_method_return_constraint_scoped(constraints, &method_key, constraint, scope)
            }
        };
    }
    changed
}

fn resolved_source_call_propagates_signature_constraints(boundary: CallArgumentBoundary) -> bool {
    matches!(
        boundary,
        CallArgumentBoundary::InternalSameType | CallArgumentBoundary::Unknown
    )
}

fn resolved_source_summary_arg_constraints(
    resolved: &Option<crate::java_ir::ResolvedCall>,
    arg_index: usize,
    binding_java_ty: &JavaType,
) -> Vec<SignatureConstraint> {
    let Some(target_ty) = resolved
        .as_ref()
        .and_then(|resolved| resolved_call_arg_target_type(resolved, arg_index))
    else {
        return Vec::new();
    };
    if !java_types_have_same_signature_boundary_identity(target_ty, binding_java_ty)
        || is_declared_java_object_type(target_ty)
        || matches!(target_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return Vec::new();
    }
    if matches!(target_ty, JavaType::String) {
        return vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
            SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
            SignatureConstraint::RequiredView(ViewShape::Str),
        ];
    }
    vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn propagate_constructor_call_site_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
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
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            for binding in constructor_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
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
                    for constraint in propagated_callee_param_constraints_for_binding(
                        &callee_constraints.params[*arg_index],
                        &binding.java_ty,
                    ) {
                        if signature_shape_constraint_propagation_policy().allows(constraint)
                            || matches!(constraint, SignatureConstraint::MutableBinding)
                        {
                            pending.push((
                                constructor_key.clone(),
                                binding.clone(),
                                constraint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding, constraint) in pending {
        changed |= add_constructor_binding_fact_constraint_scoped(
            constraints,
            &constructor_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_resolved_source_mutable_argument_binding_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        if !constraints.methods.contains_key(&caller_key) {
            continue;
        }
        for call_site in &method_usage.call_sites {
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
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            for binding in method_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
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
                    if callee_param_requires_mutable_binding(&callee_constraints.params[*arg_index])
                    {
                        pending.push((
                            caller_key.clone(),
                            binding.clone(),
                            SignatureConstraint::MutableBinding,
                        ));
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding, constraint) in pending {
        changed |= add_method_binding_fact_constraint_scoped(
            constraints,
            &method_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_resolved_source_mutable_receiver_binding_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        if !constraints.methods.contains_key(&caller_key) {
            continue;
        }
        for call_site in &method_usage.call_sites {
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
            if !callee_constraints
                .receiver
                .contains(&ReceiverConstraint::MutableReceiver)
            {
                continue;
            }
            for binding in method_usage.all_binding_facts() {
                if binding.uses.iter().any(|usage| {
                    matches!(
                        usage,
                        BindingUse::ReceiverForCall { name, boundary, .. }
                            if name == &call_site.name && *boundary == call_site.boundary
                    )
                }) {
                    pending.push((
                        caller_key.clone(),
                        binding.clone(),
                        SignatureConstraint::MutableBinding,
                    ));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding, constraint) in pending {
        changed |= add_method_binding_fact_constraint_scoped(
            constraints,
            &method_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn callee_param_requires_mutable_binding(constraints: &[SignatureConstraint]) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow)
        )
    }) && constraints
        .iter()
        .any(|constraint| matches!(constraint, SignatureConstraint::MutableBinding))
}

fn propagated_callee_param_constraints(
    constraints: &[SignatureConstraint],
) -> Vec<&SignatureConstraint> {
    let mutable_borrow_param = constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow)
        )
    }) && constraints
        .iter()
        .any(|constraint| matches!(constraint, SignatureConstraint::MutableBinding));
    constraints
        .iter()
        .filter(|constraint| {
            signature_shape_constraint_propagation_policy().allows(constraint)
                || (mutable_borrow_param
                    && matches!(constraint, SignatureConstraint::MutableBinding))
        })
        .collect()
}

fn propagated_callee_param_constraints_for_binding<'a>(
    constraints: &'a [SignatureConstraint],
    caller_binding_ty: &JavaType,
) -> Vec<&'a SignatureConstraint> {
    let callee_slot_is_erased_object = constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject)
                | SignatureConstraint::RequiredView(ViewShape::ErasedObjectView)
        )
    });
    propagated_callee_param_constraints(constraints)
        .into_iter()
        .filter(|constraint| {
            !callee_erased_object_slot_constraint_should_stay_local(
                constraint,
                caller_binding_ty,
                callee_slot_is_erased_object,
            )
        })
        .collect()
}

fn callee_erased_object_slot_constraint_should_stay_local(
    constraint: &SignatureConstraint,
    caller_binding_ty: &JavaType,
    callee_slot_is_erased_object: bool,
) -> bool {
    if is_declared_java_object_type(caller_binding_ty) || !callee_slot_is_erased_object {
        return false;
    }
    matches!(
        constraint,
        SignatureConstraint::RequiredOwnership(_)
            | SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject)
            | SignatureConstraint::RequiredView(ViewShape::ErasedObjectView)
            | SignatureConstraint::RequiredStorageNullability(_)
            | SignatureConstraint::StorageSlot
    )
}

fn propagate_constructor_delegation_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
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
            for binding in constructor_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
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
                                binding.clone(),
                                constraint.clone(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding, constraint) in pending {
        changed |= add_constructor_binding_fact_constraint_scoped(
            constraints,
            &constructor_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
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
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            for binding in method_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
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
                    let callee_param_constraints = &callee_constraints.params[*arg_index];
                    for constraint in caller_binding_constraints_for_callee_param(
                        method_binding_fact_slot_requirements(caller_constraints, binding),
                    ) {
                        if !should_propagate_caller_binding_constraint_to_callee_param(
                            &constraint,
                            callee_param_constraints,
                        ) {
                            continue;
                        }
                        pending.push((callee_key.clone(), *arg_index, constraint));
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
            changed |= add_method_param_constraint_scoped(
                constraints,
                &callee_key,
                &param_name,
                constraint,
                scope,
            );
        }
    }
    changed
}

fn propagate_call_argument_collection_carrier_java_types_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(caller_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(caller_constraints) = constraints.methods.get(&caller_key) else {
            continue;
        };
        for call_site in &method_usage.call_sites {
            let Some(resolved) = &call_site.resolved else {
                continue;
            };
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            let callee_key = MethodKey {
                type_name: resolved.declaring_type.clone(),
                rust_name: resolved.rust_name.clone(),
            };
            let Some(callee_constraints) = constraints.methods.get(&callee_key) else {
                continue;
            };
            if callee_constraints.public_api {
                continue;
            }
            for binding in method_usage.all_binding_facts() {
                for binding_use in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
                    } = binding_use
                    else {
                        continue;
                    };
                    if name != &call_site.name
                        || *boundary != call_site.boundary
                        || *arg_index >= callee_constraints.param_java_types.len()
                    {
                        continue;
                    }
                    let actual = effective_method_binding_java_type(caller_constraints, binding);
                    let target = &callee_constraints.param_java_types[*arg_index];
                    if let Some(refined) =
                        refined_collection_carrier_java_type_for_slot(&actual, target)
                    {
                        pending.push((callee_key.clone(), *arg_index, refined));
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (callee_key, arg_index, refined) in unique_unambiguous_param_java_type_refinements(pending)
    {
        let Some(callee_constraints) = constraints.methods.get_mut(&callee_key) else {
            continue;
        };
        let Some(current) = callee_constraints.param_java_types.get_mut(arg_index) else {
            continue;
        };
        if *current == refined {
            continue;
        }
        if refined_collection_carrier_java_type_for_slot(&refined, current).is_none() {
            continue;
        }
        *current = refined;
        changed = true;
    }
    changed
}

fn effective_method_binding_java_type(
    method_constraints: &MethodConstraintSet,
    binding: &BindingUseFacts,
) -> JavaType {
    if let Some(binding_id) = binding.binding.as_ref() {
        if let Some(local) = method_constraints.local_by_binding(binding_id) {
            return local.java_ty.clone();
        }
    } else if let Some(local) = method_constraints.local(&binding.name) {
        return local.java_ty.clone();
    }
    if let Some(index) = method_constraints.param_index(&binding.name)
        && let Some(java_ty) = method_constraints.param_java_types.get(index)
    {
        return java_ty.clone();
    }
    binding.java_ty.clone()
}

fn unique_unambiguous_param_java_type_refinements(
    pending: Vec<(MethodKey, usize, JavaType)>,
) -> Vec<(MethodKey, usize, JavaType)> {
    let mut grouped: Vec<(MethodKey, usize, Vec<JavaType>)> = Vec::new();
    for (key, index, java_ty) in pending {
        if let Some((_, _, types)) = grouped
            .iter_mut()
            .find(|(existing_key, existing_index, _)| {
                existing_key == &key && *existing_index == index
            })
        {
            if !types.contains(&java_ty) {
                types.push(java_ty);
            }
        } else {
            grouped.push((key, index, vec![java_ty]));
        }
    }
    grouped
        .into_iter()
        .filter_map(|(key, index, mut types)| {
            (types.len() == 1).then(|| (key, index, types.remove(0)))
        })
        .collect()
}

fn propagate_constructor_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
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
            if !resolved_source_call_propagates_signature_constraints(call_site.boundary) {
                continue;
            }
            for binding in constructor_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
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
                    let callee_param_constraints = &callee_constraints.params[*arg_index];
                    for constraint in caller_binding_constraints_for_callee_param(
                        constructor_binding_fact_slot_requirements(
                            constructor_constraints,
                            binding,
                        ),
                    ) {
                        if !should_propagate_caller_binding_constraint_to_callee_param(
                            &constraint,
                            callee_param_constraints,
                        ) {
                            continue;
                        }
                        pending.push((callee_key.clone(), *arg_index, constraint));
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
            changed |= add_method_param_constraint_scoped(
                constraints,
                &callee_key,
                &param_name,
                constraint,
                scope,
            );
        }
    }
    changed
}

fn should_propagate_caller_binding_constraint_to_callee_param(
    constraint: &SignatureConstraint,
    callee_param_constraints: &[SignatureConstraint],
) -> bool {
    if matches!(constraint, SignatureConstraint::StorageSlot)
        && callee_param_constraints.contains(&SignatureConstraint::StorageSlot)
    {
        return false;
    }
    if matches!(
        constraint,
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
    ) && callee_param_constraints.iter().any(|callee_constraint| {
        matches!(
            callee_constraint,
            SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
            )
        )
    }) {
        return false;
    }
    if matches!(
        constraint,
        SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow)
    ) && callee_param_constraints.iter().any(|callee_constraint| {
        matches!(
            callee_constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
        )
    }) && !callee_param_constraints.iter().any(|callee_constraint| {
        matches!(
            callee_constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow)
        )
    }) {
        return false;
    }
    true
}

fn caller_binding_constraints_for_callee_param(
    constraints: Vec<&SignatureConstraint>,
) -> Vec<SignatureConstraint> {
    let mut propagated = Vec::new();
    if constraints
        .iter()
        .any(|constraint| matches!(constraint, SignatureConstraint::StorageSlot))
    {
        push_unique_signature_constraints(
            &mut propagated,
            &[
                SignatureConstraint::StorageSlot,
                SignatureConstraint::RequiredView(ViewShape::Native),
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            ],
        );
    }
    for constraint in constraints {
        if signature_shape_constraint_propagation_policy().allows(constraint) {
            push_unique_signature_constraints(&mut propagated, &[constraint.clone()]);
        }
    }
    propagated
}

fn propagate_constructor_delegation_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_constructor_binding_target()) {
        return false;
    }
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
            for binding in constructor_usage.all_binding_facts() {
                for usage in &binding.uses {
                    let BindingUse::PassedToCall {
                        name,
                        arg_index,
                        boundary,
                        ..
                    } = usage
                    else {
                        continue;
                    };
                    if name != &call_site.name || *boundary != call_site.boundary {
                        continue;
                    }
                    for constraint in caller_binding_constraints_for_callee_param(
                        constructor_binding_fact_slot_requirements(
                            constructor_constraints,
                            binding,
                        ),
                    ) {
                        pending.push((callee_key.clone(), *arg_index, constraint));
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
            changed |= add_constructor_param_constraint_scoped(
                constraints,
                &callee_key,
                &param_name,
                constraint,
                scope,
            );
        }
    }
    changed
}

fn propagate_null_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let mut method_pending = Vec::new();
    let mut constructor_pending = Vec::new();
    for method_usage in usage.methods.values() {
        for call_site in &method_usage.call_sites {
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
            for arg_index in call_site
                .null_arg_indices
                .iter()
                .chain(call_site.nullable_source_call_arg_indices.iter())
            {
                method_pending.push((callee_key.clone(), *arg_index));
            }
        }
        for call_site in &method_usage.constructor_call_sites {
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
            for arg_index in call_site
                .null_arg_indices
                .iter()
                .chain(call_site.nullable_source_call_arg_indices.iter())
            {
                constructor_pending.push((callee_key.clone(), *arg_index));
            }
        }
    }
    for constructor_usage in usage.constructors.values() {
        for call_site in &constructor_usage.call_sites {
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
            for arg_index in call_site
                .null_arg_indices
                .iter()
                .chain(call_site.nullable_source_call_arg_indices.iter())
            {
                method_pending.push((callee_key.clone(), *arg_index));
            }
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
            for arg_index in call_site
                .null_arg_indices
                .iter()
                .chain(call_site.nullable_source_call_arg_indices.iter())
            {
                constructor_pending.push((callee_key.clone(), *arg_index));
            }
        }
    }

    let nullability = SignatureConstraint::RequiredStorageNullability(NullabilityState::MaybeNull);
    let mut changed = false;
    for (callee_key, arg_index) in method_pending {
        let param_name = constraints
            .methods
            .get(&callee_key)
            .and_then(|method_constraints| method_constraints.param_names.get(arg_index))
            .cloned();
        if let Some(param_name) = param_name {
            changed |= add_method_param_constraint_scoped(
                constraints,
                &callee_key,
                &param_name,
                nullability.clone(),
                scope,
            );
        }
    }
    for (callee_key, arg_index) in constructor_pending {
        let param_name = constraints
            .constructors
            .get(&callee_key)
            .and_then(|constructor_constraints| constructor_constraints.param_names.get(arg_index))
            .cloned();
        if let Some(param_name) = param_name {
            changed |= add_constructor_param_constraint_scoped(
                constraints,
                &callee_key,
                &param_name,
                nullability.clone(),
                scope,
            );
        }
    }
    changed
}

fn propagate_call_result_constraints_to_bindings(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for binding in method_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::AssignedFromCall { name, boundary, .. } = use_site else {
                    continue;
                };
                for constraint in internal_call_return_constraints(
                    constraints,
                    &method_usage.call_sites,
                    name,
                    *boundary,
                ) {
                    pending.push((method_key.clone(), binding.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding, constraint) in pending {
        let applied = add_method_binding_fact_constraint_scoped(
            constraints,
            &method_key,
            &binding,
            constraint.clone(),
            scope,
        );
        changed |= applied;
        if !applied
            && constraints
                .methods
                .get(&method_key)
                .and_then(|method_constraints| method_constraints.param_index(&binding.name))
                .is_some()
        {
            changed |=
                add_method_param_constraint(constraints, &method_key, &binding.name, constraint);
        }
    }
    changed
}

fn propagate_constructor_call_result_constraints_to_bindings(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_constructor_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        for binding in constructor_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::AssignedFromCall { name, boundary, .. } = use_site else {
                    continue;
                };
                for constraint in internal_call_return_constraints(
                    constraints,
                    &constructor_usage.call_sites,
                    name,
                    *boundary,
                ) {
                    pending.push((key.clone(), binding.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding, constraint) in pending {
        changed |= add_constructor_binding_fact_constraint_scoped(
            constraints,
            &constructor_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_call_result_constraints_to_fields(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_field_target()) {
        return false;
    }
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
                ..
            } = field_use
            else {
                continue;
            };
            if !scope_permits_field_target(scope, &method_key.type_name, field_name) {
                continue;
            }
            for constraint in internal_call_return_constraints(
                constraints,
                &method_usage.call_sites,
                name,
                *boundary,
            ) {
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
        changed |=
            add_field_constraint_scoped(constraints, &type_name, &field_name, constraint, scope);
    }
    changed
}

fn propagate_constructor_call_result_constraints_to_fields(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_field_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        for field_use in &constructor_usage.field_uses {
            let FieldUse::AssignedFromCall {
                field_name,
                name,
                boundary,
                ..
            } = field_use
            else {
                continue;
            };
            if !scope_permits_field_target(scope, &constructor_key.type_name, field_name) {
                continue;
            }
            for constraint in internal_call_return_constraints(
                constraints,
                &constructor_usage.call_sites,
                name,
                *boundary,
            ) {
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
        changed |=
            add_field_constraint_scoped(constraints, &type_name, &field_name, constraint, scope);
    }
    changed
}

fn propagate_call_result_constraints_to_returns(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_return_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        if !scope_permits_method_return_target(scope, &method_key) {
            continue;
        }
        for returned in &method_usage.returned_call_results {
            for constraint in internal_call_return_constraints(
                constraints,
                &method_usage.call_sites,
                &returned.name,
                returned.boundary,
            ) {
                pending.push((method_key.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, constraint) in pending {
        changed |= add_method_return_constraint_scoped(constraints, &method_key, constraint, scope);
    }
    changed
}

fn internal_call_return_constraints<'a>(
    constraints: &'a SignatureConstraintState,
    call_sites: &'a [crate::usage::CallSiteUse],
    name: &str,
    boundary: CallArgumentBoundary,
) -> Vec<&'a SignatureConstraint> {
    call_sites
        .iter()
        .filter(|call_site| call_site.name == name && call_site.boundary == boundary)
        .filter_map(|call_site| internal_call_site_method_constraints(constraints, call_site))
        .flat_map(|callee_constraints| callee_constraints.return_constraints.iter())
        .filter(|constraint| signature_shape_constraint_propagation_policy().allows(constraint))
        .collect()
}

fn internal_call_site_method_constraints<'a>(
    constraints: &'a SignatureConstraintState,
    call_site: &crate::usage::CallSiteUse,
) -> Option<&'a MethodConstraintSet> {
    if let Some(resolved) = call_site.resolved.as_ref() {
        return constraints.methods.get(&MethodKey {
            type_name: resolved.declaring_type.clone(),
            rust_name: resolved.rust_name.clone(),
        });
    }

    let receiver_type = call_site
        .receiver_type
        .as_ref()
        .and_then(java_type_name_for_signature_dispatch)?;
    let mut matches = constraints.methods.iter().filter_map(|(key, constraints)| {
        (crate::facts::external_type_name_matches(&key.type_name, receiver_type)
            && key.rust_name == call_site.name)
            .then_some(constraints)
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

enum PendingSignatureConstraint {
    Binding {
        binding: BindingUseFacts,
        constraint: SignatureConstraint,
    },
    Return(SignatureConstraint),
}

fn propagate_method_local_alias_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    _scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let mut changed = false;
    loop {
        let mut pending = Vec::new();
        for (usage_key, method_usage) in &usage.methods {
            let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
                continue;
            };
            let Some(method_constraints) = constraints.methods.get(&method_key) else {
                continue;
            };
            for binding in method_usage.all_binding_facts() {
                for use_site in &binding.uses {
                    let BindingUse::AssignedToLocal {
                        local_name,
                        local_binding,
                    } = use_site
                    else {
                        continue;
                    };
                    for constraint in local_slot_requirements_for_assignment(
                        &method_constraints.locals,
                        local_name,
                        local_binding.as_ref(),
                    ) {
                        pending.push((method_key.clone(), binding.clone(), constraint.clone()));
                    }
                }
            }
        }
        let mut round_changed = false;
        for (method_key, binding, constraint) in pending {
            round_changed |= match binding.binding.as_ref().map(|binding| binding.kind) {
                Some(BindingKind::Param) => {
                    add_method_param_constraint(constraints, &method_key, &binding.name, constraint)
                }
                Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => {
                    add_method_local_constraint_by_binding(
                        constraints,
                        &method_key,
                        binding.binding.as_ref().expect("local binding id"),
                        constraint,
                    )
                }
                Some(BindingKind::Field | BindingKind::StaticField) | None => {
                    add_method_param_constraint(
                        constraints,
                        &method_key,
                        &binding.name,
                        constraint.clone(),
                    ) | add_method_local_constraint(
                        constraints,
                        &method_key,
                        &binding.name,
                        constraint,
                    )
                }
            };
        }
        if !round_changed {
            break;
        }
        changed = true;
    }
    changed
}

fn propagate_constructor_local_alias_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_constructor_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&key) else {
            continue;
        };
        for binding in constructor_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal {
                    local_name,
                    local_binding,
                } = use_site
                else {
                    continue;
                };
                for constraint in local_slot_requirements_for_assignment(
                    &constructor_constraints.locals,
                    local_name,
                    local_binding.as_ref(),
                ) {
                    pending.push((key.clone(), binding.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding, constraint) in pending {
        changed |= add_constructor_binding_fact_constraint_scoped(
            constraints,
            &constructor_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_method_local_alias_source_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_local_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal {
                    local_name,
                    local_binding,
                } = use_site
                else {
                    continue;
                };
                if local_binding.is_none()
                    && !scope_permits_method_local_target(
                        constraints,
                        scope,
                        &method_key,
                        local_name,
                    )
                {
                    continue;
                }
                for constraint in method_binding_fact_slot_requirements(method_constraints, binding)
                {
                    pending.push((
                        method_key.clone(),
                        local_name.clone(),
                        local_binding.clone(),
                        constraint.clone(),
                    ));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, local_name, local_binding, constraint) in pending {
        changed |= if let Some(local_binding) = local_binding {
            add_method_local_constraint_by_binding_scoped(
                constraints,
                &method_key,
                &local_binding,
                &local_name,
                constraint,
                scope,
            )
        } else {
            add_method_local_constraint_scoped(
                constraints,
                &method_key,
                &local_name,
                constraint,
                scope,
            )
        };
    }
    changed
}

fn propagate_constructor_local_alias_source_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_constructor_local_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&key) else {
            continue;
        };
        for binding in constructor_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::AssignedToLocal {
                    local_name,
                    local_binding,
                } = use_site
                else {
                    continue;
                };
                if local_binding.is_none()
                    && !scope_permits_constructor_local_target(constraints, scope, &key, local_name)
                {
                    continue;
                }
                for constraint in
                    constructor_binding_fact_slot_requirements(constructor_constraints, binding)
                {
                    pending.push((
                        key.clone(),
                        local_name.clone(),
                        local_binding.clone(),
                        constraint.clone(),
                    ));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, local_name, local_binding, constraint) in pending {
        changed |= if let Some(local_binding) = local_binding {
            add_constructor_local_constraint_by_binding_scoped(
                constraints,
                &constructor_key,
                &local_binding,
                &local_name,
                constraint,
                scope,
            )
        } else {
            add_constructor_local_constraint_scoped(
                constraints,
                &constructor_key,
                &local_name,
                constraint,
                scope,
            )
        };
    }
    changed
}

fn propagate_method_field_storage_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        for binding in method_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::StoredInField { field_name } = use_site else {
                    continue;
                };
                for constraint in
                    field_writer_shape_requirements(constraints, &method_key.type_name, field_name)
                {
                    pending.push((method_key.clone(), binding.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (method_key, binding, constraint) in pending {
        changed |= add_method_binding_fact_constraint_scoped(
            constraints,
            &method_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_constructor_field_storage_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_constructor_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        for binding in constructor_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::StoredInField { field_name } = use_site else {
                    continue;
                };
                for constraint in field_writer_shape_requirements(
                    constraints,
                    &constructor_key.type_name,
                    field_name,
                ) {
                    pending.push((key.clone(), binding.clone(), constraint.clone()));
                }
            }
        }
    }
    let mut changed = false;
    for (constructor_key, binding, constraint) in pending {
        changed |= add_constructor_binding_fact_constraint_scoped(
            constraints,
            &constructor_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_constructor_binding_field_nullability(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_field_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (constructor_key, constructor_usage) in &usage.constructors {
        let key = ConstructorKey {
            type_name: constructor_key.type_name.clone(),
            rust_name: constructor_key.rust_name.clone(),
        };
        let Some(constructor_constraints) = constraints.constructors.get(&key) else {
            continue;
        };
        for binding in constructor_usage.all_binding_facts() {
            for use_site in &binding.uses {
                let BindingUse::StoredInField { field_name } = use_site else {
                    continue;
                };
                if !scope_permits_field_target(scope, &constructor_key.type_name, field_name) {
                    continue;
                }
                for constraint in
                    constructor_binding_fact_slot_requirements(constructor_constraints, binding)
                {
                    if matches!(
                        constraint,
                        SignatureConstraint::RequiredStorageNullability(
                            NullabilityState::DefinitelyNull | NullabilityState::MaybeNull
                        )
                    ) {
                        pending.push((
                            constructor_key.type_name.clone(),
                            field_name.clone(),
                            constraint.clone(),
                        ));
                    }
                }
            }
        }
    }
    let mut changed = false;
    for (type_name, field_name, constraint) in pending {
        changed |=
            add_field_constraint_scoped(constraints, &type_name, &field_name, constraint, scope);
    }
    changed
}

fn propagate_method_return_binding_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_binding_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        if !scope_permits_method_return_target(scope, &method_key) {
            continue;
        }
        for binding in method_usage.all_binding_facts() {
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
                pending.push((method_key.clone(), binding.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, binding, constraint) in pending {
        changed |= add_method_binding_fact_constraint_scoped(
            constraints,
            &method_key,
            &binding,
            constraint,
            scope,
        );
    }
    changed
}

fn propagate_method_binding_return_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_return_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        let Some(method_constraints) = constraints.methods.get(&method_key) else {
            continue;
        };
        for binding in method_usage.all_binding_facts() {
            if !binding
                .uses
                .iter()
                .any(|use_site| matches!(use_site, BindingUse::Returned))
            {
                continue;
            }
            for constraint in method_binding_fact_slot_requirements(method_constraints, binding) {
                pending.push((method_key.clone(), constraint.clone()));
            }
        }
    }
    let mut changed = false;
    for (method_key, constraint) in pending {
        changed |= add_method_return_constraint_scoped(constraints, &method_key, constraint, scope);
    }
    changed
}

fn propagate_method_return_field_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_field_target()) {
        return false;
    }
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
            if !scope_permits_field_target(scope, &method_key.type_name, field_name) {
                continue;
            }
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
        changed |=
            add_field_constraint_scoped(constraints, &type_name, &field_name, constraint, scope);
    }
    changed
}

fn propagate_method_field_return_constraints(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if scope.is_some_and(|scope| !scope.has_method_return_target()) {
        return false;
    }
    let mut pending = Vec::new();
    for (usage_key, method_usage) in &usage.methods {
        let Some(method_key) = method_key_for_usage(constraints, usage_key) else {
            continue;
        };
        if !scope_permits_method_return_target(scope, &method_key) {
            continue;
        }
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
        changed |= add_method_return_constraint_scoped(constraints, &method_key, constraint, scope);
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

fn field_writer_shape_requirements<'a>(
    constraints: &'a SignatureConstraintState,
    type_name: &str,
    field_name: &str,
) -> impl Iterator<Item = &'a SignatureConstraint> {
    field_shape_requirements(constraints, type_name, field_name).filter(|constraint| {
        !matches!(
            constraint,
            SignatureConstraint::RequiredStorageNullability(_)
        )
    })
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

fn local_slot_requirements_for_assignment<'a>(
    locals: &'a [LocalConstraintSet],
    local_name: &str,
    local_binding: Option<&BindingId>,
) -> Vec<&'a SignatureConstraint> {
    if let Some(local_binding) = local_binding {
        return locals
            .iter()
            .find(|local| local.binding == *local_binding)
            .into_iter()
            .flat_map(|local| local.constraints.iter())
            .filter(|constraint| signature_slot_constraint_propagation_policy().allows(constraint))
            .collect();
    }
    let _ = local_name;
    Vec::new()
}

fn binding_slot_requirements<'a>(
    method_constraints: &'a MethodConstraintSet,
    binding_name: &str,
) -> Vec<&'a SignatureConstraint> {
    let mut requirements = Vec::new();
    if let Some(param_index) = method_constraints.param_index(binding_name) {
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

fn method_binding_fact_slot_requirements<'a>(
    method_constraints: &'a MethodConstraintSet,
    binding: &BindingUseFacts,
) -> Vec<&'a SignatureConstraint> {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => method_constraints
            .param_index(&binding.name)
            .map(|param_index| {
                method_constraints.params[param_index]
                    .iter()
                    .filter(|constraint| {
                        signature_slot_constraint_propagation_policy().allows(constraint)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => binding
            .binding
            .as_ref()
            .and_then(|binding| method_constraints.local_by_binding(binding))
            .map(|local| {
                local
                    .constraints
                    .iter()
                    .filter(|constraint| {
                        signature_slot_constraint_propagation_policy().allows(constraint)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            binding_slot_requirements(method_constraints, &binding.name)
        }
    }
}

fn constructor_binding_slot_requirements<'a>(
    constructor_constraints: &'a ConstructorConstraintSet,
    binding_name: &str,
) -> Vec<&'a SignatureConstraint> {
    let mut requirements = Vec::new();
    if let Some(param_index) = constructor_constraints.param_index(binding_name) {
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

fn constructor_binding_fact_slot_requirements<'a>(
    constructor_constraints: &'a ConstructorConstraintSet,
    binding: &BindingUseFacts,
) -> Vec<&'a SignatureConstraint> {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => constructor_constraints
            .param_index(&binding.name)
            .map(|param_index| {
                constructor_constraints.params[param_index]
                    .iter()
                    .filter(|constraint| {
                        signature_slot_constraint_propagation_policy().allows(constraint)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => binding
            .binding
            .as_ref()
            .and_then(|binding| constructor_constraints.local_by_binding(binding))
            .map(|local| {
                local
                    .constraints
                    .iter()
                    .filter(|constraint| {
                        signature_slot_constraint_propagation_policy().allows(constraint)
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            constructor_binding_slot_requirements(constructor_constraints, &binding.name)
        }
    }
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

fn add_field_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    type_name: &str,
    field_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let field = FieldKey {
        type_name: type_name.to_string(),
        field_name: field_name.to_string(),
    };
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::Field {
            field: field.clone(),
        })
    {
        return false;
    }
    add_field_constraint(constraints, type_name, field_name, constraint)
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
    let Some(param_index) = method_constraints.param_index(param_name) else {
        return false;
    };
    let param_constraints = &mut method_constraints.params[param_index];
    if borrowed_slice_param_rejects_constraint(param_constraints, &constraint) {
        return false;
    }
    let normalized =
        normalize_borrowed_slice_param_constraints_after_insert(param_constraints, &constraint);
    if param_constraints.contains(&constraint) {
        normalized
    } else {
        param_constraints.push(constraint);
        true
    }
}

fn normalize_borrowed_slice_param_constraints_after_insert(
    constraints: &mut Vec<SignatureConstraint>,
    incoming: &SignatureConstraint,
) -> bool {
    let mut projected = constraints.clone();
    if !projected.contains(incoming) {
        projected.push(incoming.clone());
    }
    if !borrowed_slice_param_constraints(&projected) {
        return false;
    }
    let before = constraints.len();
    constraints
        .retain(|constraint| !borrowed_slice_param_rejects_constraint(&projected, constraint));
    constraints.len() != before
}

fn normalize_borrowed_slice_param_constraints(constraints: &mut Vec<SignatureConstraint>) {
    if !borrowed_slice_param_constraints(constraints) {
        return;
    }
    let projected = constraints.clone();
    constraints
        .retain(|constraint| !borrowed_slice_param_rejects_constraint(&projected, constraint));
}

fn normalize_param_borrowed_slice_constraints(
    param: &Param,
    constraints: &mut Vec<SignatureConstraint>,
) {
    if param.is_varargs && java_varargs_erased_object_carrier_type(&param.ty) {
        return;
    }
    if param.ty == JavaType::String {
        normalize_declared_string_param_constraints(constraints);
        return;
    }
    normalize_borrowed_slice_param_constraints(constraints);
}

fn normalize_declared_string_param_constraints(constraints: &mut Vec<SignatureConstraint>) {
    let preserve_borrowed_str = constraints.contains(&SignatureConstraint::RequiredOwnership(
        OwnershipShape::SharedBorrow,
    )) && constraints
        .contains(&SignatureConstraint::RequiredView(ViewShape::Str));
    constraints.retain(|constraint| {
        !matches!(
            constraint,
            SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject)
                | SignatureConstraint::RequiredView(ViewShape::ErasedObjectView)
                | SignatureConstraint::RequiredView(ViewShape::Slice)
        )
    });
    push_unique_signature_constraints(
        constraints,
        &[SignatureConstraint::RequiredDispatch(
            DispatchShape::Concrete,
        )],
    );
    if !preserve_borrowed_str {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredView(ViewShape::Native)],
        );
    }
}

fn borrowed_slice_param_rejects_constraint(
    constraints: &[SignatureConstraint],
    constraint: &SignatureConstraint,
) -> bool {
    if constraints.contains(&SignatureConstraint::StorageSlot) {
        return matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow)
                | SignatureConstraint::RequiredView(ViewShape::Slice)
        );
    }
    match constraint {
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned) => {
            borrowed_slice_param_constraints(constraints)
                || mutable_borrow_param_constraints(constraints)
        }
        SignatureConstraint::RequiredView(ViewShape::Native) => {
            borrowed_slice_param_constraints(constraints)
        }
        SignatureConstraint::StorageSlot => false,
        _ => false,
    }
}

fn borrowed_slice_param_constraints(constraints: &[SignatureConstraint]) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredView(ViewShape::Slice)
        )
    }) && constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
            )
        )
    })
}

fn mutable_borrow_param_constraints(constraints: &[SignatureConstraint]) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow)
        )
    })
}

fn add_method_param_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    param_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let Some(param_index) = constraints
        .methods
        .get(method_key)
        .and_then(|method_constraints| method_constraints.param_index(param_name))
    else {
        return false;
    };
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::MethodParam {
            method: method_key.clone(),
            name: param_name.to_string(),
            index: param_index,
        })
    {
        return false;
    }
    add_method_param_constraint(constraints, method_key, param_name, constraint)
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

fn add_method_return_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::MethodReturn {
            method: method_key.clone(),
        })
    {
        return false;
    }
    add_method_return_constraint(constraints, method_key, constraint)
}

fn add_method_binding_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    binding_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    add_method_param_constraint_scoped(
        constraints,
        method_key,
        binding_name,
        constraint.clone(),
        scope,
    ) | add_method_local_constraint_scoped(constraints, method_key, binding_name, constraint, scope)
}

fn add_method_binding_fact_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    binding: &BindingUseFacts,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => add_method_param_constraint_scoped(
            constraints,
            method_key,
            &binding.name,
            constraint,
            scope,
        ),
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => {
            add_method_local_constraint_by_binding_scoped(
                constraints,
                method_key,
                binding.binding.as_ref().expect("local binding id"),
                &binding.name,
                constraint,
                scope,
            )
        }
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            add_method_binding_constraint_scoped(
                constraints,
                method_key,
                &binding.name,
                constraint,
                scope,
            )
        }
    }
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
    let Some(local_constraints) = method_constraints.local_mut(local_name) else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn add_method_local_constraint_by_binding(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    binding: &BindingId,
    constraint: SignatureConstraint,
) -> bool {
    let Some(method_constraints) = constraints.methods.get_mut(method_key) else {
        return false;
    };
    let Some(local_constraints) = method_constraints.local_mut_by_binding(binding) else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn add_method_local_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    local_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if constraints
        .methods
        .get(method_key)
        .and_then(|method_constraints| method_constraints.local(local_name))
        .is_none()
    {
        return false;
    }
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::MethodLocal {
            method: method_key.clone(),
            name: local_name.to_string(),
            binding: None,
        })
    {
        return false;
    }
    add_method_local_constraint(constraints, method_key, local_name, constraint)
}

fn add_method_local_constraint_by_binding_scoped(
    constraints: &mut SignatureConstraintState,
    method_key: &MethodKey,
    binding: &BindingId,
    local_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if constraints
        .methods
        .get(method_key)
        .and_then(|method_constraints| method_constraints.local_by_binding(binding))
        .is_none()
    {
        return false;
    }
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::MethodLocal {
            method: method_key.clone(),
            name: local_name.to_string(),
            binding: Some(binding.clone()),
        })
        && !scope.permits(&SignatureDependencyNode::MethodLocal {
            method: method_key.clone(),
            name: local_name.to_string(),
            binding: None,
        })
    {
        return false;
    }
    add_method_local_constraint_by_binding(constraints, method_key, binding, constraint)
}

fn add_constructor_binding_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    binding_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    add_constructor_param_constraint_scoped(
        constraints,
        constructor_key,
        binding_name,
        constraint.clone(),
        scope,
    ) | add_constructor_local_constraint_scoped(
        constraints,
        constructor_key,
        binding_name,
        constraint,
        scope,
    )
}

fn add_constructor_binding_fact_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    binding: &BindingUseFacts,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    match binding.binding.as_ref().map(|binding| binding.kind) {
        Some(BindingKind::Param) => add_constructor_param_constraint_scoped(
            constraints,
            constructor_key,
            &binding.name,
            constraint,
            scope,
        ),
        Some(BindingKind::Local | BindingKind::LoopVariable | BindingKind::CatchParam) => {
            add_constructor_local_constraint_by_binding_scoped(
                constraints,
                constructor_key,
                binding.binding.as_ref().expect("local binding id"),
                &binding.name,
                constraint,
                scope,
            )
        }
        Some(BindingKind::Field | BindingKind::StaticField) | None => {
            add_constructor_binding_constraint_scoped(
                constraints,
                constructor_key,
                &binding.name,
                constraint,
                scope,
            )
        }
    }
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
    let Some(param_index) = constructor_constraints.param_index(param_name) else {
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

fn add_constructor_param_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    param_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    let Some(param_index) = constraints
        .constructors
        .get(constructor_key)
        .and_then(|constructor_constraints| constructor_constraints.param_index(param_name))
    else {
        return false;
    };
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::ConstructorParam {
            constructor: constructor_key.clone(),
            name: param_name.to_string(),
            index: param_index,
        })
    {
        return false;
    }
    add_constructor_param_constraint(constraints, constructor_key, param_name, constraint)
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
    let Some(local_constraints) = constructor_constraints.local_mut(local_name) else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn add_constructor_local_constraint_by_binding(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    binding: &BindingId,
    constraint: SignatureConstraint,
) -> bool {
    let Some(constructor_constraints) = constraints.constructors.get_mut(constructor_key) else {
        return false;
    };
    let Some(local_constraints) = constructor_constraints.local_mut_by_binding(binding) else {
        return false;
    };
    if local_constraints.constraints.contains(&constraint) {
        false
    } else {
        local_constraints.constraints.push(constraint);
        true
    }
}

fn add_constructor_local_constraint_scoped(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    local_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if constraints
        .constructors
        .get(constructor_key)
        .and_then(|constructor_constraints| constructor_constraints.local(local_name))
        .is_none()
    {
        return false;
    }
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::ConstructorLocal {
            constructor: constructor_key.clone(),
            name: local_name.to_string(),
            binding: None,
        })
    {
        return false;
    }
    add_constructor_local_constraint(constraints, constructor_key, local_name, constraint)
}

fn add_constructor_local_constraint_by_binding_scoped(
    constraints: &mut SignatureConstraintState,
    constructor_key: &ConstructorKey,
    binding: &BindingId,
    local_name: &str,
    constraint: SignatureConstraint,
    scope: Option<SignatureMutationScope<'_>>,
) -> bool {
    if constraints
        .constructors
        .get(constructor_key)
        .and_then(|constructor_constraints| constructor_constraints.local_by_binding(binding))
        .is_none()
    {
        return false;
    }
    if let Some(scope) = scope
        && !scope.permits(&SignatureDependencyNode::ConstructorLocal {
            constructor: constructor_key.clone(),
            name: local_name.to_string(),
            binding: Some(binding.clone()),
        })
        && !scope.permits(&SignatureDependencyNode::ConstructorLocal {
            constructor: constructor_key.clone(),
            name: local_name.to_string(),
            binding: None,
        })
    {
        return false;
    }
    add_constructor_local_constraint_by_binding(constraints, constructor_key, binding, constraint)
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
        if let Some(param) = method.params.get(index) {
            push_constraint_conflict_diagnostics(
                diagnostics,
                format!(
                    "{}::{} parameter {}",
                    method_key.type_name, method_key.rust_name, param_name
                ),
                param_constraints,
            );
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
        if let Some(param) = ctor.params.get(index) {
            push_constraint_conflict_diagnostics(
                diagnostics,
                format!(
                    "{}::{} parameter {}",
                    constructor_key.type_name, constructor_key.rust_name, param_name
                ),
                param_constraints,
            );
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
    for conflict in merged.conflict_diagnostics() {
        diagnostics.push(SignatureDiagnostic {
            code: "signature_constraint_conflict",
            message: format!(
                "{slot} has conflicting {:?} constraints; kept {:?}, rejected {:?}; {}",
                conflict.axis, conflict.kept, conflict.rejected, conflict.message
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
        | SignatureOwnedRecoveryPolicy::ProvenByExplicitClone
        | SignatureOwnedRecoveryPolicy::ProvenByCloneBoxAdapter
        | SignatureOwnedRecoveryPolicy::ProvenByFactoryCopy
        | SignatureOwnedRecoveryPolicy::ProvenByImmutableSharedHandle => {}
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
        SignatureOwnedRecoveryPolicy::UnsupportedFacadeOwnedExternalHandle => {
            diagnostics.push(SignatureDiagnostic {
                code: "signature_owned_recovery_requires_facade_transfer",
                message: format!(
                    "{slot} needs owned recovery from a borrowed facade-owned external handle; {java_ty:?} requires an explicit facade ownership transfer"
                ),
            });
        }
        SignatureOwnedRecoveryPolicy::UnsupportedFfiOwnedHandle => {
            diagnostics.push(SignatureDiagnostic {
                code: "signature_owned_recovery_requires_ffi_transfer",
                message: format!(
                    "{slot} needs owned recovery from a borrowed FFI-owned handle; {java_ty:?} requires an explicit FFI ownership transfer"
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

pub fn signature_owned_recovery_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureOwnedRecoveryCapabilityPolicy {
    SignatureOwnedRecoveryCapabilityPolicy {
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
        OwnedRecoveryCapability::CloneBoxAdapter { .. } => {
            SignatureOwnedRecoveryPolicy::ProvenByCloneBoxAdapter
        }
        OwnedRecoveryCapability::FactoryCopy { .. } => {
            SignatureOwnedRecoveryPolicy::ProvenByFactoryCopy
        }
        OwnedRecoveryCapability::ImmutableSharedHandle { .. } => {
            SignatureOwnedRecoveryPolicy::ProvenByImmutableSharedHandle
        }
        OwnedRecoveryCapability::ConservativeClone => {
            SignatureOwnedRecoveryPolicy::AuditConservativeClone
        }
        OwnedRecoveryCapability::FacadeOwnedExternalHandle { .. } => {
            SignatureOwnedRecoveryPolicy::UnsupportedFacadeOwnedExternalHandle
        }
        OwnedRecoveryCapability::FfiOwnedHandle { .. } => {
            SignatureOwnedRecoveryPolicy::UnsupportedFfiOwnedHandle
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
    support_program: &Program,
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
        collect_type_signatures(
            ty,
            support_program,
            representations,
            constraints,
            facts,
            &mut plan,
        );
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
        let key = MethodKey {
            type_name: ty.name.clone(),
            rust_name: method_rust_name(method),
        };
        self.methods
            .get(&key)
            .or_else(|| self.unique_method_signature_for_simple_owner(&key))
            .unwrap_or_else(|| {
            panic!(
                "signature solver must plan every lowered method: missing {}::{}, available for type: [{}], nearby types: [{}]",
                key.type_name,
                key.rust_name,
                self.available_method_names_for_type(&key.type_name),
                self.nearby_method_type_names(&key.type_name)
            )
        })
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

    pub fn method_signature_by_java_key(
        &self,
        key: &JavaCallableKey,
    ) -> Option<&MethodSignaturePlan> {
        self.methods_by_java_key.get(key)
    }

    pub fn method_signature_by_base_name_and_arity(
        &self,
        type_name: &str,
        base_name: &str,
        arity: usize,
    ) -> Option<(String, &MethodSignaturePlan)> {
        if method_signature_owner_query_is_ambiguous(self.methods.keys(), type_name) {
            return None;
        }
        let base_name = crate::parser::to_snake_case(base_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, signature)| {
                method_signature_type_name_matches(&key.type_name, type_name)
                    && signature.params.len() == arity
                    && (key.rust_name == base_name
                        || key
                            .rust_name
                            .strip_prefix(&base_name)
                            .is_some_and(|suffix| suffix.starts_with('_')))
            })
            .collect::<Vec<_>>();
        matches.sort_by(|(left, _), (right, _)| left.rust_name.cmp(&right.rust_name));
        match matches.as_slice() {
            [(key, signature)] => Some((key.rust_name.clone(), *signature)),
            _ => None,
        }
    }

    pub fn method_signature_by_base_name_and_arg_types(
        &self,
        type_name: &str,
        base_name: &str,
        arg_types: &[JavaType],
    ) -> Option<(String, &MethodSignaturePlan)> {
        if method_signature_owner_query_is_ambiguous(self.methods.keys(), type_name) {
            return None;
        }
        let base_name = crate::parser::to_snake_case(base_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, signature)| {
                method_signature_type_name_matches(&key.type_name, type_name)
                    && method_signature_base_name_matches(&key.rust_name, &base_name)
                    && method_signature_accepts_arg_types(signature, arg_types)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|(left, _), (right, _)| left.rust_name.cmp(&right.rust_name));
        match matches.as_slice() {
            [(key, signature)] => Some((key.rust_name.clone(), *signature)),
            _ => None,
        }
    }

    pub fn method_signature_by_base_name_and_arg_types_with_facts(
        &self,
        type_name: &str,
        base_name: &str,
        arg_types: &[JavaType],
        facts: &FactSet,
    ) -> Option<(String, &MethodSignaturePlan)> {
        if method_signature_owner_query_is_ambiguous(self.methods.keys(), type_name) {
            return None;
        }
        let base_name = crate::parser::to_snake_case(base_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, signature)| {
                method_signature_type_name_matches(&key.type_name, type_name)
                    && method_signature_base_name_matches(&key.rust_name, &base_name)
                    && method_signature_accepts_arg_types_with_facts(signature, arg_types, facts)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|(left, _), (right, _)| left.rust_name.cmp(&right.rust_name));
        match matches.as_slice() {
            [(key, signature)] => Some((key.rust_name.clone(), *signature)),
            _ => None,
        }
    }

    pub fn has_method_signature_by_base_name_accepting_arity(
        &self,
        type_name: &str,
        base_name: &str,
        arity: usize,
    ) -> bool {
        self.method_signature_by_base_name_accepting_arity(type_name, base_name, arity)
            .is_some()
    }

    pub fn method_signature_by_base_name_accepting_arity(
        &self,
        type_name: &str,
        base_name: &str,
        arity: usize,
    ) -> Option<(String, &MethodSignaturePlan)> {
        if method_signature_owner_query_is_ambiguous(self.methods.keys(), type_name) {
            return None;
        }
        let base_name = crate::parser::to_snake_case(base_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, signature)| {
                method_signature_type_name_matches(&key.type_name, type_name)
                    && (key.rust_name == base_name
                        || key
                            .rust_name
                            .strip_prefix(&base_name)
                            .is_some_and(|suffix| suffix.starts_with('_')))
                    && method_signature_accepts_arity(signature, arity)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|(left, _), (right, _)| left.rust_name.cmp(&right.rust_name));
        match matches.as_slice() {
            [(key, signature)] => Some((key.rust_name.clone(), *signature)),
            _ => None,
        }
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

    pub fn method_return_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<RustSlotShape> {
        self.method_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn method_return_shape_by_java_key(&self, key: &JavaCallableKey) -> Option<RustSlotShape> {
        self.method_signature_by_java_key(key)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn field_shape(&self, type_name: &str, field_name: &str) -> Option<&RustSlotShape> {
        self.fields.get(&FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        })
    }

    pub fn required_field_shape(&self, type_name: &str, field_name: &str) -> RustSlotShape {
        let key = FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        };
        self.fields
            .get(&key)
            .or_else(|| self.unique_field_shape_for_simple_owner(&key))
            .cloned()
            .unwrap_or_else(|| {
            panic!(
                "signature solver must plan every lowered field: missing {}::{}, available for type: [{}], nearby types: [{}]",
                key.type_name,
                key.field_name,
                self.available_field_names_for_type(&key.type_name),
                self.nearby_field_type_names(&key.type_name)
            )
        })
    }

    fn available_method_names_for_type(&self, type_name: &str) -> String {
        let mut names = self
            .methods
            .keys()
            .filter(|key| key.type_name == type_name)
            .map(|key| key.rust_name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }

    fn nearby_method_type_names(&self, type_name: &str) -> String {
        nearby_type_names(
            self.methods.keys().map(|key| key.type_name.as_str()),
            type_name,
        )
    }

    fn unique_method_signature_for_simple_owner(
        &self,
        missing: &MethodKey,
    ) -> Option<&MethodSignaturePlan> {
        let missing_simple = type_name_simple_segment(&missing.type_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, _)| {
                key.rust_name == missing.rust_name
                    && key.type_name != missing.type_name
                    && !distinct_qualified_type_owners(&missing.type_name, &key.type_name)
                    && type_name_simple_segment(&key.type_name) == missing_simple
            })
            .map(|(_, signature)| signature)
            .collect::<Vec<_>>();
        matches.dedup_by(|left, right| std::ptr::eq(*left, *right));
        match matches.as_slice() {
            [signature] => Some(*signature),
            _ => None,
        }
    }

    fn available_field_names_for_type(&self, type_name: &str) -> String {
        let mut names = self
            .fields
            .keys()
            .filter(|key| key.type_name == type_name)
            .map(|key| key.field_name.as_str())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }

    fn nearby_field_type_names(&self, type_name: &str) -> String {
        nearby_type_names(
            self.fields.keys().map(|key| key.type_name.as_str()),
            type_name,
        )
    }

    fn unique_field_shape_for_simple_owner(&self, missing: &FieldKey) -> Option<&RustSlotShape> {
        let missing_simple = type_name_simple_segment(&missing.type_name);
        let mut matches = self
            .fields
            .iter()
            .filter(|(key, _)| {
                key.field_name == missing.field_name
                    && key.type_name != missing.type_name
                    && !distinct_qualified_type_owners(&missing.type_name, &key.type_name)
                    && type_name_simple_segment(&key.type_name) == missing_simple
            })
            .map(|(_, shape)| shape)
            .collect::<Vec<_>>();
        matches.dedup_by(|left, right| std::ptr::eq(*left, *right));
        match matches.as_slice() {
            [shape] => Some(*shape),
            _ => None,
        }
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

    pub fn local_plan_by_binding<'a>(
        &self,
        locals: &'a [LocalPlan],
        binding: &BindingId,
    ) -> Option<&'a LocalPlan> {
        locals.iter().find(|local| local.binding == *binding)
    }

    pub fn local_shape_by_binding<'a>(
        &self,
        locals: &'a [LocalPlan],
        binding: &BindingId,
    ) -> Option<&'a RustSlotShape> {
        self.local_plan_by_binding(locals, binding)
            .map(|local| &local.shape)
    }

    pub fn required_local_plan_by_binding<'a>(
        &self,
        locals: &'a [LocalPlan],
        binding: &BindingId,
    ) -> &'a LocalPlan {
        self.local_plan_by_binding(locals, binding)
            .expect("signature solver must plan every lowered local binding")
    }

    pub fn required_local_shape_by_binding(
        &self,
        locals: &[LocalPlan],
        binding: &BindingId,
    ) -> RustSlotShape {
        self.local_shape_by_binding(locals, binding)
            .cloned()
            .expect("signature solver must plan every lowered local binding")
    }

    pub fn local_binding_mutability_by_binding(
        &self,
        locals: &[LocalPlan],
        binding: &BindingId,
    ) -> Option<bool> {
        self.local_plan_by_binding(locals, binding)
            .map(|local| local.binding_mutable)
    }
}

fn nearby_type_names<'a>(names: impl Iterator<Item = &'a str>, target: &str) -> String {
    let target_simple = type_name_simple_segment(target);
    let mut nearby = names
        .filter(|name| {
            name == &target
                || type_name_simple_segment(name) == target_simple
                || name.contains(target_simple)
                || target.contains(type_name_simple_segment(name))
        })
        .take(12)
        .collect::<Vec<_>>();
    nearby.sort_unstable();
    nearby.dedup();
    nearby.join(", ")
}

fn type_name_simple_segment(type_name: &str) -> &str {
    type_name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(type_name)
}

fn distinct_qualified_type_owners(left: &str, right: &str) -> bool {
    left != right
        && (left.contains('.') || left.contains('$'))
        && (right.contains('.') || right.contains('$'))
}

fn method_signature_base_name_matches(rust_name: &str, base_name: &str) -> bool {
    rust_name == base_name
        || rust_name
            .strip_prefix(base_name)
            .is_some_and(|suffix| suffix.starts_with('_'))
}

fn method_signature_accepts_arity(signature: &MethodSignaturePlan, arity: usize) -> bool {
    if signature.params.len() == arity {
        return true;
    }
    signature
        .params
        .last()
        .is_some_and(|param| param.is_varargs && arity >= signature.params.len().saturating_sub(1))
}

fn method_signature_accepts_arg_types(
    signature: &MethodSignaturePlan,
    arg_types: &[JavaType],
) -> bool {
    if signature.params.len() == arg_types.len()
        && signature
            .params
            .iter()
            .zip(arg_types)
            .all(|(param, actual)| java_signature_argument_matches(actual, &param.java_ty))
    {
        return true;
    }
    let Some(varargs_param) = signature.params.last() else {
        return false;
    };
    if !varargs_param.is_varargs {
        return false;
    }
    let fixed_count = signature.params.len().saturating_sub(1);
    if arg_types.len() < fixed_count {
        return false;
    }
    if !signature
        .params
        .iter()
        .take(fixed_count)
        .zip(arg_types.iter().take(fixed_count))
        .all(|(param, actual)| java_signature_argument_matches(actual, &param.java_ty))
    {
        return false;
    }
    let element_ty = crate::types::sequence_element_type(&varargs_param.java_ty)
        .unwrap_or_else(|| varargs_param.java_ty.clone());
    arg_types
        .iter()
        .skip(fixed_count)
        .all(|actual| java_signature_argument_matches(actual, &element_ty))
}

fn method_signature_accepts_arg_types_with_facts(
    signature: &MethodSignaturePlan,
    arg_types: &[JavaType],
    facts: &FactSet,
) -> bool {
    if signature.params.len() == arg_types.len()
        && signature
            .params
            .iter()
            .zip(arg_types)
            .all(|(param, actual)| {
                java_signature_argument_matches_with_facts(actual, &param.java_ty, facts)
            })
    {
        return true;
    }
    let Some(varargs_param) = signature.params.last() else {
        return false;
    };
    if !varargs_param.is_varargs {
        return false;
    }
    let fixed_count = signature.params.len().saturating_sub(1);
    if arg_types.len() < fixed_count {
        return false;
    }
    if !signature
        .params
        .iter()
        .take(fixed_count)
        .zip(arg_types.iter().take(fixed_count))
        .all(|(param, actual)| {
            java_signature_argument_matches_with_facts(actual, &param.java_ty, facts)
        })
    {
        return false;
    }
    let element_ty = crate::types::sequence_element_type(&varargs_param.java_ty)
        .unwrap_or_else(|| varargs_param.java_ty.clone());
    arg_types
        .iter()
        .skip(fixed_count)
        .all(|actual| java_signature_argument_matches_with_facts(actual, &element_ty, facts))
}

fn method_signature_type_name_matches(key_type_name: &str, query_type_name: &str) -> bool {
    if key_type_name == query_type_name {
        return true;
    }
    method_signature_type_name_aliases(key_type_name)
        .iter()
        .any(|alias| alias == query_type_name)
        || method_signature_type_name_aliases(query_type_name)
            .iter()
            .any(|alias| alias == key_type_name)
}

fn method_signature_owner_query_is_ambiguous<'a>(
    owners: impl Iterator<Item = &'a MethodKey>,
    query_type_name: &str,
) -> bool {
    if query_type_name.contains('.') || query_type_name.contains('$') {
        return false;
    }
    let mut matches = HashSet::new();
    for owner in owners {
        if owner.type_name == query_type_name {
            return false;
        }
        if method_signature_type_name_aliases(&owner.type_name)
            .iter()
            .any(|alias| alias == query_type_name)
        {
            matches.insert(owner.type_name.as_str());
            if matches.len() > 1 {
                return true;
            }
        }
    }
    false
}

fn legacy_signature_owner_query_is_ambiguous<'a>(
    owners: impl Iterator<Item = &'a str>,
    query_type_name: &str,
) -> bool {
    if query_type_name.contains('.') || query_type_name.contains('$') {
        return false;
    }
    let mut matches = HashSet::new();
    for owner in owners {
        if owner == query_type_name {
            return false;
        }
        if method_signature_type_name_aliases(owner)
            .iter()
            .any(|alias| alias == query_type_name)
        {
            matches.insert(owner);
            if matches.len() > 1 {
                return true;
            }
        }
    }
    false
}

fn legacy_signature_owner_fact_matches(
    fact_type_name: &str,
    query_type_name: &str,
    query_is_ambiguous: bool,
) -> bool {
    if fact_type_name == query_type_name {
        return true;
    }
    !query_is_ambiguous
        && !distinct_qualified_type_owners(fact_type_name, query_type_name)
        && crate::facts::external_type_name_matches(fact_type_name, query_type_name)
}

fn java_signature_argument_matches(actual: &JavaType, target: &JavaType) -> bool {
    if matches!(actual, JavaType::Unknown(_)) || matches!(target, JavaType::Unknown(_)) {
        return false;
    }
    let actual = crate::types::optional_element_type(actual).unwrap_or_else(|| actual.clone());
    let target = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    java_types_match_with_external_aliases(&actual, &target)
}

fn java_signature_argument_matches_with_facts(
    actual: &JavaType,
    target: &JavaType,
    facts: &FactSet,
) -> bool {
    if java_signature_argument_matches(actual, target) {
        return true;
    }
    let actual = crate::types::optional_element_type(actual).unwrap_or_else(|| actual.clone());
    let target = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    let (Some(actual_name), Some(target_name)) = (java_type_name(&actual), java_type_name(&target))
    else {
        return false;
    };
    if facts
        .external_conversion_adapter_for(actual_name, target_name)
        .is_some()
        || crate::external_packs::external_conversion_adapter_for(actual_name, target_name)
            .is_some()
    {
        return true;
    }
    facts
        .type_capabilities_for(&actual)
        .implements
        .iter()
        .any(|implemented| {
            java_type_name(implemented)
                .is_some_and(|name| crate::facts::external_type_name_matches(name, target_name))
        })
}

fn java_type_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

fn method_signature_type_name_aliases(type_name: &str) -> Vec<String> {
    let simple = type_name
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(type_name);
    let mut aliases = vec![simple.to_string(), crate::parser::to_snake_case(simple)];
    aliases.sort();
    aliases.dedup();
    aliases
}

fn collect_type_signatures(
    ty: &TypeDecl,
    support_program: &Program,
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
                    constrained_field_shape(
                        &field.ty,
                        representations,
                        &field_constraints,
                        facts,
                        &ty.type_params,
                    ),
                );
            }
            crate::java_ir::Member::Method(method) => {
                let method_key = MethodKey {
                    type_name: ty.name.clone(),
                    rust_name: method_rust_name(method),
                };
                let method_constraints = constraints.methods.get(&method_key);
                // In scope here: the enclosing type's parameters plus this
                // method's own, the method's shadowing on a name clash.
                let type_param_scope = type_params_in_scope(&ty.type_params, &method.type_params);
                let signature = MethodSignaturePlan {
                    params: method
                        .params
                        .iter()
                        .enumerate()
                        .map(|(index, param)| {
                            let constraints = method_constraints
                                .and_then(|constraints| constraints.params.get(index))
                                .cloned()
                                .unwrap_or_default();
                            let java_ty = method_constraints
                                .and_then(|constraints| constraints.param_java_types.get(index))
                                .unwrap_or(&param.ty);
                            constrained_param_plan(
                                param,
                                java_ty,
                                representations,
                                &constraints,
                                facts,
                                &type_param_scope,
                            )
                        })
                        .collect(),
                    return_shape: {
                        let constraints = method_constraints
                            .map(|constraints| constraints.return_constraints.clone())
                            .unwrap_or_default();
                        constrained_return_shape(
                            ty,
                            method,
                            representations,
                            method_constraints,
                            &constraints,
                            facts,
                            &type_param_scope,
                        )
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
                                &type_param_scope,
                            )
                        })
                        .unwrap_or_default(),
                    receiver: constrained_receiver_plan(
                        method_constraints
                            .map(|constraints| constraints.receiver.as_slice())
                            .unwrap_or(&[]),
                    ),
                };
                plan.methods_by_java_key.insert(
                    JavaCallableKey::source_method_for_type(ty, method),
                    signature.clone(),
                );
                plan.methods.insert(method_key, signature);
            }
            crate::java_ir::Member::Constructor(ctor) => {
                let constructor_key = ConstructorKey {
                    type_name: ty.name.clone(),
                    rust_name: constructor_rust_name(ctor),
                };
                let constructor_constraints = constraints.constructors.get(&constructor_key);
                let type_param_scope = type_params_in_scope(&ty.type_params, &ctor.type_params);
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
                                constrained_param_plan(
                                    param,
                                    &param.ty,
                                    representations,
                                    &constraints,
                                    facts,
                                    &type_param_scope,
                                )
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
                                    &type_param_scope,
                                )
                            })
                            .unwrap_or_default(),
                    },
                );
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signatures(
                    nested,
                    support_program,
                    representations,
                    constraints,
                    facts,
                    plan,
                );
            }
            crate::java_ir::Member::Unsupported { .. } => {}
        }
    }
    for inherited in
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
    {
        let field_key = FieldKey {
            type_name: ty.name.clone(),
            field_name: inherited.field.name.clone(),
        };
        let declaring_ty = find_signature_type_decl(support_program, &inherited.declaring_type);
        let mut field_constraints =
            constraints
                .fields
                .get(&field_key)
                .cloned()
                .unwrap_or_else(|| {
                    field_shape_constraints_with_owner_name(
                        &inherited.declaring_type,
                        &inherited.field,
                        facts,
                    )
                });
        push_unique_signature_constraints(
            &mut field_constraints,
            &field_shape_constraints_with_owner_name(
                &inherited.declaring_type,
                &inherited.field,
                facts,
            ),
        );
        if let Some(declaring_ty) = declaring_ty {
            push_unique_signature_constraints(
                &mut field_constraints,
                &delegated_null_constructor_field_constraints(declaring_ty, &inherited.field),
            );
        }
        plan.fields.insert(
            field_key,
            constrained_field_shape(
                &inherited.field.ty,
                representations,
                &field_constraints,
                facts,
                declaring_ty
                    .map(|declaring_ty| declaring_ty.type_params.as_slice())
                    .unwrap_or(&[]),
            ),
        );
    }
    for inherited in
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
    {
        let field_key = FieldKey {
            type_name: ty.name.clone(),
            field_name: inherited.rust_name.clone(),
        };
        let mut external_constraints = constraints
            .fields
            .get(&field_key)
            .cloned()
            .unwrap_or_else(field_shape_constraints);
        push_unique_signature_constraints(
            &mut external_constraints,
            &java_platform_facade_concrete_native_constraints(
                &inherited.field_type,
                &HashSet::new(),
            ),
        );
        if let Some(nullability) = inherited.field_nullability {
            external_constraints.push(SignatureConstraint::RequiredStorageNullability(nullability));
        }
        plan.fields.insert(
            field_key,
            constrained_field_shape(
                &inherited.field_type,
                representations,
                &external_constraints,
                facts,
                &[],
            ),
        );
    }
    // Source-declared members first. `or_insert` means whoever registers a key
    // first wins, and an *external* inherited signature is a guess (often unit)
    // while a source-inherited one is solved. Registering the external ones
    // first meant a class that inherits an interface `default` through its
    // superclass got `fn greet(&self)` with no return type, because an
    // unresolved call had recorded the superclass as an external API receiver.
    for inherited in
        crate::inherited_members::inherited_instance_method_refs_for_type(ty, support_program)
    {
        let method_key = MethodKey {
            type_name: ty.name.clone(),
            rust_name: inherited.rust_name.clone(),
        };
        let java_key = JavaCallableKey::source_method_for_type(ty, inherited.method);
        let signature =
            inherited_source_method_signature(inherited, representations, constraints, facts);
        plan.methods.entry(method_key).or_insert(signature.clone());
        plan.methods_by_java_key
            .entry(java_key)
            .or_insert(signature);
    }
    for inherited in
        crate::inherited_members::external_inherited_instance_methods_for_type(ty, facts)
    {
        let method_key = MethodKey {
            type_name: ty.name.clone(),
            rust_name: inherited.rust_name.clone(),
        };
        plan.methods.entry(method_key).or_insert_with(|| {
            external_inherited_method_signature(&inherited, representations, facts)
        });
    }
}

fn inherited_source_method_signature(
    inherited: crate::inherited_members::InheritedInstanceMethodRef<'_>,
    representations: &RepresentationPlan,
    constraints: &SignatureConstraintState,
    facts: &FactSet,
) -> MethodSignaturePlan {
    let method_constraints = constraints.methods.get(&MethodKey {
        type_name: inherited.declaring_type.to_string(),
        rust_name: method_rust_name(inherited.method),
    });
    let type_param_scope = type_params_in_scope(
        &inherited.declaring_ty.type_params,
        &inherited.method.type_params,
    );
    MethodSignaturePlan {
        params: inherited
            .method
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let constraints = method_constraints
                    .and_then(|constraints| constraints.params.get(index))
                    .cloned()
                    .unwrap_or_default();
                let java_ty = method_constraints
                    .and_then(|constraints| constraints.param_java_types.get(index))
                    .unwrap_or(&param.ty);
                constrained_param_plan(
                    param,
                    java_ty,
                    representations,
                    &constraints,
                    facts,
                    &type_param_scope,
                )
            })
            .collect(),
        return_shape: {
            let constraints = method_constraints
                .map(|constraints| constraints.return_constraints.clone())
                .unwrap_or_default();
            constrained_return_shape(
                inherited.declaring_ty,
                inherited.method,
                representations,
                method_constraints,
                &constraints,
                facts,
                &type_param_scope,
            )
        },
        locals: inherited
            .method
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
                    &type_param_scope,
                )
            })
            .unwrap_or_default(),
        receiver: constrained_receiver_plan(
            method_constraints
                .map(|constraints| constraints.receiver.as_slice())
                .unwrap_or(&[]),
        ),
    }
}

fn external_inherited_method_signature(
    inherited: &crate::inherited_members::ExternalInheritedInstanceMethod,
    representations: &RepresentationPlan,
    facts: &FactSet,
) -> MethodSignaturePlan {
    MethodSignaturePlan {
        params: (0..inherited.arg_count)
            .map(|index| {
                let java_ty = JavaType::Unknown("_".to_string());
                ParamPlan {
                    name: format!("arg{index}"),
                    java_ty: java_ty.clone(),
                    is_varargs: false,
                    shape: solve_signature_shape_with_facts(
                        representations,
                        SignatureShapeRequest {
                            java_ty,
                            role: SignatureSlotRole::Param { is_varargs: false },
                            constraints: Vec::new(),
                            type_params: Vec::new(),
                        },
                        facts,
                    )
                    .expect("external inherited param signature shape is always present"),
                    binding_mutable: false,
                }
            })
            .collect(),
        return_shape: inherited.fact.return_type.clone().and_then(|java_ty| {
            solve_signature_shape_with_facts(
                representations,
                SignatureShapeRequest {
                    java_ty,
                    role: SignatureSlotRole::Return,
                    constraints: Vec::new(),
                    type_params: Vec::new(),
                },
                facts,
            )
        }),
        locals: Vec::new(),
        receiver: match inherited.fact.receiver_mutability {
            ReceiverMutabilityFact::MutatesReceiver => ReceiverPlan::Mutable,
            _ => ReceiverPlan::Shared,
        },
    }
}

pub fn method_param_shape_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    method_param_shape_constraints_with_declared_source_types(
        ty,
        method,
        param,
        usage,
        facts,
        &HashSet::new(),
    )
}

fn method_param_shape_constraints_with_declared_source_types(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let method_usage = usage.source_method_usage(ty, method);
    let binding = method_usage.and_then(|method_usage| {
        method_usage.binding_for_declaration_with_name_fallback(
            param.binding.as_ref(),
            &param.name,
            param.binding.is_none(),
        )
    });
    let field_receiver_mutated_names = method
        .body
        .as_ref()
        .map(collect_field_receiver_mutated_names)
        .unwrap_or_default();
    let map_view_receiver_mutated_names = method
        .body
        .as_ref()
        .map(collect_map_view_backing_receiver_mutated_names)
        .unwrap_or_default();
    let field_receiver_read_names = method
        .body
        .as_ref()
        .map(collect_field_receiver_read_names)
        .unwrap_or_default();
    let array_element_mutated_names = method
        .body
        .as_ref()
        .map(collect_array_element_mutated_names)
        .unwrap_or_default();
    let facade_mut_borrow_argument_names = method
        .body
        .as_ref()
        .map(|body| {
            let mut local_types = method
                .params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect::<HashMap<_, _>>();
            collect_facade_mut_borrow_argument_names(body, facts, &mut local_types)
        })
        .unwrap_or_default();
    let mut boundary_kind = method_param_boundary_policy(ty, method, param, usage).kind;
    if field_receiver_mutated_names.contains(&param.name)
        || array_element_mutated_names.contains(&param.name)
        || facade_mut_borrow_argument_names.contains(&param.name)
        || map_view_receiver_mutated_names.contains(&param.name)
    {
        boundary_kind = None;
    }
    let type_env = signature_type_param_env_for_method(ty, method);
    let binding_has_mutating_external_receiver = binding.is_some_and(|binding| {
        binding_has_mutating_external_receiver_call(
            binding,
            facts,
            &type_env,
            declared_source_types,
        )
    });
    if binding_has_mutating_external_receiver {
        boundary_kind = None;
    }
    let public_readonly_receiver_read_product = boundary_kind
        == Some(SignatureBoundaryKind::PublicApi)
        && !java_types_have_same_signature_boundary_identity(&method.return_type, &param.ty)
        && java_type_is_obvious_read_product(&method.return_type)
        && binding.is_some_and(|binding| {
            binding_is_fully_modeled_by_readonly_external_receivers(binding, facts)
        });
    if public_readonly_receiver_read_product {
        boundary_kind = None;
    }
    let unknown_boundary_fully_modeled_by_external_arguments = boundary_kind
        == Some(SignatureBoundaryKind::UnknownApi)
        && binding.is_some_and(|binding| {
            unknown_boundary_is_fully_modeled_by_external_argument_effects(binding, facts)
        });
    if unknown_boundary_fully_modeled_by_external_arguments {
        boundary_kind = None;
    }
    if boundary_kind != Some(SignatureBoundaryKind::PublicApi)
        && binding
            .is_some_and(|binding| internal_object_binding_is_shared_borrow_safe(binding, facts))
    {
        boundary_kind = None;
    }
    if boundary_kind == Some(SignatureBoundaryKind::UnknownApi)
        && binding.is_some_and(|binding| {
            binding_is_fully_modeled_by_readonly_external_receivers(binding, facts)
        })
    {
        boundary_kind = None;
    }
    let mut constraints =
        signature_slot_baseline_policy(&param.ty, boundary_kind, facts).constraints;
    push_unique_signature_constraints(
        &mut constraints,
        &signature_object_representation_fact_constraints(&param.ty, facts),
    );
    if let Some(binding) = binding {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints_with_declared_source_types(
                binding,
                facts,
                &type_env,
                true,
                boundary_kind != Some(SignatureBoundaryKind::PublicApi)
                    || public_readonly_receiver_read_product,
                declared_source_types,
            ),
        );
        if boundary_kind != Some(SignatureBoundaryKind::UnknownApi)
            || unknown_boundary_fully_modeled_by_external_arguments
        {
            push_unique_signature_constraints(
                &mut constraints,
                &external_argument_binding_constraints(binding, facts),
            );
        }
        push_unique_signature_constraints(
            &mut constraints,
            &facade_abi_argument_binding_constraints(binding, facts),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &source_receiver_binding_constraints(binding, usage),
        );
        if method_param_borrowed_content_policy_with_facts(ty, method, param, usage, facts)
            .is_none()
            && boundary_kind != Some(SignatureBoundaryKind::PublicApi)
            && !signature_capability_policy(&param.ty, facts)
                .capability_ownership
                .is_some_and(|ownership| ownership == OwnershipShape::CopyValue)
            && internal_object_binding_is_shared_borrow_safe(binding, facts)
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredOwnership(
                    OwnershipShape::SharedBorrow,
                )],
            );
            push_unique_signature_constraints(
                &mut constraints,
                &borrowed_content_type_policy(&param.ty, facts)
                    .policy
                    .map(signature_constraints_for_borrowed_content_policy)
                    .unwrap_or_default(),
            );
        }
    }
    if let Some(body) = &method.body {
        push_unique_signature_constraints(
            &mut constraints,
            &source_binding_nullability_constraints_for_declaration(
                body,
                &param.name,
                param.binding.as_ref(),
                &method.params,
            ),
        );
        if field_receiver_mutated_names.contains(&param.name)
            || map_view_receiver_mutated_names.contains(&param.name)
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow),
                    SignatureConstraint::MutableBinding,
                ],
            );
        } else if collect_mutated_local_names(body).contains(&param.name)
            && !binding_has_mutating_external_receiver
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::MutableBinding],
            );
            if matches!(param.ty, JavaType::Array(_)) {
                push_unique_signature_constraints(
                    &mut constraints,
                    &[SignatureConstraint::RequiredOwnership(
                        OwnershipShape::MutableBorrow,
                    )],
                );
            } else if borrowed_content_type_policy(&param.ty, facts)
                .policy
                .is_some()
            {
                push_unique_signature_constraints(
                    &mut constraints,
                    &[
                        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                        SignatureConstraint::RequiredView(ViewShape::Native),
                    ],
                );
            }
        }
        if array_element_mutated_names.contains(&param.name) {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow),
                    SignatureConstraint::MutableBinding,
                ],
            );
        }
        if facade_mut_borrow_argument_names.contains(&param.name) {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow),
                    SignatureConstraint::MutableBinding,
                ],
            );
        }
        if field_receiver_read_names.contains(&param.name)
            && signature_declared_type_has_parsed_source_capability(
                &param.ty,
                &EffectiveFacts::new(facts),
            )
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
                    SignatureConstraint::RequiredView(ViewShape::Native),
                ],
            );
        }
    }
    let public_borrowed_content_constraints =
        if boundary_kind == Some(SignatureBoundaryKind::PublicApi) {
            method_param_borrowed_content_policy_with_facts(ty, method, param, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    if !public_borrowed_content_constraints.is_empty() {
        constraints.retain(|constraint| {
            !matches!(
                constraint,
                SignatureConstraint::RequiredView(ViewShape::Native)
                    | SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            )
        });
        push_unique_signature_constraints(&mut constraints, &public_borrowed_content_constraints);
    }
    push_unique_signature_constraints(
        &mut constraints,
        &source_object_varargs_carrier_constraints(param),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_concrete_native_constraints(&param.ty, declared_source_types),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_subclass_concrete_native_constraints(
            &param.ty,
            facts,
            declared_source_types,
        ),
    );
    if !declared_java_platform_facade_type(&param.ty, declared_source_types) {
        push_unique_signature_constraints(
            &mut constraints,
            &internal_exact_final_erased_facade_constraints(&param.ty, facts),
        );
    }
    if boundary_kind.is_none() {
        constraints.extend(
            method_param_borrowed_content_policy_with_facts(ty, method, param, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
    }
    if method_return_type_param_name(method)
        .is_some_and(|type_param| java_type_is_simple_type_param(&param.ty, type_param))
    {
        retain_non_ownership_view_constraints(&mut constraints);
        push_unique_signature_constraints(&mut constraints, &owned_native_signature_constraints());
    }
    normalize_param_borrowed_slice_constraints(param, &mut constraints);
    constraints
}

fn signature_type_param_env_for_method(ty: &TypeDecl, method: &MethodDecl) -> TypeEnv {
    let mut env = signature_type_param_env_for_type(ty);
    for (name, bound) in method
        .type_params
        .iter()
        .zip(method.type_param_bounds.iter())
    {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
    env
}

fn signature_type_param_env_for_constructor(ty: &TypeDecl, ctor: &ConstructorDecl) -> TypeEnv {
    let mut env = signature_type_param_env_for_type(ty);
    for (name, bound) in ctor.type_params.iter().zip(ctor.type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
    env
}

fn signature_type_param_env_for_type(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for (name, bound) in ty.type_params.iter().zip(ty.type_param_bounds.iter()) {
        if let Some(bound) = bound {
            env.insert_type_param_bound(name.clone(), bound.clone());
        }
    }
    env
}

fn method_return_type_param_name(method: &MethodDecl) -> Option<&str> {
    let type_name = simple_type_param_name(&method.return_type)?;
    method
        .type_params
        .iter()
        .any(|type_param| type_param == type_name)
        .then_some(type_name)
}

fn simple_type_param_name(java_ty: &JavaType) -> Option<&str> {
    match java_ty {
        JavaType::Named { name, args } if args.is_empty() => Some(name.as_str()),
        JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    }
}

fn java_type_is_simple_type_param(java_ty: &JavaType, type_param: &str) -> bool {
    simple_type_param_name(java_ty) == Some(type_param)
}

fn retain_non_ownership_view_constraints(constraints: &mut Vec<SignatureConstraint>) {
    constraints.retain(|constraint| {
        !matches!(
            constraint,
            SignatureConstraint::RequiredOwnership(_) | SignatureConstraint::RequiredView(_)
        )
    });
}

fn owned_native_signature_constraints() -> [SignatureConstraint; 2] {
    [
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn source_object_varargs_carrier_constraints(param: &Param) -> Vec<SignatureConstraint> {
    if !param.is_varargs {
        return Vec::new();
    }
    if !java_varargs_erased_object_carrier_type(&param.ty) {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn java_varargs_erased_object_carrier_type(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Array(element_ty) => java_type_is_erased_object_boundary(element_ty),
        _ => java_type_is_erased_object_boundary(java_ty),
    }
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
        ) || (fact_policy.object_representation_facts
            && !exact_final_erased_facade_boundary_should_use_native(java_ty, facts)),
    }
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

pub fn signature_native_boundary_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SignatureNativeBoundaryPolicy {
    let capability = signature_capability_policy_with_effective_facts(java_ty, facts);
    let owner_requirement = capability
        .capability_ownership
        .unwrap_or_else(|| conservative_ownership(java_ty));
    let mut constraints = vec![
        SignatureConstraint::RequiredOwnership(owner_requirement),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ];
    push_unique_signature_constraints(
        &mut constraints,
        &exact_final_erased_facade_native_boundary_constraints(java_ty, facts),
    );
    SignatureNativeBoundaryPolicy { constraints }
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
    push_unique_signature_constraints(
        &mut constraints,
        &signature_erased_object_boundary_constraints_with_effective_facts(java_ty, facts),
    );
    SignatureSlotBaselinePolicy { constraints }
}

fn signature_erased_object_boundary_constraints_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    let fact_policy =
        signature_boundary_representation_fact_policy_with_effective_facts(java_ty, facts);
    if !is_declared_java_object_type(java_ty)
        || signature_declared_type_has_parsed_source_capability(java_ty, facts)
        || java_type_is_map_entry_tuple_carrier(java_ty)
        || fact_policy.object_representation_facts
    {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject),
        SignatureConstraint::RequiredView(ViewShape::ErasedObjectView),
    ]
}

fn java_type_is_map_entry_tuple_carrier(java_ty: &JavaType) -> bool {
    let JavaType::Named { name, args } = java_ty else {
        return false;
    };
    !args.is_empty()
        && matches!(
            name.rsplit('.')
                .next()
                .unwrap_or(name)
                .rsplit('$')
                .next()
                .unwrap_or(name),
            "Entry"
        )
}

fn signature_object_representation_fact_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    signature_object_representation_fact_constraints_with_effective_facts(java_ty, &effective_facts)
}

fn signature_object_representation_fact_constraints_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    if crate::stdlib::java_collection_kind_for_type(java_ty).is_some() {
        return Vec::new();
    }
    if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty) {
        return Vec::new();
    }
    let Some(override_fact) = signature_object_representation_override_fact_for(java_ty, facts)
    else {
        return Vec::new();
    };
    if override_fact.provenance == FactProvenance::ParsedSource
        && matches!(override_fact.strategy, HierarchyStrategy::ConcreteStruct)
    {
        return Vec::new();
    }
    hierarchy_strategy_signature_dispatch(&override_fact.strategy)
        .map(|dispatch| vec![SignatureConstraint::RequiredDispatch(dispatch)])
        .unwrap_or_default()
}

fn signature_object_representation_override_fact_for<'a>(
    java_ty: &JavaType,
    facts: &'a EffectiveFacts<'_>,
) -> Option<&'a HierarchyOverride> {
    let type_name = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name.as_str(),
        _ => return None,
    };
    let exact_external_pack_override = facts
        .hierarchy_overrides()
        .iter()
        .filter(|override_fact| {
            override_fact.provenance == FactProvenance::ExternalLibraryPack
                && override_fact.type_name.contains('.')
                && override_fact.type_name == type_name
                && !external_erased_override_conflicts_with_parsed_source(
                    override_fact,
                    type_name,
                    facts,
                )
        })
        .max_by_key(|override_fact| {
            signature_hierarchy_strategy_specificity_rank(&override_fact.strategy)
        })
        .map(|override_fact| override_fact);
    exact_external_pack_override.or_else(|| {
        facts
            .hierarchy_overrides()
            .iter()
            .filter(|override_fact| {
                signature_hierarchy_override_type_name_matches(&override_fact.type_name, type_name)
            })
            .filter(|override_fact| {
                !external_erased_override_conflicts_with_parsed_source(
                    override_fact,
                    type_name,
                    facts,
                )
            })
            .filter(|override_fact| {
                !matches!(override_fact.strategy, HierarchyStrategy::ErasedObject)
                    || type_name.contains('.')
            })
            .max_by_key(|override_fact| {
                (
                    signature_fact_precedence_key(override_fact.provenance),
                    signature_hierarchy_strategy_specificity_rank(&override_fact.strategy),
                )
            })
    })
}

fn external_erased_override_conflicts_with_parsed_source(
    override_fact: &HierarchyOverride,
    query_type_name: &str,
    facts: &EffectiveFacts<'_>,
) -> bool {
    override_fact.provenance != FactProvenance::UserOverride
        && matches!(override_fact.strategy, HierarchyStrategy::ErasedObject)
        && facts.has_parsed_source_type_capability_for_name(query_type_name)
}

fn signature_declared_type_has_parsed_source_capability(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            facts.has_parsed_source_type_capability_for_name(name)
        }
        _ => false,
    }
}

fn signature_hierarchy_override_type_name_matches(
    fact_type_name: &str,
    query_type_name: &str,
) -> bool {
    fact_type_name == query_type_name
        || fact_type_name
            .rsplit('.')
            .next()
            .is_some_and(|simple_name| simple_name == query_type_name)
}

fn signature_fact_precedence_key(provenance: FactProvenance) -> u8 {
    match provenance {
        FactProvenance::ConservativeFallback => 0,
        FactProvenance::CorpusAnalysis => 1,
        FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => 2,
        FactProvenance::ParsedSource => 3,
        FactProvenance::UserOverride => 4,
    }
}

fn signature_hierarchy_strategy_specificity_rank(strategy: &HierarchyStrategy) -> u8 {
    match strategy {
        HierarchyStrategy::ClosedEnum { .. } => 4,
        HierarchyStrategy::DynTrait { .. } => 3,
        HierarchyStrategy::ConcreteStruct => 2,
        HierarchyStrategy::ErasedObject => 1,
    }
}

fn exact_final_erased_facade_boundary_should_use_native(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    !is_declared_java_object_type(java_ty)
        && matches!(java_ty, JavaType::Named { .. })
        && matches!(
            facts.object_representation_override_strategy_for(java_ty),
            Some(HierarchyStrategy::ErasedObject)
        )
        && facts.is_final_type(java_ty)
}

fn exact_final_erased_facade_native_boundary_constraints(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    if !exact_final_erased_facade_boundary_should_use_native(java_ty, facts) {
        return Vec::new();
    }
    vec![SignatureConstraint::RequiredDispatch(
        DispatchShape::Concrete,
    )]
}

fn is_declared_java_object_type(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Named { name, .. } if name == "Object" || name == "java.lang.Object"
    ) || matches!(java_ty, JavaType::Unknown(name) if name == "Object" || name == "java.lang.Object")
}

pub fn signature_slot_dispatch_requirement_policy_in_scope(
    java_ty: &JavaType,
    role: SignatureSlotRole,
    source_dispatch: DispatchShape,
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
) -> SignatureSlotDispatchRequirementPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    signature_slot_dispatch_requirement_policy_with_effective_facts_in_scope(
        java_ty,
        role,
        source_dispatch,
        &effective_facts,
        type_param_scope,
    )
}

/// The type parameters in scope at the slot being solved: the enclosing type's
/// plus the current callable's, with the callable's shadowing.
///
/// Scope matters. A global list of every parameter name in the program would let
/// a parameter named `ELEM` in one class silence a class named `ELEM` used in
/// another, and vice versa — which is the same cross-scope conflation that
/// binding identity exists to prevent.
pub type TypeParamScope<'a> = &'a [String];

/// The parameters visible at a slot: the enclosing type's plus the callable's.
/// A callable parameter shadows a type parameter of the same name, but since
/// membership is all that is asked, the union answers both.
fn type_params_in_scope(type_params: &[String], callable_params: &[String]) -> Vec<String> {
    let mut scope = type_params.to_vec();
    scope.extend(callable_params.iter().cloned());
    scope
}

/// Whether this type names a type parameter that is in scope here.
///
/// A type parameter is never an unknown runtime object: its capability is
/// exactly its declared bound, so the slot must stay the parameter.
fn java_type_is_type_param_in_scope(java_ty: &JavaType, scope: TypeParamScope<'_>) -> bool {
    let name = match java_ty {
        JavaType::Named { name, args } if args.is_empty() => name,
        JavaType::Unknown(name) => name,
        _ => return false,
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    scope.iter().any(|param| param == simple)
}

pub fn signature_slot_dispatch_requirement_policy_with_effective_facts_in_scope(
    java_ty: &JavaType,
    role: SignatureSlotRole,
    source_dispatch: DispatchShape,
    facts: &EffectiveFacts<'_>,
    type_param_scope: TypeParamScope<'_>,
) -> SignatureSlotDispatchRequirementPolicy {
    if crate::stdlib::java_collection_kind_for_type(java_ty).is_some() {
        return SignatureSlotDispatchRequirementPolicy {
            constraints: Vec::new(),
        };
    }
    if java_type_is_map_entry_tuple_carrier(java_ty) {
        return SignatureSlotDispatchRequirementPolicy {
            constraints: Vec::new(),
        };
    }
    let mut requirements = java_type_name_for_signature_dispatch(java_ty)
        .map(|type_name| facts.dispatch_requirements_for(type_name))
        .unwrap_or_default();
    if signature_declared_type_has_parsed_source_capability(java_ty, facts)
        || java_type_is_type_param_in_scope(java_ty, type_param_scope)
    {
        requirements.retain(|requirement| {
            *requirement != DispatchRequirementKind::NeedsUnknownRuntimeObject
        });
    }
    let constraints = signature_slot_dispatch_requirement_constraints(
        role,
        source_dispatch.clone(),
        &requirements,
    );
    SignatureSlotDispatchRequirementPolicy { constraints }
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
                | SignatureConstraint::RequiredStorageNullability(_)
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
            .source_method_usage(ty, method)
            .and_then(|method_usage| {
                method_usage.binding_for_declaration_with_name_fallback(
                    param.binding.as_ref(),
                    &param.name,
                    param.binding.is_none(),
                )
            })
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
            .and_then(|ctor_usage| {
                ctor_usage.binding_for_declaration_with_name_fallback(
                    param.binding.as_ref(),
                    &param.name,
                    param.binding.is_none(),
                )
            })
            .and_then(|binding| binding_boundary_policy(binding).kind)
    };
    ConstructorParamBoundaryPolicy {
        type_name: ty.name.clone(),
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
    let Some(next) = next else {
        return SignatureBoundaryKindMerge {
            kept: current,
            rejected: None,
        };
    };
    let Some(current) = current else {
        return SignatureBoundaryKindMerge {
            kept: Some(next),
            rejected: None,
        };
    };
    if current == next {
        return SignatureBoundaryKindMerge {
            kept: Some(current),
            rejected: None,
        };
    }
    if signature_boundary_kind_priority_policy(current).rank
        >= signature_boundary_kind_priority_policy(next).rank
    {
        SignatureBoundaryKindMerge {
            kept: Some(current),
            rejected: Some(next),
        }
    } else {
        SignatureBoundaryKindMerge {
            kept: Some(next),
            rejected: Some(current),
        }
    }
}

pub fn signature_boundary_kind_priority_policy(
    requirement: SignatureBoundaryKind,
) -> SignatureBoundaryKindPriorityPolicy {
    let rank = match requirement {
        SignatureBoundaryKind::EscapingStorage => 30,
        SignatureBoundaryKind::PublicApi => 20,
        SignatureBoundaryKind::UnknownApi => 10,
    };
    SignatureBoundaryKindPriorityPolicy { requirement, rank }
}

pub fn constructor_param_shape_constraints(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    constructor_param_shape_constraints_with_declared_source_types(
        ty,
        ctor,
        param,
        usage,
        facts,
        &HashSet::new(),
    )
}

fn constructor_param_shape_constraints_with_declared_source_types(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let ctor_usage = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name: constructor_rust_name(ctor),
    });
    let binding = ctor_usage.and_then(|ctor_usage| {
        ctor_usage.binding_for_declaration_with_name_fallback(
            param.binding.as_ref(),
            &param.name,
            param.binding.is_none(),
        )
    });
    let mut boundary_kind = constructor_param_boundary_policy(ty, ctor, param, usage).kind;
    if boundary_kind == Some(SignatureBoundaryKind::PublicApi)
        && binding.is_some_and(|binding| {
            binding_is_fully_modeled_by_readonly_external_receivers(binding, facts)
        })
    {
        boundary_kind = None;
    }
    let unknown_boundary_fully_modeled_by_external_arguments = boundary_kind
        == Some(SignatureBoundaryKind::UnknownApi)
        && binding.is_some_and(|binding| {
            unknown_boundary_is_fully_modeled_by_external_argument_effects(binding, facts)
        });
    if unknown_boundary_fully_modeled_by_external_arguments {
        boundary_kind = None;
    }
    let mut constraints =
        signature_slot_baseline_policy(&param.ty, boundary_kind, facts).constraints;
    push_unique_signature_constraints(
        &mut constraints,
        &signature_object_representation_fact_constraints(&param.ty, facts),
    );
    if let Some(binding) = binding {
        let type_env = signature_type_param_env_for_constructor(ty, ctor);
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts, &type_env, true, true),
        );
        if boundary_kind != Some(SignatureBoundaryKind::UnknownApi)
            || unknown_boundary_fully_modeled_by_external_arguments
        {
            push_unique_signature_constraints(
                &mut constraints,
                &external_argument_binding_constraints(binding, facts),
            );
        }
    }
    if let Some(body) = &ctor.body {
        push_unique_signature_constraints(
            &mut constraints,
            &source_binding_nullability_constraints_for_declaration(
                body,
                &param.name,
                param.binding.as_ref(),
                &ctor.params,
            ),
        );
    }
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_concrete_native_constraints(&param.ty, declared_source_types),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_subclass_concrete_native_constraints(
            &param.ty,
            facts,
            declared_source_types,
        ),
    );
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
    java_ty: &JavaType,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
) -> ParamPlan {
    let mut constraints = constraints.to_vec();
    normalize_param_borrowed_slice_constraints(param, &mut constraints);
    let shape = solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: java_ty.clone(),
            role: SignatureSlotRole::Param {
                is_varargs: param.is_varargs,
            },
            constraints: constraints.clone(),
            type_params: type_param_scope.to_vec(),
        },
        facts,
    )
    .expect("parameter signature shape is always present");
    ParamPlan {
        name: param.name.clone(),
        java_ty: java_ty.clone(),
        is_varargs: param.is_varargs,
        shape,
        binding_mutable: signature_constraints_require_mutable_binding(&constraints),
    }
}

pub fn solve_signature_shape_with_facts(
    representations: &RepresentationPlan,
    request: SignatureShapeRequest,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let source_dispatch = representations.dispatch_for(&request.java_ty);
    let dispatch_policy = signature_slot_dispatch_requirement_policy_in_scope(
        &request.java_ty,
        request.role,
        source_dispatch,
        facts,
        &request.type_params,
    );
    solve_signature_shape_with_extra_constraints(
        representations,
        request,
        dispatch_policy.constraints,
    )
}

fn hierarchy_strategy_signature_dispatch(strategy: &HierarchyStrategy) -> Option<DispatchShape> {
    match strategy {
        HierarchyStrategy::DynTrait { trait_name } => Some(DispatchShape::DynTrait {
            trait_name: trait_name.clone(),
        }),
        HierarchyStrategy::ClosedEnum { enum_name, .. } => Some(DispatchShape::ClosedEnum {
            enum_name: crate::java_naming::rust_name_for_java_named_type(enum_name),
        }),
        HierarchyStrategy::ErasedObject => Some(DispatchShape::ErasedObject),
        HierarchyStrategy::ConcreteStruct => Some(DispatchShape::Concrete),
    }
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
    base_shape.map(|shape| {
        let mut shape = constraints.apply_to_shape(shape);
        if java_type_is_erased_object_boundary(&request.java_ty) {
            apply_required_dispatch(&mut shape, DispatchShape::ErasedObject);
            apply_required_view(&mut shape, ViewShape::ErasedObjectView);
            apply_required_ownership(&mut shape, OwnershipShape::Owned);
        }
        shape
    })
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
            if !(shape.ownership == OwnershipShape::CopyValue
                && matches!(
                    requirement,
                    OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
                ))
            {
                apply_required_ownership(&mut shape, requirement);
            }
        }
        if let Some(requirement) = &self.slot_view_req {
            apply_required_view(&mut shape, requirement.clone());
        }
        normalize_borrowed_array_slot_view(&mut shape);
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

    pub fn conflict_diagnostics(&self) -> Vec<SignatureConstraintConflictDiagnostic> {
        self.conflicts
            .iter()
            .map(signature_constraint_conflict_diagnostic)
            .collect()
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
                self.mutable_binding =
                    merge_signature_flag_requirement_policy(self.mutable_binding, true).kept;
            }
            SignatureConstraint::StorageSlot => {
                self.storage_slot =
                    merge_signature_flag_requirement_policy(self.storage_slot, true).kept;
            }
            SignatureConstraint::NullableSequenceElement => {}
        }
    }
}

fn normalize_borrowed_array_slot_view(shape: &mut RustSlotShape) {
    if !matches!(
        shape.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) || shape.view != ViewShape::Native
    {
        return;
    }
    let is_array_like = matches!(shape.java_ty, JavaType::Array(_))
        || optional_element_type(&shape.java_ty)
            .is_some_and(|element_ty| matches!(element_ty, JavaType::Array(_)));
    if is_array_like {
        apply_required_view(shape, ViewShape::Slice);
    }
}

pub fn merge_signature_flag_requirement_policy(current: bool, next: bool) -> SignatureFlagMerge {
    SignatureFlagMerge {
        kept: current || next,
    }
}

pub fn signature_constraint_provenance(
    constraint: &SignatureConstraint,
) -> SignatureConstraintProvenance {
    match constraint {
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned) => {
            SignatureConstraintProvenance::OwnedRequirement
        }
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow) => {
            SignatureConstraintProvenance::BorrowedRequirement
        }
        SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow) => {
            SignatureConstraintProvenance::MutableBorrowRequirement
        }
        SignatureConstraint::RequiredOwnership(OwnershipShape::CopyValue) => {
            SignatureConstraintProvenance::CopyRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::Native) => {
            SignatureConstraintProvenance::NativeViewRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::Str) => {
            SignatureConstraintProvenance::StringViewRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::Slice) => {
            SignatureConstraintProvenance::SliceViewRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::DynTraitView) => {
            SignatureConstraintProvenance::DynTraitViewRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::EnumView) => {
            SignatureConstraintProvenance::EnumViewRequirement
        }
        SignatureConstraint::RequiredView(ViewShape::ErasedObjectView) => {
            SignatureConstraintProvenance::ErasedObjectRequirement
        }
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete) => {
            SignatureConstraintProvenance::ConcreteDispatchRequirement
        }
        SignatureConstraint::RequiredDispatch(DispatchShape::DynTrait { .. }) => {
            SignatureConstraintProvenance::DynTraitRequirement
        }
        SignatureConstraint::RequiredDispatch(DispatchShape::BoxedDynTrait { .. }) => {
            SignatureConstraintProvenance::BoxedDynTraitRequirement
        }
        SignatureConstraint::RequiredDispatch(DispatchShape::ClosedEnum { .. }) => {
            SignatureConstraintProvenance::ClosedEnumRequirement
        }
        SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject) => {
            SignatureConstraintProvenance::ErasedObjectRequirement
        }
        SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull) => {
            SignatureConstraintProvenance::NonNullStorageRequirement
        }
        SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNull)
        | SignatureConstraint::RequiredStorageNullability(NullabilityState::MaybeNull) => {
            SignatureConstraintProvenance::OptionalStorageRequirement
        }
        SignatureConstraint::RequiredStorageNullability(NullabilityState::Unknown) => {
            SignatureConstraintProvenance::UnknownStorageRequirement
        }
        SignatureConstraint::NullableSequenceElement => {
            SignatureConstraintProvenance::OptionalStorageRequirement
        }
        SignatureConstraint::MutableBinding => {
            SignatureConstraintProvenance::MutableBindingRequirement
        }
        SignatureConstraint::StorageSlot => SignatureConstraintProvenance::StorageSlotRequirement,
    }
}

pub fn signature_constraint_provenance_label(
    provenance: SignatureConstraintProvenance,
) -> &'static str {
    match provenance {
        SignatureConstraintProvenance::OwnedRequirement => "owned",
        SignatureConstraintProvenance::BorrowedRequirement => "borrowed view",
        SignatureConstraintProvenance::MutableBorrowRequirement => "mutable borrow",
        SignatureConstraintProvenance::CopyRequirement => "copy value",
        SignatureConstraintProvenance::NativeViewRequirement => "native Rust view",
        SignatureConstraintProvenance::StringViewRequirement => "string slice view",
        SignatureConstraintProvenance::SliceViewRequirement => "slice view",
        SignatureConstraintProvenance::DynTraitViewRequirement => "dyn trait view",
        SignatureConstraintProvenance::EnumViewRequirement => "enum view",
        SignatureConstraintProvenance::ErasedObjectRequirement => "erased object",
        SignatureConstraintProvenance::ConcreteDispatchRequirement => "concrete dispatch",
        SignatureConstraintProvenance::DynTraitRequirement => "Box<dyn Trait>",
        SignatureConstraintProvenance::BoxedDynTraitRequirement => "Box<dyn Trait>",
        SignatureConstraintProvenance::ClosedEnumRequirement => "closed enum",
        SignatureConstraintProvenance::OptionalStorageRequirement => "Option",
        SignatureConstraintProvenance::NonNullStorageRequirement => "non-null storage",
        SignatureConstraintProvenance::UnknownStorageRequirement => "unknown nullability",
        SignatureConstraintProvenance::MutableBindingRequirement => "mutable binding",
        SignatureConstraintProvenance::StorageSlotRequirement => "storage slot",
    }
}

pub fn signature_constraint_conflict_diagnostic(
    conflict: &SignatureConstraintConflict,
) -> SignatureConstraintConflictDiagnostic {
    let kept_provenance = signature_constraint_provenance(&conflict.kept);
    let rejected_provenance = signature_constraint_provenance(&conflict.rejected);
    SignatureConstraintConflictDiagnostic {
        axis: conflict.axis,
        kept: conflict.kept.clone(),
        rejected: conflict.rejected.clone(),
        kept_provenance,
        rejected_provenance,
        message: format!(
            "{:?} join kept {:?} because it requires {}; rejected {:?} from {}",
            conflict.axis,
            conflict.kept,
            signature_constraint_provenance_label(kept_provenance),
            conflict.rejected,
            signature_constraint_provenance_label(rejected_provenance)
        ),
    }
}

pub fn merge_ownership_requirement_policy(
    current: Option<OwnershipShape>,
    next: OwnershipShape,
) -> SignatureRequirementMerge<OwnershipShape> {
    if matches!(
        (current, next),
        (Some(OwnershipShape::Owned), OwnershipShape::CopyValue)
            | (Some(OwnershipShape::CopyValue), OwnershipShape::Owned)
    ) {
        return SignatureRequirementMerge {
            kept: OwnershipShape::Owned,
            rejected: None,
        };
    }
    let merge = join_ranked_lattice(current, next, ownership_lattice_rank);
    SignatureRequirementMerge {
        kept: merge.kept,
        rejected: merge.rejected,
    }
}

pub fn merge_view_requirement_policy(
    current: Option<ViewShape>,
    next: ViewShape,
) -> SignatureRequirementMerge<ViewShape> {
    let merge = join_ranked_lattice(current, next, view_lattice_rank);
    SignatureRequirementMerge {
        kept: merge.kept,
        rejected: merge.rejected,
    }
}

pub fn merge_dispatch_requirement_policy(
    current: Option<DispatchShape>,
    next: DispatchShape,
) -> SignatureRequirementMerge<DispatchShape> {
    let merge = join_ranked_lattice(current, next, dispatch_shape_lattice_rank);
    SignatureRequirementMerge {
        kept: merge.kept,
        rejected: merge.rejected,
    }
}

pub fn merge_storage_nullability_requirement_policy(
    current: Option<NullabilityState>,
    next: NullabilityState,
) -> SignatureRequirementMerge<NullabilityState> {
    let merge = join_ranked_lattice(current, next, storage_nullability_lattice_rank);
    SignatureRequirementMerge {
        kept: merge.kept,
        rejected: merge.rejected,
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
    field_shape_constraints_with_owner_name(&ty.name, field, facts)
}

fn field_shape_constraints_with_owner_name(
    owner_type_name: &str,
    field: &FieldDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = field_shape_constraints();
    push_unique_signature_constraints(
        &mut constraints,
        &field_nullability_shape_constraints_with_owner_name(owner_type_name, field, facts),
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

fn field_nullability_shape_constraints_with_owner_name(
    owner_type_name: &str,
    field: &FieldDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let nullability = facts
        .field_nullability_for(owner_type_name, &field.name)
        .or_else(|| {
            field
                .initializer
                .as_ref()
                .and_then(source_expr_nullability_state)
        });
    nullability
        .map(SignatureConstraint::RequiredStorageNullability)
        .into_iter()
        .collect()
}

pub fn field_shape_constraints_with_usage(
    ty: &TypeDecl,
    field: &FieldDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = field_shape_constraints_with_facts(ty, field, facts);
    push_unique_signature_constraints(
        &mut constraints,
        &delegated_null_constructor_field_constraints(ty, field),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &external_call_return_field_constraints(ty, field, usage, facts),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &native_collection_receiver_field_constraints(ty, field),
    );
    if let Some(kind) = field_boundary_policy(ty, field, usage).kind {
        push_unique_signature_constraints(
            &mut constraints,
            &signature_boundary_policy(kind, &field.ty, facts).constraints,
        );
    }
    constraints
}

fn field_shape_constraints_with_usage_and_declared_source_types(
    ty: &TypeDecl,
    field: &FieldDecl,
    usage: &UsageFacts,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let mut constraints = field_shape_constraints_with_facts(ty, field, facts);
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_concrete_native_constraints(&field.ty, declared_source_types),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &java_platform_facade_subclass_concrete_native_constraints(
            &field.ty,
            facts,
            declared_source_types,
        ),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &delegated_null_constructor_field_constraints(ty, field),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &external_call_return_field_constraints(ty, field, usage, facts),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &native_collection_receiver_field_constraints(ty, field),
    );
    if let Some(kind) = field_boundary_policy(ty, field, usage).kind {
        push_unique_signature_constraints(
            &mut constraints,
            &signature_boundary_policy(kind, &field.ty, facts).constraints,
        );
    }
    constraints
}

fn delegated_null_constructor_field_constraints(
    ty: &TypeDecl,
    field: &FieldDecl,
) -> Vec<SignatureConstraint> {
    if matches!(field.ty, JavaType::Primitive(_) | JavaType::Void) {
        return Vec::new();
    }
    let field_name = field.name.as_str();
    for ctor in ty.members.iter().filter_map(|member| match member {
        crate::java_ir::Member::Constructor(ctor) => Some(ctor),
        _ => None,
    }) {
        let Some(body) = ctor.body.as_ref() else {
            continue;
        };
        let Some(Stmt::ConstructorInvocation {
            kind: crate::java_ir::ConstructorInvocationKind::This,
            args,
            resolved,
        }) = body.statements.first()
        else {
            continue;
        };
        let Some(target) = resolved
            .as_ref()
            .and_then(|resolved| constructor_decl_for_resolved_name(ty, &resolved.rust_name))
            .or_else(|| {
                constructor_decl_for_delegated_field_assignment(ty, args.len(), field_name)
            })
        else {
            continue;
        };
        let Some(target_param_index) = target.params.iter().position(|param| {
            target_constructor_param_initializes_field(target, &param.name, field_name)
        }) else {
            continue;
        };
        if args
            .get(target_param_index)
            .is_some_and(|arg| matches!(arg, Expr::Literal(Literal::Null)))
        {
            return vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull,
            )];
        }
    }
    Vec::new()
}

fn native_collection_receiver_field_constraints(
    ty: &TypeDecl,
    field: &FieldDecl,
) -> Vec<SignatureConstraint> {
    if crate::stdlib::java_collection_kind_for_type(&field.ty).is_none()
        || !field_is_native_collection_receiver(ty, &field.name, &field.ty)
    {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
    ]
}

fn field_is_native_collection_receiver(
    ty: &TypeDecl,
    field_name: &str,
    field_ty: &JavaType,
) -> bool {
    ty.members.iter().any(|member| match member {
        crate::java_ir::Member::Method(method) => method.body.as_ref().is_some_and(|body| {
            block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
        }),
        crate::java_ir::Member::Constructor(ctor) => ctor.body.as_ref().is_some_and(|body| {
            block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
        }),
        crate::java_ir::Member::Type(nested) => {
            field_is_native_collection_receiver(nested, field_name, field_ty)
        }
        crate::java_ir::Member::Field(field) => field.initializer.as_ref().is_some_and(|expr| {
            expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
        }),
        crate::java_ir::Member::Unsupported { .. } => false,
    })
}

fn block_uses_field_as_native_collection_receiver(
    block: &Block,
    field_name: &str,
    field_ty: &JavaType,
) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_uses_field_as_native_collection_receiver(stmt, field_name, field_ty))
}

fn stmt_uses_field_as_native_collection_receiver(
    stmt: &Stmt,
    field_name: &str,
    field_ty: &JavaType,
) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } | Stmt::Return(initializer) => {
            initializer.as_ref().is_some_and(|expr| {
                expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
            })
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
        }
        Stmt::Assign { target, value } => {
            expr_uses_field_as_native_collection_receiver(target, field_name, field_ty)
                || expr_uses_field_as_native_collection_receiver(value, field_name, field_ty)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_uses_field_as_native_collection_receiver(condition, field_name, field_ty)
                || block_uses_field_as_native_collection_receiver(then_block, field_name, field_ty)
                || else_block.as_ref().is_some_and(|block| {
                    block_uses_field_as_native_collection_receiver(block, field_name, field_ty)
                })
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_uses_field_as_native_collection_receiver(condition, field_name, field_ty)
                || block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| {
                stmt_uses_field_as_native_collection_receiver(stmt, field_name, field_ty)
            }) || condition.as_ref().is_some_and(|condition| {
                expr_uses_field_as_native_collection_receiver(condition, field_name, field_ty)
            }) || update.iter().any(|expr| {
                expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
            }) || block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_uses_field_as_native_collection_receiver(iterable, field_name, field_ty)
                || block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
        }
        Stmt::Switch { selector, cases } => {
            expr_uses_field_as_native_collection_receiver(selector, field_name, field_ty)
                || cases.iter().any(|case| {
                    case.statements.iter().any(|stmt| {
                        stmt_uses_field_as_native_collection_receiver(stmt, field_name, field_ty)
                    })
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(|stmt| {
                stmt_uses_field_as_native_collection_receiver(stmt, field_name, field_ty)
            }) || block_uses_field_as_native_collection_receiver(body, field_name, field_ty)
                || catches.iter().any(|catch| {
                    block_uses_field_as_native_collection_receiver(
                        &catch.body,
                        field_name,
                        field_ty,
                    )
                })
                || finally_block.as_ref().is_some_and(|block| {
                    block_uses_field_as_native_collection_receiver(block, field_name, field_ty)
                })
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)),
        Stmt::LocalType(nested) => {
            field_is_native_collection_receiver(nested, field_name, field_ty)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_uses_field_as_native_collection_receiver(block, field_name, field_ty)
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => false,
    }
}

fn expr_uses_field_as_native_collection_receiver(
    expr: &Expr,
    field_name: &str,
    field_ty: &JavaType,
) -> bool {
    match expr {
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } => {
            (crate::stdlib::native_collection_receiver_method_known(field_ty, name)
                && expr_is_this_field(receiver, field_name))
                || expr_uses_field_as_native_collection_receiver(receiver, field_name, field_ty)
                || args.iter().any(|arg| {
                    expr_uses_field_as_native_collection_receiver(arg, field_name, field_ty)
                })
        }
        Expr::Call { args, .. }
        | Expr::New {
            args, body: None, ..
        } => args
            .iter()
            .any(|arg| expr_uses_field_as_native_collection_receiver(arg, field_name, field_ty)),
        Expr::New {
            args,
            body: Some(body),
            ..
        } => {
            args.iter()
                .any(|arg| expr_uses_field_as_native_collection_receiver(arg, field_name, field_ty))
                || body.iter().any(|member| match member {
                    crate::java_ir::Member::Field(field) => {
                        field.initializer.as_ref().is_some_and(|expr| {
                            expr_uses_field_as_native_collection_receiver(
                                expr, field_name, field_ty,
                            )
                        })
                    }
                    crate::java_ir::Member::Method(method) => {
                        method.body.as_ref().is_some_and(|body| {
                            block_uses_field_as_native_collection_receiver(
                                body, field_name, field_ty,
                            )
                        })
                    }
                    crate::java_ir::Member::Constructor(ctor) => {
                        ctor.body.as_ref().is_some_and(|body| {
                            block_uses_field_as_native_collection_receiver(
                                body, field_name, field_ty,
                            )
                        })
                    }
                    crate::java_ir::Member::Type(nested) => {
                        field_is_native_collection_receiver(nested, field_name, field_ty)
                    }
                    crate::java_ir::Member::Unsupported { .. } => false,
                })
        }
        Expr::FieldAccess(receiver, _) => {
            expr_uses_field_as_native_collection_receiver(receiver, field_name, field_ty)
        }
        Expr::ArrayAccess(array, index) => {
            expr_uses_field_as_native_collection_receiver(array, field_name, field_ty)
                || expr_uses_field_as_native_collection_receiver(index, field_name, field_ty)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(|expr| {
                expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
            }) || initializer.iter().flatten().any(|expr| {
                expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
            })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)),
        Expr::Binary { left, right, .. } => {
            expr_uses_field_as_native_collection_receiver(left, field_name, field_ty)
                || expr_uses_field_as_native_collection_receiver(right, field_name, field_ty)
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_uses_field_as_native_collection_receiver(condition, field_name, field_ty)
                || expr_uses_field_as_native_collection_receiver(then_expr, field_name, field_ty)
                || expr_uses_field_as_native_collection_receiver(else_expr, field_name, field_ty)
        }
        Expr::Switch { selector, cases } => {
            expr_uses_field_as_native_collection_receiver(selector, field_name, field_ty)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| {
                matches!(
                    label,
                    SwitchLabel::Expr(expr)
                        if expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
                )
            }) || expr_uses_field_as_native_collection_receiver(&case.value, field_name, field_ty)
                })
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                expr_uses_field_as_native_collection_receiver(expr, field_name, field_ty)
            }
            LambdaBody::Block(block) => {
                block_uses_field_as_native_collection_receiver(block, field_name, field_ty)
            }
        },
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn expr_is_this_field(expr: &Expr, field_name: &str) -> bool {
    match expr {
        Expr::Var(name) => name == field_name,
        Expr::FieldAccess(receiver, field) => {
            field == field_name && matches!(receiver.as_ref(), Expr::This)
        }
        _ => false,
    }
}

fn constructor_decl_for_resolved_name<'a>(
    ty: &'a TypeDecl,
    rust_name: &str,
) -> Option<&'a ConstructorDecl> {
    ty.members.iter().find_map(|member| match member {
        crate::java_ir::Member::Constructor(ctor)
            if constructor_rust_name(ctor) == rust_name
                || ctor.rust_name.as_deref() == Some(rust_name) =>
        {
            Some(ctor)
        }
        _ => None,
    })
}

fn constructor_decl_for_delegated_field_assignment<'a>(
    ty: &'a TypeDecl,
    arity: usize,
    field_name: &str,
) -> Option<&'a ConstructorDecl> {
    ty.members.iter().find_map(|member| match member {
        crate::java_ir::Member::Constructor(ctor)
            if ctor.params.len() == arity
                && ctor.params.iter().any(|param| {
                    target_constructor_param_initializes_field(ctor, &param.name, field_name)
                }) =>
        {
            Some(ctor)
        }
        _ => None,
    })
}

fn target_constructor_param_initializes_field(
    ctor: &ConstructorDecl,
    param_name: &str,
    field_name: &str,
) -> bool {
    let Some(body) = ctor.body.as_ref() else {
        return false;
    };
    body.statements.iter().any(|stmt| match stmt {
        Stmt::Assign { target, value } => {
            constructor_assignment_targets_field(target, field_name)
                && matches!(value, Expr::Var(name) if name == param_name)
        }
        Stmt::Expr(Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        }) => {
            constructor_assignment_targets_field(left, field_name)
                && matches!(right.as_ref(), Expr::Var(name) if name == param_name)
        }
        _ => false,
    })
}

fn constructor_assignment_targets_field(expr: &Expr, field_name: &str) -> bool {
    match expr {
        Expr::Var(name) => name == field_name,
        Expr::FieldAccess(receiver, field)
            if matches!(receiver.as_ref(), Expr::This) && field == field_name =>
        {
            true
        }
        _ => false,
    }
}

fn find_signature_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    unique_signature_type_decl_match(collect_signature_type_decl_matches(program, |ty| {
        ty.name == type_name
    }))
    .or_else(|| {
        unique_signature_type_decl_match(collect_signature_type_decl_matches(program, |ty| {
            ty.name != type_name && crate::facts::external_type_name_matches(&ty.name, type_name)
        }))
    })
}

fn collect_signature_type_decl_matches<'a>(
    program: &'a Program,
    matches_query: impl Fn(&TypeDecl) -> bool,
) -> Vec<&'a TypeDecl> {
    let mut matches = Vec::new();
    for ty in &program.types {
        collect_signature_type_decl_matches_in_type(ty, &matches_query, &mut matches);
    }
    matches
}

fn collect_signature_type_decl_matches_in_type<'a>(
    ty: &'a TypeDecl,
    matches_query: &impl Fn(&TypeDecl) -> bool,
    matches: &mut Vec<&'a TypeDecl>,
) {
    if matches_query(ty) {
        matches.push(ty);
    }
    for member in &ty.members {
        if let crate::java_ir::Member::Type(nested) = member {
            collect_signature_type_decl_matches_in_type(nested, matches_query, matches);
        }
    }
}

fn unique_signature_type_decl_match(matches: Vec<&TypeDecl>) -> Option<&TypeDecl> {
    match matches.as_slice() {
        [ty] => Some(*ty),
        _ => None,
    }
}

pub fn external_call_return_field_constraints(
    ty: &TypeDecl,
    field: &FieldDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
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
    for field_use in method_field_use.chain(constructor_field_use) {
        let FieldUse::AssignedFromCall {
            field_name,
            name,
            receiver_type,
            boundary: CallArgumentBoundary::Unknown,
        } = field_use
        else {
            continue;
        };
        if field_name == &field.name {
            push_external_call_return_field_constraints(
                &mut constraints,
                receiver_type.as_ref(),
                name,
                &field.ty,
                facts,
            );
        }
    }
    constraints
}

fn push_external_call_return_field_constraints(
    constraints: &mut Vec<SignatureConstraint>,
    receiver_type: Option<&JavaType>,
    name: &str,
    field_ty: &JavaType,
    facts: &FactSet,
) {
    let receiver_type = receiver_type.and_then(external_api_receiver_type_key_without_type_env);
    let Some(effect) = facts.external_api_effect_for_receiver(receiver_type.as_deref(), name)
    else {
        return;
    };
    if effect.return_nullability != NullabilityState::Unknown
        && external_effect_return_nullability_is_signature_evidence(effect)
    {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                effect.return_nullability,
            )],
        );
    }
    if effect.return_type.as_ref().is_some_and(|return_ty| {
        java_types_have_same_signature_boundary_identity(return_ty, field_ty)
    }) {
        push_unique_signature_constraints(
            constraints,
            &[
                SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
                SignatureConstraint::RequiredView(ViewShape::Native),
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            ],
        );
    }
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
    type_param_scope: TypeParamScope<'_>,
) -> RustSlotShape {
    let mut shape = solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: java_ty.clone(),
            role: SignatureSlotRole::Storage,
            constraints: constraints.to_vec(),
            type_params: type_param_scope.to_vec(),
        },
        facts,
    )
    .expect("field signature shape is always present");
    if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) {
        if source_java_enum_storage_is_copy(representations, java_ty) {
            apply_required_ownership(&mut shape, OwnershipShape::CopyValue);
            apply_required_dispatch(&mut shape, DispatchShape::Concrete);
            apply_required_view(&mut shape, ViewShape::Native);
        } else if !signature_capability_policy(java_ty, facts)
            .capability_ownership
            .is_some_and(|ownership| ownership == OwnershipShape::CopyValue)
        {
            apply_required_ownership(&mut shape, OwnershipShape::Owned);
        }
        if matches!(shape.dispatch, DispatchShape::Concrete) {
            apply_required_view(&mut shape, ViewShape::Native);
        }
    }
    shape
}

fn source_java_enum_storage_is_copy(
    representations: &RepresentationPlan,
    java_ty: &JavaType,
) -> bool {
    matches!(
        representations.representation_for(java_ty),
        crate::object_model::ObjectRepresentation::JavaEnum { .. }
    )
}

fn collect_type_signature_constraints(
    ty: &TypeDecl,
    usage: &UsageFacts,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
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
                    field_shape_constraints_with_usage_and_declared_source_types(
                        ty,
                        field,
                        usage,
                        facts,
                        declared_source_types,
                    ),
                );
            }
            crate::java_ir::Member::Method(method) => {
                let captured = captured_binding_names_for_method(facts, &ty.name, &method.name);
                let method_usage = usage.source_method_usage(ty, method);
                let param_names = method
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let params = method
                    .params
                    .iter()
                    .map(|param| {
                        let mut constraints =
                            method_param_shape_constraints_with_declared_source_types(
                                ty,
                                method,
                                param,
                                usage,
                                facts,
                                declared_source_types,
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
                    .collect::<Vec<_>>();
                let locals = method
                    .body
                    .as_ref()
                    .map(|body| {
                        collect_local_constraint_sets(
                            body,
                            &method.params,
                            method_usage,
                            facts,
                            &captured,
                            declared_source_types,
                        )
                    })
                    .unwrap_or_default();
                constraints.methods.insert(
                    MethodKey {
                        type_name: ty.name.clone(),
                        rust_name: method_rust_name(method),
                    },
                    MethodConstraintSet {
                        public_api: signature_member_api_boundary_policy(&method.modifiers)
                            .public_api,
                        param_index_by_name: index_names(&param_names),
                        param_java_types: method
                            .params
                            .iter()
                            .map(|param| param.ty.clone())
                            .collect(),
                        local_index_by_name: index_local_constraints(&locals),
                        local_index_by_binding: index_local_constraints_by_binding(&locals),
                        param_names,
                        params,
                        return_constraints:
                            method_return_shape_constraints_with_declared_source_types(
                                ty,
                                method,
                                usage,
                                facts,
                                declared_source_types,
                            ),
                        locals,
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
                let param_names = ctor
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>();
                let params = ctor
                    .params
                    .iter()
                    .map(|param| {
                        let mut constraints =
                            constructor_param_shape_constraints_with_declared_source_types(
                                ty,
                                ctor,
                                param,
                                usage,
                                facts,
                                declared_source_types,
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
                    .collect::<Vec<_>>();
                let locals = {
                    let constructor_usage = usage.constructors.get(&UsageConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: constructor_rust_name(ctor),
                    });
                    ctor.body
                        .as_ref()
                        .map(|body| {
                            collect_local_constraint_sets(
                                body,
                                &ctor.params,
                                constructor_usage,
                                facts,
                                &captured,
                                declared_source_types,
                            )
                        })
                        .unwrap_or_default()
                };
                constraints.constructors.insert(
                    ConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: constructor_rust_name(ctor),
                    },
                    ConstructorConstraintSet {
                        public_api: signature_member_api_boundary_policy(&ctor.modifiers)
                            .public_api,
                        param_index_by_name: index_names(&param_names),
                        local_index_by_name: index_local_constraints(&locals),
                        local_index_by_binding: index_local_constraints_by_binding(&locals),
                        param_names,
                        params,
                        locals,
                    },
                );
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signature_constraints(
                    nested,
                    usage,
                    facts,
                    declared_source_types,
                    constraints,
                );
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

pub fn source_binding_nullability_constraints(
    block: &Block,
    name: &str,
) -> Vec<SignatureConstraint> {
    if !block_directly_declares_local_name(block, name) {
        return Vec::new();
    }
    collect_binding_nullability_state_map(block).constraints_for_name(name)
}

pub fn source_binding_nullability_constraints_for_declaration(
    block: &Block,
    name: &str,
    binding: Option<&BindingId>,
    params: &[Param],
) -> Vec<SignatureConstraint> {
    let source_nullability = collect_binding_nullability_state_map_with_params(block, params);
    source_nullability.constraints_for_declaration(name, binding)
}

#[derive(Debug, Clone, Default)]
struct SourceNullabilityMap {
    by_name: HashMap<String, NullabilityState>,
    by_binding: HashMap<BindingId, NullabilityState>,
    return_by_binding: HashMap<BindingId, NullabilityState>,
}

impl SourceNullabilityMap {
    fn constraints_for_name(&self, name: &str) -> Vec<SignatureConstraint> {
        self.by_name
            .get(name)
            .copied()
            .map(source_nullability_constraint)
            .unwrap_or_default()
    }

    fn constraints_for_binding(&self, binding: &BindingId) -> Vec<SignatureConstraint> {
        self.by_binding
            .get(binding)
            .copied()
            .map(source_nullability_constraint)
            .unwrap_or_default()
    }

    fn constraints_for_declaration(
        &self,
        name: &str,
        binding: Option<&BindingId>,
    ) -> Vec<SignatureConstraint> {
        binding
            .map(|binding| self.constraints_for_binding(binding))
            .unwrap_or_else(|| self.constraints_for_name(name))
    }

    fn state_for_var(
        &self,
        name: &str,
        scopes: &[HashMap<String, BindingId>],
    ) -> Option<NullabilityState> {
        if let Some(binding) = scoped_binding(scopes, name) {
            return Some(
                self.return_by_binding
                    .get(binding)
                    .copied()
                    .or_else(|| self.by_binding.get(binding).copied())
                    .unwrap_or(NullabilityState::Unknown),
            );
        }
        self.by_name.get(name).copied()
    }
}

fn source_nullability_constraint(state: NullabilityState) -> Vec<SignatureConstraint> {
    vec![SignatureConstraint::RequiredStorageNullability(state)]
}

pub fn source_method_return_nullability_constraints(
    method: &MethodDecl,
) -> Vec<SignatureConstraint> {
    source_method_return_nullability_constraints_with_context(method, None)
}

fn typed_source_method_return_nullability_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let Some(body) = &method.body else {
        return Vec::new();
    };
    let ctx = SourceReturnNullabilityContext::for_method(ty, method, body, facts);
    source_method_return_nullability_constraints_with_context(method, Some(&ctx))
}

fn source_method_return_nullability_constraints_with_context(
    method: &MethodDecl,
    ctx: Option<&SourceReturnNullabilityContext<'_>>,
) -> Vec<SignatureConstraint> {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return Vec::new();
    }
    let Some(body) = &method.body else {
        return Vec::new();
    };
    let source_nullability =
        collect_binding_nullability_state_map_with_params(body, &method.params);
    let mut states = Vec::new();
    let mut scopes = source_nullability_scopes_for_method(method);
    collect_source_return_nullability_states_from_stmts(
        &body.statements,
        &source_nullability,
        ctx,
        &mut scopes,
        &mut states,
    );
    let state = merge_source_nullability_states(&states);
    if matches!(state, NullabilityState::Unknown) {
        Vec::new()
    } else {
        vec![SignatureConstraint::RequiredStorageNullability(state)]
    }
}

fn returned_local_initializer_source_call_nullability_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return Vec::new();
    }
    let Some(body) = &method.body else {
        return Vec::new();
    };
    let mut local_states = HashMap::<String, NullabilityState>::new();
    let mut return_states = Vec::new();
    collect_returned_local_initializer_call_states_from_stmts(
        &body.statements,
        ty,
        method,
        facts,
        &mut local_states,
        &mut return_states,
    );
    let state = merge_source_nullability_states(&return_states);
    if matches!(state, NullabilityState::Unknown) {
        Vec::new()
    } else {
        vec![SignatureConstraint::RequiredStorageNullability(state)]
    }
}

fn collect_returned_local_initializer_call_states_from_stmts(
    statements: &[Stmt],
    ty: &TypeDecl,
    current_method: &MethodDecl,
    facts: &FactSet,
    local_states: &mut HashMap<String, NullabilityState>,
    return_states: &mut Vec<NullabilityState>,
) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                name, initializer, ..
            } => {
                match initializer.as_ref().and_then(|initializer| {
                    same_type_source_call_nullability_state(ty, current_method, initializer, facts)
                }) {
                    Some(state) => {
                        local_states.insert(name.clone(), state);
                    }
                    None => {
                        local_states.remove(name);
                    }
                }
            }
            Stmt::Assign { target, value } => {
                if let Expr::Var(name) = target {
                    match same_type_source_call_nullability_state(ty, current_method, value, facts)
                    {
                        Some(state) => {
                            local_states.insert(name.clone(), state);
                        }
                        None => {
                            local_states.remove(name);
                        }
                    }
                }
            }
            Stmt::Return(Some(Expr::Var(name))) => {
                if let Some(state) = local_states.get(name).copied() {
                    return_states.push(state);
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_returned_local_initializer_call_states_from_stmts(
                    &then_block.statements,
                    ty,
                    current_method,
                    facts,
                    &mut local_states.clone(),
                    return_states,
                );
                if let Some(else_block) = else_block {
                    collect_returned_local_initializer_call_states_from_stmts(
                        &else_block.statements,
                        ty,
                        current_method,
                        facts,
                        &mut local_states.clone(),
                        return_states,
                    );
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_returned_local_initializer_call_states_from_stmts(
                    &block.statements,
                    ty,
                    current_method,
                    facts,
                    &mut local_states.clone(),
                    return_states,
                )
            }
            _ => {}
        }
    }
}

fn same_type_source_call_nullability_state(
    ty: &TypeDecl,
    current_method: &MethodDecl,
    expr: &Expr,
    facts: &FactSet,
) -> Option<NullabilityState> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if !matches!(
        receiver.as_deref(),
        None | Some(Expr::This) | Some(Expr::Super)
    ) {
        return None;
    }
    if current_method.name == *name && current_method.params.len() == args.len() {
        return None;
    }
    ty.members.iter().find_map(|member| {
        let crate::java_ir::Member::Method(callee) = member else {
            return None;
        };
        if callee.name != *name || callee.params.len() != args.len() {
            return None;
        }
        typed_source_method_return_nullability_constraints(ty, callee, facts)
            .into_iter()
            .find_map(|constraint| match constraint {
                SignatureConstraint::RequiredStorageNullability(state) => Some(state),
                _ => None,
            })
    })
}

struct SourceReturnNullabilityContext<'a> {
    type_name: &'a str,
    declared_return_type: &'a JavaType,
    facts: &'a FactSet,
    field_names: HashSet<String>,
    binding_types: HashMap<String, JavaType>,
    type_param_bounds: HashMap<String, JavaType>,
}

impl<'a> SourceReturnNullabilityContext<'a> {
    fn for_method(
        ty: &'a TypeDecl,
        method: &'a MethodDecl,
        _body: &Block,
        facts: &'a FactSet,
    ) -> Self {
        let mut field_names = HashSet::new();
        let mut binding_types = method
            .params
            .iter()
            .map(|param| (param.name.clone(), param.ty.clone()))
            .collect::<HashMap<_, _>>();
        let type_param_bounds = ty
            .type_params
            .iter()
            .zip(ty.type_param_bounds.iter())
            .filter_map(|(name, bound)| bound.clone().map(|bound| (name.clone(), bound)))
            .chain(
                method
                    .type_params
                    .iter()
                    .zip(method.type_param_bounds.iter())
                    .filter_map(|(name, bound)| bound.clone().map(|bound| (name.clone(), bound))),
            )
            .collect::<HashMap<_, _>>();
        for member in &ty.members {
            if let crate::java_ir::Member::Field(field) = member {
                field_names.insert(field.name.clone());
                field_names.insert(crate::parser::to_snake_case(&field.name));
                binding_types.insert(field.name.clone(), field.ty.clone());
                binding_types.insert(crate::parser::to_snake_case(&field.name), field.ty.clone());
            }
        }
        Self {
            type_name: &ty.name,
            declared_return_type: &method.return_type,
            facts,
            field_names,
            binding_types,
            type_param_bounds,
        }
    }

    fn field_nullability_for_expr(
        &self,
        expr: &Expr,
        scopes: &[HashMap<String, BindingId>],
    ) -> Option<NullabilityState> {
        let field_name = match expr {
            Expr::Var(name)
                if self.field_names.contains(name) && scoped_binding(scopes, name).is_none() =>
            {
                name.as_str()
            }
            Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => field.as_str(),
            _ => return None,
        };
        self.facts.field_nullability_for(self.type_name, field_name)
    }

    fn expr_nullability_state(&self, expr: &Expr) -> Option<NullabilityState> {
        if let Some(state) = self.type_param_bound_call_nullability_state(expr) {
            return Some(state);
        }
        if let Some(state) = self.external_call_nullability_state(expr) {
            return Some(state);
        }
        if expr_is_java_map_get_call(expr, &self.binding_types) {
            return Some(NullabilityState::MaybeNull);
        }
        source_expr_nullability_state(expr)
    }

    fn external_call_nullability_state(&self, expr: &Expr) -> Option<NullabilityState> {
        let Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } = expr
        else {
            return None;
        };
        if let Some(resolved) = resolved {
            let state = crate::stdlib::external_api_return_nullability(
                Some(&resolved.declaring_type),
                name,
                args.len(),
                crate::facts::ExternalApiRole::Getter,
            );
            return (!matches!(state, NullabilityState::Unknown)).then_some(state);
        }
        let receiver_ty = receiver
            .as_deref()
            .and_then(|receiver| self.java_type_for_expr(receiver));
        let receiver_key = receiver_ty
            .as_ref()
            .and_then(java_type_name_for_signature_dispatch);
        let state = crate::stdlib::external_api_return_nullability(
            receiver_key.as_deref(),
            name,
            args.len(),
            crate::facts::ExternalApiRole::Getter,
        );
        (!matches!(state, NullabilityState::Unknown)).then_some(state)
    }

    fn java_type_for_expr(&self, expr: &Expr) -> Option<JavaType> {
        match expr {
            Expr::Literal(literal) => Some(crate::types::literal_type(literal)),
            Expr::Var(name) => self.binding_types.get(name).cloned(),
            Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => {
                self.binding_types.get(field).cloned()
            }
            Expr::Ternary {
                then_expr,
                else_expr,
                ..
            } => {
                let then_ty = self.java_type_for_expr(then_expr)?;
                let else_ty = self.java_type_for_expr(else_expr)?;
                if then_ty == else_ty {
                    Some(then_ty)
                } else {
                    Some(JavaType::Unknown("ternary".to_string()))
                }
            }
            Expr::Call {
                receiver,
                name,
                args,
                resolved,
            } => resolved
                .as_ref()
                .map(|call| call.return_type.clone())
                .or_else(|| {
                    let receiver_ty = receiver
                        .as_deref()
                        .and_then(|receiver| self.java_type_for_expr(receiver));
                    let arg_types = args
                        .iter()
                        .map(|arg| self.java_type_for_expr(arg))
                        .collect::<Option<Vec<_>>>()?;
                    crate::stdlib::known_external_method_return_type_for_call(
                        receiver_ty.as_ref(),
                        name,
                        &arg_types,
                    )
                }),
            Expr::New { ty, .. } | Expr::Cast { ty, .. } => Some(ty.clone()),
            Expr::ClassLiteral(ty) => Some(crate::types::class_literal_type(ty)),
            Expr::ArrayAccess(array, _) => {
                crate::types::sequence_element_type_after_optional(&self.java_type_for_expr(array)?)
            }
            Expr::ArrayCreation {
                element_type,
                dimensions,
                initializer,
            } => Some(
                (0..initializer.as_ref().map_or(dimensions.len(), |_| 1).max(1))
                    .fold(element_type.clone(), |ty, _| JavaType::Array(Box::new(ty))),
            ),
            Expr::ArrayInitializer(elements) => elements
                .first()
                .and_then(|element| self.java_type_for_expr(element))
                .map(|element_ty| JavaType::Array(Box::new(element_ty))),
            Expr::Unary { expr, .. } => self.java_type_for_expr(expr),
            Expr::Binary { .. }
            | Expr::FieldAccess(_, _)
            | Expr::This
            | Expr::Super
            | Expr::Switch { .. }
            | Expr::InstanceOf { .. }
            | Expr::Lambda { .. }
            | Expr::MethodReference { .. }
            | Expr::Unsupported { .. }
            | Expr::Raw(_) => None,
        }
    }

    fn type_param_bound_call_nullability_state(&self, expr: &Expr) -> Option<NullabilityState> {
        let Expr::Call {
            receiver: Some(receiver),
            resolved,
            ..
        } = expr
        else {
            return None;
        };
        let bound = self.type_param_bound_for_receiver(receiver)?;
        if let Some(resolved) = resolved {
            return java_type_name_for_signature_dispatch(bound).and_then(|bound_name| {
                crate::facts::external_type_name_matches(bound_name, &resolved.declaring_type)
                    .then(|| type_param_bound_method_return_nullability(&resolved.return_type))
            });
        }
        Some(type_param_bound_method_return_nullability(
            self.declared_return_type,
        ))
    }

    fn type_param_bound_for_receiver(&self, receiver: &Expr) -> Option<&JavaType> {
        let receiver_ty = receiver_expr_java_type(receiver, &self.binding_types)?;
        let receiver_ty = crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
        let type_param = match receiver_ty {
            JavaType::Unknown(type_param) => type_param,
            JavaType::Named { name, args } if args.is_empty() => name,
            _ => return None,
        };
        self.type_param_bounds.get(&type_param)
    }
}

fn type_param_bound_method_return_nullability(java_ty: &JavaType) -> NullabilityState {
    match java_ty {
        JavaType::Void | JavaType::Primitive(_) => NullabilityState::DefinitelyNonNull,
        JavaType::String
        | JavaType::Named { .. }
        | JavaType::Array(_)
        | JavaType::Wildcard(_)
        | JavaType::Unknown(_) => NullabilityState::MaybeNull,
    }
}

fn is_this_receiver_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::This) || matches!(expr, Expr::Var(name) if name == "this")
}

fn collect_binding_nullability_state_map(block: &Block) -> SourceNullabilityMap {
    collect_binding_nullability_state_map_with_params(block, &[])
}

fn collect_binding_nullability_state_map_with_params(
    block: &Block,
    params: &[Param],
) -> SourceNullabilityMap {
    let mut by_name = HashMap::<String, NullabilityState>::new();
    collect_binding_nullability_states_from_stmts(&block.statements, &mut by_name);
    for duplicate in collect_duplicate_declared_local_names(block) {
        by_name.remove(&duplicate);
    }
    remove_resolver_bound_nullability_names(&mut by_name, block, params);
    let mut by_binding = HashMap::<BindingId, NullabilityState>::new();
    let mut return_by_binding = HashMap::<BindingId, NullabilityState>::new();
    let mut scopes = source_nullability_scopes_for_params(params);
    collect_binding_nullability_states_by_binding_from_stmts(
        &block.statements,
        &mut scopes,
        &mut by_binding,
        &mut return_by_binding,
    );
    SourceNullabilityMap {
        by_name,
        by_binding,
        return_by_binding,
    }
}

fn collect_source_return_nullability_states_from_stmts(
    statements: &[Stmt],
    source_nullability: &SourceNullabilityMap,
    ctx: Option<&SourceReturnNullabilityContext<'_>>,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    states: &mut Vec<NullabilityState>,
) {
    let mut refinements = HashSet::new();
    collect_source_return_nullability_states_from_stmts_with_refinements(
        statements,
        source_nullability,
        ctx,
        scopes,
        &mut refinements,
        states,
    );
}

fn source_nullability_scopes_for_method(method: &MethodDecl) -> Vec<HashMap<String, BindingId>> {
    source_nullability_scopes_for_params(&method.params)
}

fn source_nullability_scopes_for_params(params: &[Param]) -> Vec<HashMap<String, BindingId>> {
    let mut root = HashMap::new();
    for param in params {
        if let Some(binding) = &param.binding {
            root.insert(param.source_name.clone(), binding.clone());
        }
    }
    vec![root]
}

fn collect_source_return_nullability_states_from_stmts_with_refinements(
    statements: &[Stmt],
    source_nullability: &SourceNullabilityMap,
    ctx: Option<&SourceReturnNullabilityContext<'_>>,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    refinements: &mut HashSet<String>,
    states: &mut Vec<NullabilityState>,
) {
    for stmt in statements {
        collect_source_return_nullability_states_from_stmt(
            stmt,
            source_nullability,
            ctx,
            scopes,
            refinements,
            states,
        );
        update_post_stmt_nullability_refinements(stmt, refinements);
    }
}

fn collect_source_return_nullability_states_from_stmt(
    stmt: &Stmt,
    source_nullability: &SourceNullabilityMap,
    ctx: Option<&SourceReturnNullabilityContext<'_>>,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    refinements: &HashSet<String>,
    states: &mut Vec<NullabilityState>,
) {
    match stmt {
        Stmt::Return(Some(expr)) => {
            states.push(source_return_expr_nullability_state(
                expr,
                source_nullability,
                ctx,
                scopes,
                refinements,
            ));
        }
        Stmt::Return(None) => states.push(NullabilityState::DefinitelyNull),
        Stmt::VarDecl {
            source_name,
            binding,
            ..
        } => {
            if let Some(binding) = binding {
                insert_scoped_binding(scopes, source_name, binding.clone());
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            scopes.push(HashMap::new());
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &then_block.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
            if let Some(else_block) = else_block {
                scopes.push(HashMap::new());
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &else_block.statements,
                    source_nullability,
                    ctx,
                    scopes,
                    &mut refinements.clone(),
                    states,
                );
                scopes.pop();
            }
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } => {
            scopes.push(HashMap::new());
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
        }
        Stmt::EnhancedFor {
            source_var_name,
            binding,
            body,
            ..
        } => {
            scopes.push(HashMap::new());
            if let Some(binding) = binding {
                insert_scoped_binding(scopes, source_var_name, binding.clone());
            }
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
        }
        Stmt::For { init, body, .. } => {
            scopes.push(HashMap::new());
            collect_source_return_nullability_states_from_stmts_with_refinements(
                init,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            scopes.push(HashMap::new());
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                scopes.push(HashMap::new());
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &case.statements,
                    source_nullability,
                    ctx,
                    scopes,
                    &mut refinements.clone(),
                    states,
                );
                scopes.pop();
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            collect_source_return_nullability_states_from_stmts_with_refinements(
                resources,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.push(HashMap::new());
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                source_nullability,
                ctx,
                scopes,
                &mut refinements.clone(),
                states,
            );
            scopes.pop();
            for catch in catches {
                scopes.push(HashMap::new());
                if let Some(binding) = &catch.binding {
                    insert_scoped_binding(scopes, &catch.source_name, binding.clone());
                }
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &catch.body.statements,
                    source_nullability,
                    ctx,
                    scopes,
                    &mut refinements.clone(),
                    states,
                );
                scopes.pop();
            }
            if let Some(finally_block) = finally_block {
                scopes.push(HashMap::new());
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &finally_block.statements,
                    source_nullability,
                    ctx,
                    scopes,
                    &mut refinements.clone(),
                    states,
                );
                scopes.pop();
            }
        }
        Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Throw(_)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn source_return_expr_nullability_state(
    expr: &Expr,
    source_nullability: &SourceNullabilityMap,
    ctx: Option<&SourceReturnNullabilityContext<'_>>,
    scopes: &[HashMap<String, BindingId>],
    refinements: &HashSet<String>,
) -> NullabilityState {
    match expr {
        Expr::Var(name) => {
            if refinements.contains(name) {
                NullabilityState::DefinitelyNonNull
            } else {
                source_nullability
                    .state_for_var(name, scopes)
                    .or_else(|| ctx.and_then(|ctx| ctx.field_nullability_for_expr(expr, scopes)))
                    .unwrap_or(NullabilityState::Unknown)
            }
        }
        Expr::FieldAccess(_, _) => ctx
            .and_then(|ctx| ctx.field_nullability_for_expr(expr, scopes))
            .unwrap_or_else(|| {
                ctx.and_then(|ctx| ctx.expr_nullability_state(expr))
                    .unwrap_or(NullabilityState::Unknown)
            }),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => merge_source_nullability_states(&[
            source_return_expr_nullability_state(
                then_expr,
                source_nullability,
                ctx,
                scopes,
                refinements,
            ),
            source_return_expr_nullability_state(
                else_expr,
                source_nullability,
                ctx,
                scopes,
                refinements,
            ),
        ]),
        _ => ctx
            .and_then(|ctx| ctx.expr_nullability_state(expr))
            .or_else(|| source_expr_nullability_state(expr))
            .unwrap_or(NullabilityState::Unknown),
    }
}

fn expr_is_java_map_get_call(expr: &Expr, binding_types: &HashMap<String, JavaType>) -> bool {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    if name != "get" || args.len() != 1 {
        return false;
    }
    receiver_expr_java_type(receiver, binding_types)
        .and_then(|ty| crate::stdlib::java_map_type_args(&ty).map(|_| ()))
        .is_some()
}

fn receiver_expr_java_type(
    expr: &Expr,
    binding_types: &HashMap<String, JavaType>,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) => binding_types.get(name).cloned(),
        Expr::FieldAccess(receiver, field) if is_this_receiver_expr(receiver) => {
            binding_types.get(field).cloned()
        }
        _ => None,
    }
}

fn update_post_stmt_nullability_refinements(stmt: &Stmt, refinements: &mut HashSet<String>) {
    let mut mutated = HashSet::new();
    collect_mutated_local_names_from_stmt(stmt, &mut mutated);
    for name in mutated {
        refinements.remove(&name);
    }
    if let Stmt::While {
        condition, body, ..
    } = stmt
    {
        let plan = plan_while_stmt(condition, body);
        if let PostLoopRefinementPlan::LocalNonNull { local_name } =
            post_loop_refinement_for_while(&plan)
        {
            refinements.insert(local_name.to_string());
        }
    }
}

fn merge_source_nullability_states(states: &[NullabilityState]) -> NullabilityState {
    states
        .iter()
        .copied()
        .reduce(crate::lattice::field_nullability_join)
        .unwrap_or(NullabilityState::Unknown)
}

fn remove_resolver_bound_nullability_names(
    by_name: &mut HashMap<String, NullabilityState>,
    block: &Block,
    params: &[Param],
) {
    for param in params {
        if param.binding.is_some() {
            remove_nullability_name_aliases(by_name, &param.name);
            remove_nullability_name_aliases(by_name, &param.source_name);
        }
    }
    remove_resolver_bound_nullability_names_from_stmts(by_name, &block.statements);
}

fn remove_resolver_bound_nullability_names_from_stmts(
    by_name: &mut HashMap<String, NullabilityState>,
    statements: &[Stmt],
) {
    for stmt in statements {
        remove_resolver_bound_nullability_names_from_stmt(by_name, stmt);
    }
}

fn remove_resolver_bound_nullability_names_from_stmt(
    by_name: &mut HashMap<String, NullabilityState>,
    stmt: &Stmt,
) {
    match stmt {
        Stmt::VarDecl {
            name,
            source_name,
            binding,
            ..
        } => {
            if binding.is_some() {
                remove_nullability_name_aliases(by_name, name);
                remove_nullability_name_aliases(by_name, source_name);
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            remove_resolver_bound_nullability_names_from_stmts(by_name, &then_block.statements);
            if let Some(else_block) = else_block {
                remove_resolver_bound_nullability_names_from_stmts(by_name, &else_block.statements);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            remove_resolver_bound_nullability_names_from_stmts(by_name, &body.statements);
        }
        Stmt::For { init, body, .. } => {
            remove_resolver_bound_nullability_names_from_stmts(by_name, init);
            remove_resolver_bound_nullability_names_from_stmts(by_name, &body.statements);
        }
        Stmt::EnhancedFor {
            var_name,
            source_var_name,
            body,
            binding,
            ..
        } => {
            if binding.is_some() {
                remove_nullability_name_aliases(by_name, var_name);
                remove_nullability_name_aliases(by_name, source_var_name);
            }
            remove_resolver_bound_nullability_names_from_stmts(by_name, &body.statements);
        }
        Stmt::Switch { cases, .. } => {
            for case in cases {
                remove_resolver_bound_nullability_names_from_stmts(by_name, &case.statements);
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            remove_resolver_bound_nullability_names_from_stmts(by_name, resources);
            remove_resolver_bound_nullability_names_from_stmts(by_name, &body.statements);
            for catch in catches {
                if catch.binding.is_some() {
                    remove_nullability_name_aliases(by_name, &catch.source_name);
                }
                remove_resolver_bound_nullability_names_from_stmts(by_name, &catch.body.statements);
            }
            if let Some(finally_block) = finally_block {
                remove_resolver_bound_nullability_names_from_stmts(
                    by_name,
                    &finally_block.statements,
                );
            }
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                match member {
                    crate::java_ir::Member::Method(method) => {
                        if let Some(body) = &method.body {
                            remove_resolver_bound_nullability_names_from_stmts(
                                by_name,
                                &body.statements,
                            );
                        }
                    }
                    crate::java_ir::Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            remove_resolver_bound_nullability_names_from_stmts(
                                by_name,
                                &body.statements,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn remove_nullability_name_aliases(by_name: &mut HashMap<String, NullabilityState>, name: &str) {
    by_name.remove(name);
    by_name.remove(&crate::parser::to_snake_case(name));
}

fn collect_binding_nullability_states_from_stmts(
    statements: &[Stmt],
    states: &mut HashMap<String, NullabilityState>,
) {
    for stmt in statements {
        collect_binding_nullability_states_from_stmt(stmt, states);
    }
}

fn collect_binding_nullability_states_from_stmt(
    stmt: &Stmt,
    states: &mut HashMap<String, NullabilityState>,
) {
    match stmt {
        Stmt::VarDecl {
            name,
            initializer: Some(initializer),
            ..
        } => {
            collect_binding_nullability_assignment(name, initializer, states);
            collect_binding_nullability_states_from_expr(initializer, states);
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target {
                collect_binding_nullability_assignment(name, value, states);
            }
            collect_binding_nullability_states_from_expr(value, states);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_binding_nullability_states_from_expr(expr, states);
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_binding_nullability_states_from_expr(condition, states);
            collect_binding_nullability_states_from_stmts(&then_block.statements, states);
            if let Some(else_block) = else_block {
                collect_binding_nullability_states_from_stmts(&else_block.statements, states);
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_binding_nullability_states_from_expr(condition, states);
            collect_binding_nullability_states_from_stmts(&body.statements, states);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            collect_binding_nullability_states_from_stmts(init, states);
            if let Some(condition) = condition {
                collect_binding_nullability_states_from_expr(condition, states);
            }
            for expr in update {
                collect_binding_nullability_states_from_expr(expr, states);
            }
            collect_binding_nullability_states_from_stmts(&body.statements, states);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_binding_nullability_states_from_expr(iterable, states);
            collect_binding_nullability_states_from_stmts(&body.statements, states);
        }
        Stmt::Switch { selector, cases } => {
            collect_binding_nullability_states_from_expr(selector, states);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_binding_nullability_states_from_expr(expr, states);
                    }
                }
                collect_binding_nullability_states_from_stmts(&case.statements, states);
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            collect_binding_nullability_states_from_stmts(resources, states);
            collect_binding_nullability_states_from_stmts(&body.statements, states);
            for catch in catches {
                collect_binding_nullability_states_from_stmts(&catch.body.statements, states);
            }
            if let Some(finally_block) = finally_block {
                collect_binding_nullability_states_from_stmts(&finally_block.statements, states);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_binding_nullability_states_from_stmts(&block.statements, states);
        }
        Stmt::LocalType(_) => {}
        Stmt::VarDecl {
            initializer: None, ..
        }
        | Stmt::Return(None)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_binding_nullability_states_from_expr(
    expr: &Expr,
    states: &mut HashMap<String, NullabilityState>,
) {
    match expr {
        Expr::Binary {
            op: BinaryOp::Eq | BinaryOp::Ne,
            left,
            right,
        } => {
            collect_null_comparison_binding(left, right, states);
            collect_null_comparison_binding(right, left, states);
            collect_null_comparison_assignment(left, right, states);
            collect_null_comparison_assignment(right, left, states);
            collect_binding_nullability_states_from_expr(left, states);
            collect_binding_nullability_states_from_expr(right, states);
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            if let Expr::Var(name) = left.as_ref() {
                collect_binding_nullability_assignment(name, right, states);
            }
            collect_binding_nullability_states_from_expr(right, states);
        }
        Expr::Binary { left, right, .. } => {
            collect_binding_nullability_states_from_expr(left, states);
            collect_binding_nullability_states_from_expr(right, states);
        }
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::FieldAccess(expr, _) => collect_binding_nullability_states_from_expr(expr, states),
        Expr::ArrayAccess(array, index) => {
            collect_binding_nullability_states_from_expr(array, states);
            collect_binding_nullability_states_from_expr(index, states);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_binding_nullability_states_from_expr(condition, states);
            collect_binding_nullability_states_from_expr(then_expr, states);
            collect_binding_nullability_states_from_expr(else_expr, states);
        }
        Expr::Switch { selector, cases } => {
            collect_binding_nullability_states_from_expr(selector, states);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_binding_nullability_states_from_expr(expr, states);
                    }
                }
                collect_binding_nullability_states_from_expr(&case.value, states);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_binding_nullability_states_from_expr(receiver, states);
            }
            for arg in args {
                collect_binding_nullability_states_from_expr(arg, states);
            }
        }
        Expr::New { args, body: _, .. } => {
            for arg in args {
                collect_binding_nullability_states_from_expr(arg, states);
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_binding_nullability_states_from_expr(dimension, states);
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    collect_binding_nullability_states_from_expr(element, states);
                }
            }
        }
        Expr::ArrayInitializer(elements) => {
            for element in elements {
                collect_binding_nullability_states_from_expr(element, states);
            }
        }
        Expr::Lambda { .. } => {}
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_binding_nullability_assignment(
    name: &str,
    value: &Expr,
    states: &mut HashMap<String, NullabilityState>,
) {
    if let Some(state) = source_expr_nullability_state(value) {
        merge_binding_nullability_state(states, name, state);
    }
}

fn collect_null_comparison_binding(
    maybe_binding: &Expr,
    maybe_null: &Expr,
    states: &mut HashMap<String, NullabilityState>,
) {
    if matches!(maybe_null, Expr::Literal(Literal::Null))
        && let Expr::Var(name) = maybe_binding
    {
        merge_binding_nullability_state(states, name, NullabilityState::MaybeNull);
    }
}

fn collect_null_comparison_assignment(
    maybe_assignment: &Expr,
    maybe_null: &Expr,
    states: &mut HashMap<String, NullabilityState>,
) {
    if !matches!(maybe_null, Expr::Literal(Literal::Null)) {
        return;
    }
    let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        ..
    } = maybe_assignment
    else {
        return;
    };
    if let Expr::Var(name) = left.as_ref() {
        merge_binding_nullability_state(states, name, NullabilityState::MaybeNull);
    }
}

fn source_expr_nullability_state(expr: &Expr) -> Option<NullabilityState> {
    match expr {
        Expr::Literal(Literal::Null) => Some(NullabilityState::DefinitelyNull),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => Some(crate::lattice::field_nullability_join(
            source_expr_nullability_state(then_expr).unwrap_or(NullabilityState::Unknown),
            source_expr_nullability_state(else_expr).unwrap_or(NullabilityState::Unknown),
        )),
        _ => None,
    }
}

fn source_return_direct_nullability_state(expr: &Expr) -> Option<NullabilityState> {
    match expr {
        Expr::New { .. } | Expr::ArrayCreation { .. } | Expr::ArrayInitializer(_) => {
            Some(NullabilityState::DefinitelyNonNull)
        }
        _ => source_expr_nullability_state(expr),
    }
}

fn initializer_proves_declared_non_null(
    expr: &Expr,
    declared_ty: &JavaType,
    facts: &FactSet,
    local_types: &HashMap<String, JavaType>,
) -> bool {
    matches!(
        source_expr_nullability_state(expr),
        Some(NullabilityState::DefinitelyNonNull)
    ) || external_initializer_proves_declared_non_null(expr, declared_ty, facts, local_types)
}

fn external_initializer_proves_declared_non_null(
    expr: &Expr,
    declared_ty: &JavaType,
    facts: &FactSet,
    local_types: &HashMap<String, JavaType>,
) -> bool {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    let Some(receiver_key) = initializer_receiver_key(receiver, local_types) else {
        return false;
    };
    facts
        .external_api_effect_for_receiver_with_arity(Some(&receiver_key), name, Some(args.len()))
        .is_some_and(|effect| {
            effect.return_type.as_ref().is_some_and(|return_ty| {
                return_type_can_initialize_declared_type(return_ty, declared_ty)
            }) && effect.return_nullability == NullabilityState::DefinitelyNonNull
        })
}

fn return_type_can_initialize_declared_type(return_ty: &JavaType, declared_ty: &JavaType) -> bool {
    java_types_have_same_signature_boundary_identity(return_ty, declared_ty)
        || (is_declared_java_object_type(declared_ty)
            && !matches!(return_ty, JavaType::Primitive(_) | JavaType::Void))
}

fn initializer_receiver_key(
    receiver: &Expr,
    local_types: &HashMap<String, JavaType>,
) -> Option<String> {
    if let Expr::Var(name) = receiver
        && let Some(java_ty) = local_types.get(name)
        && let Some(type_key) = external_api_receiver_type_key_without_type_env(java_ty)
    {
        return Some(type_key);
    }
    static_initializer_receiver_key(receiver)
}

fn static_initializer_receiver_key(receiver: &Expr) -> Option<String> {
    match receiver {
        Expr::Var(name) => Some(crate::stdlib::canonical_external_receiver_type_key(name)),
        Expr::FieldAccess(_, _) => static_initializer_receiver_java_name(receiver)
            .map(|name| crate::stdlib::canonical_external_receiver_type_key(&name)),
        _ => None,
    }
}

fn static_initializer_receiver_java_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(receiver, field) => Some(format!(
            "{}.{}",
            static_initializer_receiver_java_name(receiver)?,
            field
        )),
        _ => None,
    }
}

fn merge_binding_nullability_state(
    states: &mut HashMap<String, NullabilityState>,
    name: &str,
    state: NullabilityState,
) {
    states
        .entry(name.to_string())
        .and_modify(|current| *current = crate::lattice::field_nullability_join(*current, state))
        .or_insert(state);
}

fn collect_binding_nullability_states_by_binding_from_stmts(
    statements: &[Stmt],
    scopes: &mut Vec<HashMap<String, BindingId>>,
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    for stmt in statements {
        collect_binding_nullability_states_by_binding_from_stmt(
            stmt,
            scopes,
            states,
            return_states,
        );
    }
}

fn collect_binding_nullability_states_by_binding_from_stmt(
    stmt: &Stmt,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    match stmt {
        Stmt::VarDecl {
            source_name,
            initializer,
            binding,
            ..
        } => {
            if let (Some(binding), Some(initializer)) = (binding, initializer)
                && let Some(state) = source_expr_nullability_state(initializer)
            {
                merge_binding_nullability_state_by_id(states, binding, state);
                merge_binding_nullability_state_by_id(return_states, binding, state);
            }
            if let (Some(binding), Some(initializer)) = (binding, initializer)
                && let Some(state) = source_return_direct_nullability_state(initializer)
            {
                merge_binding_nullability_state_by_id(return_states, binding, state);
            }
            if let Some(initializer) = initializer {
                collect_binding_nullability_states_by_binding_from_expr(
                    initializer,
                    scopes,
                    states,
                    return_states,
                );
            }
            if let Some(binding) = binding {
                insert_scoped_binding(scopes, source_name, binding.clone());
            }
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target
                && let Some(binding) = scoped_binding(scopes, name).cloned()
                && let Some(state) = source_expr_nullability_state(value)
            {
                merge_binding_nullability_state_by_id(states, &binding, state);
                merge_binding_nullability_state_by_id(return_states, &binding, state);
            }
            if let Expr::Var(name) = target
                && let Some(binding) = scoped_binding(scopes, name).cloned()
                && let Some(state) = source_return_direct_nullability_state(value)
            {
                merge_binding_nullability_state_by_id(return_states, &binding, state);
            }
            collect_binding_nullability_states_by_binding_from_expr(
                value,
                scopes,
                states,
                return_states,
            );
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_binding_nullability_states_by_binding_from_expr(
                expr,
                scopes,
                states,
                return_states,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_binding_nullability_states_by_binding_from_expr(
                condition,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_child_block(
                then_block,
                scopes,
                states,
                return_states,
            );
            if let Some(else_block) = else_block {
                collect_binding_nullability_states_by_binding_from_child_block(
                    else_block,
                    scopes,
                    states,
                    return_states,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_binding_nullability_states_by_binding_from_expr(
                condition,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_child_block(
                body,
                scopes,
                states,
                return_states,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            scopes.push(HashMap::new());
            collect_binding_nullability_states_by_binding_from_stmts(
                init,
                scopes,
                states,
                return_states,
            );
            if let Some(condition) = condition {
                collect_binding_nullability_states_by_binding_from_expr(
                    condition,
                    scopes,
                    states,
                    return_states,
                );
            }
            for expr in update {
                collect_binding_nullability_states_by_binding_from_expr(
                    expr,
                    scopes,
                    states,
                    return_states,
                );
            }
            collect_binding_nullability_states_by_binding_from_child_block(
                body,
                scopes,
                states,
                return_states,
            );
            scopes.pop();
        }
        Stmt::EnhancedFor {
            source_var_name,
            iterable,
            body,
            binding,
            ..
        } => {
            collect_binding_nullability_states_by_binding_from_expr(
                iterable,
                scopes,
                states,
                return_states,
            );
            scopes.push(HashMap::new());
            if let Some(binding) = binding {
                insert_scoped_binding(scopes, source_var_name, binding.clone());
            }
            collect_binding_nullability_states_by_binding_from_stmts(
                &body.statements,
                scopes,
                states,
                return_states,
            );
            scopes.pop();
        }
        Stmt::Switch { selector, cases } => {
            collect_binding_nullability_states_by_binding_from_expr(
                selector,
                scopes,
                states,
                return_states,
            );
            for case in cases {
                scopes.push(HashMap::new());
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_binding_nullability_states_by_binding_from_expr(
                            expr,
                            scopes,
                            states,
                            return_states,
                        );
                    }
                }
                collect_binding_nullability_states_by_binding_from_stmts(
                    &case.statements,
                    scopes,
                    states,
                    return_states,
                );
                scopes.pop();
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            collect_binding_nullability_states_by_binding_from_stmts(
                resources,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_child_block(
                body,
                scopes,
                states,
                return_states,
            );
            for catch in catches {
                scopes.push(HashMap::new());
                if let Some(binding) = &catch.binding {
                    insert_scoped_binding(scopes, &catch.source_name, binding.clone());
                }
                collect_binding_nullability_states_by_binding_from_stmts(
                    &catch.body.statements,
                    scopes,
                    states,
                    return_states,
                );
                scopes.pop();
            }
            if let Some(finally_block) = finally_block {
                collect_binding_nullability_states_by_binding_from_child_block(
                    finally_block,
                    scopes,
                    states,
                    return_states,
                );
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_binding_nullability_states_by_binding_from_child_block(
                block,
                scopes,
                states,
                return_states,
            );
        }
        Stmt::LocalType(_)
        | Stmt::Return(None)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_binding_nullability_states_by_binding_from_child_block(
    block: &Block,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    scopes.push(HashMap::new());
    collect_binding_nullability_states_by_binding_from_stmts(
        &block.statements,
        scopes,
        states,
        return_states,
    );
    scopes.pop();
}

fn collect_binding_nullability_states_by_binding_from_expr(
    expr: &Expr,
    scopes: &mut Vec<HashMap<String, BindingId>>,
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    match expr {
        Expr::Binary {
            op: BinaryOp::Eq | BinaryOp::Ne,
            left,
            right,
        } => {
            collect_null_comparison_binding_by_id(left, right, scopes, states, return_states);
            collect_null_comparison_binding_by_id(right, left, scopes, states, return_states);
            collect_null_comparison_assignment_by_id(left, right, scopes, states, return_states);
            collect_null_comparison_assignment_by_id(right, left, scopes, states, return_states);
            collect_binding_nullability_states_by_binding_from_expr(
                left,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_expr(
                right,
                scopes,
                states,
                return_states,
            );
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            if let Expr::Var(name) = left.as_ref()
                && let Some(binding) = scoped_binding(scopes, name).cloned()
                && let Some(state) = source_expr_nullability_state(right)
            {
                merge_binding_nullability_state_by_id(states, &binding, state);
                merge_binding_nullability_state_by_id(return_states, &binding, state);
            }
            if let Expr::Var(name) = left.as_ref()
                && let Some(binding) = scoped_binding(scopes, name).cloned()
                && let Some(state) = source_return_direct_nullability_state(right)
            {
                merge_binding_nullability_state_by_id(return_states, &binding, state);
            }
            collect_binding_nullability_states_by_binding_from_expr(
                right,
                scopes,
                states,
                return_states,
            );
        }
        Expr::Binary { left, right, .. } => {
            collect_binding_nullability_states_by_binding_from_expr(
                left,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_expr(
                right,
                scopes,
                states,
                return_states,
            );
        }
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::FieldAccess(expr, _) => {
            collect_binding_nullability_states_by_binding_from_expr(
                expr,
                scopes,
                states,
                return_states,
            );
        }
        Expr::ArrayAccess(array, index) => {
            collect_binding_nullability_states_by_binding_from_expr(
                array,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_expr(
                index,
                scopes,
                states,
                return_states,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_binding_nullability_states_by_binding_from_expr(
                condition,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_expr(
                then_expr,
                scopes,
                states,
                return_states,
            );
            collect_binding_nullability_states_by_binding_from_expr(
                else_expr,
                scopes,
                states,
                return_states,
            );
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_binding_nullability_states_by_binding_from_expr(
                    receiver,
                    scopes,
                    states,
                    return_states,
                );
            }
            for arg in args {
                collect_binding_nullability_states_by_binding_from_expr(
                    arg,
                    scopes,
                    states,
                    return_states,
                );
            }
        }
        _ => {}
    }
}

fn collect_null_comparison_binding_by_id(
    maybe_binding: &Expr,
    maybe_null: &Expr,
    scopes: &[HashMap<String, BindingId>],
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    if matches!(maybe_null, Expr::Literal(Literal::Null))
        && let Expr::Var(name) = maybe_binding
        && let Some(binding) = scoped_binding(scopes, name)
    {
        merge_binding_nullability_state_by_id(states, binding, NullabilityState::MaybeNull);
        merge_binding_nullability_state_by_id(return_states, binding, NullabilityState::MaybeNull);
    }
}

fn collect_null_comparison_assignment_by_id(
    maybe_assignment: &Expr,
    maybe_null: &Expr,
    scopes: &[HashMap<String, BindingId>],
    states: &mut HashMap<BindingId, NullabilityState>,
    return_states: &mut HashMap<BindingId, NullabilityState>,
) {
    if !matches!(maybe_null, Expr::Literal(Literal::Null)) {
        return;
    }
    let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        ..
    } = maybe_assignment
    else {
        return;
    };
    if let Expr::Var(name) = left.as_ref()
        && let Some(binding) = scoped_binding(scopes, name)
    {
        merge_binding_nullability_state_by_id(states, binding, NullabilityState::MaybeNull);
        merge_binding_nullability_state_by_id(return_states, binding, NullabilityState::MaybeNull);
    }
}

fn merge_binding_nullability_state_by_id(
    states: &mut HashMap<BindingId, NullabilityState>,
    binding: &BindingId,
    state: NullabilityState,
) {
    states
        .entry(binding.clone())
        .and_modify(|current| *current = crate::lattice::field_nullability_join(*current, state))
        .or_insert(state);
}

fn insert_scoped_binding(
    scopes: &mut [HashMap<String, BindingId>],
    name: &str,
    binding: BindingId,
) {
    if let Some(scope) = scopes.last_mut() {
        scope.insert(name.to_string(), binding);
    }
}

fn scoped_binding<'a>(
    scopes: &'a [HashMap<String, BindingId>],
    name: &str,
) -> Option<&'a BindingId> {
    scopes.iter().rev().find_map(|scope| scope.get(name))
}

fn collect_local_constraint_sets(
    block: &crate::java_ir::Block,
    params: &[crate::java_ir::Param],
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
    declared_source_types: &HashSet<String>,
) -> Vec<LocalConstraintSet> {
    let mut locals = Vec::new();
    let mut local_types = params
        .iter()
        .map(|param| (param.name.clone(), param.ty.clone()))
        .collect::<HashMap<_, _>>();
    let mut mutated = collect_mutated_local_names(block);
    mutated.extend(collect_facade_mut_borrow_argument_names(
        block,
        facts,
        &mut local_types.clone(),
    ));
    let source_nullability = collect_binding_nullability_state_map_with_params(block, params);
    let liveness = crate::liveness::LivenessAnalysis::for_body(block);
    let mut binding_occurrences = HashMap::new();
    collect_local_constraint_sets_from_stmts(
        &block.statements,
        &mutated,
        &liveness,
        usage,
        facts,
        captured,
        &source_nullability,
        &mut local_types,
        &mut binding_occurrences,
        &mut locals,
        declared_source_types,
    );
    locals
}

fn collect_duplicate_declared_local_names(block: &crate::java_ir::Block) -> HashSet<String> {
    let mut counts = HashMap::<String, usize>::new();
    collect_declared_local_name_counts(&block.statements, &mut counts);
    counts
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect()
}

fn block_directly_declares_local_name(block: &crate::java_ir::Block, name: &str) -> bool {
    block.statements.iter().any(|stmt| {
        match stmt {
        crate::java_ir::Stmt::VarDecl { name: local, .. } => local == name,
        crate::java_ir::Stmt::For { init, .. } => init.iter().any(|stmt| {
            matches!(stmt, crate::java_ir::Stmt::VarDecl { name: local, .. } if local == name)
        }),
        _ => false,
    }
    })
}

fn collect_declared_local_name_counts(
    statements: &[crate::java_ir::Stmt],
    counts: &mut HashMap<String, usize>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { name, .. } => {
                *counts.entry(name.clone()).or_default() += 1;
            }
            Stmt::For { init, body, .. } => {
                collect_declared_local_name_counts(init, counts);
                collect_declared_local_name_counts(&body.statements, counts);
            }
            Stmt::EnhancedFor { var_name, body, .. } => {
                *counts.entry(var_name.clone()).or_default() += 1;
                collect_declared_local_name_counts(&body.statements, counts);
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_declared_local_name_counts(&then_block.statements, counts);
                if let Some(else_block) = else_block {
                    collect_declared_local_name_counts(&else_block.statements, counts);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_declared_local_name_counts(&body.statements, counts);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_declared_local_name_counts(&case.statements, counts);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_declared_local_name_counts(resources, counts);
                collect_declared_local_name_counts(&body.statements, counts);
                for catch in catches {
                    *counts.entry(catch.source_name.clone()).or_default() += 1;
                    collect_declared_local_name_counts(&catch.body.statements, counts);
                }
                if let Some(finally_block) = finally_block {
                    collect_declared_local_name_counts(&finally_block.statements, counts);
                }
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

fn collect_local_constraint_sets_from_stmts(
    statements: &[crate::java_ir::Stmt],
    mutated: &HashSet<String>,
    liveness: &crate::liveness::LivenessAnalysis,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
    source_nullability: &SourceNullabilityMap,
    local_types: &mut HashMap<String, JavaType>,
    binding_occurrences: &mut HashMap<(BindingKind, String), usize>,
    locals: &mut Vec<LocalConstraintSet>,
    declared_source_types: &HashSet<String>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                source_name,
                initializer,
                binding,
                ..
            } => {
                let declared_binding = binding.clone();
                let binding = declared_binding.clone().unwrap_or_else(|| {
                    next_local_binding_id(BindingKind::Local, source_name, binding_occurrences)
                });
                let mut constraints = local_shape_constraints_for_stmt_with_usage(
                    name,
                    ty,
                    mutated,
                    liveness,
                    stmt,
                    usage,
                    Some(&binding),
                    facts,
                    declared_source_types,
                );
                if initializer.as_ref().is_some_and(|initializer| {
                    local_initializer_requires_owned_storage(initializer, ty)
                }) {
                    push_unique_signature_constraints(
                        &mut constraints,
                        &[
                            SignatureConstraint::StorageSlot,
                            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                            SignatureConstraint::RequiredView(ViewShape::Native),
                        ],
                    );
                }
                if !initializer.as_ref().is_some_and(|initializer| {
                    initializer_proves_declared_non_null(initializer, ty, facts, local_types)
                }) {
                    let source_constraints = source_nullability
                        .constraints_for_declaration(name, declared_binding.as_ref());
                    push_unique_signature_constraints(&mut constraints, &source_constraints);
                }
                apply_capture_constraints(&mut constraints, captured, name, ty, facts);
                let java_ty = initializer
                    .as_ref()
                    .and_then(|initializer| {
                        initializer_collection_carrier_java_type_for_declared_type(ty, initializer)
                    })
                    .unwrap_or_else(|| ty.clone());
                locals.push(LocalConstraintSet {
                    binding,
                    name: name.clone(),
                    java_ty: java_ty.clone(),
                    constraints,
                });
                local_types.insert(name.clone(), java_ty);
            }
            Stmt::For { init, body, .. } => {
                let mut for_types = local_types.clone();
                collect_local_constraint_sets_from_stmts(
                    init,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut for_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut for_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                source_var_name,
                body,
                binding,
                ..
            } => {
                let declared_binding = binding.clone();
                let binding = declared_binding.clone().unwrap_or_else(|| {
                    next_local_binding_id(
                        BindingKind::LoopVariable,
                        source_var_name,
                        binding_occurrences,
                    )
                });
                let mut constraints = local_shape_constraints_for_stmt_with_usage(
                    var_name,
                    var_type,
                    mutated,
                    liveness,
                    stmt,
                    usage,
                    Some(&binding),
                    facts,
                    declared_source_types,
                );
                let source_constraints = source_nullability
                    .constraints_for_declaration(var_name, declared_binding.as_ref());
                push_unique_signature_constraints(&mut constraints, &source_constraints);
                apply_capture_constraints(&mut constraints, captured, var_name, var_type, facts);
                locals.push(LocalConstraintSet {
                    binding,
                    name: var_name.clone(),
                    java_ty: var_type.clone(),
                    constraints,
                });
                let mut body_types = local_types.clone();
                body_types.insert(var_name.clone(), var_type.clone());
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut body_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                let mut then_types = local_types.clone();
                collect_local_constraint_sets_from_stmts(
                    &then_block.statements,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut then_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
                if let Some(else_block) = else_block {
                    let mut else_types = local_types.clone();
                    collect_local_constraint_sets_from_stmts(
                        &else_block.statements,
                        mutated,
                        liveness,
                        usage,
                        facts,
                        captured,
                        source_nullability,
                        &mut else_types,
                        binding_occurrences,
                        locals,
                        declared_source_types,
                    );
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                let mut body_types = local_types.clone();
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut body_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    let mut case_types = local_types.clone();
                    collect_local_constraint_sets_from_stmts(
                        &case.statements,
                        mutated,
                        liveness,
                        usage,
                        facts,
                        captured,
                        source_nullability,
                        &mut case_types,
                        binding_occurrences,
                        locals,
                        declared_source_types,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut try_types = local_types.clone();
                collect_local_constraint_sets_from_stmts(
                    resources,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut try_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    liveness,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    &mut try_types,
                    binding_occurrences,
                    locals,
                    declared_source_types,
                );
                for catch in catches {
                    let mut catch_types = local_types.clone();
                    if let Some(catch_ty) = catch.types.first() {
                        catch_types.insert(catch.source_name.clone(), catch_ty.clone());
                    }
                    collect_local_constraint_sets_from_stmts(
                        &catch.body.statements,
                        mutated,
                        liveness,
                        usage,
                        facts,
                        captured,
                        source_nullability,
                        &mut catch_types,
                        binding_occurrences,
                        locals,
                        declared_source_types,
                    );
                }
                if let Some(finally_block) = finally_block {
                    let mut finally_types = local_types.clone();
                    collect_local_constraint_sets_from_stmts(
                        &finally_block.statements,
                        mutated,
                        liveness,
                        usage,
                        facts,
                        captured,
                        source_nullability,
                        &mut finally_types,
                        binding_occurrences,
                        locals,
                        declared_source_types,
                    );
                }
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

fn local_initializer_requires_owned_storage(initializer: &Expr, ty: &JavaType) -> bool {
    if !matches!(ty, JavaType::String) {
        return false;
    }
    match initializer {
        Expr::Literal(Literal::String(_)) => true,
        Expr::Binary { op, .. } if *op == BinaryOp::Add => true,
        Expr::Call { name, .. } => matches!(
            name.as_str(),
            "format"
                | "toString"
                | "to_string"
                | "substring"
                | "trim"
                | "toLowerCase"
                | "toUpperCase"
                | "replace"
                | "replaceFirst"
                | "replaceAll"
        ),
        _ => false,
    }
}

fn collect_local_plans_from_constraints(
    block: &crate::java_ir::Block,
    representations: &RepresentationPlan,
    constraints: &[LocalConstraintSet],
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
) -> Vec<LocalPlan> {
    let mut locals = Vec::new();
    let mut binding_occurrences = HashMap::new();
    collect_local_plans_from_stmts(
        &block.statements,
        representations,
        constraints,
        facts,
        type_param_scope,
        &mut binding_occurrences,
        &mut locals,
    );
    locals
}

fn collect_local_plans_from_stmts(
    statements: &[crate::java_ir::Stmt],
    representations: &RepresentationPlan,
    constraints: &[LocalConstraintSet],
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
    binding_occurrences: &mut HashMap<(BindingKind, String), usize>,
    locals: &mut Vec<LocalPlan>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                source_name,
                initializer,
                binding,
                ..
            } => {
                let binding = binding.clone().unwrap_or_else(|| {
                    next_local_binding_id(BindingKind::Local, source_name, binding_occurrences)
                });
                let binding_constraints = local_constraints_for_binding(constraints, &binding);
                locals.push(local_plan(
                    binding,
                    name,
                    ty,
                    initializer.as_ref(),
                    representations,
                    binding_constraints,
                    facts,
                    type_param_scope,
                ));
            }
            Stmt::For { init, body, .. } => {
                collect_local_plans_from_stmts(
                    init,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
                    locals,
                );
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
                    locals,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                source_var_name,
                body,
                binding,
                ..
            } => {
                let binding = binding.clone().unwrap_or_else(|| {
                    next_local_binding_id(
                        BindingKind::LoopVariable,
                        source_var_name,
                        binding_occurrences,
                    )
                });
                let binding_constraints = local_constraints_for_binding(constraints, &binding);
                locals.push(local_plan(
                    binding,
                    var_name,
                    var_type,
                    None,
                    representations,
                    binding_constraints,
                    facts,
                    type_param_scope,
                ));
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
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
                    type_param_scope,
                    binding_occurrences,
                    locals,
                );
                if let Some(else_block) = else_block {
                    collect_local_plans_from_stmts(
                        &else_block.statements,
                        representations,
                        constraints,
                        facts,
                        type_param_scope,
                        binding_occurrences,
                        locals,
                    );
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
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
                        type_param_scope,
                        binding_occurrences,
                        locals,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_local_plans_from_stmts(
                    resources,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
                    locals,
                );
                collect_local_plans_from_stmts(
                    &body.statements,
                    representations,
                    constraints,
                    facts,
                    type_param_scope,
                    binding_occurrences,
                    locals,
                );
                for catch in catches {
                    collect_local_plans_from_stmts(
                        &catch.body.statements,
                        representations,
                        constraints,
                        facts,
                        type_param_scope,
                        binding_occurrences,
                        locals,
                    );
                }
                if let Some(finally_block) = finally_block {
                    collect_local_plans_from_stmts(
                        &finally_block.statements,
                        representations,
                        constraints,
                        facts,
                        type_param_scope,
                        binding_occurrences,
                        locals,
                    );
                }
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

fn local_constraints_for_binding<'a>(
    constraints: &'a [LocalConstraintSet],
    binding: &BindingId,
) -> &'a [SignatureConstraint] {
    constraints
        .iter()
        .find(|constraints| constraints.binding == *binding)
        .map(|constraints| constraints.constraints.as_slice())
        .unwrap_or(&[])
}

fn next_local_binding_id(
    kind: BindingKind,
    name: &str,
    occurrences: &mut HashMap<(BindingKind, String), usize>,
) -> BindingId {
    let key = (kind, name.to_string());
    let entry = occurrences.entry(key).or_default();
    let occurrence = *entry;
    *entry += 1;
    BindingId::new(kind, name, occurrence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Span;
    use crate::java_ir::PrimitiveType;
    use crate::java_ir::{Block, Expr, Literal, TypeKind};
    use crate::usage::UsageMethodKey;

    fn binding_fact(name: &str, binding: Option<BindingId>) -> BindingUseFacts {
        BindingUseFacts {
            name: name.to_string(),
            binding,
            java_ty: JavaType::String,
            uses: Vec::new(),
        }
    }

    fn test_owner_type() -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: "Owner".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn named_test_type(name: &str) -> TypeDecl {
        TypeDecl {
            name: name.to_string(),
            ..test_owner_type()
        }
    }

    #[test]
    fn signature_type_lookup_refuses_ambiguous_simple_package_peer() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![named_test_type("a.Base"), named_test_type("b.Base")],
        };

        assert!(find_signature_type_decl(&program, "Base").is_none());
        assert_eq!(
            find_signature_type_decl(&program, "a.Base").map(|ty| ty.name.as_str()),
            Some("a.Base")
        );
    }

    fn string_param(binding: BindingId) -> Param {
        Param {
            name: "value".to_string(),
            source_name: "value".to_string(),
            ty: JavaType::String,
            is_varargs: false,
            binding: Some(binding),
        }
    }

    fn private_static_returning_param_method(param: Param) -> MethodDecl {
        MethodDecl {
            name: "borrow".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: vec![param],
            modifiers: vec![Modifier::Static],
            body: Some(Block {
                statements: vec![
                    Stmt::Block(Block {
                        statements: vec![
                            Stmt::VarDecl {
                                ty: JavaType::String,
                                name: "value".to_string(),
                                source_name: "value".to_string(),
                                initializer: None,
                                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
                            },
                            Stmt::Assign {
                                target: Expr::Var("value".to_string()),
                                value: Expr::Literal(Literal::String("shadow".to_string())),
                            },
                        ],
                    }),
                    Stmt::Return(Some(Expr::Var("value".to_string()))),
                ],
            }),
            span: Span::default(),
        }
    }

    fn return_string_method(statements: Vec<Stmt>) -> MethodDecl {
        MethodDecl {
            name: "make".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: Vec::new(),
            body: Some(Block { statements }),
            span: Span::default(),
        }
    }

    fn helper_call_expr() -> Expr {
        Expr::Call {
            receiver: None,
            name: "helper".to_string(),
            args: Vec::new(),
            resolved: None,
        }
    }

    #[test]
    fn call_result_alias_for_visible_local_still_detected() {
        let method = return_string_method(vec![
            Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(helper_call_expr()),
                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);

        assert!(method_returns_local_initialized_from_call(&method));
    }

    #[test]
    fn call_result_alias_from_inner_shadow_does_not_leak_by_name() {
        let method = return_string_method(vec![
            Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(Expr::Literal(Literal::String("outer".to_string()))),
                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
            },
            Stmt::Block(Block {
                statements: vec![Stmt::VarDecl {
                    ty: JavaType::String,
                    name: "value".to_string(),
                    source_name: "value".to_string(),
                    initializer: Some(helper_call_expr()),
                    binding: Some(BindingId::new(BindingKind::Local, "value", 1)),
                }],
            }),
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);

        assert!(!method_returns_local_initialized_from_call(&method));
    }

    #[test]
    fn call_result_alias_is_cleared_by_visible_reassignment() {
        let method = return_string_method(vec![
            Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(helper_call_expr()),
                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
            },
            Stmt::Assign {
                target: Expr::Var("value".to_string()),
                value: Expr::Literal(Literal::String("literal".to_string())),
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);

        assert!(!method_returns_local_initialized_from_call(&method));
    }

    #[test]
    fn returned_local_initializer_call_nullability_clears_visible_reassignment() {
        let mut helper = return_string_method(vec![Stmt::Return(Some(Expr::Literal(
            Literal::String("nonnull".to_string()),
        )))]);
        helper.name = "helper".to_string();
        let caller = return_string_method(vec![
            Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(helper_call_expr()),
                binding: Some(BindingId::new(BindingKind::Local, "value", 0)),
            },
            Stmt::Assign {
                target: Expr::Var("value".to_string()),
                value: Expr::Literal(Literal::String("literal".to_string())),
            },
            Stmt::Return(Some(Expr::Var("value".to_string()))),
        ]);
        let mut ty = test_owner_type();
        ty.members = vec![
            crate::java_ir::Member::Method(helper),
            crate::java_ir::Member::Method(caller.clone()),
        ];

        assert!(
            returned_local_initializer_source_call_nullability_constraints(
                &ty,
                &caller,
                &FactSet::default()
            )
            .is_empty(),
            "a source-call initializer must not keep controlling returned local nullability after visible reassignment"
        );
    }

    fn usage_with_param_and_shadow_facts(
        ty: &TypeDecl,
        method: &MethodDecl,
        param_binding: BindingId,
        param_uses: Vec<BindingUse>,
    ) -> UsageFacts {
        let shadow_binding = BindingId::new(BindingKind::Local, "value", 0);
        let mut method_usage = MethodUsageFacts::default();
        method_usage.bindings_by_id.insert(
            param_binding.clone(),
            BindingUseFacts {
                name: "value".to_string(),
                binding: Some(param_binding),
                java_ty: JavaType::String,
                uses: param_uses,
            },
        );
        method_usage.bindings_by_id.insert(
            shadow_binding.clone(),
            BindingUseFacts {
                name: "value".to_string(),
                binding: Some(shadow_binding.clone()),
                java_ty: JavaType::String,
                uses: vec![BindingUse::Mutated],
            },
        );
        method_usage.bindings.insert(
            "value".to_string(),
            BindingUseFacts {
                name: "value".to_string(),
                binding: Some(shadow_binding),
                java_ty: JavaType::String,
                uses: vec![BindingUse::Mutated],
            },
        );
        let mut usage = UsageFacts::default();
        usage.methods.insert(
            UsageMethodKey {
                type_name: "Owner".to_string(),
                method_name: "borrow".to_string(),
            },
            method_usage.clone(),
        );
        usage.methods_by_java_key.insert(
            JavaCallableKey::source_method_for_type(ty, method),
            method_usage,
        );
        usage
    }

    #[test]
    fn local_constraints_are_selected_by_binding_id() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let constraints = vec![
            LocalConstraintSet {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: vec![SignatureConstraint::RequiredOwnership(
                    OwnershipShape::Owned,
                )],
            },
            LocalConstraintSet {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: vec![SignatureConstraint::MutableBinding],
            },
        ];

        assert_eq!(
            local_constraints_for_binding(&constraints, &first_binding),
            [SignatureConstraint::RequiredOwnership(
                OwnershipShape::Owned
            )]
            .as_slice()
        );
        assert_eq!(
            local_constraints_for_binding(&constraints, &second_binding),
            [SignatureConstraint::MutableBinding].as_slice()
        );
        assert!(
            local_constraints_for_binding(
                &constraints,
                &BindingId::new(BindingKind::Local, "value", 2)
            )
            .is_empty()
        );
    }

    #[test]
    fn bound_local_mutability_does_not_fall_back_to_name_set_when_liveness_is_missing() {
        let binding = BindingId::new(BindingKind::Local, "value", 0);
        let stmt = Stmt::VarDecl {
            ty: JavaType::String,
            name: "value".to_string(),
            source_name: "value".to_string(),
            initializer: None,
            binding: Some(binding.clone()),
        };
        let mutated = HashSet::from(["value".to_string()]);

        let bound_constraints = local_shape_constraints_for_stmt_with_usage(
            "value",
            &JavaType::String,
            &mutated,
            &crate::liveness::LivenessAnalysis::default(),
            &stmt,
            None,
            Some(&binding),
            &FactSet::default(),
            &HashSet::new(),
        );
        assert!(
            !bound_constraints
                .iter()
                .any(|constraint| matches!(constraint, SignatureConstraint::MutableBinding)),
            "resolver-bound declarations must not reuse whole-method name-set mutability when exact liveness evidence is missing"
        );

        let idless_constraints = local_shape_constraints_for_stmt_with_usage(
            "value",
            &JavaType::String,
            &mutated,
            &crate::liveness::LivenessAnalysis::default(),
            &stmt,
            None,
            None,
            &FactSet::default(),
            &HashSet::new(),
        );
        assert!(
            idless_constraints
                .iter()
                .any(|constraint| matches!(constraint, SignatureConstraint::MutableBinding)),
            "id-less compatibility declarations keep the legacy name-set fallback"
        );
    }

    #[test]
    fn duplicate_local_name_signature_plan_lookup_uses_binding_id() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let first_shape = RustSlotShape::conservative(java_ty.clone());
        let mut second_shape = RustSlotShape::conservative(java_ty.clone());
        second_shape.ownership = OwnershipShape::SharedBorrow;
        let locals = vec![
            LocalPlan {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                shape: first_shape.clone(),
                binding_mutable: false,
            },
            LocalPlan {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty,
                shape: second_shape.clone(),
                binding_mutable: true,
            },
        ];
        let plan = SignaturePlan::default();

        assert_eq!(
            plan.local_shape_by_binding(&locals, &first_binding),
            Some(&first_shape)
        );
        assert_eq!(
            plan.local_shape_by_binding(&locals, &second_binding),
            Some(&second_shape)
        );
        assert_eq!(
            plan.required_local_shape_by_binding(&locals, &second_binding),
            second_shape
        );
        assert_eq!(
            plan.local_binding_mutability_by_binding(&locals, &first_binding),
            Some(false)
        );
        assert_eq!(
            plan.local_binding_mutability_by_binding(&locals, &second_binding),
            Some(true)
        );
    }

    #[test]
    fn method_local_constraint_insertion_can_target_binding_id() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let method = MethodKey {
            type_name: "Owner".to_string(),
            rust_name: "run".to_string(),
        };
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let locals = vec![
            LocalConstraintSet {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: Vec::new(),
            },
            LocalConstraintSet {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty,
                constraints: Vec::new(),
            },
        ];
        let mut constraints = SignatureConstraintState::default();
        constraints.methods.insert(
            method.clone(),
            MethodConstraintSet {
                local_index_by_name: index_local_constraints(&locals),
                local_index_by_binding: index_local_constraints_by_binding(&locals),
                locals,
                ..MethodConstraintSet::default()
            },
        );

        assert!(add_method_local_constraint_by_binding(
            &mut constraints,
            &method,
            &second_binding,
            SignatureConstraint::MutableBinding
        ));
        let method_constraints = constraints.methods.get(&method).unwrap();
        assert_eq!(
            local_constraints_for_binding(&method_constraints.locals, &first_binding),
            [].as_slice()
        );
        assert_eq!(
            local_constraints_for_binding(&method_constraints.locals, &second_binding),
            [SignatureConstraint::MutableBinding].as_slice()
        );
    }

    #[test]
    fn ambiguous_same_name_method_local_lookup_fails_closed_but_binding_lookup_still_works() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let method = MethodKey {
            type_name: "Owner".to_string(),
            rust_name: "run".to_string(),
        };
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let locals = vec![
            LocalConstraintSet {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: Vec::new(),
            },
            LocalConstraintSet {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty,
                constraints: Vec::new(),
            },
        ];
        let mut constraints = SignatureConstraintState::default();
        constraints.methods.insert(
            method.clone(),
            MethodConstraintSet {
                local_index_by_name: index_local_constraints(&locals),
                local_index_by_binding: index_local_constraints_by_binding(&locals),
                locals,
                ..MethodConstraintSet::default()
            },
        );

        assert!(
            !add_method_local_constraint(
                &mut constraints,
                &method,
                "value",
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            ),
            "ambiguous id-less method local lookup must fail closed"
        );
        assert!(add_method_local_constraint_by_binding(
            &mut constraints,
            &method,
            &second_binding,
            SignatureConstraint::MutableBinding
        ));

        let method_constraints = constraints.methods.get(&method).unwrap();
        assert!(method_constraints.local("value").is_none());
        assert_eq!(
            local_constraints_for_binding(&method_constraints.locals, &first_binding),
            [].as_slice()
        );
        assert_eq!(
            local_constraints_for_binding(&method_constraints.locals, &second_binding),
            [SignatureConstraint::MutableBinding].as_slice()
        );
    }

    #[test]
    fn ambiguous_same_name_constructor_local_lookup_fails_closed_but_binding_lookup_still_works() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let constructor = ConstructorKey {
            type_name: "Owner".to_string(),
            rust_name: "new_0".to_string(),
        };
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let locals = vec![
            LocalConstraintSet {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: Vec::new(),
            },
            LocalConstraintSet {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty,
                constraints: Vec::new(),
            },
        ];
        let mut constraints = SignatureConstraintState::default();
        constraints.constructors.insert(
            constructor.clone(),
            ConstructorConstraintSet {
                local_index_by_name: index_local_constraints(&locals),
                local_index_by_binding: index_local_constraints_by_binding(&locals),
                locals,
                ..ConstructorConstraintSet::default()
            },
        );

        assert!(
            !add_constructor_local_constraint(
                &mut constraints,
                &constructor,
                "value",
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned)
            ),
            "ambiguous id-less constructor local lookup must fail closed"
        );
        assert!(add_constructor_local_constraint_by_binding(
            &mut constraints,
            &constructor,
            &second_binding,
            SignatureConstraint::MutableBinding
        ));

        let constructor_constraints = constraints.constructors.get(&constructor).unwrap();
        assert!(constructor_constraints.local("value").is_none());
        assert_eq!(
            local_constraints_for_binding(&constructor_constraints.locals, &first_binding),
            [].as_slice()
        );
        assert_eq!(
            local_constraints_for_binding(&constructor_constraints.locals, &second_binding),
            [SignatureConstraint::MutableBinding].as_slice()
        );
    }

    #[test]
    fn binding_usage_for_bound_local_does_not_fall_back_by_name() {
        let first = BindingId::new(BindingKind::Local, "value", 0);
        let second = BindingId::new(BindingKind::Local, "value", 1);
        let mut usage = MethodUsageFacts::default();
        usage
            .bindings_by_id
            .insert(first.clone(), binding_fact("value", Some(first)));
        usage.bindings.insert(
            "value".to_string(),
            binding_fact("value", Some(second.clone())),
        );

        assert!(
            binding_usage_for_local(Some(&usage), Some(&second), "value", true).is_none(),
            "a resolver-assigned local id must not silently rejoin usage by name"
        );
        assert!(
            binding_usage_for_local(Some(&usage), None, "value", true).is_none(),
            "id-less compatibility callers must reject stale name facts that point at a missing binding id"
        );
        assert!(
            binding_usage_for_local(Some(&usage), None, "value", false).is_none(),
            "duplicate-name compatibility callers must still reject name fallback"
        );
        usage
            .bindings
            .insert("value".to_string(), binding_fact("value", None));
        assert!(
            binding_usage_for_local(Some(&usage), None, "value", true).is_none(),
            "id-less compatibility callers must reject same-name bound history"
        );
    }

    #[test]
    fn effective_method_binding_java_type_for_bound_local_does_not_fall_back_by_name() {
        let stale_binding = BindingId::new(BindingKind::Local, "value", 0);
        let queried_binding = BindingId::new(BindingKind::Local, "value", 1);
        let stale_local_ty = JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::String],
        };
        let locals = vec![LocalConstraintSet {
            binding: stale_binding.clone(),
            name: "value".to_string(),
            java_ty: stale_local_ty,
            constraints: Vec::new(),
        }];
        let method_constraints = MethodConstraintSet {
            local_index_by_name: index_local_constraints(&locals),
            local_index_by_binding: index_local_constraints_by_binding(&locals),
            locals,
            ..MethodConstraintSet::default()
        };
        let queried = binding_fact("value", Some(queried_binding));

        assert_eq!(
            effective_method_binding_java_type(&method_constraints, &queried),
            JavaType::String,
            "a bound local with no exact constraint must use its own usage type, not same-name constraints"
        );
    }

    #[test]
    fn bound_nullability_constraints_do_not_fall_back_by_name() {
        let binding = BindingId::new(BindingKind::Local, "value", 0);
        let block = Block {
            statements: vec![Stmt::VarDecl {
                ty: JavaType::String,
                name: "value".to_string(),
                source_name: "value".to_string(),
                initializer: Some(Expr::Literal(Literal::Null)),
                binding: Some(binding.clone()),
            }],
        };
        let source_nullability = collect_binding_nullability_state_map(&block);

        assert_eq!(
            source_nullability.constraints_for_binding(&binding),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
        assert!(
            source_nullability.constraints_for_name("value").is_empty(),
            "resolver-bound source declarations must not leak through name-only nullability compatibility lookup"
        );
    }

    #[test]
    fn source_return_nullability_field_shadow_expires_after_block() {
        let choose = BindingId::new(BindingKind::Param, "choose", 0);
        let shadow = BindingId::new(BindingKind::Local, "value", 0);
        let method = MethodDecl {
            name: "keep".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Unknown("Object".to_string()),
            throws: Vec::new(),
            params: vec![Param {
                name: "choose".to_string(),
                source_name: "choose".to_string(),
                ty: JavaType::Primitive(PrimitiveType::Boolean),
                is_varargs: false,
                binding: Some(choose),
            }],
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![
                    Stmt::If {
                        condition: Expr::Var("choose".to_string()),
                        then_block: Block {
                            statements: vec![Stmt::VarDecl {
                                ty: JavaType::Unknown("Object".to_string()),
                                name: "value".to_string(),
                                source_name: "value".to_string(),
                                initializer: Some(Expr::Literal(Literal::Null)),
                                binding: Some(shadow),
                            }],
                        },
                        else_block: None,
                    },
                    Stmt::Return(Some(Expr::Var("value".to_string()))),
                ],
            }),
            span: Span::default(),
        };
        let ty = TypeDecl {
            kind: TypeKind::Class,
            name: "FieldShadowReturnNullability".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: vec![
                crate::java_ir::Member::Field(FieldDecl {
                    name: "value".to_string(),
                    ty: JavaType::Unknown("Object".to_string()),
                    modifiers: Vec::new(),
                    initializer: None,
                    span: Span::default(),
                }),
                crate::java_ir::Member::Method(method.clone()),
            ],
            static_init_blocks: Vec::new(),
            span: Span::default(),
        };
        let mut facts = FactSet::default();
        facts
            .field_nullability_facts
            .push(crate::facts::FieldNullabilityFact {
                field: crate::facts::FieldKey {
                    type_name: "FieldShadowReturnNullability".to_string(),
                    field_name: "value".to_string(),
                },
                state: NullabilityState::DefinitelyNonNull,
                provenance: crate::facts::FieldNullabilityProvenance::ConstructorAssignments,
            });

        assert_eq!(
            typed_source_method_return_nullability_constraints(&ty, &method, &facts),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNonNull
            )]
        );
    }

    #[test]
    fn source_return_nullability_in_scope_binding_does_not_fall_back_to_field() {
        let local = BindingId::new(BindingKind::Local, "value", 0);
        let method = MethodDecl {
            name: "keep".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Unknown("Object".to_string()),
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![
                    Stmt::VarDecl {
                        ty: JavaType::Unknown("Object".to_string()),
                        name: "value".to_string(),
                        source_name: "value".to_string(),
                        initializer: None,
                        binding: Some(local),
                    },
                    Stmt::Return(Some(Expr::Var("value".to_string()))),
                ],
            }),
            span: Span::default(),
        };
        let ty = TypeDecl {
            kind: TypeKind::Class,
            name: "InScopeFieldShadowReturnNullability".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: vec![
                crate::java_ir::Member::Field(FieldDecl {
                    name: "value".to_string(),
                    ty: JavaType::Unknown("Object".to_string()),
                    modifiers: Vec::new(),
                    initializer: None,
                    span: Span::default(),
                }),
                crate::java_ir::Member::Method(method.clone()),
            ],
            static_init_blocks: Vec::new(),
            span: Span::default(),
        };
        let mut facts = FactSet::default();
        facts
            .field_nullability_facts
            .push(crate::facts::FieldNullabilityFact {
                field: crate::facts::FieldKey {
                    type_name: "InScopeFieldShadowReturnNullability".to_string(),
                    field_name: "value".to_string(),
                },
                state: NullabilityState::DefinitelyNonNull,
                provenance: crate::facts::FieldNullabilityProvenance::ConstructorAssignments,
            });

        assert!(
            typed_source_method_return_nullability_constraints(&ty, &method, &facts).is_empty(),
            "a scoped resolver-bound local with no nullability state must fail closed instead of borrowing a same-name field"
        );
    }

    #[test]
    fn source_return_nullability_scopes_for_initializer_binding() {
        let binding = BindingId::new(BindingKind::Local, "value", 0);
        let method = MethodDecl {
            name: "first".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: vec![Modifier::Static],
            body: Some(Block {
                statements: vec![Stmt::For {
                    init: vec![Stmt::VarDecl {
                        ty: JavaType::String,
                        name: "value".to_string(),
                        source_name: "value".to_string(),
                        initializer: Some(Expr::Literal(Literal::Null)),
                        binding: Some(binding),
                    }],
                    condition: None,
                    update: Vec::new(),
                    body: Block {
                        statements: vec![Stmt::Return(Some(Expr::Var("value".to_string())))],
                    },
                    label: None,
                }],
            }),
            span: Span::default(),
        };

        assert_eq!(
            source_method_return_nullability_constraints(&method),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
    }

    #[test]
    fn source_return_nullability_param_scope_uses_source_name() {
        let binding = BindingId::new(BindingKind::Param, "self", 0);
        let method = MethodDecl {
            name: "first".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: vec![Param {
                name: "self__".to_string(),
                source_name: "self".to_string(),
                ty: JavaType::String,
                is_varargs: false,
                binding: Some(binding),
            }],
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![
                    Stmt::Assign {
                        target: Expr::Var("self".to_string()),
                        value: Expr::Literal(Literal::Null),
                    },
                    Stmt::Return(Some(Expr::Var("self".to_string()))),
                ],
            }),
            span: Span::default(),
        };

        assert_eq!(
            source_method_return_nullability_constraints(&method),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
    }

    #[test]
    fn source_return_nullability_local_scope_uses_source_name() {
        let binding = BindingId::new(BindingKind::Local, "self", 0);
        let method = MethodDecl {
            name: "first".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::String,
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![
                    Stmt::VarDecl {
                        ty: JavaType::String,
                        name: "self__".to_string(),
                        source_name: "self".to_string(),
                        initializer: None,
                        binding: Some(binding),
                    },
                    Stmt::Assign {
                        target: Expr::Var("self".to_string()),
                        value: Expr::Literal(Literal::Null),
                    },
                    Stmt::Return(Some(Expr::Var("self".to_string()))),
                ],
            }),
            span: Span::default(),
        };

        assert_eq!(
            source_method_return_nullability_constraints(&method),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
    }

    #[test]
    fn source_return_nullability_scopes_enhanced_for_binding() {
        let binding = BindingId::new(BindingKind::LoopVariable, "item", 0);
        let method = MethodDecl {
            name: "first".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Unknown("Object".to_string()),
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![Stmt::EnhancedFor {
                    var_type: JavaType::Unknown("Object".to_string()),
                    var_name: "item".to_string(),
                    source_var_name: "item".to_string(),
                    iterable: Expr::Var("values".to_string()),
                    body: Block {
                        statements: vec![
                            Stmt::Assign {
                                target: Expr::Var("item".to_string()),
                                value: Expr::Literal(Literal::Null),
                            },
                            Stmt::Return(Some(Expr::Var("item".to_string()))),
                        ],
                    },
                    label: None,
                    binding: Some(binding),
                }],
            }),
            span: Span::default(),
        };

        assert_eq!(
            source_method_return_nullability_constraints(&method),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
    }

    #[test]
    fn source_return_nullability_enhanced_for_scope_uses_source_name() {
        let binding = BindingId::new(BindingKind::LoopVariable, "self", 0);
        let method = MethodDecl {
            name: "first".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Unknown("Object".to_string()),
            throws: Vec::new(),
            params: Vec::new(),
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![Stmt::EnhancedFor {
                    var_type: JavaType::Unknown("Object".to_string()),
                    var_name: "self__".to_string(),
                    source_var_name: "self".to_string(),
                    iterable: Expr::Var("values".to_string()),
                    body: Block {
                        statements: vec![
                            Stmt::Assign {
                                target: Expr::Var("self".to_string()),
                                value: Expr::Literal(Literal::Null),
                            },
                            Stmt::Return(Some(Expr::Var("self".to_string()))),
                        ],
                    },
                    label: None,
                    binding: Some(binding),
                }],
            }),
            span: Span::default(),
        };

        assert_eq!(
            source_method_return_nullability_constraints(&method),
            vec![SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNull
            )]
        );
    }

    #[test]
    fn returned_object_alias_constraints_use_param_binding_not_shadow_name() {
        let binding = BindingId::new(BindingKind::Param, "value", 0);
        let param = string_param(binding.clone());
        let method = private_static_returning_param_method(param);
        let ty = test_owner_type();
        let facts = FactSet::default();

        let returned_only = usage_with_param_and_shadow_facts(
            &ty,
            &method,
            binding.clone(),
            vec![BindingUse::Returned],
        );
        assert_eq!(
            internal_returned_object_alias_borrow_constraints(&ty, &method, &returned_only, &facts),
            vec![SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            )],
            "a same-name shadow local must not veto the resolver-bound returned parameter"
        );

        let mutated_exact = usage_with_param_and_shadow_facts(
            &ty,
            &method,
            binding,
            vec![BindingUse::Returned, BindingUse::Mutated],
        );
        assert!(
            internal_returned_object_alias_borrow_constraints(&ty, &method, &mutated_exact, &facts)
                .is_empty(),
            "mutation of the exact parameter binding still rejects shared-borrow return"
        );
    }

    #[test]
    fn method_signature_presence_fails_closed_for_ambiguous_simple_package_owner() {
        let method = MethodSignaturePlan {
            params: Vec::new(),
            return_shape: None,
            locals: Vec::new(),
            receiver: ReceiverPlan::None,
        };
        let mut plan = SignaturePlan::default();
        plan.methods.insert(
            MethodKey {
                type_name: "a.Item".to_string(),
                rust_name: "other".to_string(),
            },
            method.clone(),
        );
        plan.methods.insert(
            MethodKey {
                type_name: "b.Item".to_string(),
                rust_name: "who".to_string(),
            },
            method.clone(),
        );

        assert!(plan.has_method_signature_by_base_name_accepting_arity("a.Item", "other", 0));
        assert!(plan.has_method_signature_by_base_name_accepting_arity("b.Item", "who", 0));
        assert!(
            !plan.has_method_signature_by_base_name_accepting_arity("Item", "who", 0),
            "bare same-simple owners must not report presence when multiple package peers exist, even if only one has the method"
        );
    }

    #[test]
    fn source_receiver_binding_constraints_do_not_borrow_package_peer_mutability() {
        let mut usage = UsageFacts::default();
        usage.methods.insert(
            UsageMethodKey {
                type_name: "a.Item".to_string(),
                method_name: "touch".to_string(),
            },
            MethodUsageFacts {
                mutates_self: true,
                ..MethodUsageFacts::default()
            },
        );
        let binding = BindingUseFacts {
            name: "item".to_string(),
            binding: Some(BindingId::new(BindingKind::Param, "item", 0)),
            java_ty: JavaType::Named {
                name: "b.Item".to_string(),
                args: Vec::new(),
            },
            uses: vec![BindingUse::ReceiverForCall {
                name: "touch".to_string(),
                boundary: CallArgumentBoundary::InternalSameType,
                has_literal_null_arg: false,
            }],
        };

        assert!(
            source_receiver_binding_constraints(&binding, &usage).is_empty(),
            "qualified package peers must not inherit receiver mutability from same-simple facts"
        );
        assert!(source_method_usage_mutates_receiver(
            &usage, "a.Item", "touch"
        ));
        assert!(!source_method_usage_mutates_receiver(
            &usage, "b.Item", "touch"
        ));
    }

    #[test]
    fn source_summary_method_effect_return_nullability_does_not_borrow_package_peer() {
        let mut facts = FactSet::default();
        facts
            .method_effect_facts
            .push(crate::facts::MethodEffectFact {
                method: crate::facts::MethodKey {
                    type_name: "a.Item".to_string(),
                    method_name: "get".to_string(),
                },
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::ParsedSource,
            });

        assert_eq!(
            source_summary_method_effect_return_nullability(&facts, Some("a.Item"), "get"),
            Some(NullabilityState::DefinitelyNonNull)
        );
        assert_eq!(
            source_summary_method_effect_return_nullability(&facts, Some("b.Item"), "get"),
            None,
            "qualified package peers must not inherit return nullability from same-simple facts"
        );
    }

    #[test]
    fn required_method_signature_does_not_borrow_distinct_package_peer() {
        let method = MethodSignaturePlan {
            params: Vec::new(),
            return_shape: None,
            locals: Vec::new(),
            receiver: ReceiverPlan::None,
        };
        let mut plan = SignaturePlan::default();
        plan.methods.insert(
            MethodKey {
                type_name: "b.Item".to_string(),
                rust_name: "who".to_string(),
            },
            method,
        );

        assert!(
            plan.unique_method_signature_for_simple_owner(&MethodKey {
                type_name: "a.Item".to_string(),
                rust_name: "who".to_string(),
            })
            .is_none(),
            "missing qualified owners must not borrow the sole same-simple peer from another package"
        );
        assert!(
            plan.unique_method_signature_for_simple_owner(&MethodKey {
                type_name: "Item".to_string(),
                rust_name: "who".to_string(),
            })
            .is_some(),
            "unique bare simple-owner compatibility remains available"
        );
    }

    #[test]
    fn required_field_shape_does_not_borrow_distinct_package_peer() {
        let mut plan = SignaturePlan::default();
        plan.fields.insert(
            FieldKey {
                type_name: "b.Item".to_string(),
                field_name: "value".to_string(),
            },
            RustSlotShape::for_storage(JavaType::String),
        );

        assert!(
            plan.unique_field_shape_for_simple_owner(&FieldKey {
                type_name: "a.Item".to_string(),
                field_name: "value".to_string(),
            })
            .is_none(),
            "missing qualified field owners must not borrow the sole same-simple peer from another package"
        );
        assert!(
            plan.unique_field_shape_for_simple_owner(&FieldKey {
                type_name: "Item".to_string(),
                field_name: "value".to_string(),
            })
            .is_some(),
            "unique bare simple-owner field compatibility remains available"
        );
    }

    #[test]
    fn idless_assignment_local_requirements_do_not_merge_same_name_locals() {
        let java_ty = JavaType::Primitive(PrimitiveType::Int);
        let first_binding = BindingId::new(BindingKind::Local, "value", 0);
        let second_binding = BindingId::new(BindingKind::Local, "value", 1);
        let locals = vec![
            LocalConstraintSet {
                binding: first_binding.clone(),
                name: "value".to_string(),
                java_ty: java_ty.clone(),
                constraints: vec![SignatureConstraint::RequiredOwnership(
                    OwnershipShape::Owned,
                )],
            },
            LocalConstraintSet {
                binding: second_binding.clone(),
                name: "value".to_string(),
                java_ty,
                constraints: vec![SignatureConstraint::StorageSlot],
            },
        ];

        assert_eq!(
            local_slot_requirements_for_assignment(&locals, "value", Some(&second_binding)),
            vec![&SignatureConstraint::StorageSlot]
        );
        assert!(
            local_slot_requirements_for_assignment(&locals, "value", None).is_empty(),
            "id-less assignment aliases must not gather constraints from every same-name local"
        );
    }

    #[test]
    fn signature_passes_do_not_iterate_legacy_binding_name_map() {
        let source = include_str!("signature.rs");
        let count = source
            .lines()
            .filter(|line| line.trim_start().starts_with("for binding in "))
            .filter(|line| {
                line.contains("method_usage.bindings.values()")
                    || line.contains("constructor_usage.bindings.values()")
            })
            .count();

        assert_eq!(
            count, 0,
            "signature passes should consume all_binding_facts() instead of iterating the legacy name map"
        );
    }

    #[test]
    fn storage_slot_argument_constraints_force_storage_shape_before_borrowed_defaults() {
        let propagated =
            caller_binding_constraints_for_callee_param(vec![&SignatureConstraint::StorageSlot]);

        assert_eq!(
            propagated,
            vec![
                SignatureConstraint::StorageSlot,
                SignatureConstraint::RequiredView(ViewShape::Native),
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            ]
        );
    }
}

fn legacy_name_set_local_mutability_constraints(
    name: &str,
    mutated: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    if mutated.contains(name) {
        vec![SignatureConstraint::MutableBinding]
    } else {
        Vec::new()
    }
}

fn local_shape_constraints_with_usage_and_declared_source_types(
    name: &str,
    java_ty: &JavaType,
    mutated: &HashSet<String>,
    usage: Option<&MethodUsageFacts>,
    binding_id: Option<&BindingId>,
    allow_name_fallback: bool,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    local_shape_constraints_with_usage_and_effective_facts(
        name,
        java_ty,
        mutated,
        usage,
        binding_id,
        allow_name_fallback,
        facts,
        declared_source_types,
        &effective_facts,
    )
}

fn local_shape_constraints_for_stmt_with_usage(
    name: &str,
    java_ty: &JavaType,
    mutated: &HashSet<String>,
    liveness: &crate::liveness::LivenessAnalysis,
    stmt: &Stmt,
    usage: Option<&MethodUsageFacts>,
    binding_id: Option<&BindingId>,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let no_name_set_mutations = HashSet::new();
    let mut constraints = local_shape_constraints_with_usage_and_declared_source_types(
        name,
        java_ty,
        &no_name_set_mutations,
        usage,
        binding_id,
        false,
        facts,
        declared_source_types,
    );
    let binding_mutated = liveness
        .binding_is_mutated_after(stmt, name)
        .unwrap_or_else(|| binding_id.is_none() && mutated.contains(name));
    if binding_mutated {
        push_unique_signature_constraints(&mut constraints, &[SignatureConstraint::MutableBinding]);
    }
    constraints
}

fn local_shape_constraints_with_usage_and_effective_facts(
    name: &str,
    java_ty: &JavaType,
    mutated: &HashSet<String>,
    usage: Option<&MethodUsageFacts>,
    binding_id: Option<&BindingId>,
    allow_name_fallback: bool,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
    effective_facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    let mut constraints = legacy_name_set_local_mutability_constraints(name, mutated);
    constraints.extend(
        signature_capability_policy_with_effective_facts(java_ty, effective_facts).constraints,
    );
    push_unique_signature_constraints(
        &mut constraints,
        &declared_local_jdk_iterator_constraints(java_ty),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &declared_local_java_platform_facade_constraints(java_ty, declared_source_types),
    );
    let binding_usage = binding_usage_for_local(usage, binding_id, name, allow_name_fallback);
    if let Some(binding) = binding_usage {
        push_unique_signature_constraints(
            &mut constraints,
            &constructor_assigned_local_storage_constraints(binding),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts, &TypeEnv::new(), false, false),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_argument_binding_constraints(binding, facts),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &facade_abi_argument_binding_constraints(binding, facts),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &native_collection_receiver_binding_constraints(binding),
        );
        if native_collection_literal_null_element_type(binding).is_some() {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::NullableSequenceElement],
            );
        }
        push_unique_signature_constraints(
            &mut constraints,
            &external_call_return_binding_constraints(binding, facts),
        );
    }
    let modeled_native_map_get_declared_value = binding_usage.is_some_and(|binding| {
        !native_map_get_declared_value_binding_constraints(binding, java_ty, effective_facts)
            .is_empty()
    });
    if let Some(kind) = binding_usage
        .and_then(|binding| binding_boundary_policy(binding).kind)
        .filter(|_| !declared_java_platform_facade_type(java_ty, declared_source_types))
        .filter(|_| !modeled_native_map_get_declared_value)
    {
        constraints.extend(
            signature_boundary_policy_with_effective_facts(kind, java_ty, effective_facts)
                .constraints,
        );
    }
    if let Some(binding) = binding_usage {
        push_unique_signature_constraints(
            &mut constraints,
            &native_map_get_declared_value_binding_constraints(binding, java_ty, effective_facts),
        );
    }
    constraints
}

fn binding_usage_for_local<'a>(
    usage: Option<&'a MethodUsageFacts>,
    binding_id: Option<&BindingId>,
    name: &str,
    allow_name_fallback: bool,
) -> Option<&'a BindingUseFacts> {
    usage.and_then(|usage| {
        usage.binding_for_declaration_with_name_fallback(binding_id, name, allow_name_fallback)
    })
}

fn native_collection_literal_null_element_type(binding: &BindingUseFacts) -> Option<JavaType> {
    if !binding.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::ReceiverForCall {
                name,
                has_literal_null_arg: true,
                ..
            } if crate::stdlib::native_collection_receiver_mutates(&binding.java_ty, name)
        )
    }) {
        return None;
    }
    let element_ty = crate::types::sequence_element_type(&binding.java_ty)?;
    if crate::types::optional_element_type(&element_ty).is_some()
        || matches!(element_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return None;
    }
    Some(crate::types::optional_java_type(element_ty))
}

fn local_java_type_with_signature_constraints(
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
) -> JavaType {
    if !constraints.contains(&SignatureConstraint::NullableSequenceElement) {
        return java_ty.clone();
    }
    let Some(element_ty) = crate::types::sequence_element_type(java_ty) else {
        return java_ty.clone();
    };
    if crate::types::optional_element_type(&element_ty).is_some()
        || matches!(element_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        return java_ty.clone();
    }
    match java_ty {
        JavaType::Named { name, args } if args.len() == 1 => JavaType::Named {
            name: name.clone(),
            args: vec![crate::types::optional_java_type(element_ty)],
        },
        JavaType::Array(inner) => JavaType::Array(Box::new(crate::types::optional_java_type(
            inner.as_ref().clone(),
        ))),
        _ => java_ty.clone(),
    }
}

fn declared_local_java_platform_facade_constraints(
    java_ty: &JavaType,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    if !declared_java_platform_facade_type(java_ty, declared_source_types) {
        return Vec::new();
    }
    let mut constraints =
        java_platform_facade_concrete_native_constraints(java_ty, declared_source_types);
    constraints.push(SignatureConstraint::RequiredOwnership(
        OwnershipShape::Owned,
    ));
    constraints
}

fn java_platform_facade_concrete_native_constraints(
    java_ty: &JavaType,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    if !declared_java_platform_facade_type(java_ty, declared_source_types) {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn java_platform_facade_subclass_concrete_native_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    if declared_java_platform_facade_type(java_ty, declared_source_types) {
        return Vec::new();
    }
    if !facts
        .reachable_hierarchy_edges_for_type(java_ty)
        .iter()
        .any(|edge| crate::stdlib::is_java_platform_facade_type_name(&edge.super_name))
    {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn declared_java_platform_facade_type(
    java_ty: &JavaType,
    declared_source_types: &HashSet<String>,
) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            !source_declared_type_name_shadows_facade(name, declared_source_types)
                && crate::stdlib::is_java_platform_facade_type_name(name)
        }
        _ => false,
    }
}

fn source_declared_type_name_shadows_facade(
    name: &str,
    declared_source_types: &HashSet<String>,
) -> bool {
    declared_source_types.contains(name)
        || declared_source_types.contains(type_name_simple_segment(name))
}

fn declared_local_jdk_iterator_constraints(java_ty: &JavaType) -> Vec<SignatureConstraint> {
    let name = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return Vec::new(),
    };
    if !matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Iterator" | "java.util.Iterator" | "JavaIterator"
    ) {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn native_map_get_declared_value_binding_constraints(
    binding: &BindingUseFacts,
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    let assigned_from_matching_map_get = binding.uses.iter().any(|usage| {
        let BindingUse::AssignedFromCall {
            name,
            receiver_type: Some(receiver_type),
            ..
        } = usage
        else {
            return false;
        };
        if name != "get" {
            return false;
        }
        crate::stdlib::java_map_type_args(receiver_type)
            .is_some_and(|(_, value_ty)| value_ty == java_ty)
    });
    if !assigned_from_matching_map_get {
        return Vec::new();
    }

    if is_declared_java_object_type(java_ty) {
        return Vec::new();
    }

    let mut constraints = vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ];

    if matches!(
        facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && facts.is_final_type(java_ty)
    {
        constraints.push(SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        ));
    }

    constraints
}

fn native_collection_receiver_binding_constraints(
    binding: &BindingUseFacts,
) -> Vec<SignatureConstraint> {
    if binding.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::ReceiverForCall { name, .. }
                if crate::stdlib::native_collection_receiver_mutates(&binding.java_ty, name)
        )
    }) {
        vec![SignatureConstraint::MutableBinding]
    } else {
        Vec::new()
    }
}

fn constructor_assigned_local_storage_constraints(
    binding: &BindingUseFacts,
) -> Vec<SignatureConstraint> {
    if matches!(binding.java_ty, JavaType::Primitive(_) | JavaType::Void) {
        return Vec::new();
    }
    if binding.uses.iter().any(|usage| {
        matches!(
            usage,
            BindingUse::AssignedFromCall {
                name,
                ..
            } if name == "new"
        )
    }) {
        vec![SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned,
        )]
    } else {
        Vec::new()
    }
}

pub fn external_receiver_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
    types: &TypeEnv,
    allow_mutable_receiver_borrow_shape: bool,
    allow_readonly_receiver_borrow_shape: bool,
) -> Vec<SignatureConstraint> {
    external_receiver_binding_constraints_with_declared_source_types(
        binding,
        facts,
        types,
        allow_mutable_receiver_borrow_shape,
        allow_readonly_receiver_borrow_shape,
        &HashSet::new(),
    )
}

fn binding_has_mutating_external_receiver_call(
    binding: &BindingUseFacts,
    facts: &FactSet,
    types: &TypeEnv,
    declared_source_types: &HashSet<String>,
) -> bool {
    if source_declared_type_name_shadows_external_receiver(&binding.java_ty, declared_source_types)
    {
        return false;
    }
    binding.uses.iter().any(|usage| {
        let BindingUse::ReceiverForCall {
            name,
            boundary: CallArgumentBoundary::Unknown,
            ..
        } = usage
        else {
            return false;
        };
        let receiver_type =
            external_api_receiver_type_key_for_method_receiver(&binding.java_ty, types, name);
        facts
            .external_api_effect_for_receiver(receiver_type.as_deref(), name)
            .is_some_and(|effect| {
                effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
            })
    })
}

fn external_receiver_binding_constraints_with_declared_source_types(
    binding: &BindingUseFacts,
    facts: &FactSet,
    types: &TypeEnv,
    allow_mutable_receiver_borrow_shape: bool,
    allow_readonly_receiver_borrow_shape: bool,
    declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    if source_declared_type_name_shadows_external_receiver(&binding.java_ty, declared_source_types)
    {
        return constraints;
    }
    for usage in &binding.uses {
        let BindingUse::ReceiverForCall { name, boundary, .. } = usage else {
            continue;
        };
        let receiver_read_boundary = matches!(
            boundary,
            CallArgumentBoundary::Unknown | CallArgumentBoundary::KnownContentRead
        );
        let receiver_external_boundary = matches!(boundary, CallArgumentBoundary::Unknown);
        let receiver_type =
            external_api_receiver_type_key_for_method_receiver(&binding.java_ty, types, name);
        if receiver_external_boundary
            && facts
                .external_ffi_receiver_handover_for_receiver(
                    receiver_type.as_deref(),
                    name,
                    &binding.java_ty,
                )
                .is_some()
        {
            continue;
        }
        if receiver_external_boundary
            && facts
                .external_api_effect_for_receiver(receiver_type.as_deref(), name)
                .is_some_and(|effect| {
                    effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
                })
        {
            if allow_mutable_receiver_borrow_shape {
                push_unique_signature_constraints(
                    &mut constraints,
                    &[SignatureConstraint::RequiredOwnership(
                        OwnershipShape::MutableBorrow,
                    )],
                );
            }
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::MutableBinding],
            );
        } else if receiver_read_boundary
            && allow_readonly_receiver_borrow_shape
            && facts
                .external_api_effect_for_receiver(receiver_type.as_deref(), name)
                .is_some_and(|effect| {
                    effect.receiver_mutability == ReceiverMutabilityFact::ReadOnly
                        && !matches!(binding.java_ty, JavaType::Primitive(_) | JavaType::Void)
                })
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredOwnership(
                    OwnershipShape::SharedBorrow,
                )],
            );
            if external_receiver_binding_should_keep_typed_erased_facade(binding, facts) {
                push_unique_signature_constraints(
                    &mut constraints,
                    &[
                        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
                        SignatureConstraint::RequiredView(ViewShape::Native),
                    ],
                );
            }
        }
    }
    constraints
}

fn source_receiver_binding_constraints(
    binding: &BindingUseFacts,
    usage: &UsageFacts,
) -> Vec<SignatureConstraint> {
    let Some(receiver_type) = java_type_name(&binding.java_ty) else {
        return Vec::new();
    };
    let mut constraints = Vec::new();
    for binding_use in &binding.uses {
        let BindingUse::ReceiverForCall { name, .. } = binding_use else {
            continue;
        };
        if source_method_usage_mutates_receiver(usage, receiver_type, name) {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow),
                    SignatureConstraint::MutableBinding,
                ],
            );
        }
    }
    constraints
}

fn source_method_usage_mutates_receiver(
    usage: &UsageFacts,
    receiver_type: &str,
    method_name: &str,
) -> bool {
    let canonical_method = crate::parser::to_snake_case(method_name);
    let owner_query_ambiguous = legacy_signature_owner_query_is_ambiguous(
        usage.methods.iter().filter_map(|(key, _)| {
            (key.method_name == method_name || key.method_name == canonical_method)
                .then_some(key.type_name.as_str())
        }),
        receiver_type,
    );
    usage.methods.iter().any(|(key, method_usage)| {
        legacy_signature_owner_fact_matches(&key.type_name, receiver_type, owner_query_ambiguous)
            && (key.method_name == method_name || key.method_name == canonical_method)
            && method_usage.mutates_self
    })
}

fn source_declared_type_name_shadows_external_receiver(
    java_ty: &JavaType,
    declared_source_types: &HashSet<String>,
) -> bool {
    let name = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return false,
    };
    source_declared_type_name_shadows_facade(name, declared_source_types)
        && crate::stdlib::is_java_platform_facade_type_name(name)
}

fn external_receiver_binding_should_keep_typed_erased_facade(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> bool {
    let Some(receiver_type) =
        crate::stdlib::external_api_receiver_owner_key(&binding.java_ty, &TypeEnv::new())
    else {
        return false;
    };
    if matches!(receiver_type.as_str(), "Object" | "java.lang.Object") {
        return false;
    }
    let erased = JavaType::Named {
        name: receiver_type,
        args: Vec::new(),
    };
    let effective_facts = EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(&erased),
        Some(HierarchyStrategy::ErasedObject)
    )
}

fn binding_is_fully_modeled_by_readonly_external_receivers(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> bool {
    let mut saw_receiver = false;
    let empty_types = TypeEnv::new();
    for usage in &binding.uses {
        match usage {
            BindingUse::ReceiverForCall { name, boundary, .. }
                if matches!(
                    boundary,
                    CallArgumentBoundary::Unknown | CallArgumentBoundary::KnownContentRead
                ) =>
            {
                saw_receiver = true;
                let receiver_type = crate::stdlib::external_api_method_receiver_owner_key(
                    &binding.java_ty,
                    &empty_types,
                    name,
                );
                if !facts
                    .external_api_effect_for_receiver(receiver_type.as_deref(), name)
                    .is_some_and(|effect| {
                        effect.receiver_mutability == ReceiverMutabilityFact::ReadOnly
                    })
                {
                    return false;
                }
            }
            BindingUse::ReceiverForCall { .. } => {}
            BindingUse::ContentRead => {}
            _ => return false,
        }
    }
    saw_receiver
}

fn internal_object_binding_is_shared_borrow_safe(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> bool {
    if matches!(binding.java_ty, JavaType::Primitive(_) | JavaType::Void) {
        return false;
    }
    if binding.uses.is_empty() {
        return borrowed_content_type_policy(&binding.java_ty, facts)
            .policy
            .is_some();
    }
    let mut saw_read = false;
    for usage in &binding.uses {
        match usage {
            BindingUse::ContentRead | BindingUse::Returned => {
                saw_read = true;
            }
            BindingUse::AssignedToLocal { .. } => {}
            BindingUse::ReceiverForCall {
                name,
                boundary: CallArgumentBoundary::Unknown,
                ..
            } => {
                if !external_receiver_call_is_readonly(&binding.java_ty, name, facts) {
                    return false;
                }
                saw_read = true;
            }
            BindingUse::ReceiverForCall { .. } => {
                saw_read = true;
            }
            BindingUse::PassedToCall {
                name,
                receiver_type,
                arg_count,
                arg_index,
                boundary: CallArgumentBoundary::Unknown,
            } => {
                if !external_call_argument_is_readonly_borrow_safe(
                    receiver_type.as_ref(),
                    name,
                    *arg_count,
                    *arg_index,
                    &binding.java_ty,
                    facts,
                ) {
                    return false;
                }
                saw_read = true;
            }
            BindingUse::PassedToCall {
                boundary: CallArgumentBoundary::KnownContentRead,
                ..
            } => {
                saw_read = true;
            }
            BindingUse::PassedToCall {
                boundary: CallArgumentBoundary::InternalSameType,
                ..
            } => {}
            BindingUse::InstanceOfTarget { .. }
            | BindingUse::AssignedFromCall { .. }
            | BindingUse::StoredInField { .. }
            | BindingUse::Mutated => return false,
        }
    }
    saw_read
}

fn external_receiver_call_is_readonly(java_ty: &JavaType, name: &str, facts: &FactSet) -> bool {
    let receiver_type =
        crate::stdlib::external_api_method_receiver_owner_key(java_ty, &TypeEnv::new(), name);
    facts
        .external_api_effect_for_receiver(receiver_type.as_deref(), name)
        .is_some_and(|effect| effect.receiver_mutability == ReceiverMutabilityFact::ReadOnly)
}

fn external_call_argument_is_readonly_borrow_safe(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    arg_index: usize,
    binding_java_ty: &JavaType,
    facts: &FactSet,
) -> bool {
    if let Some(effect) =
        external_api_argument_effect_for_call_arg(receiver_type, name, arg_index, facts)
    {
        return effect.argument_mutability != ReceiverMutabilityFact::MutatesReceiver
            && effect.alias_policy != ExternalArgumentAliasPolicy::Escapes;
    }
    if let Some(effect) = external_constructor_argument_effect_for_call_arg(
        receiver_type,
        name,
        arg_count,
        arg_index,
        facts,
    ) {
        return effect.argument_mutability != ReceiverMutabilityFact::MutatesReceiver
            && effect.alias_policy != ExternalArgumentAliasPolicy::Escapes;
    }
    let has_readonly_effect = receiver_type
        .and_then(external_api_receiver_type_key_without_type_env)
        .and_then(|receiver_type| {
            facts.external_api_effect_for_receiver(Some(&receiver_type), name)
        })
        .or_else(|| {
            receiver_type
                .is_none()
                .then(|| facts.external_api_effect_for(name))
                .flatten()
        })
        .is_some_and(|effect| {
            effect.receiver_mutability == ReceiverMutabilityFact::ReadOnly
                && !matches!(binding_java_ty, JavaType::Primitive(_) | JavaType::Void)
        });
    has_readonly_effect
        || (looks_like_readonly_boolean_validator_name(name)
            && !matches!(binding_java_ty, JavaType::Primitive(_) | JavaType::Void))
}

fn looks_like_readonly_boolean_validator_name(name: &str) -> bool {
    name.starts_with("is")
        || name.starts_with("has")
        || name.starts_with("can")
        || name.starts_with("contains")
}

pub fn external_argument_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    for usage in &binding.uses {
        let BindingUse::PassedToCall {
            name,
            receiver_type,
            arg_count,
            arg_index,
            boundary,
        } = usage
        else {
            continue;
        };
        if *boundary == CallArgumentBoundary::InternalSameType {
            continue;
        }
        if map_entry_set_structural_argument_is_handled_by_lowering(
            receiver_type.as_ref(),
            name,
            *arg_count,
            *arg_index,
            &binding.java_ty,
        ) {
            continue;
        }
        let effect = external_api_argument_effect_for_call_arg(
            receiver_type.as_ref(),
            name,
            *arg_index,
            facts,
        );
        let native_collection_constraints = native_collection_argument_constraints_for_binding(
            receiver_type.as_ref(),
            name,
            *arg_count,
            *arg_index,
            &binding.java_ty,
            facts,
        );
        let constructor_constraints = external_constructor_argument_effect_for_call_arg(
            receiver_type.as_ref(),
            name,
            *arg_count,
            *arg_index,
            facts,
        )
        .or_else(|| {
            external_constructor_argument_effect_for_actual_binding_type(
                receiver_type.as_ref(),
                name,
                *arg_count,
                *arg_index,
                &binding.java_ty,
            )
        })
        .map(|effect| {
            external_constructor_argument_effect_constraints_for_binding(&effect, &binding.java_ty)
        })
        .unwrap_or_default();
        if *boundary == CallArgumentBoundary::Unknown
            && effect.is_none()
            && native_collection_constraints.is_empty()
            && constructor_constraints.is_empty()
            && !unknown_external_argument_default_non_null_applies(
                receiver_type.as_ref(),
                name,
                facts,
            )
        {
            continue;
        }
        if let Some(effect) = effect {
            push_unique_signature_constraints(
                &mut constraints,
                &external_argument_effect_constraints_for_binding(
                    effect,
                    receiver_type.as_ref(),
                    name,
                    *arg_index,
                    &binding.java_ty,
                    facts,
                ),
            );
        } else if !native_collection_constraints.is_empty() {
            push_unique_signature_constraints(&mut constraints, &native_collection_constraints);
        } else if !constructor_constraints.is_empty() {
            push_unique_signature_constraints(&mut constraints, &constructor_constraints);
        } else {
            // External reference/callback parameters default to non-null unless
            // explicit facts or observed null call sites prove a nullable slot.
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredStorageNullability(
                    NullabilityState::DefinitelyNonNull,
                )],
            );
        }
    }
    constraints
}

fn map_entry_set_structural_argument_is_handled_by_lowering(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    arg_index: usize,
    binding_java_ty: &JavaType,
) -> bool {
    if !matches!((name, arg_count, arg_index), ("contains" | "remove", 1, 0)) {
        return false;
    }
    if !java_type_is_map_entry_tuple_carrier(binding_java_ty) {
        return false;
    }
    let Some(JavaType::Named { args, .. }) = receiver_type else {
        return false;
    };
    let Some(element_ty) = args.first() else {
        return false;
    };
    java_type_is_map_entry_tuple_carrier(element_ty)
}

fn native_collection_argument_constraints_for_binding(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    arg_index: usize,
    binding_java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let Some(receiver_type) = receiver_type else {
        return Vec::new();
    };
    let Some((key_ty, value_ty)) = crate::stdlib::java_map_type_args(receiver_type) else {
        return Vec::new();
    };
    let target = match (name, arg_count, arg_index) {
        ("get" | "containsKey" | "contains_key", 1, 0) => {
            NativeCollectionArgumentTarget::Borrowed(key_ty)
        }
        ("put" | "insert", 2, 0) => NativeCollectionArgumentTarget::Owned(key_ty),
        ("put" | "insert", 2, 1) => NativeCollectionArgumentTarget::Owned(value_ty),
        _ => return Vec::new(),
    };
    let expected_ty = target.java_ty();
    if !java_types_match_with_external_aliases(expected_ty, binding_java_ty) {
        return Vec::new();
    }
    match target {
        NativeCollectionArgumentTarget::Borrowed(java_ty) => {
            borrowed_native_collection_argument_constraints(java_ty, facts)
        }
        NativeCollectionArgumentTarget::Owned(java_ty) => {
            owned_native_collection_argument_constraints(java_ty, facts)
        }
    }
}

enum NativeCollectionArgumentTarget<'a> {
    Borrowed(&'a JavaType),
    Owned(&'a JavaType),
}

impl<'a> NativeCollectionArgumentTarget<'a> {
    fn java_ty(&self) -> &'a JavaType {
        match self {
            NativeCollectionArgumentTarget::Borrowed(java_ty)
            | NativeCollectionArgumentTarget::Owned(java_ty) => java_ty,
        }
    }
}

fn borrowed_native_collection_argument_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) {
        constraints.push(SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        ));
    }
    constraints.push(SignatureConstraint::RequiredDispatch(
        DispatchShape::Concrete,
    ));
    if java_ty == &JavaType::String {
        constraints.push(SignatureConstraint::RequiredView(ViewShape::Str));
    } else if matches!(java_ty, JavaType::Array(_)) {
        constraints.push(SignatureConstraint::RequiredView(ViewShape::Slice));
    } else if !declared_external_return_has_non_concrete_representation(java_ty, facts) {
        constraints.push(SignatureConstraint::RequiredView(ViewShape::Native));
    }
    constraints
}

fn owned_native_collection_argument_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
        && !signature_capability_policy(java_ty, facts)
            .capability_ownership
            .is_some_and(|ownership| ownership == OwnershipShape::CopyValue)
    {
        constraints.push(SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned,
        ));
    }
    constraints.push(SignatureConstraint::RequiredDispatch(
        DispatchShape::Concrete,
    ));
    constraints.push(SignatureConstraint::RequiredView(ViewShape::Native));
    constraints
}

fn unknown_external_argument_default_non_null_applies(
    receiver_type: Option<&JavaType>,
    name: &str,
    facts: &FactSet,
) -> bool {
    let Some(receiver_type) = receiver_type else {
        return true;
    };
    let Some(receiver_key) = external_api_receiver_type_key_without_type_env(receiver_type) else {
        return false;
    };
    facts
        .external_api_effect_for_receiver(Some(&receiver_key), name)
        .is_some()
        || crate::external_packs::external_api_effect_for_receiver(Some(&receiver_key), name)
            .is_some()
}

fn facade_abi_argument_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    for usage in &binding.uses {
        let BindingUse::PassedToCall {
            name,
            receiver_type,
            arg_count: _,
            arg_index,
            boundary,
        } = usage
        else {
            continue;
        };
        if *boundary == CallArgumentBoundary::InternalSameType {
            continue;
        }
        for abi in matching_facade_abi_argument_facts(facts, receiver_type.as_ref(), name) {
            let Some(slot) = abi.args.get(*arg_index) else {
                continue;
            };
            if slot.contains_mut_borrow() {
                push_unique_signature_constraints(
                    &mut constraints,
                    &[
                        SignatureConstraint::RequiredOwnership(OwnershipShape::MutableBorrow),
                        SignatureConstraint::MutableBinding,
                    ],
                );
            }
        }
    }
    constraints
}

fn matching_facade_abi_argument_facts<'a>(
    facts: &'a FactSet,
    receiver_type: Option<&JavaType>,
    name: &str,
) -> Vec<&'a FacadeAbiFact> {
    let candidates = facts
        .facade_abi_facts
        .iter()
        .filter(|abi| abi.key.member.0 == name)
        .collect::<Vec<_>>();
    if let Some(receiver_type) = receiver_type
        && let Some(receiver_key) = external_api_receiver_type_key_without_type_env(receiver_type)
    {
        let matched = candidates
            .iter()
            .copied()
            .filter(|abi| {
                crate::stdlib::canonical_external_receiver_type_key(&abi.key.owner.type_name)
                    == receiver_key
            })
            .collect::<Vec<_>>();
        if !matched.is_empty() {
            return matched;
        }
    }
    if candidates.len() == 1 {
        candidates
    } else {
        Vec::new()
    }
}

fn unknown_boundary_is_fully_modeled_by_external_argument_effects(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> bool {
    let mut saw_unknown = false;
    for usage in &binding.uses {
        match usage {
            BindingUse::PassedToCall {
                name,
                receiver_type,
                arg_count,
                arg_index,
                boundary: CallArgumentBoundary::Unknown,
            } => {
                saw_unknown = true;
                if external_api_argument_effect_for_call_arg(
                    receiver_type.as_ref(),
                    name,
                    *arg_index,
                    facts,
                )
                .is_none()
                    && external_constructor_argument_effect_for_call_arg(
                        receiver_type.as_ref(),
                        name,
                        *arg_count,
                        *arg_index,
                        facts,
                    )
                    .or_else(|| {
                        external_constructor_argument_effect_for_actual_binding_type(
                            receiver_type.as_ref(),
                            name,
                            *arg_count,
                            *arg_index,
                            &binding.java_ty,
                        )
                    })
                    .is_none()
                    && native_collection_argument_constraints_for_binding(
                        receiver_type.as_ref(),
                        name,
                        *arg_count,
                        *arg_index,
                        &binding.java_ty,
                        facts,
                    )
                    .is_empty()
                {
                    return false;
                }
            }
            BindingUse::ReceiverForCall {
                boundary: CallArgumentBoundary::Unknown,
                ..
            }
            | BindingUse::AssignedFromCall {
                boundary: CallArgumentBoundary::Unknown,
                ..
            } => return false,
            _ => {}
        }
    }
    saw_unknown
}

fn external_constructor_argument_effect_for_call_arg(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    arg_index: usize,
    facts: &FactSet,
) -> Option<crate::facts::ExternalConstructorArgumentEffectFact> {
    if name != "new" {
        return None;
    }
    let receiver_type = receiver_type.and_then(external_api_receiver_type_key_without_type_env)?;
    facts
        .external_constructor_argument_effect_for(&receiver_type, arg_count, arg_index)
        .cloned()
        .or_else(|| {
            crate::external_packs::java_platform_constructor_argument_effect_for(
                &receiver_type,
                arg_count,
                arg_index,
            )
        })
}

fn external_constructor_argument_effect_for_actual_binding_type(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_count: usize,
    arg_index: usize,
    binding_java_ty: &JavaType,
) -> Option<crate::facts::ExternalConstructorArgumentEffectFact> {
    if name != "new" {
        return None;
    }
    let receiver_type = receiver_type.and_then(external_api_receiver_type_key_without_type_env)?;
    crate::external_packs::java_platform_constructor_argument_effect_for_actual_arg(
        &receiver_type,
        arg_count,
        arg_index,
        binding_java_ty,
    )
}

fn external_constructor_argument_effect_constraints_for_binding(
    effect: &crate::facts::ExternalConstructorArgumentEffectFact,
    binding_java_ty: &JavaType,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    constraints.push(SignatureConstraint::RequiredStorageNullability(
        external_parameter_nullability_policy(effect.parameter_nullability),
    ));
    if let Some(parameter_ty) = &effect.parameter_type {
        match effect.alias_policy {
            ExternalArgumentAliasPolicy::Borrowed if parameter_ty == &JavaType::String => {
                constraints.push(SignatureConstraint::RequiredOwnership(
                    OwnershipShape::SharedBorrow,
                ));
                constraints.push(SignatureConstraint::RequiredDispatch(
                    DispatchShape::Concrete,
                ));
                constraints.push(SignatureConstraint::RequiredView(ViewShape::Str));
            }
            ExternalArgumentAliasPolicy::Borrowed if matches!(parameter_ty, JavaType::Array(_)) => {
                constraints.push(SignatureConstraint::RequiredOwnership(
                    OwnershipShape::SharedBorrow,
                ));
                constraints.push(SignatureConstraint::RequiredView(ViewShape::Slice));
            }
            ExternalArgumentAliasPolicy::Borrowed
                if !matches!(parameter_ty, JavaType::Primitive(_) | JavaType::Void) =>
            {
                constraints.push(SignatureConstraint::RequiredOwnership(
                    OwnershipShape::SharedBorrow,
                ));
            }
            ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes
                if !matches!(parameter_ty, JavaType::Primitive(_) | JavaType::Void) =>
            {
                constraints.push(SignatureConstraint::RequiredOwnership(
                    OwnershipShape::Owned,
                ));
            }
            ExternalArgumentAliasPolicy::Unknown
            | ExternalArgumentAliasPolicy::Borrowed
            | ExternalArgumentAliasPolicy::NoAlias
            | ExternalArgumentAliasPolicy::Escapes => {}
        }
        if java_types_have_same_signature_boundary_identity(parameter_ty, binding_java_ty)
            && !is_declared_java_object_type(parameter_ty)
        {
            constraints.push(SignatureConstraint::RequiredDispatch(
                DispatchShape::Concrete,
            ));
            constraints.push(SignatureConstraint::RequiredView(ViewShape::Native));
        }
    }
    if effect.argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && effect
            .parameter_type
            .as_ref()
            .is_none_or(|java_ty| !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void))
    {
        constraints.push(SignatureConstraint::RequiredOwnership(
            OwnershipShape::MutableBorrow,
        ));
        constraints.push(SignatureConstraint::MutableBinding);
    }
    constraints
}

fn external_api_argument_effect_for_call_arg<'a>(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_index: usize,
    facts: &'a FactSet,
) -> Option<&'a ExternalApiArgumentEffectFact> {
    receiver_type
        .and_then(external_api_receiver_type_key_without_type_env)
        .and_then(|receiver_type| {
            facts.external_api_argument_effect_for_receiver(Some(&receiver_type), name, arg_index)
        })
        .or_else(|| facts.external_api_argument_effect_for_receiver(None, name, arg_index))
}

fn external_argument_effect_constraints_for_binding(
    effect: &ExternalApiArgumentEffectFact,
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_index: usize,
    binding_java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    if effect.alias_policy == ExternalArgumentAliasPolicy::Borrowed
        && external_ffi_handover_for_call_arg(
            receiver_type,
            name,
            arg_index,
            binding_java_ty,
            facts,
        )
        .is_some()
    {
        return vec![SignatureConstraint::RequiredStorageNullability(
            external_parameter_nullability_policy(effect.parameter_nullability),
        )];
    }
    let mut constraints = external_argument_effect_constraints(effect);
    if effect.parameter_type.as_ref().is_some_and(|parameter_ty| {
        java_types_have_same_signature_boundary_identity(parameter_ty, binding_java_ty)
            && !is_declared_java_object_type(parameter_ty)
            && (parameter_ty != &JavaType::String
                || !matches!(effect.alias_policy, ExternalArgumentAliasPolicy::Borrowed))
    }) {
        push_unique_signature_constraints(
            &mut constraints,
            &[
                SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        );
    }
    constraints
}

fn external_ffi_handover_for_call_arg<'a>(
    receiver_type: Option<&JavaType>,
    name: &str,
    arg_index: usize,
    binding_java_ty: &JavaType,
    facts: &'a FactSet,
) -> Option<&'a crate::facts::ExternalFfiHandoverFact> {
    receiver_type
        .and_then(external_api_receiver_type_key_without_type_env)
        .and_then(|receiver_type| {
            facts.external_ffi_argument_handover_for_receiver(
                Some(&receiver_type),
                name,
                arg_index,
                binding_java_ty,
            )
        })
        .or_else(|| {
            facts.external_ffi_argument_handover_for_receiver(
                None,
                name,
                arg_index,
                binding_java_ty,
            )
        })
}

pub fn external_argument_effect_constraints(
    effect: &ExternalApiArgumentEffectFact,
) -> Vec<SignatureConstraint> {
    let java_ty = effect
        .parameter_type
        .clone()
        .unwrap_or_else(|| JavaType::Unknown("Object".to_string()));
    let mut constraints = Vec::new();

    let parameter_nullability = external_parameter_nullability_policy(effect.parameter_nullability);
    constraints.push(SignatureConstraint::RequiredStorageNullability(
        parameter_nullability,
    ));

    match effect.alias_policy {
        ExternalArgumentAliasPolicy::Borrowed if java_ty == JavaType::String => {
            constraints.push(SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            ));
            constraints.push(SignatureConstraint::RequiredDispatch(
                DispatchShape::Concrete,
            ));
            constraints.push(SignatureConstraint::RequiredView(ViewShape::Str));
        }
        ExternalArgumentAliasPolicy::Borrowed if matches!(java_ty, JavaType::Array(_)) => {
            constraints.push(SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            ));
            constraints.push(SignatureConstraint::RequiredView(ViewShape::Slice));
        }
        ExternalArgumentAliasPolicy::Borrowed
            if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            constraints.push(SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            ));
        }
        ExternalArgumentAliasPolicy::NoAlias | ExternalArgumentAliasPolicy::Escapes
            if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) =>
        {
            constraints.push(SignatureConstraint::RequiredOwnership(
                OwnershipShape::Owned,
            ));
        }
        ExternalArgumentAliasPolicy::Unknown
        | ExternalArgumentAliasPolicy::Borrowed
        | ExternalArgumentAliasPolicy::NoAlias
        | ExternalArgumentAliasPolicy::Escapes => {}
    }

    if effect.argument_mutability == ReceiverMutabilityFact::MutatesReceiver
        && !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void)
    {
        constraints.push(SignatureConstraint::RequiredOwnership(
            OwnershipShape::MutableBorrow,
        ));
        constraints.push(SignatureConstraint::MutableBinding);
    }

    constraints
}

pub fn external_call_return_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    for usage in &binding.uses {
        let BindingUse::AssignedFromCall {
            name,
            receiver_type,
            boundary: CallArgumentBoundary::Unknown | CallArgumentBoundary::KnownContentRead,
        } = usage
        else {
            continue;
        };
        push_external_call_return_constraints(
            &mut constraints,
            receiver_type
                .as_ref()
                .and_then(external_api_receiver_type_key_without_type_env),
            name,
            None,
            facts,
        );
    }
    constraints
}

fn push_external_call_return_constraints(
    constraints: &mut Vec<SignatureConstraint>,
    receiver_type: Option<String>,
    name: &str,
    arg_count: Option<usize>,
    facts: &FactSet,
) {
    if native_collection_get_return_may_be_null(receiver_type.as_deref(), name, arg_count) {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull,
            )],
        );
        return;
    }
    if let Some(return_nullability) =
        source_summary_method_effect_return_nullability(facts, receiver_type.as_deref(), name)
    {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                return_nullability,
            )],
        );
    }
    if let Some(effect) =
        facts.external_api_effect_for_receiver_with_arity(receiver_type.as_deref(), name, arg_count)
        && effect.return_nullability != NullabilityState::Unknown
        && external_effect_return_nullability_is_signature_evidence(effect)
    {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                effect.return_nullability,
            )],
        );
    }
}

fn source_summary_method_effect_return_nullability(
    facts: &FactSet,
    receiver_type: Option<&str>,
    name: &str,
) -> Option<NullabilityState> {
    let receiver_type = receiver_type?;
    let owner_query_ambiguous = legacy_signature_owner_query_is_ambiguous(
        facts.method_effect_facts.iter().filter_map(|effect| {
            (effect.method.method_name == name
                || crate::parser::to_snake_case(&effect.method.method_name) == name)
                .then_some(effect.method.type_name.as_str())
        }),
        receiver_type,
    );
    let mut states = facts.method_effect_facts.iter().filter_map(|effect| {
        (legacy_signature_owner_fact_matches(
            &effect.method.type_name,
            receiver_type,
            owner_query_ambiguous,
        ) && (effect.method.method_name == name
            || crate::parser::to_snake_case(&effect.method.method_name) == name)
            && effect.return_nullability != NullabilityState::Unknown)
            .then_some(effect.return_nullability)
    });
    let state = states.next()?;
    states.all(|next| next == state).then_some(state)
}

fn native_collection_get_return_may_be_null(
    receiver_type: Option<&str>,
    name: &str,
    arg_count: Option<usize>,
) -> bool {
    if name != "get" || !matches!(arg_count, Some(1) | None) {
        return false;
    }
    let Some(receiver_type) = receiver_type else {
        return false;
    };
    let receiver_type = crate::stdlib::canonical_external_receiver_type_key(receiver_type);
    let simple = receiver_type.rsplit('.').next().unwrap_or(&receiver_type);
    matches!(
        simple,
        "List" | "ArrayList" | "LinkedList" | "Map" | "HashMap" | "LinkedHashMap"
    )
}

fn external_effect_return_nullability_is_signature_evidence(
    effect: &crate::facts::ExternalApiEffectFact,
) -> bool {
    effect.return_type.is_some()
        || matches!(
            effect.provenance,
            FactProvenance::UserOverride | FactProvenance::ExternalLibraryPack
        )
}

fn external_api_receiver_type_key(java_ty: &JavaType, types: &TypeEnv) -> Option<String> {
    crate::stdlib::external_api_receiver_owner_key(java_ty, types)
}

fn external_api_receiver_type_key_for_method_receiver(
    java_ty: &JavaType,
    types: &TypeEnv,
    method_name: &str,
) -> Option<String> {
    crate::stdlib::external_api_method_receiver_owner_key(java_ty, types, method_name)
}

fn external_api_receiver_type_key_without_type_env(java_ty: &JavaType) -> Option<String> {
    crate::stdlib::external_api_receiver_owner_key(java_ty, &TypeEnv::new())
}

fn java_types_have_same_signature_boundary_identity(left: &JavaType, right: &JavaType) -> bool {
    let left_element = crate::types::optional_element_type(left);
    let right_element = crate::types::optional_element_type(right);
    if left_element.is_some() || right_element.is_some() {
        return java_types_have_same_signature_boundary_identity(
            left_element.as_ref().unwrap_or(left),
            right_element.as_ref().unwrap_or(right),
        );
    }
    if left == right {
        return true;
    }
    if matches!(left, JavaType::Primitive(_) | JavaType::Void)
        || matches!(right, JavaType::Primitive(_) | JavaType::Void)
    {
        return left == right;
    }
    let empty_types = TypeEnv::new();
    let Some(left_key) = external_api_receiver_type_key(left, &empty_types) else {
        return false;
    };
    let Some(right_key) = external_api_receiver_type_key(right, &empty_types) else {
        return false;
    };
    left_key == right_key
        || simple_java_type_identity(&left_key) == simple_java_type_identity(&right_key)
}

fn simple_java_type_identity(name: &str) -> &str {
    name.rsplit(['.', '$']).next().unwrap_or(name)
}

fn java_type_is_obvious_read_product(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Primitive(_) | JavaType::String => true,
        JavaType::Array(_) => true,
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            let key = crate::stdlib::canonical_external_receiver_type_key(name);
            matches!(
                simple_java_type_identity(&key),
                "Map"
                    | "HashMap"
                    | "LinkedHashMap"
                    | "TreeMap"
                    | "List"
                    | "ArrayList"
                    | "LinkedList"
                    | "Set"
                    | "HashSet"
                    | "LinkedHashSet"
                    | "TreeSet"
                    | "Collection"
                    | "Iterator"
                    | "Iterable"
                    | "Optional"
                    | "Result"
            )
        }
        JavaType::Void | JavaType::Wildcard(_) => false,
    }
}

fn local_plan(
    binding: BindingId,
    name: &str,
    java_ty: &JavaType,
    initializer: Option<&Expr>,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
) -> LocalPlan {
    let effective_facts = EffectiveFacts::new(facts);
    let constraints =
        local_constraints_without_stale_nullable_call_result(initializer, constraints);
    let java_ty = local_java_type_with_signature_constraints(java_ty, &constraints);
    let shape = if declared_typed_initializer_should_anchor_local_type(&java_ty, initializer, facts)
    {
        RustSlotShape::for_storage(java_ty.clone())
    } else if local_constraints_select_concrete_borrowed_value(
        &java_ty,
        &constraints,
        &effective_facts,
    ) {
        let mut shape =
            apply_signature_constraints(RustSlotShape::conservative(java_ty.clone()), &constraints);
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Native);
        apply_required_ownership(&mut shape, OwnershipShape::SharedBorrow);
        shape
    } else {
        solve_signature_shape_with_facts(
            representations,
            SignatureShapeRequest {
                java_ty: java_ty.clone(),
                role: SignatureSlotRole::Storage,
                constraints: constraints.to_vec(),
                type_params: type_param_scope.to_vec(),
            },
            facts,
        )
        .expect("local signature shape is always present")
    };
    LocalPlan {
        binding,
        name: name.to_string(),
        java_ty,
        shape,
        binding_mutable: signature_constraints_require_mutable_binding(&constraints),
    }
}

fn local_constraints_without_stale_nullable_call_result<'a>(
    initializer: Option<&Expr>,
    constraints: &'a [SignatureConstraint],
) -> std::borrow::Cow<'a, [SignatureConstraint]> {
    let Some(Expr::Call { .. }) = initializer else {
        return std::borrow::Cow::Borrowed(constraints);
    };
    if !signature_constraints_prove_non_null_storage(constraints) {
        return std::borrow::Cow::Borrowed(constraints);
    }
    std::borrow::Cow::Owned(
        constraints
            .iter()
            .filter(|constraint| {
                !matches!(
                    constraint,
                    SignatureConstraint::RequiredStorageNullability(
                        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
                    )
                )
            })
            .cloned()
            .collect(),
    )
}

fn declared_typed_initializer_should_anchor_local_type(
    declared_ty: &JavaType,
    initializer: Option<&Expr>,
    facts: &FactSet,
) -> bool {
    if matches!(
        declared_ty,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Wildcard(_)
    ) {
        return false;
    }
    let Some(Expr::Call {
        receiver,
        name,
        args,
        resolved: _,
    }) = initializer
    else {
        return false;
    };
    receiver
        .as_deref()
        .and_then(static_initializer_receiver_key)
        .and_then(|receiver_key| {
            facts.external_api_effect_for_receiver_with_arity(
                Some(&receiver_key),
                name,
                Some(args.len()),
            )
        })
        .and_then(|effect| effect.return_type.as_ref())
        .is_some_and(|return_ty| return_type_can_initialize_declared_type(return_ty, declared_ty))
}

fn initializer_collection_carrier_java_type_for_declared_type(
    declared_ty: &JavaType,
    initializer: &Expr,
) -> Option<JavaType> {
    let Expr::New { ty, .. } = initializer else {
        return None;
    };
    refined_collection_carrier_java_type_for_slot(ty, declared_ty)
}

fn refined_collection_carrier_java_type_for_slot(
    actual: &JavaType,
    target: &JavaType,
) -> Option<JavaType> {
    let actual_kind = crate::stdlib::java_collection_kind_for_type(actual)?;
    let target_kind = crate::stdlib::java_collection_kind_for_type(target)?;
    if actual_kind == target_kind
        || !collection_kinds_share_carrier_family(actual_kind, target_kind)
    {
        return None;
    }
    collection_java_type_with_target_args(actual, target)
}

fn collection_kinds_share_carrier_family(
    actual: crate::stdlib::JavaCollectionKind,
    target: crate::stdlib::JavaCollectionKind,
) -> bool {
    (actual.is_map_like() && target.is_map_like()) || (actual.is_set_like() && target.is_set_like())
}

fn collection_java_type_with_target_args(
    carrier: &JavaType,
    target: &JavaType,
) -> Option<JavaType> {
    match (carrier, target) {
        (
            JavaType::Named {
                args: carrier_args, ..
            },
            JavaType::Named {
                args: target_args, ..
            },
        ) if carrier_args == target_args => Some(carrier.clone()),
        (
            JavaType::Named { name, .. },
            JavaType::Named {
                args: target_args, ..
            },
        ) => Some(JavaType::Named {
            name: name.clone(),
            args: target_args.clone(),
        }),
        _ => None,
    }
}

fn local_constraints_select_concrete_borrowed_value(
    java_ty: &JavaType,
    constraints: &[SignatureConstraint],
    facts: &EffectiveFacts<'_>,
) -> bool {
    !matches!(
        java_ty,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Wildcard(_)
    ) && !matches!(conservative_ownership(java_ty), OwnershipShape::CopyValue)
        && !facts.type_capabilities_for(java_ty).copy
        && constraints.contains(&SignatureConstraint::RequiredDispatch(
            DispatchShape::Concrete,
        ))
        && constraints.contains(&SignatureConstraint::RequiredView(ViewShape::Native))
        && constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        ))
        && !constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::MutableBorrow,
        ))
        && !constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned,
        ))
        && !constraints.contains(&SignatureConstraint::StorageSlot)
}

fn collect_mutated_local_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_mutated_local_names_from_stmts(&block.statements, &mut mutated);
    mutated
}

fn collect_array_element_mutated_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_array_element_mutated_names_from_stmts(&block.statements, &mut mutated);
    mutated
}

fn collect_map_view_backing_receiver_mutated_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    let mut aliases = HashMap::new();
    collect_map_view_backing_receiver_mutated_names_from_stmts(
        &block.statements,
        &mut aliases,
        &mut mutated,
    );
    mutated
}

fn collect_facade_mut_borrow_argument_names(
    block: &Block,
    facts: &FactSet,
    local_types: &mut HashMap<String, JavaType>,
) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_facade_mut_borrow_argument_names_from_stmts(
        &block.statements,
        facts,
        local_types,
        &mut mutated,
    );
    mutated
}

fn collect_field_receiver_mutated_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_field_receiver_mutated_names_from_stmts(&block.statements, &mut mutated);
    mutated
}

fn collect_map_view_backing_receiver_mutated_names_from_stmts(
    statements: &[Stmt],
    aliases: &mut HashMap<String, String>,
    mutated: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                name,
                source_name,
                initializer,
                ..
            } => {
                if let Some(expr) = initializer {
                    collect_map_view_backing_receiver_mutated_names_from_expr(
                        expr, aliases, mutated,
                    );
                    if let Some(view_backing_receiver) = map_view_call_backing_receiver(expr)
                        && let Some(root) = receiver_root_name(view_backing_receiver)
                    {
                        aliases.insert(name.clone(), root.to_string());
                        aliases.insert(source_name.clone(), root.to_string());
                    }
                }
            }
            Stmt::Assign { target, value } => {
                collect_map_view_backing_receiver_mutated_names_from_expr(target, aliases, mutated);
                collect_map_view_backing_receiver_mutated_names_from_expr(value, aliases, mutated);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) => {
                collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    condition, aliases, mutated,
                );
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &then_block.statements,
                    &mut aliases.clone(),
                    mutated,
                );
                if let Some(else_block) = else_block {
                    collect_map_view_backing_receiver_mutated_names_from_stmts(
                        &else_block.statements,
                        &mut aliases.clone(),
                        mutated,
                    );
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    condition, aliases, mutated,
                );
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut aliases.clone(),
                    mutated,
                );
            }
            Stmt::DoWhile {
                body, condition, ..
            } => {
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut aliases.clone(),
                    mutated,
                );
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    condition, aliases, mutated,
                );
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let mut loop_aliases = aliases.clone();
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    init,
                    &mut loop_aliases,
                    mutated,
                );
                if let Some(condition) = condition {
                    collect_map_view_backing_receiver_mutated_names_from_expr(
                        condition,
                        &loop_aliases,
                        mutated,
                    );
                }
                for expr in update {
                    collect_map_view_backing_receiver_mutated_names_from_expr(
                        expr,
                        &loop_aliases,
                        mutated,
                    );
                }
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut loop_aliases,
                    mutated,
                );
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    iterable, aliases, mutated,
                );
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut aliases.clone(),
                    mutated,
                );
            }
            Stmt::Throw(expr) => {
                collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
            }
            Stmt::Switch { selector, cases } => {
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    selector, aliases, mutated,
                );
                for case in cases {
                    collect_map_view_backing_receiver_mutated_names_from_stmts(
                        &case.statements,
                        &mut aliases.clone(),
                        mutated,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut try_aliases = aliases.clone();
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    resources,
                    &mut try_aliases,
                    mutated,
                );
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut try_aliases,
                    mutated,
                );
                for catch in catches {
                    collect_map_view_backing_receiver_mutated_names_from_stmts(
                        &catch.body.statements,
                        &mut aliases.clone(),
                        mutated,
                    );
                }
                if let Some(finally_block) = finally_block {
                    collect_map_view_backing_receiver_mutated_names_from_stmts(
                        &finally_block.statements,
                        &mut aliases.clone(),
                        mutated,
                    );
                }
            }
            Stmt::Block(block) => collect_map_view_backing_receiver_mutated_names_from_stmts(
                &block.statements,
                &mut aliases.clone(),
                mutated,
            ),
            Stmt::LabeledBlock { body, .. } => {
                collect_map_view_backing_receiver_mutated_names_from_stmts(
                    &body.statements,
                    &mut aliases.clone(),
                    mutated,
                );
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

fn collect_field_receiver_read_names(block: &Block) -> HashSet<String> {
    let mut read = HashSet::new();
    collect_field_receiver_read_names_from_stmts(&block.statements, &mut read);
    read
}

fn collect_facade_mut_borrow_argument_names_from_stmts(
    statements: &[Stmt],
    facts: &FactSet,
    local_types: &mut HashMap<String, JavaType>,
    mutated: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                initializer: Some(expr),
                name,
                ty,
                ..
            } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    expr,
                    facts,
                    local_types,
                    mutated,
                );
                local_types.insert(name.clone(), ty.clone());
            }
            Stmt::VarDecl {
                initializer: None,
                name,
                ty,
                ..
            } => {
                local_types.insert(name.clone(), ty.clone());
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    expr,
                    facts,
                    local_types,
                    mutated,
                );
            }
            Stmt::Assign { target, value } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    target,
                    facts,
                    local_types,
                    mutated,
                );
                collect_facade_mut_borrow_argument_names_from_expr(
                    value,
                    facts,
                    local_types,
                    mutated,
                );
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    condition,
                    facts,
                    local_types,
                    mutated,
                );
                let mut then_types = local_types.clone();
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &then_block.statements,
                    facts,
                    &mut then_types,
                    mutated,
                );
                if let Some(else_block) = else_block {
                    let mut else_types = local_types.clone();
                    collect_facade_mut_borrow_argument_names_from_stmts(
                        &else_block.statements,
                        facts,
                        &mut else_types,
                        mutated,
                    );
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                body, condition, ..
            } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    condition,
                    facts,
                    local_types,
                    mutated,
                );
                let mut body_types = local_types.clone();
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &body.statements,
                    facts,
                    &mut body_types,
                    mutated,
                );
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                let mut for_types = local_types.clone();
                collect_facade_mut_borrow_argument_names_from_stmts(
                    init,
                    facts,
                    &mut for_types,
                    mutated,
                );
                if let Some(condition) = condition {
                    collect_facade_mut_borrow_argument_names_from_expr(
                        condition, facts, &for_types, mutated,
                    );
                }
                for expr in update {
                    collect_facade_mut_borrow_argument_names_from_expr(
                        expr, facts, &for_types, mutated,
                    );
                }
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &body.statements,
                    facts,
                    &mut for_types,
                    mutated,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                iterable,
                body,
                ..
            } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    iterable,
                    facts,
                    local_types,
                    mutated,
                );
                let mut body_types = local_types.clone();
                body_types.insert(var_name.clone(), var_type.clone());
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &body.statements,
                    facts,
                    &mut body_types,
                    mutated,
                );
            }
            Stmt::Switch { selector, cases } => {
                collect_facade_mut_borrow_argument_names_from_expr(
                    selector,
                    facts,
                    local_types,
                    mutated,
                );
                for case in cases {
                    let mut case_types = local_types.clone();
                    collect_facade_mut_borrow_argument_names_from_stmts(
                        &case.statements,
                        facts,
                        &mut case_types,
                        mutated,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                let mut try_types = local_types.clone();
                collect_facade_mut_borrow_argument_names_from_stmts(
                    resources,
                    facts,
                    &mut try_types,
                    mutated,
                );
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &body.statements,
                    facts,
                    &mut try_types,
                    mutated,
                );
                for catch in catches {
                    let mut catch_types = try_types.clone();
                    if let Some(catch_ty) = catch.types.first() {
                        catch_types.insert(catch.source_name.clone(), catch_ty.clone());
                    }
                    collect_facade_mut_borrow_argument_names_from_stmts(
                        &catch.body.statements,
                        facts,
                        &mut catch_types,
                        mutated,
                    );
                }
                if let Some(finally_block) = finally_block {
                    let mut finally_types = try_types.clone();
                    collect_facade_mut_borrow_argument_names_from_stmts(
                        &finally_block.statements,
                        facts,
                        &mut finally_types,
                        mutated,
                    );
                }
            }
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    collect_facade_mut_borrow_argument_names_from_expr(
                        arg,
                        facts,
                        local_types,
                        mutated,
                    );
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                let mut block_types = local_types.clone();
                collect_facade_mut_borrow_argument_names_from_stmts(
                    &block.statements,
                    facts,
                    &mut block_types,
                    mutated,
                );
            }
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_facade_mut_borrow_argument_names_from_expr(
    expr: &Expr,
    facts: &FactSet,
    local_types: &HashMap<String, JavaType>,
    mutated: &mut HashSet<String>,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            for abi in facts.facade_abi_facts.iter().filter(|abi| {
                abi.key.member.0 == *name
                    && abi.key.arity == args.len()
                    && abi.args.len() == args.len()
            }) {
                if abi.receiver == ReceiverAbi::MutBorrowedSelf
                    && receiver.as_deref().is_some_and(|receiver| {
                        facade_abi_matches_receiver_expr(abi, receiver, local_types)
                    })
                    && let Some(Expr::Var(name)) = receiver.as_deref()
                {
                    mutated.insert(name.clone());
                }
                for (index, slot) in abi.args.iter().enumerate() {
                    if !slot.contains_mut_borrow() {
                        continue;
                    }
                    if let Some(Expr::Var(name)) = args.get(index) {
                        mutated.insert(name.clone());
                    }
                }
            }
            if let Some(receiver) = receiver {
                collect_facade_mut_borrow_argument_names_from_expr(
                    receiver,
                    facts,
                    local_types,
                    mutated,
                );
            }
            for arg in args {
                collect_facade_mut_borrow_argument_names_from_expr(
                    arg,
                    facts,
                    local_types,
                    mutated,
                );
            }
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            collect_facade_mut_borrow_argument_names_from_expr(
                receiver,
                facts,
                local_types,
                mutated,
            );
        }
        Expr::Binary { left, right, .. } => {
            collect_facade_mut_borrow_argument_names_from_expr(left, facts, local_types, mutated);
            collect_facade_mut_borrow_argument_names_from_expr(right, facts, local_types, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_facade_mut_borrow_argument_names_from_expr(
                condition,
                facts,
                local_types,
                mutated,
            );
            collect_facade_mut_borrow_argument_names_from_expr(
                then_expr,
                facts,
                local_types,
                mutated,
            );
            collect_facade_mut_borrow_argument_names_from_expr(
                else_expr,
                facts,
                local_types,
                mutated,
            );
        }
        Expr::Switch { selector, cases } => {
            collect_facade_mut_borrow_argument_names_from_expr(
                selector,
                facts,
                local_types,
                mutated,
            );
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_facade_mut_borrow_argument_names_from_expr(
                            expr,
                            facts,
                            local_types,
                            mutated,
                        );
                    }
                }
                collect_facade_mut_borrow_argument_names_from_expr(
                    &case.value,
                    facts,
                    local_types,
                    mutated,
                );
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_facade_mut_borrow_argument_names_from_expr(array, facts, local_types, mutated);
            collect_facade_mut_borrow_argument_names_from_expr(index, facts, local_types, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_facade_mut_borrow_argument_names_from_expr(
                    expr,
                    facts,
                    local_types,
                    mutated,
                );
            }
            for expr in initializer.iter().flatten() {
                collect_facade_mut_borrow_argument_names_from_expr(
                    expr,
                    facts,
                    local_types,
                    mutated,
                );
            }
        }
        Expr::ArrayInitializer(items) => {
            for expr in items {
                collect_facade_mut_borrow_argument_names_from_expr(
                    expr,
                    facts,
                    local_types,
                    mutated,
                );
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_facade_mut_borrow_argument_names_from_expr(
                    arg,
                    facts,
                    local_types,
                    mutated,
                );
            }
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_facade_mut_borrow_argument_names_from_expr(expr, facts, local_types, mutated);
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn facade_abi_matches_receiver_expr(
    abi: &FacadeAbiFact,
    receiver: &Expr,
    local_types: &HashMap<String, JavaType>,
) -> bool {
    let Some(receiver_ty) = local_receiver_type(receiver, local_types) else {
        return false;
    };
    facade_abi_owner_matches_java_type(abi, receiver_ty)
}

fn local_receiver_type<'a>(
    receiver: &'a Expr,
    local_types: &'a HashMap<String, JavaType>,
) -> Option<&'a JavaType> {
    match receiver {
        Expr::Var(name) => local_types.get(name),
        _ => None,
    }
}

fn facade_abi_owner_matches_java_type(abi: &FacadeAbiFact, receiver_ty: &JavaType) -> bool {
    let owner = &abi.key.owner.type_name;
    match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::facts::external_type_name_matches(owner, name)
        }
        _ => false,
    }
}

fn collect_field_receiver_read_names_from_stmts(statements: &[Stmt], read: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } => {
                collect_field_receiver_read_names_from_expr(target, read);
                collect_field_receiver_read_names_from_expr(value, read);
            }
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    collect_field_receiver_read_names_from_expr(expr, read);
                }
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_field_receiver_read_names_from_expr(expr, read);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_field_receiver_read_names_from_expr(condition, read);
                collect_field_receiver_read_names_from_stmts(&then_block.statements, read);
                if let Some(else_block) = else_block {
                    collect_field_receiver_read_names_from_stmts(&else_block.statements, read);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                collect_field_receiver_read_names_from_expr(condition, read);
                collect_field_receiver_read_names_from_stmts(&body.statements, read);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_field_receiver_read_names_from_stmts(init, read);
                if let Some(condition) = condition {
                    collect_field_receiver_read_names_from_expr(condition, read);
                }
                for expr in update {
                    collect_field_receiver_read_names_from_expr(expr, read);
                }
                collect_field_receiver_read_names_from_stmts(&body.statements, read);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_field_receiver_read_names_from_expr(iterable, read);
                collect_field_receiver_read_names_from_stmts(&body.statements, read);
            }
            Stmt::Switch { selector, cases } => {
                collect_field_receiver_read_names_from_expr(selector, read);
                for case in cases {
                    collect_field_receiver_read_names_from_stmts(&case.statements, read);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_field_receiver_read_names_from_stmts(resources, read);
                collect_field_receiver_read_names_from_stmts(&body.statements, read);
                for catch in catches {
                    collect_field_receiver_read_names_from_stmts(&catch.body.statements, read);
                }
                if let Some(finally_block) = finally_block {
                    collect_field_receiver_read_names_from_stmts(&finally_block.statements, read);
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_field_receiver_read_names_from_stmts(&block.statements, read);
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

fn collect_field_receiver_read_names_from_expr(expr: &Expr, read: &mut HashSet<String>) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            if let Expr::Var(name) = receiver.as_ref() {
                read.insert(name.clone());
            }
            collect_field_receiver_read_names_from_expr(receiver, read);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_field_receiver_read_names_from_expr(expr, read)
        }
        Expr::Binary { left, right, .. } => {
            collect_field_receiver_read_names_from_expr(left, read);
            collect_field_receiver_read_names_from_expr(right, read);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_field_receiver_read_names_from_expr(condition, read);
            collect_field_receiver_read_names_from_expr(then_expr, read);
            collect_field_receiver_read_names_from_expr(else_expr, read);
        }
        Expr::Switch { selector, cases } => {
            collect_field_receiver_read_names_from_expr(selector, read);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_field_receiver_read_names_from_expr(expr, read);
                    }
                }
                collect_field_receiver_read_names_from_expr(&case.value, read);
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_field_receiver_read_names_from_expr(array, read);
            collect_field_receiver_read_names_from_expr(index, read);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_field_receiver_read_names_from_expr(expr, read);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_field_receiver_read_names_from_expr(expr, read);
                }
            }
        }
        Expr::ArrayInitializer(elements) | Expr::New { args: elements, .. } => {
            for expr in elements {
                collect_field_receiver_read_names_from_expr(expr, read);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_field_receiver_read_names_from_expr(receiver, read);
            }
            for arg in args {
                collect_field_receiver_read_names_from_expr(arg, read);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_field_receiver_read_names_from_expr(expr, read),
            LambdaBody::Block(block) => {
                collect_field_receiver_read_names_from_stmts(&block.statements, read);
            }
        },
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_field_receiver_mutated_names_from_stmts(
    statements: &[Stmt],
    mutated: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } => {
                collect_field_receiver_assignment_target_name(target, mutated);
                collect_field_receiver_mutated_names_from_expr(value, mutated);
            }
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    collect_field_receiver_mutated_names_from_expr(expr, mutated);
                }
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_field_receiver_mutated_names_from_expr(expr, mutated);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_field_receiver_mutated_names_from_expr(condition, mutated);
                collect_field_receiver_mutated_names_from_stmts(&then_block.statements, mutated);
                if let Some(else_block) = else_block {
                    collect_field_receiver_mutated_names_from_stmts(
                        &else_block.statements,
                        mutated,
                    );
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                collect_field_receiver_mutated_names_from_expr(condition, mutated);
                collect_field_receiver_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_field_receiver_mutated_names_from_stmts(init, mutated);
                if let Some(condition) = condition {
                    collect_field_receiver_mutated_names_from_expr(condition, mutated);
                }
                for expr in update {
                    collect_field_receiver_mutated_names_from_expr(expr, mutated);
                }
                collect_field_receiver_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_field_receiver_mutated_names_from_expr(iterable, mutated);
                collect_field_receiver_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::Switch { selector, cases } => {
                collect_field_receiver_mutated_names_from_expr(selector, mutated);
                for case in cases {
                    collect_field_receiver_mutated_names_from_stmts(&case.statements, mutated);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_field_receiver_mutated_names_from_stmts(resources, mutated);
                collect_field_receiver_mutated_names_from_stmts(&body.statements, mutated);
                for catch in catches {
                    collect_field_receiver_mutated_names_from_stmts(
                        &catch.body.statements,
                        mutated,
                    );
                }
                if let Some(finally_block) = finally_block {
                    collect_field_receiver_mutated_names_from_stmts(
                        &finally_block.statements,
                        mutated,
                    );
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_field_receiver_mutated_names_from_stmts(&block.statements, mutated);
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

fn collect_map_view_backing_receiver_mutated_names_from_expr(
    expr: &Expr,
    aliases: &HashMap<String, String>,
    mutated: &mut HashSet<String>,
) {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            if let Some(receiver) = receiver
                && crate::stdlib::native_collection_method_name_mutates_receiver(name)
                && let Some(view_backing_receiver) = map_view_call_backing_receiver(receiver)
                && let Some(root) = receiver_root_name(view_backing_receiver)
            {
                mutated.insert(root.to_string());
            }
            if let Some(Expr::Var(receiver_name)) = receiver.as_deref()
                && crate::stdlib::native_collection_method_name_mutates_receiver(name)
                && let Some(root) = aliases.get(receiver_name)
            {
                mutated.insert(root.clone());
            }
            if let Some(receiver) = receiver {
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    receiver, aliases, mutated,
                );
            }
            for arg in args {
                collect_map_view_backing_receiver_mutated_names_from_expr(arg, aliases, mutated);
            }
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
        }
        Expr::Binary { left, right, .. } => {
            collect_map_view_backing_receiver_mutated_names_from_expr(left, aliases, mutated);
            collect_map_view_backing_receiver_mutated_names_from_expr(right, aliases, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_map_view_backing_receiver_mutated_names_from_expr(condition, aliases, mutated);
            collect_map_view_backing_receiver_mutated_names_from_expr(then_expr, aliases, mutated);
            collect_map_view_backing_receiver_mutated_names_from_expr(else_expr, aliases, mutated);
        }
        Expr::Switch { selector, cases } => {
            collect_map_view_backing_receiver_mutated_names_from_expr(selector, aliases, mutated);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_map_view_backing_receiver_mutated_names_from_expr(
                            expr, aliases, mutated,
                        );
                    }
                }
                collect_map_view_backing_receiver_mutated_names_from_expr(
                    &case.value,
                    aliases,
                    mutated,
                );
            }
        }
        Expr::FieldAccess(receiver, _) => {
            collect_map_view_backing_receiver_mutated_names_from_expr(receiver, aliases, mutated);
        }
        Expr::ArrayAccess(array, index) => {
            collect_map_view_backing_receiver_mutated_names_from_expr(array, aliases, mutated);
            collect_map_view_backing_receiver_mutated_names_from_expr(index, aliases, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_map_view_backing_receiver_mutated_names_from_expr(
                        expr, aliases, mutated,
                    );
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for expr in items {
                collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_map_view_backing_receiver_mutated_names_from_expr(arg, aliases, mutated);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_map_view_backing_receiver_mutated_names_from_expr(expr, aliases, mutated);
            }
            LambdaBody::Block(block) => collect_map_view_backing_receiver_mutated_names_from_stmts(
                &block.statements,
                &mut aliases.clone(),
                mutated,
            ),
        },
        Expr::MethodReference { .. }
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Literal(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_field_receiver_mutated_names_from_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            collect_field_receiver_assignment_target_name(left, mutated);
            collect_field_receiver_mutated_names_from_expr(right, mutated);
        }
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_field_receiver_assignment_target_name(expr, mutated),
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::FieldAccess(expr, _) => {
            collect_field_receiver_mutated_names_from_expr(expr, mutated)
        }
        Expr::Binary { left, right, .. } => {
            collect_field_receiver_mutated_names_from_expr(left, mutated);
            collect_field_receiver_mutated_names_from_expr(right, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_field_receiver_mutated_names_from_expr(condition, mutated);
            collect_field_receiver_mutated_names_from_expr(then_expr, mutated);
            collect_field_receiver_mutated_names_from_expr(else_expr, mutated);
        }
        Expr::Switch { selector, cases } => {
            collect_field_receiver_mutated_names_from_expr(selector, mutated);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_field_receiver_mutated_names_from_expr(expr, mutated);
                    }
                }
                collect_field_receiver_mutated_names_from_expr(&case.value, mutated);
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_field_receiver_mutated_names_from_expr(array, mutated);
            collect_field_receiver_mutated_names_from_expr(index, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_field_receiver_mutated_names_from_expr(expr, mutated);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_field_receiver_mutated_names_from_expr(expr, mutated);
                }
            }
        }
        Expr::ArrayInitializer(elements) | Expr::New { args: elements, .. } => {
            for expr in elements {
                collect_field_receiver_mutated_names_from_expr(expr, mutated);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(name) = collection_view_mutating_receiver_name(receiver.as_deref(), expr) {
                mutated.insert(name);
            }
            if let Some(receiver) = receiver {
                collect_field_receiver_mutated_names_from_expr(receiver, mutated);
            }
            for arg in args {
                collect_field_receiver_mutated_names_from_expr(arg, mutated);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_field_receiver_mutated_names_from_expr(expr, mutated),
            LambdaBody::Block(block) => {
                collect_field_receiver_mutated_names_from_stmts(&block.statements, mutated);
            }
        },
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collection_view_mutating_receiver_name(receiver: Option<&Expr>, call: &Expr) -> Option<String> {
    let Expr::Call { name, args, .. } = call else {
        return None;
    };
    let Expr::Call {
        receiver: Some(map_receiver),
        name: view_name,
        args: view_args,
        ..
    } = receiver?
    else {
        return None;
    };
    if !view_args.is_empty() {
        return None;
    }
    let mutates_view = match (view_name.as_str(), name.as_str(), args.len()) {
        ("keySet" | "key_set", "remove", 1) | ("keySet" | "key_set", "clear", 0) => true,
        ("values", "remove", 1) | ("values", "clear", 0) => true,
        ("entrySet" | "entry_set", "remove", 1) | ("entrySet" | "entry_set", "clear", 0) => true,
        _ => false,
    };
    if !mutates_view {
        return None;
    }
    let Expr::Var(name) = map_receiver.as_ref() else {
        return None;
    };
    Some(name.clone())
}

fn collect_field_receiver_assignment_target_name(expr: &Expr, mutated: &mut HashSet<String>) {
    if let Expr::FieldAccess(receiver, _) = expr
        && let Expr::Var(name) = receiver.as_ref()
    {
        mutated.insert(name.clone());
    }
}

fn collect_array_element_mutated_names_from_stmts(
    statements: &[Stmt],
    mutated: &mut HashSet<String>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } => {
                collect_array_element_assignment_target_name(target, mutated);
                collect_array_element_mutated_names_from_expr(value, mutated);
            }
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    collect_array_element_mutated_names_from_expr(expr, mutated);
                }
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_array_element_mutated_names_from_expr(expr, mutated);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_array_element_mutated_names_from_expr(condition, mutated);
                collect_array_element_mutated_names_from_stmts(&then_block.statements, mutated);
                if let Some(else_block) = else_block {
                    collect_array_element_mutated_names_from_stmts(&else_block.statements, mutated);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                collect_array_element_mutated_names_from_expr(condition, mutated);
                collect_array_element_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_array_element_mutated_names_from_stmts(init, mutated);
                if let Some(condition) = condition {
                    collect_array_element_mutated_names_from_expr(condition, mutated);
                }
                for expr in update {
                    collect_array_element_mutated_names_from_expr(expr, mutated);
                }
                collect_array_element_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_array_element_mutated_names_from_expr(iterable, mutated);
                collect_array_element_mutated_names_from_stmts(&body.statements, mutated);
            }
            Stmt::Switch { selector, cases } => {
                collect_array_element_mutated_names_from_expr(selector, mutated);
                for case in cases {
                    collect_array_element_mutated_names_from_stmts(&case.statements, mutated);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_array_element_mutated_names_from_stmts(resources, mutated);
                collect_array_element_mutated_names_from_stmts(&body.statements, mutated);
                for catch in catches {
                    collect_array_element_mutated_names_from_stmts(&catch.body.statements, mutated);
                }
                if let Some(finally_block) = finally_block {
                    collect_array_element_mutated_names_from_stmts(
                        &finally_block.statements,
                        mutated,
                    );
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_array_element_mutated_names_from_stmts(&block.statements, mutated);
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

fn collect_array_element_mutated_names_from_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            collect_array_element_assignment_target_name(left, mutated);
            collect_array_element_mutated_names_from_expr(right, mutated);
        }
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_array_element_assignment_target_name(expr, mutated),
        Expr::Unary { expr, .. }
        | Expr::Cast { expr, .. }
        | Expr::InstanceOf { expr, .. }
        | Expr::FieldAccess(expr, _) => {
            collect_array_element_mutated_names_from_expr(expr, mutated)
        }
        Expr::Binary { left, right, .. } => {
            collect_array_element_mutated_names_from_expr(left, mutated);
            collect_array_element_mutated_names_from_expr(right, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_array_element_mutated_names_from_expr(condition, mutated);
            collect_array_element_mutated_names_from_expr(then_expr, mutated);
            collect_array_element_mutated_names_from_expr(else_expr, mutated);
        }
        Expr::Switch { selector, cases } => {
            collect_array_element_mutated_names_from_expr(selector, mutated);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_array_element_mutated_names_from_expr(expr, mutated);
                    }
                }
                collect_array_element_mutated_names_from_expr(&case.value, mutated);
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_array_element_mutated_names_from_expr(array, mutated);
            collect_array_element_mutated_names_from_expr(index, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_array_element_mutated_names_from_expr(expr, mutated);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_array_element_mutated_names_from_expr(expr, mutated);
                }
            }
        }
        Expr::ArrayInitializer(elements) | Expr::New { args: elements, .. } => {
            for expr in elements {
                collect_array_element_mutated_names_from_expr(expr, mutated);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_array_element_mutated_names_from_expr(receiver, mutated);
            }
            for arg in args {
                collect_array_element_mutated_names_from_expr(arg, mutated);
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => collect_array_element_mutated_names_from_expr(expr, mutated),
            LambdaBody::Block(block) => {
                collect_array_element_mutated_names_from_stmts(&block.statements, mutated);
            }
        },
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_array_element_assignment_target_name(expr: &Expr, mutated: &mut HashSet<String>) {
    if let Expr::ArrayAccess(_, _) = expr
        && let Some(name) = array_access_root_name(expr)
    {
        mutated.insert(name.to_string());
    }
}

fn collect_mutated_local_names_from_stmts(statements: &[Stmt], mutated: &mut HashSet<String>) {
    for stmt in statements {
        collect_mutated_local_names_from_stmt(stmt, mutated);
    }
}

fn collect_mutated_local_names_from_stmt(stmt: &Stmt, mutated: &mut HashSet<String>) {
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
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
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
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
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

fn collect_mutated_local_names_from_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_assignment_target_name(expr, mutated),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_mutated_local_names_from_expr(expr, mutated);
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            collect_assignment_target_name(left, mutated);
            collect_mutated_local_names_from_expr(right, mutated);
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
        Expr::Switch { selector, cases } => {
            collect_mutated_local_names_from_expr(selector, mutated);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_mutated_local_names_from_expr(expr, mutated);
                    }
                }
                collect_mutated_local_names_from_expr(&case.value, mutated);
            }
        }
        Expr::FieldAccess(receiver, _) => collect_mutated_local_names_from_expr(receiver, mutated),
        Expr::ArrayAccess(array, index) => {
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_mutated_local_names_from_expr(expr, mutated);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_mutated_local_names_from_expr(expr, mutated);
                }
            }
        }
        Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                collect_mutated_local_names_from_expr(expr, mutated);
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            for (arg_index, arg) in args.iter().enumerate() {
                if crate::stdlib::static_collection_argument_mutates(
                    receiver.as_deref().and_then(static_receiver_name),
                    name,
                    args.len(),
                    arg_index,
                ) {
                    collect_assignment_target_name(arg, mutated);
                }
            }
            if let Some(receiver) = receiver {
                if crate::stdlib::native_collection_method_name_mutates_receiver(name)
                    && let Some(view_backing_receiver) = map_view_call_backing_receiver(receiver)
                    && let Some(name) = receiver_root_name(view_backing_receiver)
                {
                    mutated.insert(name.to_string());
                }
                if crate::stdlib::native_collection_method_name_mutates_receiver(name)
                    && let Some(name) = receiver_root_name(receiver)
                {
                    mutated.insert(name.to_string());
                }
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
        Expr::Lambda { params, body } => {
            let mut lambda_mutated = HashSet::new();
            match body {
                LambdaBody::Expr(expr) => {
                    collect_mutated_local_names_from_expr(expr, &mut lambda_mutated);
                }
                LambdaBody::Block(block) => {
                    collect_mutated_local_names_from_stmts(&block.statements, &mut lambda_mutated);
                }
            }
            for param in params {
                lambda_mutated.remove(&param.name);
            }
            mutated.extend(lambda_mutated);
        }
        Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn map_view_call_backing_receiver(receiver: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(map_receiver),
        name,
        args,
        ..
    } = receiver
    else {
        return None;
    };
    if args.is_empty()
        && matches!(
            name.as_str(),
            "keySet" | "key_set" | "values" | "entrySet" | "entry_set"
        )
    {
        Some(map_receiver)
    } else {
        None
    }
}

fn collect_assignment_target_name(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            mutated.insert(name.clone());
        }
        Expr::ArrayAccess(array, index) => {
            if let Some(name) = array_access_root_name(expr) {
                mutated.insert(name.to_string());
            }
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::FieldAccess(receiver, _) => {
            if let Expr::Var(name) = receiver.as_ref() {
                mutated.insert(name.clone());
            }
            collect_mutated_local_names_from_expr(receiver, mutated);
        }
        _ => collect_mutated_local_names_from_expr(expr, mutated),
    }
}

fn array_access_root_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::ArrayAccess(array, _) => array_access_root_name(array),
        _ => None,
    }
}

fn static_receiver_name(receiver: &Expr) -> Option<&str> {
    match receiver {
        Expr::Var(name) => Some(name),
        _ => None,
    }
}

fn receiver_root_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::FieldAccess(receiver, _) | Expr::ArrayAccess(receiver, _) => {
            receiver_root_name(receiver)
        }
        _ => None,
    }
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
    method_return_shape_constraints_with_declared_source_types(
        ty,
        method,
        usage,
        facts,
        &HashSet::new(),
    )
}

fn method_return_shape_constraints_with_declared_source_types(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
    _declared_source_types: &HashSet<String>,
) -> Vec<SignatureConstraint> {
    if method.body.is_none()
        && !method.modifiers.contains(&Modifier::Abstract)
        && method_return_type_param_name(method).is_none()
        && source_summary_bodyless_return_may_be_null(&method.return_type)
    {
        return vec![SignatureConstraint::RequiredStorageNullability(
            NullabilityState::MaybeNull,
        )];
    }
    if source_static_initialized_singleton_getter(ty, method).is_some() {
        return vec![
            SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull),
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredView(ViewShape::Native),
        ];
    }
    let mut boundary_kind = method_return_boundary_policy(ty, method, usage, facts).kind;
    let borrowed_object_return = source_method_return_should_borrow_object(ty, method, facts);
    if borrowed_object_return {
        boundary_kind = None;
    }
    let mut constraints =
        signature_slot_baseline_policy(&method.return_type, boundary_kind, facts).constraints;
    push_unique_signature_constraints(
        &mut constraints,
        &signature_object_representation_fact_constraints(&method.return_type, facts),
    );
    if borrowed_object_return {
        push_unique_signature_constraints(
            &mut constraints,
            &[SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            )],
        );
    }
    let method_effect_nullability =
        facts
            .method_effect_for(&ty.name, &method.name)
            .and_then(|effect| {
                (effect.return_nullability != NullabilityState::Unknown)
                    .then_some(effect.return_nullability)
            });
    if !matches!(method.return_type, JavaType::Void)
        && let Some(return_nullability) = method_effect_nullability
    {
        push_unique_signature_constraints(
            &mut constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                return_nullability,
            )],
        );
    }
    if method_return_type_param_name(method).is_some() {
        retain_non_ownership_view_constraints(&mut constraints);
        push_unique_signature_constraints(&mut constraints, &owned_native_signature_constraints());
    }
    let method_usage = usage.source_method_usage(ty, method);
    let returns_call_result =
        method_usage.is_some_and(|method_usage| !method_usage.returned_call_results.is_empty());
    let returns_unknown_call_result = method_usage.is_some_and(|method_usage| {
        method_usage_returns_unknown_call_result(method_usage, &method.return_type, facts)
    });
    let returns_call_result_alias = method_returns_local_initialized_from_call(method);
    let typed_return_nullability =
        typed_source_method_return_nullability_constraints(ty, method, facts);
    let flow_return_nullability = if boundary_kind == Some(SignatureBoundaryKind::PublicApi) {
        nullable_storage_constraints(&typed_return_nullability)
    } else {
        typed_return_nullability
    };
    let refined_local_returns_prove_non_null =
        method_returns_only_post_refined_non_null_locals(method);
    let returned_initializer_nullability = if returns_call_result || returns_call_result_alias {
        Vec::new()
    } else {
        returned_local_initializer_source_call_nullability_constraints(ty, method, facts)
    };
    let flow_proves_non_null_return =
        signature_constraints_prove_non_null_storage(&flow_return_nullability)
            || refined_local_returns_prove_non_null;
    let mut source_return_nullability = flow_return_nullability;
    if refined_local_returns_prove_non_null {
        push_unique_signature_constraints(
            &mut source_return_nullability,
            &[SignatureConstraint::RequiredStorageNullability(
                NullabilityState::DefinitelyNonNull,
            )],
        );
    }
    push_unique_signature_constraints(
        &mut source_return_nullability,
        &returned_initializer_nullability,
    );
    if flow_proves_non_null_return {
        retain_only_storage_nullability(
            &mut source_return_nullability,
            NullabilityState::DefinitelyNonNull,
        );
        constraints.retain(|constraint| {
            !matches!(
                constraint,
                SignatureConstraint::RequiredStorageNullability(_)
            )
        });
    }
    push_unique_signature_constraints(&mut constraints, &source_return_nullability);
    let external_return_constraints =
        external_call_return_method_return_constraints(ty, method, usage, facts);
    push_unique_signature_constraints(&mut constraints, &external_return_constraints);
    push_unique_signature_constraints(
        &mut constraints,
        &nullable_external_non_concrete_return_constraints(
            &method.return_type,
            &source_return_nullability,
            &external_return_constraints,
            facts,
        ),
    );
    let borrowed_return_content_policy =
        borrowed_return_content_policy_from_param_with_facts(ty, method, usage, facts);
    if borrowed_return_content_policy.is_none()
        && !returns_unknown_call_result
        && !returns_call_result_alias
        && !borrowed_object_return
        && boundary_kind != Some(SignatureBoundaryKind::PublicApi)
        && declared_return_should_use_concrete_shape(
            method,
            method_effect_nullability,
            &source_return_nullability,
            &external_return_constraints,
            facts,
        )
    {
        constraints.retain(|constraint| {
            !matches!(
                constraint,
                SignatureConstraint::RequiredStorageNullability(_)
            )
        });
        push_unique_signature_constraints(
            &mut constraints,
            &[
                SignatureConstraint::RequiredStorageNullability(
                    NullabilityState::DefinitelyNonNull,
                ),
                SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
                SignatureConstraint::RequiredView(ViewShape::Native),
            ],
        );
    }
    push_unique_signature_constraints(
        &mut constraints,
        &anonymous_declared_return_constraints(method, facts),
    );
    if boundary_kind.is_none() {
        push_unique_signature_constraints(
            &mut constraints,
            &internal_exact_final_erased_facade_constraints(&method.return_type, facts),
        );
        constraints.extend(
            borrowed_return_content_policy
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &internal_returned_object_alias_borrow_constraints(ty, method, usage, facts),
        );
    }
    constraints
}

fn method_returns_only_post_refined_non_null_locals(method: &MethodDecl) -> bool {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    let mut saw_return = false;
    let mut refinements = HashSet::new();
    block_returns_only_refined_non_null_locals(body, &mut refinements, &mut saw_return)
        && saw_return
}

fn block_returns_only_refined_non_null_locals(
    block: &Block,
    refinements: &mut HashSet<String>,
    saw_return: &mut bool,
) -> bool {
    for stmt in &block.statements {
        if !stmt_returns_only_refined_non_null_locals(stmt, refinements, saw_return) {
            return false;
        }
        update_post_stmt_nullability_refinements(stmt, refinements);
    }
    true
}

fn stmt_returns_only_refined_non_null_locals(
    stmt: &Stmt,
    refinements: &HashSet<String>,
    saw_return: &mut bool,
) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::Var(name))) if refinements.contains(name) => {
            *saw_return = true;
            true
        }
        Stmt::Return(Some(expr)) => {
            *saw_return = true;
            matches!(
                source_return_direct_nullability_state(expr),
                Some(NullabilityState::DefinitelyNonNull)
            )
        }
        Stmt::Return(None) => false,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            let mut then_refinements = refinements.clone();
            if !block_returns_only_refined_non_null_locals(
                then_block,
                &mut then_refinements,
                saw_return,
            ) {
                return false;
            }
            if let Some(else_block) = else_block {
                let mut else_refinements = refinements.clone();
                block_returns_only_refined_non_null_locals(
                    else_block,
                    &mut else_refinements,
                    saw_return,
                )
            } else {
                true
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            let mut block_refinements = refinements.clone();
            block_returns_only_refined_non_null_locals(block, &mut block_refinements, saw_return)
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            let mut body_refinements = refinements.clone();
            if !block_returns_only_refined_non_null_locals(body, &mut body_refinements, saw_return)
            {
                return false;
            }
            for catch in catches {
                let mut catch_refinements = refinements.clone();
                if !block_returns_only_refined_non_null_locals(
                    &catch.body,
                    &mut catch_refinements,
                    saw_return,
                ) {
                    return false;
                }
            }
            if let Some(finally_block) = finally_block {
                let mut finally_refinements = refinements.clone();
                block_returns_only_refined_non_null_locals(
                    finally_block,
                    &mut finally_refinements,
                    saw_return,
                )
            } else {
                true
            }
        }
        Stmt::Switch { cases, .. } => cases.iter().all(|case| {
            let mut case_refinements = refinements.clone();
            let block = Block {
                statements: case.statements.clone(),
            };
            block_returns_only_refined_non_null_locals(&block, &mut case_refinements, saw_return)
        }),
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::EnhancedFor { body, .. } => {
            let mut body_refinements = refinements.clone();
            block_returns_only_refined_non_null_locals(body, &mut body_refinements, saw_return)
        }
        Stmt::For { init, body, .. } => {
            let mut for_refinements = refinements.clone();
            let init_block = Block {
                statements: init.clone(),
            };
            block_returns_only_refined_non_null_locals(
                &init_block,
                &mut for_refinements,
                saw_return,
            ) && block_returns_only_refined_non_null_locals(body, &mut for_refinements, saw_return)
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Throw(_)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => true,
    }
}

fn method_usage_returns_unknown_call_result(
    method_usage: &MethodUsageFacts,
    declared_return: &JavaType,
    facts: &FactSet,
) -> bool {
    (!java_type_is_string_like_signature_type(declared_return)
        && method_usage.returned_call_results.is_empty()
        && method_usage.call_sites.iter().any(|call_site| {
            call_site.result_returned && call_site.boundary == CallArgumentBoundary::Unknown
        }))
        || method_usage.returned_call_results.iter().any(|returned| {
            returned.boundary == CallArgumentBoundary::Unknown
                && !returned_external_call_result_matches_declared_return(
                    returned,
                    declared_return,
                    facts,
                )
                && !returned_native_collection_get_result_matches_declared_return(
                    returned,
                    declared_return,
                    facts,
                )
        })
}

fn java_type_is_string_like_signature_type(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::String)
        || matches!(
            java_ty,
            JavaType::Named { name, .. } | JavaType::Unknown(name)
                if crate::stdlib::canonical_external_receiver_type_key(name) == "String"
        )
}

fn source_summary_bodyless_return_may_be_null(java_ty: &JavaType) -> bool {
    crate::stdlib::java_collection_kind_for_type(java_ty).is_some()
}

fn source_method_return_should_borrow_object(
    ty: &TypeDecl,
    method: &MethodDecl,
    facts: &FactSet,
) -> bool {
    if source_static_initialized_singleton_getter(ty, method).is_some() {
        return false;
    }
    if method_return_type_param_name(method).is_some() {
        return false;
    }
    if !java_return_type_can_borrow_as_object(&method.return_type, facts) {
        return false;
    }
    if ty.kind == TypeKind::Interface {
        return true;
    }
    direct_self_field_return_type(ty, method).is_some_and(|field_ty| field_ty == method.return_type)
}

fn source_static_initialized_singleton_getter<'a>(
    ty: &'a TypeDecl,
    method: &MethodDecl,
) -> Option<&'a crate::java_ir::FieldDecl> {
    if !method.modifiers.contains(&Modifier::Static) || !method.params.is_empty() {
        return None;
    }
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(Expr::Var(field_name)))] = body.statements.as_slice() else {
        return None;
    };
    ty.members.iter().find_map(|member| {
        let crate::java_ir::Member::Field(field) = member else {
            return None;
        };
        if field.name != *field_name
            || !field.modifiers.contains(&Modifier::Static)
            || !java_types_have_same_signature_boundary_identity(&field.ty, &method.return_type)
        {
            return None;
        }
        static_field_has_same_type_new_initializer(field, &method.return_type).then_some(field)
    })
}

fn static_field_has_same_type_new_initializer(
    field: &crate::java_ir::FieldDecl,
    return_type: &JavaType,
) -> bool {
    let Some(Expr::New { ty, args, body, .. }) = &field.initializer else {
        return false;
    };
    args.is_empty()
        && body.as_ref().is_none_or(|members| members.is_empty())
        && java_types_have_same_signature_boundary_identity(ty, return_type)
}

fn direct_self_field_return_type(ty: &TypeDecl, method: &MethodDecl) -> Option<JavaType> {
    let body = method.body.as_ref()?;
    let [Stmt::Return(Some(expr))] = body.statements.as_slice() else {
        return None;
    };
    let field_name = match expr {
        Expr::Var(name) => name,
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => field,
        _ => return None,
    };
    ty.members.iter().find_map(|member| match member {
        crate::java_ir::Member::Field(field) if field.name == *field_name => Some(field.ty.clone()),
        _ => None,
    })
}

fn java_return_type_can_borrow_as_object(java_ty: &JavaType, facts: &FactSet) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    if matches!(
        effective_facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) {
        return false;
    }
    match java_ty {
        JavaType::Primitive(_) | JavaType::String | JavaType::Void | JavaType::Array(_) => {
            return false;
        }
        JavaType::Named { name, args } => {
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Object"
                || crate::stdlib::java_collection_kind_for_type(java_ty).is_some()
                || effective_facts
                    .type_capabilities_for(java_ty)
                    .sequence_like
                    .is_some()
            {
                return false;
            }
            if args.is_empty()
                && effective_facts.object_safe_trait_fact_for_name(name) == Some(true)
            {
                return true;
            }
        }
        JavaType::Unknown(name) => {
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Object" {
                return false;
            }
        }
        JavaType::Wildcard(_) => return false,
    }
    !matches!(conservative_ownership(java_ty), OwnershipShape::CopyValue)
        && !effective_facts.type_capabilities_for(java_ty).copy
}

fn method_returns_local_initialized_from_call(method: &MethodDecl) -> bool {
    let Some(body) = &method.body else {
        return false;
    };
    block_returns_local_initialized_from_call(body, &mut HashMap::new())
}

fn block_returns_local_initialized_from_call(
    block: &Block,
    visible_call_aliases: &mut HashMap<String, bool>,
) -> bool {
    for stmt in &block.statements {
        if stmt_returns_local_initialized_from_call(stmt, visible_call_aliases) {
            return true;
        }
    }
    false
}

fn stmt_returns_local_initialized_from_call(
    stmt: &Stmt,
    visible_call_aliases: &mut HashMap<String, bool>,
) -> bool {
    match stmt {
        Stmt::VarDecl {
            name, initializer, ..
        } => {
            let is_call_alias = initializer.as_ref().is_some_and(|initializer| {
                expr_is_call_result_or_call_alias(initializer, visible_call_aliases)
            });
            visible_call_aliases.insert(name.clone(), is_call_alias);
            false
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target {
                let is_call_alias = expr_is_call_result_or_call_alias(value, visible_call_aliases);
                visible_call_aliases.insert(name.clone(), is_call_alias);
            }
            false
        }
        Stmt::Return(Some(Expr::Var(name))) => visible_call_aliases.get(name) == Some(&true),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_returns_local_initialized_from_call(then_block, &mut visible_call_aliases.clone())
                || else_block.as_ref().is_some_and(|block| {
                    block_returns_local_initialized_from_call(
                        block,
                        &mut visible_call_aliases.clone(),
                    )
                })
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_returns_local_initialized_from_call(block, &mut visible_call_aliases.clone())
        }
        _ => false,
    }
}

fn expr_is_call_result_or_call_alias(
    expr: &Expr,
    visible_call_aliases: &HashMap<String, bool>,
) -> bool {
    match expr {
        Expr::Call { .. } | Expr::New { .. } => true,
        Expr::Var(name) => visible_call_aliases.get(name) == Some(&true),
        Expr::Cast { expr, .. } => expr_is_call_result_or_call_alias(expr, visible_call_aliases),
        _ => false,
    }
}

fn declared_return_should_use_concrete_shape(
    method: &MethodDecl,
    _method_effect_nullability: Option<NullabilityState>,
    source_return_constraints: &[SignatureConstraint],
    external_return_constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> bool {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return false;
    }
    if signature_capability_policy(&method.return_type, facts)
        .capability_ownership
        .is_some_and(|ownership| ownership == OwnershipShape::CopyValue)
    {
        return false;
    }
    if declared_return_has_non_concrete_representation(&method.return_type, facts) {
        return false;
    }
    !constraints_require_nullable_storage(source_return_constraints)
        && !constraints_require_nullable_storage(external_return_constraints)
}

fn declared_return_has_non_concrete_representation(java_ty: &JavaType, facts: &FactSet) -> bool {
    if declared_return_is_jdk_structural_interface(java_ty) {
        return true;
    }
    let effective_facts = EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::DynTrait { .. } | HierarchyStrategy::ErasedObject)
    ) || facts.object_safe_trait_fact_for(java_ty) == Some(true)
}

fn nullable_external_non_concrete_return_constraints(
    java_ty: &JavaType,
    source_return_constraints: &[SignatureConstraint],
    external_return_constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    if !constraints_require_nullable_storage(source_return_constraints)
        && !constraints_require_nullable_storage(external_return_constraints)
    {
        return Vec::new();
    }
    if !declared_external_return_has_non_concrete_representation(java_ty, facts) {
        return Vec::new();
    }
    let effective_facts = EffectiveFacts::new(facts);
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        effective_facts.object_representation_override_strategy_for(java_ty)
    {
        return vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredDispatch(DispatchShape::BoxedDynTrait {
                trait_name: trait_name.clone(),
            }),
        ];
    }
    if let JavaType::Named { name, args } = java_ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        return vec![
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
            SignatureConstraint::RequiredDispatch(DispatchShape::BoxedDynTrait {
                trait_name: name.clone(),
            }),
        ];
    }
    vec![
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject),
        SignatureConstraint::RequiredView(ViewShape::ErasedObjectView),
    ]
}

fn declared_external_return_has_non_concrete_representation(
    java_ty: &JavaType,
    facts: &FactSet,
) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    matches!(
        effective_facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::DynTrait { .. } | HierarchyStrategy::ErasedObject)
    ) || facts.object_safe_trait_fact_for(java_ty) == Some(true)
}

fn declared_return_is_jdk_structural_interface(java_ty: &JavaType) -> bool {
    let name = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return false,
    };
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Iterable" | "Iterator" | "JavaIterator" | "ListIterator" | "Spliterator" | "Enumeration"
    )
}

fn constraints_require_nullable_storage(constraints: &[SignatureConstraint]) -> bool {
    constraints.iter().any(|constraint| {
        matches!(
            constraint,
            SignatureConstraint::RequiredStorageNullability(
                NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
            )
        )
    })
}

fn signature_constraints_prove_non_null_storage(constraints: &[SignatureConstraint]) -> bool {
    constraints.contains(&SignatureConstraint::RequiredStorageNullability(
        NullabilityState::DefinitelyNonNull,
    ))
}

fn nullable_storage_constraints(constraints: &[SignatureConstraint]) -> Vec<SignatureConstraint> {
    constraints
        .iter()
        .filter(|constraint| {
            matches!(
                constraint,
                SignatureConstraint::RequiredStorageNullability(
                    NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
                )
            )
        })
        .cloned()
        .collect()
}

fn retain_only_storage_nullability(
    constraints: &mut Vec<SignatureConstraint>,
    nullability: NullabilityState,
) {
    constraints.retain(|constraint| match constraint {
        SignatureConstraint::RequiredStorageNullability(candidate) => *candidate == nullability,
        _ => true,
    });
}

fn anonymous_declared_return_constraints(
    method: &MethodDecl,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    if matches!(method.return_type, JavaType::Void)
        || is_declared_java_object_type(&method.return_type)
    {
        return Vec::new();
    }
    if !method_returns_matching_anonymous_declared(method) {
        return Vec::new();
    }
    if let Some(trait_name) =
        anonymous_declared_return_boxed_dyn_trait_name(&method.return_type, facts)
    {
        return vec![
            SignatureConstraint::RequiredDispatch(DispatchShape::BoxedDynTrait { trait_name }),
            SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
        ];
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
        SignatureConstraint::RequiredOwnership(OwnershipShape::Owned),
    ]
}

fn anonymous_declared_return_boxed_dyn_trait_name(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Option<String> {
    let effective_facts = EffectiveFacts::new(facts);
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        effective_facts.object_representation_override_strategy_for(java_ty)
    {
        return Some(trait_name.clone());
    }
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    (args.is_empty() && facts.object_safe_trait_fact_for_name(name) == Some(true))
        .then(|| crate::java_naming::rust_name_for_java_named_type(name))
}

fn internal_exact_final_erased_facade_constraints(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let effective_facts = EffectiveFacts::new(facts);
    if matches!(java_ty, JavaType::Void) || is_declared_java_object_type(java_ty) {
        return Vec::new();
    }
    if !matches!(
        effective_facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) || !effective_facts.is_final_type(java_ty)
    {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ]
}

fn internal_returned_object_alias_borrow_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    if signature_member_api_boundary_policy(&method.modifiers).public_api {
        return Vec::new();
    }
    if matches!(method.return_type, JavaType::Primitive(_) | JavaType::Void) {
        return Vec::new();
    }
    if method_return_type_param_name(method).is_some() {
        return Vec::new();
    }
    let exactly_returned_param_names = method
        .body
        .as_ref()
        .map(|body| exact_returned_var_names_from_stmts(&body.statements))
        .unwrap_or_default();
    let Some(method_usage) = usage.source_method_usage(ty, method) else {
        return Vec::new();
    };
    let returned_borrow_safe_alias = method.params.iter().any(|param| {
        let Some(binding) = method_usage.binding_for_declaration_with_name_fallback(
            param.binding.as_ref(),
            &param.name,
            param.binding.is_none(),
        ) else {
            return false;
        };
        let returned_by_exact_binding = binding
            .uses
            .iter()
            .any(|usage| matches!(usage, BindingUse::Returned));
        let compatibility_name_returned = param.binding.is_none()
            && exactly_returned_param_names.contains(param.name.as_str())
            && !legacy_idless_method_assigns_to_binding(method, &param.name);
        (returned_by_exact_binding || compatibility_name_returned)
            && java_types_have_same_signature_boundary_identity(
                &binding.java_ty,
                &method.return_type,
            )
            && internal_object_binding_is_shared_borrow_safe(binding, facts)
    });
    returned_borrow_safe_alias
        .then(|| {
            vec![SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
            )]
        })
        .unwrap_or_default()
}

fn exact_returned_var_names_from_stmts(statements: &[Stmt]) -> HashSet<&str> {
    let mut names = HashSet::new();
    collect_exact_returned_var_names_from_stmts(statements, &mut names);
    names
}

fn collect_exact_returned_var_names_from_stmts<'a>(
    statements: &'a [Stmt],
    names: &mut HashSet<&'a str>,
) {
    for stmt in statements {
        match stmt {
            Stmt::Return(Some(Expr::Var(name))) => {
                names.insert(name.as_str());
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_exact_returned_var_names_from_stmts(&then_block.statements, names);
                if let Some(else_block) = else_block {
                    collect_exact_returned_var_names_from_stmts(&else_block.statements, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::EnhancedFor { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_exact_returned_var_names_from_stmts(&body.statements, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_exact_returned_var_names_from_stmts(&case.statements, names);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_exact_returned_var_names_from_stmts(resources, names);
                collect_exact_returned_var_names_from_stmts(&body.statements, names);
                for catch in catches {
                    collect_exact_returned_var_names_from_stmts(&catch.body.statements, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_exact_returned_var_names_from_stmts(&finally_block.statements, names);
                }
            }
            Stmt::VarDecl { .. }
            | Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

pub fn external_call_return_method_return_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    let Some(method_usage) = usage.source_method_usage(ty, method) else {
        return constraints;
    };
    for returned in &method_usage.returned_call_results {
        push_external_call_return_constraints(
            &mut constraints,
            returned_call_receiver_type_for_external_lookup(returned),
            &returned.name,
            Some(returned.arg_count),
            facts,
        );
        if returned_external_call_result_matches_declared_return(
            returned,
            &method.return_type,
            facts,
        ) {
            push_unique_signature_constraints(
                &mut constraints,
                &[SignatureConstraint::RequiredDispatch(
                    DispatchShape::Concrete,
                )],
            );
        }
        if returned_native_collection_get_result_matches_declared_return(
            returned,
            &method.return_type,
            facts,
        ) && !declared_return_has_non_concrete_representation(&method.return_type, facts)
        {
            push_unique_signature_constraints(
                &mut constraints,
                &[
                    SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
                    SignatureConstraint::RequiredView(ViewShape::Native),
                ],
            );
        }
    }
    constraints
}

pub fn method_return_boundary_policy(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> MethodReturnBoundaryPolicy {
    let mut kind = None;
    if signature_member_api_boundary_policy(&method.modifiers).public_api {
        kind =
            merge_signature_boundary_kind_policy(kind, Some(SignatureBoundaryKind::PublicApi)).kept;
    } else if let Some(method_usage) = usage.source_method_usage(ty, method) {
        let returns_matching_anonymous_declared =
            method_returns_matching_anonymous_declared(method);
        let returns_unknown_call_result =
            method_usage_returns_unknown_call_result(method_usage, &method.return_type, facts);
        kind = merge_signature_boundary_kind_policy(
            kind,
            (returns_unknown_call_result && !returns_matching_anonymous_declared)
                .then_some(SignatureBoundaryKind::UnknownApi),
        )
        .kept;
    }
    MethodReturnBoundaryPolicy {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
        kind,
    }
}

fn method_returns_matching_anonymous_declared(method: &MethodDecl) -> bool {
    if matches!(method.return_type, JavaType::Void)
        || is_declared_java_object_type(&method.return_type)
    {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    block_returns_matching_anonymous_declared(body, &method.return_type)
}

fn block_returns_matching_anonymous_declared(block: &Block, declared: &JavaType) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_returns_matching_anonymous_declared(stmt, declared))
}

fn stmt_returns_matching_anonymous_declared(stmt: &Stmt, declared: &JavaType) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::New {
            ty, body: Some(_), ..
        })) => anonymous_constructor_type_matches_declared_return(ty, declared),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_returns_matching_anonymous_declared(then_block, declared)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_returns_matching_anonymous_declared(block, declared))
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            case.statements
                .iter()
                .any(|stmt| stmt_returns_matching_anonymous_declared(stmt, declared))
        }),
        Stmt::Try {
            body,
            catches,
            finally_block,
            ..
        } => {
            block_returns_matching_anonymous_declared(body, declared)
                || catches
                    .iter()
                    .any(|catch| block_returns_matching_anonymous_declared(&catch.body, declared))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_returns_matching_anonymous_declared(block, declared))
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_returns_matching_anonymous_declared(block, declared)
        }
        _ => false,
    }
}

fn anonymous_constructor_type_matches_declared_return(
    anonymous_ty: &JavaType,
    declared: &JavaType,
) -> bool {
    let Some(anonymous_name) = java_type_name_for_signature_dispatch(anonymous_ty) else {
        return false;
    };
    let Some(declared_name) = java_type_name_for_signature_dispatch(declared) else {
        return false;
    };
    crate::facts::external_type_name_matches(anonymous_name, declared_name)
}

fn returned_external_call_result_matches_declared_return(
    returned: &ReturnedCallResultUse,
    declared_return: &JavaType,
    facts: &FactSet,
) -> bool {
    let receiver_type = returned_call_receiver_type_for_external_lookup(returned);
    let Some(effect) = facts.external_api_effect_for_receiver_with_arity(
        receiver_type.as_deref(),
        &returned.name,
        Some(returned.arg_count),
    ) else {
        return false;
    };
    let Some(return_type) = effect.return_type.as_ref() else {
        return false;
    };
    java_types_match_with_external_aliases(return_type, declared_return)
}

fn returned_native_collection_get_result_matches_declared_return(
    returned: &ReturnedCallResultUse,
    declared_return: &JavaType,
    _facts: &FactSet,
) -> bool {
    if returned.name != "get" || returned.arg_count != 1 {
        return false;
    }
    let Some(receiver_type) = &returned.receiver_type else {
        return false;
    };
    let value_type = crate::stdlib::java_map_type_args(receiver_type)
        .map(|(_, value_ty)| value_ty.clone())
        .or_else(|| crate::types::sequence_element_type(receiver_type));
    value_type.is_some_and(|value_type| {
        java_types_match_with_external_aliases(&value_type, declared_return)
    })
}

fn returned_call_receiver_type_for_external_lookup(
    returned: &ReturnedCallResultUse,
) -> Option<String> {
    returned
        .resolved
        .as_ref()
        .map(|resolved| resolved.declaring_type.clone())
        .or_else(|| {
            returned
                .receiver_type
                .as_ref()
                .and_then(|receiver| match receiver {
                    JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.clone()),
                    _ => None,
                })
        })
}

fn java_types_match_with_external_aliases(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (
            JavaType::Named {
                name: left,
                args: left_args,
            },
            JavaType::Named {
                name: right,
                args: right_args,
            },
        ) => {
            crate::facts::external_type_name_matches(left, right)
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| java_types_match_with_external_aliases(left, right))
        }
        _ => left == right,
    }
}

fn constrained_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    method_constraints: Option<&MethodConstraintSet>,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
    type_param_scope: TypeParamScope<'_>,
) -> Option<RustSlotShape> {
    let return_type = method_return_type_for_signature(method).unwrap_or(&method.return_type);
    let public_api_return = signature_member_api_boundary_policy(&method.modifiers).public_api;
    let merged_constraints = MergedSignatureConstraints::from_constraints(constraints);
    let borrowed_string_return_requested =
        constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
        )) && string_return_can_use_borrowed_view(method, method_constraints);
    if java_type_is_string_like_signature_type(return_type)
        && !constraints_require_nullable_storage(constraints)
        && !public_api_return
        && borrowed_string_return_requested
    {
        let constraints = constraints
            .iter()
            .filter(|constraint| {
                !matches!(
                    constraint,
                    SignatureConstraint::RequiredStorageNullability(
                        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
                    )
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut shape = apply_signature_constraints(
            RustSlotShape::conservative(JavaType::String),
            &constraints,
        );
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Str);
        apply_required_ownership(&mut shape, OwnershipShape::SharedBorrow);
        return Some(shape);
    }
    if java_type_is_string_like_signature_type(return_type)
        && !constraints_require_nullable_storage(constraints)
        && (public_api_return
            || (!borrowed_string_return_requested
                && merged_constraints.required_owner != Some(OwnershipShape::SharedBorrow)
                && merged_constraints.slot_view_req != Some(ViewShape::Str)))
    {
        let constraints = constraints
            .iter()
            .filter(|constraint| {
                !matches!(
                    constraint,
                    SignatureConstraint::RequiredStorageNullability(
                        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
                    )
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut shape = apply_signature_constraints(
            RustSlotShape::conservative(JavaType::String),
            &constraints,
        );
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Native);
        apply_required_ownership(&mut shape, OwnershipShape::Owned);
        return Some(shape);
    }
    if !java_type_is_erased_object_boundary(return_type)
        && !java_type_is_rust_crate_mapped_external(return_type, facts)
        && return_constraints_select_concrete_declared_value(ty, method, constraints, facts)
    {
        let constraints = constraints
            .iter()
            .filter(|constraint| {
                !matches!(
                    constraint,
                    SignatureConstraint::RequiredStorageNullability(
                        NullabilityState::MaybeNull | NullabilityState::DefinitelyNull
                    )
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut shape = apply_signature_constraints(
            RustSlotShape::conservative(return_type.clone()),
            &constraints,
        );
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Native);
        apply_required_ownership(&mut shape, OwnershipShape::Owned);
        return Some(shape);
    }
    let mut shape = solve_signature_shape_with_facts(
        representations,
        SignatureShapeRequest {
            java_ty: return_type.clone(),
            role: SignatureSlotRole::Return,
            constraints: constraints.to_vec(),
            type_params: type_param_scope.to_vec(),
        },
        facts,
    )?;
    if source_java_enum_storage_is_copy(representations, return_type) {
        apply_required_ownership(&mut shape, OwnershipShape::Owned);
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Native);
    } else if source_method_return_should_borrow_object(ty, method, facts) {
        apply_required_ownership(&mut shape, OwnershipShape::SharedBorrow);
    }
    Some(shape)
}

fn string_return_can_use_borrowed_view(
    method: &MethodDecl,
    method_constraints: Option<&MethodConstraintSet>,
) -> bool {
    let Some(body) = method.body.as_ref() else {
        return true;
    };
    let returned_names = exact_returned_var_names_from_stmts(&body.statements);
    if returned_names.is_empty() {
        return true;
    }
    for returned_name in returned_names {
        if !method
            .params
            .iter()
            .any(|param| param.name == returned_name)
        {
            continue;
        }
        let Some(method_constraints) = method_constraints else {
            continue;
        };
        let binding_constraints = binding_slot_requirements(method_constraints, returned_name)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let merged = MergedSignatureConstraints::from_constraints(&binding_constraints);
        if merged.required_owner != Some(OwnershipShape::SharedBorrow) {
            return false;
        }
    }
    true
}

fn method_return_type_for_signature(method: &MethodDecl) -> Option<&JavaType> {
    let declared_collection_name = raw_collection_type_name(&method.return_type)?;
    let body = method.body.as_ref()?;
    let local_types = method_declared_binding_types(method, body);
    let mut returned_types = Vec::new();
    if !collect_returned_var_types_from_stmts(&body.statements, &local_types, &mut returned_types) {
        return None;
    }
    let first = returned_types.first()?;
    if collection_type_name(first).as_deref() != Some(declared_collection_name.as_str()) {
        return None;
    }
    if first_type_args(first).is_empty() {
        return None;
    }
    returned_types.iter().all(|ty| ty == first).then_some(first)
}

fn raw_collection_type_name(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::Named { name, args } if args.is_empty() => {
            collection_type_name(java_ty).or_else(|| {
                Some(crate::stdlib::canonical_external_receiver_type_key(name))
                    .filter(|_| crate::stdlib::java_collection_kind_for_type(java_ty).is_some())
            })
        }
        JavaType::Unknown(name) => collection_type_name(java_ty).or_else(|| {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
                .filter(|_| crate::stdlib::java_collection_kind_for_type(java_ty).is_some())
        }),
        _ => None,
    }
}

fn collection_type_name(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::java_collection_kind_for_type(java_ty)?;
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        _ => None,
    }
}

fn first_type_args(java_ty: &JavaType) -> &[JavaType] {
    match java_ty {
        JavaType::Named { args, .. } => args,
        _ => &[],
    }
}

fn method_declared_binding_types<'a>(
    method: &'a MethodDecl,
    body: &'a Block,
) -> HashMap<&'a str, Option<&'a JavaType>> {
    let mut types = HashMap::new();
    for param in &method.params {
        insert_declared_binding_type(&mut types, &param.name, &param.ty);
    }
    collect_declared_binding_types_from_stmts(&body.statements, &mut types);
    types
}

fn insert_declared_binding_type<'a>(
    types: &mut HashMap<&'a str, Option<&'a JavaType>>,
    name: &'a str,
    java_ty: &'a JavaType,
) {
    types
        .entry(name)
        .and_modify(|existing| {
            if existing
                .as_ref()
                .is_some_and(|existing| *existing != java_ty)
            {
                *existing = None;
            }
        })
        .or_insert(Some(java_ty));
}

fn collect_declared_binding_types_from_stmts<'a>(
    statements: &'a [Stmt],
    types: &mut HashMap<&'a str, Option<&'a JavaType>>,
) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                insert_declared_binding_type(types, name, ty);
            }
            Stmt::For { init, body, .. } => {
                collect_declared_binding_types_from_stmts(init, types);
                collect_declared_binding_types_from_stmts(&body.statements, types);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                insert_declared_binding_type(types, var_name, var_type);
                collect_declared_binding_types_from_stmts(&body.statements, types);
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_declared_binding_types_from_stmts(&then_block.statements, types);
                if let Some(else_block) = else_block {
                    collect_declared_binding_types_from_stmts(&else_block.statements, types);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_declared_binding_types_from_stmts(&body.statements, types);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_declared_binding_types_from_stmts(&case.statements, types);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_declared_binding_types_from_stmts(resources, types);
                collect_declared_binding_types_from_stmts(&body.statements, types);
                for catch in catches {
                    collect_declared_binding_types_from_stmts(&catch.body.statements, types);
                }
                if let Some(finally_block) = finally_block {
                    collect_declared_binding_types_from_stmts(&finally_block.statements, types);
                }
            }
            Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_returned_var_types_from_stmts<'a>(
    statements: &'a [Stmt],
    local_types: &HashMap<&'a str, Option<&'a JavaType>>,
    out: &mut Vec<&'a JavaType>,
) -> bool {
    for stmt in statements {
        if !collect_returned_var_types_from_stmt(stmt, local_types, out) {
            return false;
        }
    }
    true
}

fn collect_returned_var_types_from_stmt<'a>(
    stmt: &'a Stmt,
    local_types: &HashMap<&'a str, Option<&'a JavaType>>,
    out: &mut Vec<&'a JavaType>,
) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::Var(name))) => {
            let Some(Some(java_ty)) = local_types.get(name.as_str()) else {
                return false;
            };
            out.push(*java_ty);
            true
        }
        Stmt::Return(Some(_)) | Stmt::Return(None) => false,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_returned_var_types_from_stmts(&then_block.statements, local_types, out)
                && else_block.as_ref().is_none_or(|else_block| {
                    collect_returned_var_types_from_stmts(&else_block.statements, local_types, out)
                })
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => {
            collect_returned_var_types_from_stmts(&body.statements, local_types, out)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .all(|case| collect_returned_var_types_from_stmts(&case.statements, local_types, out)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            collect_returned_var_types_from_stmts(resources, local_types, out)
                && collect_returned_var_types_from_stmts(&body.statements, local_types, out)
                && catches.iter().all(|catch| {
                    collect_returned_var_types_from_stmts(&catch.body.statements, local_types, out)
                })
                && finally_block.as_ref().is_none_or(|finally_block| {
                    collect_returned_var_types_from_stmts(
                        &finally_block.statements,
                        local_types,
                        out,
                    )
                })
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Throw(_)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => true,
    }
}

fn java_type_is_erased_object_boundary(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Object" | "java.lang.Object"
        ),
        JavaType::Wildcard(_) => true,
        _ => false,
    }
}

fn java_type_is_rust_crate_mapped_external(java_ty: &JavaType, facts: &FactSet) -> bool {
    let Some(name) = java_type_name(java_ty) else {
        return false;
    };
    facts
        .external_library_pack_for_type(name)
        .is_some_and(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
        || crate::external_packs::external_library_pack_for_type(name)
            .is_some_and(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
}

fn return_constraints_select_concrete_declared_value(
    ty: &TypeDecl,
    method: &MethodDecl,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> bool {
    let explicit_concrete_dispatch = constraints.contains(&SignatureConstraint::RequiredDispatch(
        DispatchShape::Concrete,
    ));
    let flow_proves_non_null_return = constraints.contains(
        &SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull),
    ) && typed_source_method_return_nullability_constraints(
        ty, method, facts,
    ) == vec![
        SignatureConstraint::RequiredStorageNullability(NullabilityState::DefinitelyNonNull),
    ];

    (explicit_concrete_dispatch || flow_proves_non_null_return)
        && constraints.contains(&SignatureConstraint::RequiredView(ViewShape::Native))
        && constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::Owned,
        ))
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
