use std::collections::{BTreeMap, HashMap, HashSet};

use crate::effective_facts::EffectiveFacts;
use crate::fact_saturation::{FiniteSaturationPolicy, SaturationStep};
use crate::facts::{
    CaptureOrigin, CaptureSource, DispatchRequirementKind, ExternalApiArgumentEffectFact,
    ExternalArgumentAliasPolicy, FactSet, HierarchyStrategy, NullabilityState,
    OwnedRecoveryCapability, ReceiverMutabilityFact,
};
use crate::graph::strongly_connected_components;
use crate::java_ir::{
    BinaryOp, Block, ConstructorDecl, Expr, FieldDecl, JavaType, Literal, MethodDecl, Modifier,
    Param, Program, Stmt, TypeDecl, UnaryOp,
};
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
    ReturnedCallResultUse, UsageConstructorKey, UsageFacts, UsageMethodKey,
};

pub type SolvedSlotShape = RustSlotShape;

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
pub struct SignatureBoundaryRolePolicy {
    pub role: SignatureBoundaryRole,
    pub constraint_owner: &'static str,
    pub participates_in_dependency_graph: bool,
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

pub fn signature_boundary_role_policies() -> Vec<SignatureBoundaryRolePolicy> {
    use SignatureBoundaryRole::*;
    vec![
        SignatureBoundaryRolePolicy {
            role: MethodParam,
            constraint_owner: "method_param_shape_constraints",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: MethodReturn,
            constraint_owner: "method_return_shape_constraints",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: ConstructorParam,
            constraint_owner: "constructor_param_shape_constraints",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: MethodLocal,
            constraint_owner: "local_shape_constraints_with_usage",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: ConstructorLocal,
            constraint_owner: "local_shape_constraints_with_usage",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: Field,
            constraint_owner: "field_shape_constraints_with_usage",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: MethodReceiver,
            constraint_owner: "receiver_constraints_with_facts_or_usage",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: CapturedValue,
            constraint_owner: "apply_capture_constraints",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: CallArgument,
            constraint_owner: "add_call_argument_dependency_edges",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: CallResult,
            constraint_owner: "external_call_return_binding_constraints",
            participates_in_dependency_graph: true,
        },
        SignatureBoundaryRolePolicy {
            role: ConstructorInvocation,
            constraint_owner: "constructor_param_shape_constraints",
            participates_in_dependency_graph: true,
        },
    ]
}

pub fn signature_boundary_role_policy(role: SignatureBoundaryRole) -> SignatureBoundaryRolePolicy {
    signature_boundary_role_policies()
        .into_iter()
        .find(|policy| policy.role == role)
        .expect("all signature boundary roles must have a policy")
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
    pub changed: bool,
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
    pub dependency_sccs: SignatureDependencySccAnalysis,
    pub pass_plan: SignatureSolverPassPlan,
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
    pub stats: SignatureSolverPassStats,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureSolverPassStats {
    pub planned_components: usize,
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
    let constraints = collect_signature_constraints(program, usage, facts);
    let result = solve_signature_constraints_to_fixpoint_result(constraints, usage);
    build_signature_plan_from_constraints(
        program,
        support_program,
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
    let stats = SignatureSolverPassStats {
        planned_components: order.len(),
        cyclic_components: analysis.stats.cyclic_component_count,
        max_component_size: analysis.stats.max_component_size,
    };

    SignatureSolverPassPlan {
        component_order: order,
        cyclic_components,
        stats,
    }
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
            add_signature_dependency_node(
                graph,
                SignatureDependencyNode::MethodLocal {
                    method: method.clone(),
                    name: local.name.clone(),
                },
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
            add_signature_dependency_node(
                graph,
                SignatureDependencyNode::ConstructorLocal {
                    constructor: constructor.clone(),
                    name: local.name.clone(),
                },
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
    for binding in method_usage.bindings.values() {
        let Some(binding_node) = method_binding_dependency_node(constraints, method, &binding.name)
        else {
            continue;
        };
        for use_site in &binding.uses {
            match use_site {
                BindingUse::AssignedToLocal { local_name } => {
                    let local = SignatureDependencyNode::MethodLocal {
                        method: method.clone(),
                        name: local_name.clone(),
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
                BindingUse::ReceiverForCall { name, boundary } => {
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
                BindingUse::ContentRead | BindingUse::Mutated => {}
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
    for binding in constructor_usage.bindings.values() {
        let Some(binding_node) =
            constructor_binding_dependency_node(constraints, constructor, &binding.name)
        else {
            continue;
        };
        for use_site in &binding.uses {
            match use_site {
                BindingUse::AssignedToLocal { local_name } => {
                    let local = SignatureDependencyNode::ConstructorLocal {
                        constructor: constructor.clone(),
                        name: local_name.clone(),
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
    if let Some(index) = method_constraints
        .param_names
        .iter()
        .position(|name| name == binding_name)
    {
        return Some(SignatureDependencyNode::MethodParam {
            method: method.clone(),
            name: binding_name.to_string(),
            index,
        });
    }
    if method_constraints
        .locals
        .iter()
        .any(|local| local.name == binding_name)
    {
        return Some(SignatureDependencyNode::MethodLocal {
            method: method.clone(),
            name: binding_name.to_string(),
        });
    }
    None
}

fn constructor_binding_dependency_node(
    constraints: &SignatureConstraintState,
    constructor: &ConstructorKey,
    binding_name: &str,
) -> Option<SignatureDependencyNode> {
    let constructor_constraints = constraints.constructors.get(constructor)?;
    if let Some(index) = constructor_constraints
        .param_names
        .iter()
        .position(|name| name == binding_name)
    {
        return Some(SignatureDependencyNode::ConstructorParam {
            constructor: constructor.clone(),
            name: binding_name.to_string(),
            index,
        });
    }
    if constructor_constraints
        .locals
        .iter()
        .any(|local| local.name == binding_name)
    {
        return Some(SignatureDependencyNode::ConstructorLocal {
            constructor: constructor.clone(),
            name: binding_name.to_string(),
        });
    }
    None
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
    let mut changed = false;
    for component_index in &pass_plan.component_order {
        let mut component_changed = true;
        while component_changed {
            let before = constraints.clone();
            apply_signature_propagation_sweep(constraints, usage);
            keep_signature_component_changes_only(constraints, &before, analysis, *component_index);
            component_changed = constraints != &before;
            changed |= component_changed;
            if !pass_plan.cyclic_components.contains(component_index) {
                break;
            }
        }
    }
    changed
}

fn keep_signature_component_changes_only(
    constraints: &mut SignatureConstraintState,
    before: &SignatureConstraintState,
    analysis: &SignatureDependencySccAnalysis,
    component_index: usize,
) {
    for (method_key, method_constraints) in &mut constraints.methods {
        let Some(before_method) = before.methods.get(method_key) else {
            continue;
        };
        for (param_index, param_constraints) in method_constraints.params.iter_mut().enumerate() {
            let Some(param_name) = method_constraints.param_names.get(param_index) else {
                continue;
            };
            let node = SignatureDependencyNode::MethodParam {
                method: method_key.clone(),
                name: param_name.clone(),
                index: param_index,
            };
            if !signature_node_in_component(analysis, &node, component_index)
                && let Some(before_constraints) = before_method.params.get(param_index)
            {
                *param_constraints = before_constraints.clone();
            }
        }

        let return_node = SignatureDependencyNode::MethodReturn {
            method: method_key.clone(),
        };
        if !signature_node_in_component(analysis, &return_node, component_index) {
            method_constraints.return_constraints = before_method.return_constraints.clone();
        }

        for local_constraints in &mut method_constraints.locals {
            let node = SignatureDependencyNode::MethodLocal {
                method: method_key.clone(),
                name: local_constraints.name.clone(),
            };
            if !signature_node_in_component(analysis, &node, component_index)
                && let Some(before_local) = before_method
                    .locals
                    .iter()
                    .find(|local| local.name == local_constraints.name)
            {
                local_constraints.constraints = before_local.constraints.clone();
            }
        }
    }

    for (constructor_key, constructor_constraints) in &mut constraints.constructors {
        let Some(before_constructor) = before.constructors.get(constructor_key) else {
            continue;
        };
        for (param_index, param_constraints) in
            constructor_constraints.params.iter_mut().enumerate()
        {
            let Some(param_name) = constructor_constraints.param_names.get(param_index) else {
                continue;
            };
            let node = SignatureDependencyNode::ConstructorParam {
                constructor: constructor_key.clone(),
                name: param_name.clone(),
                index: param_index,
            };
            if !signature_node_in_component(analysis, &node, component_index)
                && let Some(before_constraints) = before_constructor.params.get(param_index)
            {
                *param_constraints = before_constraints.clone();
            }
        }

        for local_constraints in &mut constructor_constraints.locals {
            let node = SignatureDependencyNode::ConstructorLocal {
                constructor: constructor_key.clone(),
                name: local_constraints.name.clone(),
            };
            if !signature_node_in_component(analysis, &node, component_index)
                && let Some(before_local) = before_constructor
                    .locals
                    .iter()
                    .find(|local| local.name == local_constraints.name)
            {
                local_constraints.constraints = before_local.constraints.clone();
            }
        }
    }

    for (field_key, field_constraints) in &mut constraints.fields {
        let node = SignatureDependencyNode::Field {
            field: field_key.clone(),
        };
        if !signature_node_in_component(analysis, &node, component_index)
            && let Some(before_constraints) = before.fields.get(field_key)
        {
            *field_constraints = before_constraints.clone();
        }
    }
}

fn signature_node_in_component(
    analysis: &SignatureDependencySccAnalysis,
    node: &SignatureDependencyNode,
    component_index: usize,
) -> bool {
    analysis
        .component_by_node
        .get(node)
        .is_some_and(|actual| *actual == component_index)
}

fn apply_signature_propagation_sweep(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
) -> bool {
    propagate_call_site_constraints(constraints, usage)
        | propagate_constructor_call_site_constraints(constraints, usage)
        | propagate_constructor_delegation_constraints(constraints, usage)
        | propagate_call_argument_constraints_to_callees(constraints, usage)
        | propagate_constructor_call_argument_constraints_to_callees(constraints, usage)
        | propagate_constructor_delegation_argument_constraints_to_callees(constraints, usage)
        | propagate_null_call_argument_constraints_to_callees(constraints, usage)
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
        | propagate_method_field_return_constraints(constraints, usage)
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

fn propagate_null_call_argument_constraints_to_callees(
    constraints: &mut SignatureConstraintState,
    usage: &UsageFacts,
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
            for arg_index in &call_site.null_arg_indices {
                method_pending.push((callee_key.clone(), *arg_index));
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
            for arg_index in &call_site.null_arg_indices {
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
            for arg_index in &call_site.null_arg_indices {
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
            changed |= add_method_param_constraint(
                constraints,
                &callee_key,
                &param_name,
                nullability.clone(),
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
            changed |= add_constructor_param_constraint(
                constraints,
                &callee_key,
                &param_name,
                nullability.clone(),
            );
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
                let BindingUse::AssignedFromCall { name, boundary, .. } = use_site else {
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
                let BindingUse::AssignedFromCall { name, boundary, .. } = use_site else {
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
                ..
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
                ..
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
                    field_writer_shape_requirements(constraints, &method_key.type_name, field_name)
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
                for constraint in field_writer_shape_requirements(
                    constraints,
                    &constructor_key.type_name,
                    field_name,
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

    pub fn method_signature_by_base_name_and_arity(
        &self,
        type_name: &str,
        base_name: &str,
        arity: usize,
    ) -> Option<(String, &MethodSignaturePlan)> {
        let base_name = crate::parser::to_snake_case(base_name);
        let mut matches = self
            .methods
            .iter()
            .filter(|(key, signature)| {
                key.type_name == type_name
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
        let field_constraints = constraints
            .fields
            .get(&field_key)
            .cloned()
            .unwrap_or_else(|| field_shape_constraints_with_facts(ty, &inherited.field, facts));
        plan.fields.insert(
            field_key,
            constrained_field_shape(
                &inherited.field.ty,
                representations,
                &field_constraints,
                facts,
            ),
        );
    }
}

pub fn method_param_shape_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let method_usage = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    });
    let binding = method_usage.and_then(|method_usage| method_usage.bindings.get(&param.name));
    let mut boundary_kind = method_param_boundary_policy(ty, method, param, usage).kind;
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
    if boundary_kind == Some(SignatureBoundaryKind::UnknownApi)
        && binding.is_some_and(|binding| {
            unknown_boundary_is_fully_modeled_by_external_argument_effects(binding, facts)
        })
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
    if let Some(binding) = binding {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(
                binding,
                facts,
                true,
                boundary_kind != Some(SignatureBoundaryKind::PublicApi)
                    || public_readonly_receiver_read_product,
            ),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_argument_binding_constraints(binding, facts),
        );
    }
    if let Some(body) = &method.body {
        push_unique_signature_constraints(
            &mut constraints,
            &source_binding_nullability_constraints(body, &param.name),
        );
    }
    push_unique_signature_constraints(
        &mut constraints,
        &internal_exact_final_erased_facade_constraints(&param.ty, facts),
    );
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
        ) || (fact_policy.object_representation_facts
            && !exact_final_erased_facade_boundary_should_use_native(java_ty, facts)),
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
    let mut constraints = vec![
        SignatureConstraint::RequiredOwnership(owner_requirement),
        SignatureConstraint::RequiredView(ViewShape::Native),
    ];
    push_unique_signature_constraints(
        &mut constraints,
        &exact_final_erased_facade_native_boundary_constraints(java_ty, facts),
    );
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
    push_unique_signature_constraints(
        &mut constraints,
        &signature_erased_object_boundary_constraints_with_effective_facts(java_ty, facts),
    );
    SignatureSlotBaselinePolicy {
        java_ty: java_ty.clone(),
        boundary_kind,
        capability_constraints,
        boundary_constraints,
        constraints,
    }
}

fn signature_erased_object_boundary_constraints_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Vec<SignatureConstraint> {
    if !is_declared_java_object_type(java_ty) || facts.has_object_representation_facts(java_ty) {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject),
        SignatureConstraint::RequiredView(ViewShape::ErasedObjectView),
    ]
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
    let ctor_usage = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name: constructor_rust_name(ctor),
    });
    let binding = ctor_usage.and_then(|ctor_usage| ctor_usage.bindings.get(&param.name));
    let mut boundary_kind = constructor_param_boundary_policy(ty, ctor, param, usage).kind;
    if boundary_kind == Some(SignatureBoundaryKind::PublicApi)
        && binding.is_some_and(|binding| {
            binding_is_fully_modeled_by_readonly_external_receivers(binding, facts)
        })
    {
        boundary_kind = None;
    }
    if boundary_kind == Some(SignatureBoundaryKind::UnknownApi)
        && binding.is_some_and(|binding| {
            unknown_boundary_is_fully_modeled_by_external_argument_effects(binding, facts)
        })
    {
        boundary_kind = None;
    }
    let mut constraints =
        signature_slot_baseline_policy(&param.ty, boundary_kind, facts).constraints;
    if let Some(binding) = binding {
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts, true, true),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_argument_binding_constraints(binding, facts),
        );
    }
    if let Some(body) = &ctor.body {
        push_unique_signature_constraints(
            &mut constraints,
            &source_binding_nullability_constraints(body, &param.name),
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
        }
    }
}

pub fn merge_signature_flag_requirement_policy(current: bool, next: bool) -> SignatureFlagMerge {
    let kept = current || next;
    SignatureFlagMerge {
        kept,
        changed: kept != current,
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
    let merge = join_ranked_lattice(current, next, ownership_lattice_rank);
    SignatureRequirementMerge {
        kept: merge.kept,
        rejected: merge.rejected,
    }
}

pub fn signature_ownership_requirement_priority_policy(
    requirement: OwnershipShape,
) -> SignatureOwnershipRequirementPriorityPolicy {
    let rank = ownership_lattice_rank(&requirement);
    SignatureOwnershipRequirementPriorityPolicy { requirement, rank }
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

pub fn signature_view_requirement_priority_policy(
    requirement: ViewShape,
) -> SignatureViewRequirementPriorityPolicy {
    let rank = view_lattice_rank(&requirement);
    SignatureViewRequirementPriorityPolicy { requirement, rank }
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

pub fn signature_dispatch_requirement_priority_policy(
    requirement: DispatchShape,
) -> SignatureDispatchRequirementPriorityPolicy {
    let rank = dispatch_shape_lattice_rank(&requirement);
    SignatureDispatchRequirementPriorityPolicy { requirement, rank }
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
    push_unique_signature_constraints(
        &mut constraints,
        &external_call_return_field_constraints(ty, field, usage, facts),
    );
    if let Some(kind) = field_boundary_policy(ty, field, usage).kind {
        push_unique_signature_constraints(
            &mut constraints,
            &signature_boundary_policy(kind, &field.ty, facts).constraints,
        );
    }
    constraints
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
            push_external_call_return_constraints(
                &mut constraints,
                receiver_type.as_ref(),
                name,
                facts,
            );
        }
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

pub fn source_binding_nullability_constraints(
    block: &Block,
    name: &str,
) -> Vec<SignatureConstraint> {
    collect_binding_nullability_constraint_map(block)
        .remove(name)
        .unwrap_or_default()
}

pub fn source_method_return_nullability_constraints(
    method: &MethodDecl,
) -> Vec<SignatureConstraint> {
    if matches!(method.return_type, JavaType::Void | JavaType::Primitive(_)) {
        return Vec::new();
    }
    let Some(body) = &method.body else {
        return Vec::new();
    };
    let binding_states = collect_binding_nullability_state_map(body);
    let mut states = Vec::new();
    collect_source_return_nullability_states_from_stmts(
        &body.statements,
        &binding_states,
        &mut states,
    );
    let state = merge_source_nullability_states(&states);
    if matches!(state, NullabilityState::Unknown) {
        Vec::new()
    } else {
        vec![SignatureConstraint::RequiredStorageNullability(state)]
    }
}

fn collect_binding_nullability_constraint_map(
    block: &Block,
) -> HashMap<String, Vec<SignatureConstraint>> {
    collect_binding_nullability_state_map(block)
        .into_iter()
        .map(|(name, state)| {
            (
                name,
                vec![SignatureConstraint::RequiredStorageNullability(state)],
            )
        })
        .collect()
}

fn collect_binding_nullability_state_map(block: &Block) -> HashMap<String, NullabilityState> {
    let mut states = HashMap::<String, NullabilityState>::new();
    collect_binding_nullability_states_from_stmts(&block.statements, &mut states);
    states
}

fn collect_source_return_nullability_states_from_stmts(
    statements: &[Stmt],
    binding_states: &HashMap<String, NullabilityState>,
    states: &mut Vec<NullabilityState>,
) {
    let mut refinements = HashSet::new();
    collect_source_return_nullability_states_from_stmts_with_refinements(
        statements,
        binding_states,
        &mut refinements,
        states,
    );
}

fn collect_source_return_nullability_states_from_stmts_with_refinements(
    statements: &[Stmt],
    binding_states: &HashMap<String, NullabilityState>,
    refinements: &mut HashSet<String>,
    states: &mut Vec<NullabilityState>,
) {
    for stmt in statements {
        collect_source_return_nullability_states_from_stmt(
            stmt,
            binding_states,
            refinements,
            states,
        );
        update_post_stmt_nullability_refinements(stmt, refinements);
    }
}

fn collect_source_return_nullability_states_from_stmt(
    stmt: &Stmt,
    binding_states: &HashMap<String, NullabilityState>,
    refinements: &HashSet<String>,
    states: &mut Vec<NullabilityState>,
) {
    match stmt {
        Stmt::Return(Some(expr)) => {
            states.push(source_return_expr_nullability_state(
                expr,
                binding_states,
                refinements,
            ));
        }
        Stmt::Return(None) => states.push(NullabilityState::DefinitelyNull),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &then_block.statements,
                binding_states,
                &mut refinements.clone(),
                states,
            );
            if let Some(else_block) = else_block {
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &else_block.statements,
                    binding_states,
                    &mut refinements.clone(),
                    states,
                );
            }
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::EnhancedFor { body, .. } => {
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                binding_states,
                &mut refinements.clone(),
                states,
            );
        }
        Stmt::Block(body) => collect_source_return_nullability_states_from_stmts_with_refinements(
            &body.statements,
            binding_states,
            &mut refinements.clone(),
            states,
        ),
        Stmt::Switch { cases, .. } => {
            for case in cases {
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &case.statements,
                    binding_states,
                    &mut refinements.clone(),
                    states,
                );
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
                binding_states,
                &mut refinements.clone(),
                states,
            );
            collect_source_return_nullability_states_from_stmts_with_refinements(
                &body.statements,
                binding_states,
                &mut refinements.clone(),
                states,
            );
            for catch in catches {
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &catch.body.statements,
                    binding_states,
                    &mut refinements.clone(),
                    states,
                );
            }
            if let Some(finally_block) = finally_block {
                collect_source_return_nullability_states_from_stmts_with_refinements(
                    &finally_block.statements,
                    binding_states,
                    &mut refinements.clone(),
                    states,
                );
            }
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
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
    binding_states: &HashMap<String, NullabilityState>,
    refinements: &HashSet<String>,
) -> NullabilityState {
    match expr {
        Expr::Var(name) if refinements.contains(name) => NullabilityState::DefinitelyNonNull,
        Expr::Var(name) => binding_states
            .get(name)
            .copied()
            .unwrap_or(NullabilityState::Unknown),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => merge_source_nullability_states(&[
            source_return_expr_nullability_state(then_expr, binding_states, refinements),
            source_return_expr_nullability_state(else_expr, binding_states, refinements),
        ]),
        _ => source_expr_nullability_state(expr).unwrap_or(NullabilityState::Unknown),
    }
}

fn update_post_stmt_nullability_refinements(stmt: &Stmt, refinements: &mut HashSet<String>) {
    let mut mutated = HashSet::new();
    collect_mutated_local_names_from_stmt(stmt, &mut mutated);
    for name in mutated {
        refinements.remove(&name);
    }
    if let Stmt::While { condition, body } = stmt {
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => {
            collect_binding_nullability_states_from_stmts(&block.statements, states);
        }
        Stmt::LocalType(ty) => {
            for member in &ty.members {
                match member {
                    crate::java_ir::Member::Method(method) => {
                        if let Some(body) = &method.body {
                            collect_binding_nullability_states_from_stmts(&body.statements, states);
                        }
                    }
                    crate::java_ir::Member::Constructor(constructor) => {
                        if let Some(body) = &constructor.body {
                            collect_binding_nullability_states_from_stmts(&body.statements, states);
                        }
                    }
                    _ => {}
                }
            }
        }
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
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_binding_nullability_states_from_expr(receiver, states);
            }
            for arg in args {
                collect_binding_nullability_states_from_expr(arg, states);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_binding_nullability_states_from_expr(arg, states);
            }
            if let Some(body) = body {
                for member in body {
                    if let crate::java_ir::Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        collect_binding_nullability_states_from_stmts(&body.statements, states);
                    }
                }
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
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_binding_nullability_states_from_expr(expr, states);
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_binding_nullability_states_from_stmts(&block.statements, states);
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
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

fn initializer_proves_declared_non_null(
    expr: &Expr,
    declared_ty: &JavaType,
    facts: &FactSet,
) -> bool {
    matches!(
        source_expr_nullability_state(expr),
        Some(NullabilityState::DefinitelyNonNull)
    ) || external_initializer_proves_declared_non_null(expr, declared_ty, facts)
}

fn external_initializer_proves_declared_non_null(
    expr: &Expr,
    declared_ty: &JavaType,
    facts: &FactSet,
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
    let Some(receiver_key) = static_initializer_receiver_key(receiver) else {
        return false;
    };
    facts
        .external_api_effect_for_receiver_with_arity(Some(&receiver_key), name, Some(args.len()))
        .is_some_and(|effect| {
            effect.return_type.as_ref() == Some(declared_ty)
                && effect.return_nullability == NullabilityState::DefinitelyNonNull
        })
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

fn collect_local_constraint_sets(
    block: &crate::java_ir::Block,
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
) -> Vec<LocalConstraintSet> {
    let mut locals = Vec::new();
    let mutated = collect_mutated_local_names(block);
    let source_nullability = collect_binding_nullability_constraint_map(block);
    let duplicate_names = collect_duplicate_declared_local_names(block);
    collect_local_constraint_sets_from_stmts(
        &block.statements,
        &mutated,
        usage,
        facts,
        captured,
        &source_nullability,
        &duplicate_names,
        &mut locals,
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
            Stmt::While { body, .. } | Stmt::Block(body) => {
                collect_declared_local_name_counts(&body.statements, counts);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_declared_local_name_counts(&case.statements, counts);
                }
            }
            Stmt::Try {
                resources, body, ..
            } => {
                collect_declared_local_name_counts(resources, counts);
                collect_declared_local_name_counts(&body.statements, counts);
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
    usage: Option<&MethodUsageFacts>,
    facts: &FactSet,
    captured: &HashSet<String>,
    source_nullability: &HashMap<String, Vec<SignatureConstraint>>,
    duplicate_names: &HashSet<String>,
    locals: &mut Vec<LocalConstraintSet>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl {
                ty,
                name,
                initializer,
            } => {
                let usage = if duplicate_names.contains(name) {
                    None
                } else {
                    usage
                };
                let mut constraints =
                    local_shape_constraints_with_usage(name, ty, mutated, usage, facts);
                if !(duplicate_names.contains(name)
                    && initializer.as_ref().is_some_and(|initializer| {
                        initializer_proves_declared_non_null(initializer, ty, facts)
                    }))
                    && let Some(source_constraints) = source_nullability.get(name)
                {
                    push_unique_signature_constraints(&mut constraints, source_constraints);
                }
                apply_capture_constraints(&mut constraints, captured, name, ty, facts);
                locals.push(LocalConstraintSet {
                    name: name.clone(),
                    java_ty: ty.clone(),
                    constraints,
                });
            }
            Stmt::For { init, body, .. } => {
                collect_local_constraint_sets_from_stmts(
                    init,
                    mutated,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    duplicate_names,
                    locals,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    duplicate_names,
                    locals,
                );
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                let usage = if duplicate_names.contains(var_name) {
                    None
                } else {
                    usage
                };
                let mut constraints =
                    local_shape_constraints_with_usage(var_name, var_type, mutated, usage, facts);
                if !duplicate_names.contains(var_name)
                    && let Some(source_constraints) = source_nullability.get(var_name)
                {
                    push_unique_signature_constraints(&mut constraints, source_constraints);
                }
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
                    source_nullability,
                    duplicate_names,
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
                    source_nullability,
                    duplicate_names,
                    locals,
                );
                if let Some(else_block) = else_block {
                    collect_local_constraint_sets_from_stmts(
                        &else_block.statements,
                        mutated,
                        usage,
                        facts,
                        captured,
                        source_nullability,
                        duplicate_names,
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
                    source_nullability,
                    duplicate_names,
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
                        source_nullability,
                        duplicate_names,
                        locals,
                    );
                }
            }
            Stmt::Try {
                resources, body, ..
            } => {
                collect_local_constraint_sets_from_stmts(
                    resources,
                    mutated,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    duplicate_names,
                    locals,
                );
                collect_local_constraint_sets_from_stmts(
                    &body.statements,
                    mutated,
                    usage,
                    facts,
                    captured,
                    source_nullability,
                    duplicate_names,
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
                    local_constraints_for_declared_type(constraints, name, ty),
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
                    local_constraints_for_declared_type(constraints, var_name, var_type),
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

fn local_constraints_for_declared_type<'a>(
    constraints: &'a [LocalConstraintSet],
    name: &str,
    declared_ty: &JavaType,
) -> &'a [SignatureConstraint] {
    constraints
        .iter()
        .find(|constraints| constraints.name == name && constraints.java_ty == *declared_ty)
        .map(|constraints| constraints.constraints.as_slice())
        .unwrap_or_else(|| local_constraints_for(constraints, name))
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
            &constructor_assigned_local_storage_constraints(binding),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_receiver_binding_constraints(binding, facts, false, false),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &native_collection_receiver_binding_constraints(binding),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &external_call_return_binding_constraints(binding, facts),
        );
        push_unique_signature_constraints(
            &mut constraints,
            &native_map_get_final_facade_value_binding_constraints(
                binding,
                java_ty,
                effective_facts,
            ),
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

fn native_map_get_final_facade_value_binding_constraints(
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
    if !matches!(
        facts.object_representation_override_strategy_for(java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) || !facts.is_final_type(java_ty)
    {
        return Vec::new();
    }
    vec![
        SignatureConstraint::RequiredDispatch(DispatchShape::Concrete),
        SignatureConstraint::RequiredView(ViewShape::Native),
        SignatureConstraint::RequiredOwnership(OwnershipShape::SharedBorrow),
    ]
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
    allow_mutable_receiver_borrow_shape: bool,
    allow_readonly_receiver_borrow_shape: bool,
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
        } else if allow_readonly_receiver_borrow_shape
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
        }
    }
    constraints
}

fn binding_is_fully_modeled_by_readonly_external_receivers(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> bool {
    let mut saw_receiver = false;
    let receiver_type = external_api_receiver_type_key(&binding.java_ty);
    for usage in &binding.uses {
        match usage {
            BindingUse::ReceiverForCall {
                name,
                boundary: CallArgumentBoundary::Unknown,
            } => {
                saw_receiver = true;
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

pub fn external_argument_binding_constraints(
    binding: &BindingUseFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    for usage in &binding.uses {
        let BindingUse::PassedToCall {
            name,
            arg_index,
            boundary: CallArgumentBoundary::Unknown,
        } = usage
        else {
            continue;
        };
        if let Some(effect) =
            facts.external_api_argument_effect_for_receiver(None, name, *arg_index)
        {
            push_unique_signature_constraints(
                &mut constraints,
                &external_argument_effect_constraints(effect),
            );
        }
    }
    constraints
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
                arg_index,
                boundary: CallArgumentBoundary::Unknown,
            } => {
                saw_unknown = true;
                if facts
                    .external_api_argument_effect_for_receiver(None, name, *arg_index)
                    .is_none()
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

pub fn external_argument_effect_constraints(
    effect: &ExternalApiArgumentEffectFact,
) -> Vec<SignatureConstraint> {
    let java_ty = effect
        .parameter_type
        .clone()
        .unwrap_or_else(|| JavaType::Unknown("Object".to_string()));
    let mut constraints = Vec::new();

    if effect.parameter_nullability != NullabilityState::Unknown {
        constraints.push(SignatureConstraint::RequiredStorageNullability(
            effect.parameter_nullability,
        ));
    }

    match effect.alias_policy {
        ExternalArgumentAliasPolicy::Borrowed if java_ty == JavaType::String => {
            constraints.push(SignatureConstraint::RequiredOwnership(
                OwnershipShape::SharedBorrow,
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
            boundary: CallArgumentBoundary::Unknown,
        } = usage
        else {
            continue;
        };
        push_external_call_return_constraints(
            &mut constraints,
            receiver_type.as_ref(),
            name,
            facts,
        );
    }
    constraints
}

fn push_external_call_return_constraints(
    constraints: &mut Vec<SignatureConstraint>,
    receiver_type: Option<&JavaType>,
    name: &str,
    facts: &FactSet,
) {
    let receiver_type = receiver_type.and_then(external_api_receiver_type_key);
    if let Some(effect) = facts.external_api_effect_for_receiver(receiver_type.as_deref(), name)
        && effect.return_nullability != NullabilityState::Unknown
    {
        push_unique_signature_constraints(
            constraints,
            &[SignatureConstraint::RequiredStorageNullability(
                effect.return_nullability,
            )],
        );
    }
}

fn external_api_receiver_type_key(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::Named { name, .. } => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        JavaType::String => Some("String".to_string()),
        JavaType::Array(_) => Some("Array".to_string()),
        JavaType::Unknown(name) => Some(crate::stdlib::canonical_external_receiver_type_key(name)),
        JavaType::Void | JavaType::Primitive(_) | JavaType::Wildcard(_) => None,
    }
}

fn java_types_have_same_signature_boundary_identity(left: &JavaType, right: &JavaType) -> bool {
    if left == right {
        return true;
    }
    if matches!(left, JavaType::Primitive(_) | JavaType::Void)
        || matches!(right, JavaType::Primitive(_) | JavaType::Void)
    {
        return left == right;
    }
    let Some(left_key) = external_api_receiver_type_key(left) else {
        return false;
    };
    let Some(right_key) = external_api_receiver_type_key(right) else {
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
    name: &str,
    java_ty: &JavaType,
    representations: &RepresentationPlan,
    constraints: &[SignatureConstraint],
    facts: &FactSet,
) -> LocalPlan {
    let shape = if local_constraints_select_concrete_borrowed_value(constraints) {
        let mut shape =
            apply_signature_constraints(RustSlotShape::conservative(java_ty.clone()), constraints);
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
            },
            facts,
        )
        .expect("local signature shape is always present")
    };
    LocalPlan {
        name: name.to_string(),
        java_ty: java_ty.clone(),
        shape,
        binding_mutable: signature_constraints_require_mutable_binding(constraints),
    }
}

fn local_constraints_select_concrete_borrowed_value(constraints: &[SignatureConstraint]) -> bool {
    constraints.contains(&SignatureConstraint::RequiredDispatch(
        DispatchShape::Concrete,
    )) && constraints.contains(&SignatureConstraint::RequiredView(ViewShape::Native))
        && constraints.contains(&SignatureConstraint::RequiredOwnership(
            OwnershipShape::SharedBorrow,
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
            if is_arrays_sort_call(receiver.as_deref(), name, args) {
                collect_assignment_target_name(&args[0], mutated);
            }
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

fn is_arrays_sort_call(receiver: Option<&Expr>, name: &str, args: &[Expr]) -> bool {
    let Some(Expr::Var(receiver_name)) = receiver else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Arrays"
        && name == "sort"
        && args.len() == 1
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
    let boundary_kind = method_return_boundary_policy(ty, method, usage, facts).kind;
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
    push_unique_signature_constraints(
        &mut constraints,
        &source_method_return_nullability_constraints(method),
    );
    push_unique_signature_constraints(
        &mut constraints,
        &external_call_return_method_return_constraints(ty, method, usage, facts),
    );
    if boundary_kind.is_none() {
        push_unique_signature_constraints(
            &mut constraints,
            &internal_exact_final_erased_facade_constraints(&method.return_type, facts),
        );
        constraints.extend(
            borrowed_return_content_policy_from_param_with_facts(ty, method, usage, facts)
                .map(signature_constraints_for_borrowed_content_policy)
                .unwrap_or_default(),
        );
    }
    constraints
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

pub fn external_call_return_method_return_constraints(
    ty: &TypeDecl,
    method: &MethodDecl,
    usage: &UsageFacts,
    facts: &FactSet,
) -> Vec<SignatureConstraint> {
    let mut constraints = Vec::new();
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return constraints;
    };
    for returned in &method_usage.returned_call_results {
        if returned.boundary != CallArgumentBoundary::Unknown {
            continue;
        }
        push_external_call_return_constraints(
            &mut constraints,
            returned.receiver_type.as_ref(),
            &returned.name,
            facts,
        );
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
    } else if let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) {
        let returns_unknown_call_result = (method_usage.returned_call_results.is_empty()
            && method_usage.call_sites.iter().any(|call_site| {
                call_site.result_returned && call_site.boundary == CallArgumentBoundary::Unknown
            }))
            || method_usage.returned_call_results.iter().any(|returned| {
                returned.boundary == CallArgumentBoundary::Unknown
                    && !returned_external_call_result_matches_declared_return(
                        returned,
                        &method.return_type,
                        facts,
                    )
            });
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

fn returned_external_call_result_matches_declared_return(
    returned: &ReturnedCallResultUse,
    declared_return: &JavaType,
    facts: &FactSet,
) -> bool {
    let receiver_type = returned
        .receiver_type
        .as_ref()
        .and_then(|receiver| match receiver {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.as_str()),
            _ => None,
        });
    let Some(effect) = facts.external_api_effect_for_receiver(receiver_type, &returned.name) else {
        return false;
    };
    let Some(return_type) = effect.return_type.as_ref() else {
        return false;
    };
    java_types_match_with_external_aliases(return_type, declared_return)
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
